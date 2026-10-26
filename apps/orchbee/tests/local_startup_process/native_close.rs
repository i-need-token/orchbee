use super::tui_observer::NativeHeader;
use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response, write_workbench_request,
};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::os::{fd::AsFd, unix::net::UnixStream};

const CLOSE_TIMEOUT: Duration = Duration::from_secs(45);
const EMPTY_OBSERVATION: Duration = Duration::from_millis(500);

struct SnapshotObserver {
    stream: UnixStream,
    next: u128,
}

impl SnapshotObserver {
    /// Connecting is fixture preparation — the cohort load and session
    /// admission carry their own startup budget; callers must complete it
    /// before opening the observation window so a slow hash cannot consume
    /// the budget the test intends to spend watching the shell.
    fn connect(fixture: &Fixture) -> Result<Self, Box<dyn Error>> {
        let deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
        let cohort = NativeCohort::load(&fixture.cohort, deadline)
            .map_err(|error| format!("observer cohort: {error}"))?;
        let admitted = ClientConnection::connect_before(
            fixture.site().client_socket_path(),
            cohort.identity(),
            deadline,
        )
        .map_err(|error| format!("snapshot observer admission: {error}"))?;
        // Keep one owned descriptor for the admitted connection; no private Server interface.
        let stream = UnixStream::from(admitted.as_fd().try_clone_to_owned()?);
        Ok(Self { stream, next: 1 })
    }

    fn snapshot(&mut self, deadline: Instant) -> Result<Snapshot, Box<dyn Error>> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or("Snapshot observation deadline expired")?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.set_write_timeout(Some(remaining))?;
        let request_id = WorkbenchRequestId::parse(&format!("{:032x}", self.next))?;
        self.next += 1;
        write_workbench_request(
            &mut self.stream,
            &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
            }),
        )?;
        let WorkbenchResponse::Snapshot(result) = read_workbench_response(&mut self.stream)? else {
            return Err("observer received a non-Snapshot reply".into());
        };
        assert!(Instant::now() < deadline, "Snapshot arrived after deadline");
        assert_eq!(result.request_id, request_id);
        let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
            return Err("observer Snapshot unavailable".into());
        };
        Ok(snapshot)
    }
}

#[derive(Clone, Copy)]
struct CloseCopy {
    locale: &'static str,
    ready: &'static str,
    title: &'static str,
    warning: &'static str,
    actions: &'static str,
}

const ENGLISH: CloseCopy = CloseCopy {
    locale: "en_US.UTF-8",
    ready: "Tab 1 · Ready · controlling",
    title: "Close Tab?",
    warning: "Closing this Tab ends its running work for everyone connected to this workspace.",
    actions: "[ Cancel  Esc ]   [ Close  Enter ]",
};
const CHINESE: CloseCopy = CloseCopy {
    locale: "zh_CN.UTF-8",
    ready: "Tab 1 · 已就绪 · 可控制",
    title: "关闭标签页？",
    warning: "关闭此标签页将终止其中运行的任务，影响所有连接到此工作区的人。",
    actions: "[ 取消  Esc ]   [ 关闭  Enter ]",
};

fn ready(client: &mut Client, checkpoint: usize, deadline: Instant, copy: CloseCopy) {
    client.wait_native_frame_text_after(
        checkpoint,
        deadline,
        NativeHeader::active("Workspace", 1, "1"),
        &[copy.ready],
        &[copy.title],
        &[],
        &mut [],
    );
}

fn prompt(client: &mut Client, copy: CloseCopy) {
    let checkpoint = client.checkpoint();
    client.send(b"\x02x");
    client.wait_close_prompt_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        "Workspace",
        &[copy.title, "", "1", "", copy.warning, "", copy.actions],
        &mut [],
    );
}

#[test]
fn localized_cancel_and_ordinary_exit_keep_the_exact_native_shell() -> TestResult {
    let _process_witness = process_witness_guard();
    for copy in [ENGLISH, CHINESE] {
        let fixture = Fixture::with_workspace();
        let mut client = Client::spawn(&fixture, copy.locale);
        ready(
            &mut client,
            0,
            Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
            copy,
        );
        // Observer preparation runs before the observation window opens:
        // connect carries its own startup budget, then the unchanged 5s
        // deadline covers exactly the shell proof and the baseline snapshot.
        let mut observer = SnapshotObserver::connect(&fixture)?;
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        client.send(shell_identity_command(&fixture, "retained", "before", true).as_bytes());
        let before = wait_file(&fixture.base.join("before"), &mut [&mut client], deadline);
        require_shell_identity(&before, "retained");
        let baseline = observer.snapshot(deadline)?;

        prompt(&mut client, copy);
        let checkpoint = client.checkpoint();
        // The confirmation's exact cancel key is Esc; a lone 0x1b resolves to
        // Escape through the bounded wait. Sending it alone keeps the armed
        // packet free of a tail that would otherwise leak into the shell.
        client.send(b"\x1b");
        ready(
            &mut client,
            checkpoint,
            Instant::now() + OBSERVATION_TIMEOUT,
            copy,
        );
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        client.send(shell_identity_command(&fixture, "", "after-cancel", false).as_bytes());
        assert_eq!(
            wait_file(
                &fixture.base.join("after-cancel"),
                &mut [&mut client],
                deadline
            ),
            before
        );
        assert_eq!(observer.snapshot(deadline)?, baseline);

        client.close_before(Instant::now() + EXIT_TIMEOUT);
        assert_eq!(
            observer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
            baseline
        );
        let mut reopened = Client::spawn(&fixture, copy.locale);
        ready(
            &mut reopened,
            0,
            Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
            copy,
        );
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        reopened.send(shell_identity_command(&fixture, "", "after-exit", false).as_bytes());
        assert_eq!(
            wait_file(
                &fixture.base.join("after-exit"),
                &mut [&mut reopened],
                deadline
            ),
            before
        );
        assert_eq!(observer.snapshot(deadline)?, baseline);
        reopened.close_before(Instant::now() + EXIT_TIMEOUT);
    }
    Ok(())
}

fn refresh_empty(client: &mut Client, peer: &mut Client, deadline: Instant, revision: Option<u64>) {
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    let success = revision.map(|revision| format!("Tabs refreshed · revision {revision}"));
    let mut required = vec![
        "No terminal tabs",
        "Ctrl+b c create Tab",
        "Workbench navigator",
        "› * 1 Workspace",
    ];
    if let Some(success) = &success {
        required.push(success);
    }
    client.wait_native_frame_text_after(
        checkpoint,
        deadline,
        NativeHeader::empty("Workspace"),
        &required,
        &[],
        &[],
        &mut [peer],
    );
    // `w` toggles the navigator: leave it visibly closed so the next `w` requests a refresh.
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_native_frame_text_after(
        checkpoint,
        deadline,
        NativeHeader::empty("Workspace"),
        &["No terminal tabs", "Ctrl+b c create Tab"],
        &["Workbench navigator"],
        &[],
        &mut [peer],
    );
}

#[test]
fn view_only_close_reaches_empty_then_manual_create_survives_bare_reopen() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let mut controller = Client::spawn(&fixture, ENGLISH.locale);
    ready(
        &mut controller,
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        ENGLISH,
    );
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    controller.send(shell_identity_command(&fixture, "shared", "shell", true).as_bytes());
    let identity = wait_file(
        &fixture.base.join("shell"),
        &mut [&mut controller],
        deadline,
    );
    let pid = require_shell_identity(&identity, "shared");
    let shell = pidfd_open(
        Pid::from_raw(i32::try_from(pid)?).ok_or("invalid shell PID")?,
        PidfdFlags::empty(),
    )?;
    let mut peer = Client::spawn(&fixture, ENGLISH.locale);
    peer.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::ViewOnly,
    );
    // Same separation: observer preparation completes before the baseline
    // observation window is created.
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    let baseline = observer.snapshot(deadline)?;
    let context = baseline
        .terminal_contexts
        .iter()
        .position(|context| {
            context.terminal_context_id == baseline.navigation.workspaces[0].terminal_context_id
        })
        .ok_or("missing initial_workspace context")?;
    assert_eq!(baseline.terminal_contexts[context].tabs.len(), 1);
    let target = baseline.terminal_contexts[context].tabs[0].tab_id.clone();
    let mut retained_processes = Vec::new();
    for name in ["orchbee-server", "orchbee-worker", "orchbee-terminal-host"] {
        let mut handles = fixture.process_handles(name)?;
        assert_eq!(handles.len(), 1, "one original {name}");
        retained_processes.push((name, handles.pop().ok_or("missing process handle")?));
    }
    let mut expected = baseline.clone();
    expected.terminal_contexts[context]
        .tabs
        .retain(|tab| tab.tab_id != target);

    // The actual view-only TUI is the sole Close issuer; the observer sends reads only.
    prompt(&mut peer, ENGLISH);
    let deadline = Instant::now() + CLOSE_TIMEOUT;
    let retired_checkpoint = controller.checkpoint();
    peer.send(b"\r");
    loop {
        controller.collect();
        peer.collect();
        let current = observer.snapshot(deadline)?;
        if current.revision != baseline.revision {
            assert!(current.revision > baseline.revision);
            expected.revision = current.revision;
            assert_eq!(
                current, expected,
                "Close must remove only the exact captured Tab"
            );
            break;
        }
        assert_eq!(current, baseline);
        thread::yield_now();
    }
    loop {
        let mut descriptor = [PollFd::new(shell.as_fd(), PollFlags::POLLIN)];
        poll(&mut descriptor, PollTimeout::ZERO)?;
        if descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "original shared shell did not exit"
        );
        controller.collect();
        peer.collect();
        thread::yield_now();
    }
    controller.wait_frame_text_after(
        retired_checkpoint,
        deadline,
        &["Terminal disconnected", "Ctrl+b q / Ctrl+b d exit"],
        &[],
        &[],
        &mut [&mut peer],
    );
    refresh_empty(&mut controller, &mut peer, deadline, None);
    refresh_empty(&mut peer, &mut controller, deadline, None);
    let observation_end = Instant::now() + EMPTY_OBSERVATION;
    loop {
        // Each cycle requires a new complete UI frame and fresh public Snapshot.
        // A cached Empty frame can precede Refresh settlement; observe its exact successful revision.
        refresh_empty(
            &mut controller,
            &mut peer,
            deadline,
            Some(expected.revision.value()),
        );
        refresh_empty(
            &mut peer,
            &mut controller,
            deadline,
            Some(expected.revision.value()),
        );
        assert_eq!(observer.snapshot(deadline)?, expected);
        if Instant::now() >= observation_end {
            break;
        }
    }
    // Exercise explicit Create from the former view-only Client's connected Empty session.
    let checkpoint = peer.checkpoint();
    peer.send(b"\x02c");
    peer.wait_ready_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let created = observer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?;
    assert!(created.revision > expected.revision);
    assert_eq!(created.terminal_contexts[context].tabs.len(), 1);
    let new_tab = &created.terminal_contexts[context].tabs[0];
    let old_tab = &baseline.terminal_contexts[context].tabs[0];
    assert_ne!(new_tab.tab_id, old_tab.tab_id);
    let (
        orchbee_workbench_protocol::PanelNode::Panel {
            panel_id: old_panel,
            primary_presentation: old_presentation,
            ..
        },
        orchbee_workbench_protocol::PanelNode::Panel {
            panel_id: new_panel,
            primary_presentation: new_presentation,
            ..
        },
    ) = (&old_tab.root, &new_tab.root)
    else {
        return Err("initial_workspace Create must retain the simple Panel shape".into());
    };
    assert_ne!(new_panel, old_panel);
    assert_ne!(
        new_presentation.presentation_id,
        old_presentation.presentation_id
    );
    let old_binding = old_presentation
        .binding
        .as_ref()
        .ok_or("old binding absent")?;
    let new_binding = new_presentation
        .binding
        .as_ref()
        .ok_or("new binding absent")?;
    assert_eq!(new_binding.worker_id, old_binding.worker_id);
    assert_ne!(new_binding.terminal_id, old_binding.terminal_id);
    expected.revision = created.revision;
    expected.terminal_contexts[context].tabs = vec![new_tab.clone()];
    assert_eq!(
        created, expected,
        "Create changes only the empty context's membership"
    );

    peer.send(shell_identity_command(&fixture, "after-close", "new-shell", true).as_bytes());
    let new_identity = wait_file(
        &fixture.base.join("new-shell"),
        &mut [&mut peer, &mut controller],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let new_pid = require_shell_identity(&new_identity, "after-close");
    let new_shell = pidfd_open(
        Pid::from_raw(i32::try_from(new_pid)?).ok_or("invalid new shell PID")?,
        PidfdFlags::empty(),
    )?;
    let checkpoint = controller.checkpoint();
    controller.send(b"\x02w");
    controller.wait_ready_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::ViewOnly,
    );
    controller.close_before(Instant::now() + EXIT_TIMEOUT);
    peer.close_before(Instant::now() + EXIT_TIMEOUT);
    let mut new_descriptor = [PollFd::new(new_shell.as_fd(), PollFlags::POLLIN)];
    assert_eq!(poll(&mut new_descriptor, PollTimeout::ZERO)?, 0);
    assert_eq!(
        observer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        expected
    );
    for (name, process) in &retained_processes {
        assert!(!process.exited()?, "Client exit must preserve {name}");
    }
    let mut reopened = Client::spawn(&fixture, ENGLISH.locale);
    ready(
        &mut reopened,
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        ENGLISH,
    );
    reopened
        .send(shell_identity_command(&fixture, "after-close", "reopened-shell", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("reopened-shell"),
            &mut [&mut reopened],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        new_identity,
        "bare Open must retain both the new shell PID and its variable"
    );
    assert_eq!(
        observer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        expected
    );
    for (name, process) in &retained_processes {
        assert!(!process.exited()?, "bare Open must preserve {name}");
        assert_eq!(fixture.processes(name), vec![process.pid()]);
    }
    reopened.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
