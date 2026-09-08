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
    fn connect(fixture: &Fixture, deadline: Instant) -> Result<Self, Box<dyn Error>> {
        let cohort = NativeCohort::load(&fixture.cohort, deadline)
            .map_err(|error| format!("observer cohort: {error}"))?;
        let admitted = ClientSessionConnection::connect_before(
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
    title: "Close terminal Tab?",
    warning: "Closing this Tab ends its execution for all attached Clients.",
    actions: "y · confirm    n / Esc · cancel",
};
const CHINESE: CloseCopy = CloseCopy {
    locale: "zh_CN.UTF-8",
    ready: "Tab 1 · 已就绪 · 可控制",
    title: "关闭终端 Tab？",
    warning: "关闭此 Tab 将终止其执行，影响所有已连接的 Client。",
    actions: "y · 确认    n / Esc · 取消",
};

fn ready(client: &mut Client, checkpoint: usize, deadline: Instant, copy: CloseCopy) {
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &["[*1 1]", copy.ready],
        &[copy.title],
        &[],
    );
}

fn prompt(client: &mut Client, copy: CloseCopy) {
    let checkpoint = client.checkpoint();
    client.send(b"\x02x");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        &[copy.title, copy.warning, copy.actions],
        &[],
        &["1"],
    );
}

#[test]
fn localized_cancel_and_ordinary_exit_keep_the_exact_native_shell() -> TestResult {
    let _process_witness = process_witness_guard();
    for copy in [ENGLISH, CHINESE] {
        let fixture = Fixture::new();
        let mut client = Client::spawn(&fixture, copy.locale);
        ready(
            &mut client,
            0,
            Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
            copy,
        );
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        client.send(shell_identity_command(&fixture, "retained", "before", true).as_bytes());
        let before = wait_file(&fixture.base.join("before"), &mut [&mut client], deadline);
        require_shell_identity(&before, "retained");
        let mut observer = SnapshotObserver::connect(&fixture, deadline)?;
        let baseline = observer.snapshot(deadline)?;

        prompt(&mut client, copy);
        let checkpoint = client.checkpoint();
        client.send(b"nMUST_NOT_REACH_SHELL");
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

fn refresh_empty(client: &mut Client, deadline: Instant) {
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &["No terminal tabs · Ctrl+B c to create a Tab"],
        &["[*1", "[ 1"],
        &[],
    );
}

#[test]
fn view_only_native_close_removes_the_shared_last_tab_without_automatic_create() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
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
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    let mut observer = SnapshotObserver::connect(&fixture, deadline)?;
    let baseline = observer.snapshot(deadline)?;
    let context = baseline
        .terminal_contexts
        .iter()
        .position(|context| {
            context.terminal_context_id == baseline.navigation.standalone_terminal_context_id
        })
        .ok_or("missing standalone context")?;
    assert_eq!(baseline.terminal_contexts[context].tabs.len(), 1);
    let target = baseline.terminal_contexts[context].tabs[0].tab_id.clone();
    let mut expected = baseline.clone();
    expected.terminal_contexts[context]
        .tabs
        .retain(|tab| tab.tab_id != target);

    // The actual view-only TUI is the sole Close issuer; the observer sends reads only.
    prompt(&mut peer, ENGLISH);
    let deadline = Instant::now() + CLOSE_TIMEOUT;
    peer.send(b"y");
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
    refresh_empty(&mut controller, deadline);
    refresh_empty(&mut peer, deadline);
    let observation_end = Instant::now() + EMPTY_OBSERVATION;
    loop {
        // Each cycle requires a new complete UI frame and fresh public Snapshot.
        refresh_empty(&mut controller, deadline);
        refresh_empty(&mut peer, deadline);
        assert_eq!(observer.snapshot(deadline)?, expected);
        if Instant::now() >= observation_end {
            break;
        }
    }
    controller.close_before(Instant::now() + EXIT_TIMEOUT);
    peer.close_before(Instant::now() + EXIT_TIMEOUT);
    assert_eq!(
        observer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        expected
    );
    Ok(())
}
