//! Real-PTY witness for the public `orchbee` / `orchbee tui --mouse` pointer
//! path: capture arming at startup, X10 and SGR press/release passthrough
//! into tracking panes, an unfocused-pane click that focuses and flushes the
//! queued press in order, read-only Client rejection without force-acquire,
//! the alternate-screen wheel-to-cursor fallback, `Ctrl+B m` capture
//! release/restore, and mode restoration on exit. Byte-level claims are
//! observed on real PTYs: host reports enter through the spawned binary's
//! outer PTY, and forwarded reports are recorded by `cat` on the pane shells'
//! own input streams.

use super::pty_client::wait_file_containing;
use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    PanelNode, ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchPanelSplitDirection, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchTerminalGeometry,
    read_workbench_response, write_workbench_request,
};
use rustix::process::{Pid as RustixPid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use std::{
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixStream,
    },
    path::{Path, PathBuf},
};

const POINTER_TIMEOUT: Duration = Duration::from_secs(60);

// The same public OBLC/OBWP preparation path native_context uses: the peer
// splits the Workbench panel while the PTY-driven Client stays the witness.
struct WorkbenchPeer {
    stream: UnixStream,
    next: u128,
}

impl WorkbenchPeer {
    fn connect(fixture: &Fixture, deadline: Instant) -> Result<Self, Box<dyn Error>> {
        let cohort = NativeCohort::load(&fixture.cohort, deadline)?;
        let admitted = ClientSessionConnection::connect_before(
            fixture.site().client_socket_path(),
            cohort.identity(),
            deadline,
        )?;
        let stream = UnixStream::from(admitted.as_fd().try_clone_to_owned()?);
        Ok(Self { stream, next: 1 })
    }

    fn request_id(&mut self) -> Result<WorkbenchRequestId, Box<dyn Error>> {
        let id = WorkbenchRequestId::parse(&format!("{:032x}", self.next))?;
        self.next += 1;
        Ok(id)
    }

    fn exchange(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
    ) -> Result<WorkbenchResponse, Box<dyn Error>> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or("Workbench observation deadline expired")?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.set_write_timeout(Some(remaining))?;
        write_workbench_request(&mut self.stream, &request)?;
        let response = read_workbench_response(&mut self.stream)?;
        assert!(Instant::now() < deadline, "Workbench response arrived late");
        Ok(response)
    }

    fn snapshot(&mut self, deadline: Instant) -> Result<Snapshot, Box<dyn Error>> {
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
        });
        let WorkbenchResponse::Snapshot(result) = self.exchange(request, deadline)? else {
            return Err("expected Snapshot response".into());
        };
        assert_eq!(result.request_id, request_id);
        let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
            return Err("Snapshot unavailable".into());
        };
        Ok(snapshot)
    }

    fn prepare(
        &mut self,
        action: WorkbenchAction,
        deadline: Instant,
    ) -> Result<WorkbenchCommandOutcome, Box<dyn Error>> {
        let snapshot = self.snapshot(deadline)?;
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            site_id: snapshot.site_id,
            expected_revision: snapshot.revision,
            action,
        });
        let WorkbenchResponse::Command(result) = self.exchange(request, deadline)? else {
            return Err("expected preparation Command response".into());
        };
        assert_eq!(result.request_id, request_id);
        Ok(result.outcome)
    }
}

/// Host-side SGR report bytes: wire coordinates are 1-based.
fn host_sgr(button: u16, column: u16, row: u16, release: bool) -> Vec<u8> {
    format!(
        "\x1b[<{button};{};{}{}",
        column + 1,
        row + 1,
        if release { 'm' } else { 'M' }
    )
    .into_bytes()
}

/// Parses `ESC[<b;x;yM|m` records out of a pane recorder's byte stream:
/// `(button, x, y, release)` in pane-local 1-based cell coordinates.
fn sgr_reports(log: &str) -> Vec<(u16, u16, u16, bool)> {
    let mut reports = Vec::new();
    let mut rest = log;
    while let Some(start) = rest.find("\x1b[<") {
        rest = &rest[start + 3..];
        let Some(end) = rest.find(['M', 'm']) else {
            break;
        };
        let release = rest.as_bytes()[end] == b'm';
        let mut fields = rest[..end].split(';');
        let (Some(button), Some(x), Some(y)) = (
            fields.next().and_then(|field| field.parse().ok()),
            fields.next().and_then(|field| field.parse().ok()),
            fields.next().and_then(|field| field.parse().ok()),
        ) else {
            break;
        };
        reports.push((button, x, y, release));
        rest = &rest[end + 1..];
    }
    reports
}

/// Parses `ESC[M cb cx cy` X10 records: `(cb, cx, cy)` raw — cb is the
/// 0x20-offset button (release reports as 0x23), cx/cy are 0x20-offset
/// 1-based pane-local cells.
fn x10_reports(log: &[u8]) -> Vec<(u8, u8, u8)> {
    let mut reports = Vec::new();
    let mut index = 0;
    while index + 6 <= log.len() {
        if &log[index..index + 3] == b"\x1b[M" {
            reports.push((log[index + 3], log[index + 4], log[index + 5]));
            index += 6;
        } else {
            index += 1;
        }
    }
    reports
}

/// Sends `bytes`, retrying until `needle` appears in a completed frame.
/// Input sent while a focus switch is still in flight drops as
/// "Terminal input unavailable" — the retry is the witness-level
/// equivalent of a user retyping once the pane's attachment settles.
/// `describe` runs only when the deadline actually expires, letting a
/// witness preserve the failing scene inside the panic detail.
fn send_until_frame_text(
    client: &mut Client,
    bytes: &[u8],
    needle: &str,
    deadline: Instant,
    describe: impl FnOnce(&mut Client) -> String,
) {
    loop {
        client.send(bytes);
        let attempt = Instant::now() + Duration::from_millis(600);
        while Instant::now() < attempt {
            client.collect();
            if client.contains(needle) {
                return;
            }
            thread::yield_now();
        }
        assert!(
            Instant::now() < deadline,
            "input never landed, frame has no {needle:?}; {}",
            describe(client)
        );
    }
}

/// Deadline witness for the unfocused-focus probe: preserves the pane
/// recorders and the client's rendered state under `$OB_TRIAGE_DIR`
/// (default `/tmp/ob-pointer-triage-<pid>`), then reports which pane's
/// recorder swallowed the marker bytes, whether every cohort role was still
/// alive, and the last frames. Pane 1's live `cat` records raw input without
/// echo, so `8;11HZQ` fragments in `pane1.log` prove the bytes were fed to
/// pane 1 — a refused focus move — while an empty log leaves input lost on
/// the pane-2 path itself.
fn preserve_pointer_triage(fixture: &Fixture, client: &mut Client) -> String {
    let root = std::env::var_os("OB_TRIAGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/ob-pointer-triage-{}", std::process::id())));
    let _created = fs::create_dir_all(&root);
    let mut detail = String::new();
    for name in ["pane1.log", "pane1.log.pid", "pane2.log", "pane2.log.pid"] {
        let source = fixture.base.join(name);
        match fs::read(&source) {
            Ok(bytes) => {
                let _written = fs::write(root.join(name), &bytes);
                detail.push_str(&format!("{name}: {} bytes; ", bytes.len()));
            }
            Err(error) => detail.push_str(&format!("{name}: unread ({error}); ")),
        }
    }
    if let Ok(log) = fs::read(fixture.base.join("pane1.log")) {
        let text = String::from_utf8_lossy(&log);
        let feed = text.matches("8;11HZQ").count();
        let prints = text.matches("printf").count();
        let x10 = x10_reports(&log).len();
        detail.push_str(&format!(
            "pane1.log feed-fragments={feed} printf={prints} x10={x10}; "
        ));
    }
    for role in ["orchbee-server", "orchbee-worker", "orchbee-terminal-host"] {
        let alive = fixture.processes(role).len();
        detail.push_str(&format!("{role} alive={alive}; "));
    }
    let diagnostic = client.diagnostic(0);
    let _written = fs::write(root.join("client-diagnostic.txt"), &diagnostic);
    format!("triage={} {detail}{diagnostic}", root.display())
}

/// Sends `bytes`, retrying until `needle` appears in `path`'s contents —
/// the same settle-proof as `send_until_frame_text`, for panes whose live
/// `cat` recorder swallows the input instead of echoing it to the screen.
fn send_until_file_containing(
    client: &mut Client,
    bytes: &[u8],
    path: &Path,
    needle: &str,
    deadline: Instant,
) {
    loop {
        client.send(bytes);
        let attempt = Instant::now() + Duration::from_millis(600);
        while Instant::now() < attempt {
            client.collect();
            if fs::read_to_string(path).is_ok_and(|log| log.contains(needle)) {
                return;
            }
            thread::yield_now();
        }
        assert!(
            Instant::now() < deadline,
            "input never landed in {}",
            path.display()
        );
    }
}

/// Splits `marker` across two `printf` commands so its complete text never
/// appears in the sent bytes: a frame match can only come from the pane
/// program's output, never from command echo — no "echo counts as ready"
/// false positive.
fn split_print(marker: &str) -> String {
    let (first, second) = marker.split_at(marker.len() / 2);
    format!("printf '{first}'; printf '{second}\\n'")
}

/// Places `marker` at the known pane-local cell `(row, column)` — both
/// 1-based, matching ANSI CUP — via cursor positioning. The pane-local
/// address is the independent oracle: the expected report coordinates come
/// from the CUP we commanded, not from the hit map or translation under
/// test.
fn place_marker(row: u16, column: u16, marker: &str) -> Vec<u8> {
    let (first, second) = marker.split_at(marker.len() / 2);
    format!("printf '\\033[{row};{column}H{first}'; printf '{second}\\n'\r").into_bytes()
}

/// `stty` + readiness marker + a fixture-owned foreground `cat`: the pane
/// records its raw input stream. The `sh -c` child writes its own pid to
/// `<name>.pid` and then `exec`s `cat` with stdout on `name`, so the test
/// pins the real recorder by pidfd and proves ownership through `/proc`
/// links — never by scanning process names. The preserved parent shell
/// prints `done` only after that `cat` exits: a fresh split marker no
/// command echo or leftover frame can satisfy, and the deterministic
/// handoff point before another recorder is sent. `cup` positions the
/// marker at a known pane-local `(row, column)` for coordinate-exact
/// assertions.
fn recorder(
    fixture: &Fixture,
    name: &str,
    modes: &str,
    cup: Option<(u16, u16)>,
    marker: &str,
    done: &str,
) -> Vec<u8> {
    let position = cup.map_or(String::new(), |(row, column)| {
        format!("\\033[{row};{column}H")
    });
    format!(
        "stty -icanon -echo; printf '{position}{modes}'; {}; sh -c 'echo $$ > {}; exec cat > {}'; {}\r",
        split_print(marker),
        fixture.base.join(format!("{name}.pid")).display(),
        fixture.base.join(name).display(),
        split_print(done),
    )
    .into_bytes()
}

/// A fixture-owned recorder `cat` pinned by pidfd. Mirrors the
/// `native_cohort` pidfd precedent — `pidfd_open` fixes the process
/// identity before any signal, so a recycled pid can never be signalled,
/// and the pidfd's readability is the kernel-level exit witness.
struct RecorderProcess {
    pidfd: OwnedFd,
}

/// One pidfd poll: is the pinned process gone?
fn pidfd_exited(pidfd: &OwnedFd) -> io::Result<bool> {
    let mut descriptor = [PollFd::new(pidfd.as_fd(), PollFlags::POLLIN)];
    poll(&mut descriptor, PollTimeout::ZERO).map_err(io::Error::from)?;
    let events = descriptor[0].revents().unwrap_or(PollFlags::POLLNVAL);
    if events.contains(PollFlags::POLLNVAL) {
        return Err(io::Error::other("recorder pidfd became invalid"));
    }
    Ok(events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR))
}

/// Opens `raw_pid` as a pidfd only while `/proc` proves it is a `cat`
/// whose stdout is `log` — the fixture-ownership binding. `Ok(None)`
/// means the child is still in its pre-exec `sh` window and the caller
/// should keep polling; a dead process is an immediate failure.
fn pin_verified(raw_pid: i32, log: &Path) -> io::Result<Option<OwnedFd>> {
    let Some(pid) = RustixPid::from_raw(raw_pid) else {
        return Err(io::Error::other("recorder wrote an invalid pid"));
    };
    let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
        Ok(pidfd) => pidfd,
        Err(rustix::io::Errno::SRCH) => {
            return Err(io::Error::other("recorder exited before pin"));
        }
        Err(error) => return Err(error.into()),
    };
    if pidfd_exited(&pidfd)? {
        return Err(io::Error::other("recorder exited before pin"));
    }
    let proc = PathBuf::from(format!("/proc/{raw_pid}"));
    let command = match fs::read(proc.join("cmdline")) {
        Ok(command) => command,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let stdout = match fs::read_link(proc.join("fd/1")) {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if pidfd_exited(&pidfd)? {
        return Err(io::Error::other("recorder exited during pin"));
    }
    if command != b"cat\0" || fs::canonicalize(&stdout)? != fs::canonicalize(log)? {
        return Ok(None);
    }
    Ok(Some(pidfd))
}

impl RecorderProcess {
    /// Pins the `cat` behind recorder `name`: waits for the child-written
    /// pid file, opens the pidfd, and requires the live `/proc` binding to
    /// this fixture's log. `client` keeps the outer PTY drained throughout.
    #[track_caller]
    fn pin(client: &mut Client, fixture: &Fixture, name: &str, deadline: Instant) -> Self {
        let pid_path = fixture.base.join(format!("{name}.pid"));
        let log = fixture.base.join(name);
        loop {
            client.collect();
            if let Ok(text) = fs::read_to_string(&pid_path)
                && let Ok(raw_pid) = text.trim().parse::<i32>()
            {
                match pin_verified(raw_pid, &log) {
                    Ok(Some(pidfd)) => return Self { pidfd },
                    Ok(None) => {}
                    Err(error) => panic!("pin {name} recorder: {error}"),
                }
            }
            assert!(
                Instant::now() < deadline,
                "recorder {name} pid was never pinned"
            );
            thread::yield_now();
        }
    }

    /// One SIGTERM through the pinned pidfd — a test-preparation switch,
    /// not a claim about the product's Ctrl-C input path.
    fn terminate(&self) -> io::Result<()> {
        match pidfd_send_signal(&self.pidfd, Signal::TERM) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Waits for the kernel-level exit while draining the outer PTY.
    #[track_caller]
    fn wait_exited(&self, client: &mut Client, deadline: Instant) {
        loop {
            client.collect();
            match pidfd_exited(&self.pidfd) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => panic!("poll recorder exit: {error}"),
            }
            assert!(
                Instant::now() < deadline,
                "recorder cat did not exit after SIGTERM"
            );
            thread::yield_now();
        }
    }
}

#[test]
fn native_pointer_reports_reach_tracking_panes_and_follow_unfocused_focus() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    let deadline = Instant::now() + POINTER_TIMEOUT;

    assert_eq!(client.ready_before(deadline), ReadyMode::Controlling);
    // Capture arms exactly once: button-event tracking + SGR; never ?1003.
    assert!(client.contains("\x1b[?1002h"), "button tracking not armed");
    assert!(client.contains("\x1b[?1006h"), "SGR format not armed");
    assert!(
        !client.contains("\x1b[?1003h"),
        "all-motion capture must stay off"
    );

    // Marker at known pane-local (row 7, column 9) via ANSI CUP: the
    // expected report coordinates come from the CUP we commanded — an
    // independent oracle, never the hit map or translation under test.
    let checkpoint = client.checkpoint();
    client.send(&place_marker(7, 9, "ZQX1"));
    client.wait_frame_text_after(checkpoint, deadline, &["ZQX1"], &[], &[], &mut []);
    let (x1, y1) = client.cell_position("ZQX1");

    // Request button tracking only — no format mode — so this pane receives
    // the legacy X10 encoding. RDY1's frame proves the mode request applied.
    client.send(b"printf '\\033[?1002h'\r");
    let checkpoint = client.checkpoint();
    client.send(b"printf 'RDY1\\n'\r");
    client.wait_frame_text_after(checkpoint, deadline, &["RDY1"], &[], &[], &mut []);
    let checkpoint = client.checkpoint();
    client.send(&recorder(&fixture, "pane1.log", "", None, "REC1", "END1"));
    client.wait_frame_text_after(checkpoint, deadline, &["REC1"], &[], &[], &mut []);
    // Pin the fixture-owned recorder before its log becomes evidence: the
    // pidfd + `/proc` binding proves the bytes below land in this cat.
    let _rec1 = RecorderProcess::pin(&mut client, &fixture, "pane1.log", deadline);

    // Press and release on the focused pane arrive in order as X10 records:
    // `ESC[M cb cx cy` with a 0x20 button press and the 0x23 release marker.
    // The click lands on ZQX1's outer cell, so the encoded coordinates must
    // be exactly pane-local (9,7) + 0x20 — translation proven, not bounded.
    client.send(&host_sgr(0, x1, y1, false));
    client.send(&host_sgr(0, x1, y1, true));
    let pane1_log = fixture.base.join("pane1.log");
    let pane1_bytes = wait_file_containing(&pane1_log, "\x1b[M#", &mut [&mut client], deadline);
    let reports = x10_reports(pane1_bytes.as_bytes());
    let [(0x20, 0x29, 0x27), (0x23, 0x29, 0x27)] = reports[..] else {
        panic!(
            "expected X10 press+release at pane-local (9,7), got {reports:?} from {pane1_bytes:?}"
        );
    };

    // A workbench-driven right split paints a second panel with its own shell.
    let mut peer = WorkbenchPeer::connect(&fixture, deadline)?;
    let snapshot = peer.snapshot(deadline)?;
    let terminal_context_id = snapshot.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("Split focus context missing")?;
    let tab = context.tabs.first().ok_or("Split focus Tab missing")?;
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        return Err("fresh native Tab was already split".into());
    };
    let outcome = peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id,
            tab_id: tab.tab_id.clone(),
            panel_id: panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Split peer")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        deadline,
    )?;
    let WorkbenchCommandOutcome::PanelSplitCreated { .. } = outcome else {
        return Err(format!("native PanelSplit did not start: {outcome:?}").into());
    };
    let checkpoint = client.checkpoint();
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &["Terminal 2 · Split peer"],
        &[],
        &[],
        &mut [],
    );

    // Focus the new pane, then prove its attachment admits input. Causal
    // probe: a single `Ctrl+B l` is refused while the peer pane is not yet
    // focusable (its record lacks a settled mode), and the refused chord
    // leaves focus on pane 1 whose live `cat` swallows the marker. The
    // chord is retried inside the same payload — once focus lands on the
    // rightmost leaf a repeated `Ctrl+B l` is refused harmlessly and the
    // marker still lands. This retry is test setup only; it does not claim
    // ordinary keys survive a focus switch. ZQX2 sits at known pane-local
    // (8,11) so the pending-focus click below asserts exact SGR
    // coordinates.
    let focus_and_marker = [b"\x02l".as_slice(), &place_marker(8, 11, "ZQX2")].concat();
    send_until_frame_text(&mut client, &focus_and_marker, "ZQX2", deadline, |client| {
        preserve_pointer_triage(&fixture, client)
    });
    let (x2, y2) = client.cell_position("ZQX2");
    client.send(b"printf '\\033[?1002h\\033[?1006h'\r");
    let checkpoint = client.checkpoint();
    client.send(b"printf 'RDY2\\n'\r");
    client.wait_frame_text_after(checkpoint, deadline, &["RDY2"], &[], &[], &mut []);
    let pane2_log = fixture.base.join("pane2.log");
    let checkpoint = client.checkpoint();
    client.send(&recorder(&fixture, "pane2.log", "", None, "REC2", "END2"));
    client.wait_frame_text_after(checkpoint, deadline, &["REC2"], &[], &[], &mut []);
    let rec2 = RecorderProcess::pin(&mut client, &fixture, "pane2.log", deadline);
    // Focus back to pane 1 and prove its attachment admits input again:
    // pane 1's live `cat` swallows the marker into pane1.log, so the file —
    // not the frame — is the landing proof. Without this the unfocused
    // click below could be eaten by a still in-flight switch instead of
    // reaching the pending-focus queue.
    client.send(b"\x02h");
    send_until_file_containing(
        &mut client,
        b"printf 'ZQX1b\\n'\r",
        &pane1_log,
        "ZQX1b",
        deadline,
    );

    // A click on the unfocused pane queues a bounded pending press, focuses
    // the pane for real, then flushes — the queued press lands only after the
    // pane's own attachment settles, and the release stays on the same target.
    client.send(&host_sgr(0, x2, y2, false));
    wait_file_containing(&pane2_log, "\x1b[<", &mut [&mut client], deadline);
    client.send(&host_sgr(0, x2, y2, true));
    let pane2_bytes = wait_file_containing(&pane2_log, "m", &mut [&mut client], deadline);
    let pane2_reports = sgr_reports(&pane2_bytes);
    // The queued press must resolve to ZQX2's exact pane-local cell (11,8)
    // under the flushed target — ordered press and release at one cell.
    let [(0, 11, 8, false), (0, 11, 8, true)] = pane2_reports[..] else {
        panic!("expected pending SGR press+release at pane-local (11,8), got {pane2_reports:?}");
    };
    // Nothing leaked back into pane 1's input stream.
    let pane1_after = fs::read_to_string(&pane1_log)?;
    assert_eq!(
        x10_reports(pane1_after.as_bytes()).len(),
        2,
        "unfocused-pane click must not leak into pane 1"
    );

    // A second Client on the same site cannot take pane 2's controller
    // lease while this client holds it — the controller lease is per
    // terminal. Whichever pane it lands on, its click on pane 2 is
    // adjudicated negatively: a localized drop notice, never bytes
    // forwarded to the pane, never a forced control acquisition.
    let mut viewer = Client::spawn(&fixture, "en_US.UTF-8");
    viewer.ready_before(deadline);
    let pane2_before = fs::read(&pane2_log)?;
    let viewer_checkpoint = viewer.checkpoint();
    viewer.send(&host_sgr(0, x2, y2, false));
    viewer.wait_any_text_after(
        viewer_checkpoint,
        deadline,
        "read-only pointer rejection notice",
        &["Pointer input dropped", "Terminal input unavailable"],
    );
    thread::sleep(Duration::from_millis(75));
    assert_eq!(
        fs::read(&pane2_log)?,
        pane2_before,
        "read-only Client input reached the pane"
    );

    // Back on the controlling Client, still focused on pane 2: alt screen +
    // tracking released, wheel notches become cursor-key fallback bytes.
    // The recorder switch is deterministic test preparation — not a claim
    // about the product's Ctrl-C path: one SIGTERM through the pinned pidfd,
    // the kernel exit witness, then the parent shell's fresh completion
    // marker proves it resumed past the dead cat before the next recorder
    // is sent exactly once.
    let checkpoint = client.checkpoint();
    rec2.terminate()?;
    rec2.wait_exited(&mut client, deadline);
    client.wait_frame_text_after(checkpoint, deadline, &["END2"], &[], &[], &mut []);
    let wheel_log = fixture.base.join("wheel.log");
    let checkpoint = client.checkpoint();
    client.send(&recorder(
        &fixture,
        "wheel.log",
        "\\033[?1002l\\033[?1049h",
        None,
        "REC3",
        "END3",
    ));
    client.wait_frame_text_after(checkpoint, deadline, &["REC3"], &[], &[], &mut []);
    let _rec3 = RecorderProcess::pin(&mut client, &fixture, "wheel.log", deadline);
    client.send(&host_sgr(64, x2, y2, false));
    // A mid-burst direction reversal drops the notch by design — wait
    // for the up flush to land before reversing so the burst closes.
    wait_file_containing(&wheel_log, "\x1b[A", &mut [&mut client], deadline);
    client.send(&host_sgr(65, x2, y2, false));
    let wheel_bytes = wait_file_containing(&wheel_log, "\x1b[B", &mut [&mut client], deadline);
    assert!(
        wheel_bytes.contains("\x1b[A"),
        "wheel-up produced no cursor key"
    );
    assert!(
        wheel_bytes.contains("\x1b[B"),
        "wheel-down produced no cursor key"
    );
    assert!(
        !wheel_bytes.contains("\x1b[<"),
        "wheel leaked as a mouse report: {wheel_bytes:?}"
    );

    // Ctrl+B m releases capture for host-native selection: the indicator
    // persists and injected reports no longer reach the pane.
    let wheel_len = fs::read(&wheel_log)?.len();
    let checkpoint = client.checkpoint();
    client.send(b"\x02m");
    client.wait_frame_text_after(checkpoint, deadline, &["mouse released"], &[], &[], &mut []);
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1002l"),
        "release did not reset button tracking"
    );
    client.send(&host_sgr(0, x2, y2, false));
    thread::sleep(Duration::from_millis(75));
    assert_eq!(
        fs::read(&wheel_log)?.len(),
        wheel_len,
        "released capture still forwarded pane input"
    );

    // A second Ctrl+B m restores capture; exiting restores the host modes.
    let checkpoint = client.checkpoint();
    client.send(b"\x02m");
    client.wait_frame_text_after(checkpoint, deadline, &[], &["mouse released"], &[], &mut []);
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1002h"),
        "restore did not re-arm button tracking"
    );
    let checkpoint = client.checkpoint();
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1006l"),
        "exit did not restore SGR format"
    );
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1002l"),
        "exit did not restore button tracking"
    );

    let viewer_checkpoint = viewer.checkpoint();
    viewer.close_before(Instant::now() + EXIT_TIMEOUT);
    assert!(
        viewer.raw_contains_after(viewer_checkpoint, b"\x1b[?1002l"),
        "viewing Client exit did not restore tracking"
    );
    Ok(())
}

#[test]
fn native_tui_mouse_off_starts_released_and_ctrl_b_m_rearms() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let mut client = Client::spawn_args(&fixture, "en_US.UTF-8", &["tui", "--mouse", "off"]);
    let deadline = Instant::now() + POINTER_TIMEOUT;

    assert_eq!(client.ready_before(deadline), ReadyMode::Controlling);
    assert!(
        !client.contains("\x1b[?1002h"),
        "--mouse off still armed button tracking"
    );
    assert!(
        !client.contains("\x1b[?1006h"),
        "--mouse off still armed SGR format"
    );
    let checkpoint = client.checkpoint();
    client.wait_frame_text_after(checkpoint, deadline, &["mouse released"], &[], &[], &mut []);

    let checkpoint = client.checkpoint();
    client.send(b"\x02m");
    client.wait_frame_text_after(checkpoint, deadline, &[], &["mouse released"], &[], &mut []);
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1002h"),
        "Ctrl+B m did not re-arm button tracking"
    );
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1006h"),
        "Ctrl+B m did not re-arm SGR format"
    );

    let checkpoint = client.checkpoint();
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1002l"),
        "exit did not restore button tracking"
    );
    Ok(())
}

/// A host PTY reporting zero pixel geometry is a real admitted terminal:
/// cell-addressed reports still encode with exact pane-local coordinates,
/// including the SGR cell fallback for a pane asking for pixel reports
/// (`?1016h`). Exact pixel positions are never fabricated.
#[test]
fn native_pointer_unknown_pixel_geometry_keeps_cell_input() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let mut client = Client::spawn_unknown_pixel(&fixture, "en_US.UTF-8");
    let deadline = Instant::now() + POINTER_TIMEOUT;

    assert_eq!(client.ready_before(deadline), ReadyMode::Controlling);
    assert!(client.contains("\x1b[?1002h"), "button tracking not armed");

    // The pane records its raw input stream with button tracking + SGR.
    // The recorder positions ORCL at the known pane-local cell (row 3,
    // column 4) via ANSI CUP; the marker text is split across two printf
    // halves so its complete form never appears in the echoed command —
    // a frame match proves the pane program ran. The expected report
    // coordinates are the independent oracle: exactly (4,3), not derived
    // from the hit map or translation under test.
    let cell_log = fixture.base.join("cell.log");
    let checkpoint = client.checkpoint();
    client.send(&recorder(
        &fixture,
        "cell.log",
        "\\033[?1002h\\033[?1006h",
        Some((3, 4)),
        "ORCL",
        "ENDC",
    ));
    client.wait_frame_text_after(checkpoint, deadline, &["ORCL"], &[], &[], &mut []);
    let cell = RecorderProcess::pin(&mut client, &fixture, "cell.log", deadline);
    let (x, y) = client.cell_position("ORCL");
    client.send(&host_sgr(0, x, y, false));
    client.send(&host_sgr(0, x, y, true));
    let bytes = wait_file_containing(&cell_log, "m", &mut [&mut client], deadline);
    assert_eq!(
        sgr_reports(&bytes)[..],
        [(0, 4, 3, false), (0, 4, 3, true)],
        "click on ORCL at pane-local (4,3) under zero pixel geometry: {bytes:?}"
    );

    // `?1016h` asks for pixel format: the next click still uses the cell
    // fallback. The recorder switch uses the same handoff as the main
    // test — SIGTERM the pinned cat, require its pidfd exit plus the
    // parent shell's fresh completion marker, then send once into a
    // shell proven alive.
    let checkpoint = client.checkpoint();
    cell.terminate()?;
    cell.wait_exited(&mut client, deadline);
    client.wait_frame_text_after(checkpoint, deadline, &["ENDC"], &[], &[], &mut []);
    let pixel_log = fixture.base.join("pixel.log");
    let checkpoint = client.checkpoint();
    client.send(&recorder(
        &fixture,
        "pixel.log",
        "\\033[?1016h",
        Some((4, 5)),
        "RECX",
        "ENDX",
    ));
    client.wait_frame_text_after(checkpoint, deadline, &["RECX"], &[], &[], &mut []);
    let _pixel = RecorderProcess::pin(&mut client, &fixture, "pixel.log", deadline);
    let (x, y) = client.cell_position("RECX");
    client.send(&host_sgr(0, x, y, false));
    client.send(&host_sgr(0, x, y, true));
    let bytes = wait_file_containing(&pixel_log, "m", &mut [&mut client], deadline);
    assert_eq!(
        sgr_reports(&bytes)[..],
        [(0, 5, 4, false), (0, 5, 4, true)],
        "pixel request keeps cell fallback at pane-local (5,4): {bytes:?}"
    );

    let checkpoint = client.checkpoint();
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    assert!(
        client.raw_contains_after(checkpoint, b"\x1b[?1002l"),
        "exit did not restore button tracking"
    );
    Ok(())
}
