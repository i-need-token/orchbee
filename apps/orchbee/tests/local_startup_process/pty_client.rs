use super::{
    native_cohort::Fixture,
    tui_observer::{CompletedFrame, ModalRow, Screen, WorkbenchHeader},
};
use nix::{
    fcntl::{FcntlArg, OFlag, fcntl, open},
    pty::{grantpt, posix_openpt, ptsname_r, unlockpt},
    sys::stat::Mode,
    sys::termios::{Termios, tcgetattr},
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::fd::{AsFd, OwnedFd},
    path::Path,
    process::{Child, ChildStderr, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) const COLUMNS: usize = 120;
pub(super) const ROWS: usize = 32;
const MAX_OUTPUT: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadyMode {
    Controlling,
    ViewOnly,
}

impl ReadyMode {
    fn footer(self, tab_number: usize) -> String {
        let mode = match self {
            Self::Controlling => "controlling",
            Self::ViewOnly => "view only",
        };
        format!("Tab {tab_number} · Ready · {mode}")
    }
}

fn ready_status(text: &str) -> bool {
    text.contains(" · Ready ·") || text.contains(" · 已就绪 ·")
}

fn initial_ready_mode(frame: &CompletedFrame) -> Option<ReadyMode> {
    if !frame.prefix_active() || !frame.has_active_tab(1, "1") {
        return None;
    }
    [ReadyMode::Controlling, ReadyMode::ViewOnly]
        .into_iter()
        .find(|mode| frame.text().contains(&mode.footer(1)))
}

pub(super) struct FrameExpectation<'a> {
    header: Option<WorkbenchHeader<'a>>,
    required: &'a [&'a str],
    forbidden: &'a [&'a str],
    exact_content_rows: &'a [&'a str],
}

impl<'a> FrameExpectation<'a> {
    pub(super) fn native(
        header: WorkbenchHeader<'a>,
        required: &'a [&'a str],
        forbidden: &'a [&'a str],
        exact_content_rows: &'a [&'a str],
    ) -> Self {
        Self {
            header: Some(header),
            required,
            forbidden,
            exact_content_rows,
        }
    }
}

pub(super) struct Client {
    child: Child,
    master: File,
    slave: OwnedFd,
    original: Termios,
    stderr: ChildStderr,
    stderr_bytes: Vec<u8>,
    screen: Screen,
    pixels: bool,
}

impl Client {
    pub(super) fn spawn(fixture: &Fixture, locale: &str) -> Self {
        Self::spawn_at(fixture, locale, &fixture.base)
    }

    pub(super) fn spawn_at(fixture: &Fixture, locale: &str, base: &Path) -> Self {
        Self::spawn_sized(fixture, locale, base, COLUMNS as u16, ROWS as u16)
    }

    pub(super) fn spawn_sized(
        fixture: &Fixture,
        locale: &str,
        base: &Path,
        columns: u16,
        rows: u16,
    ) -> Self {
        Self::spawn_full(fixture, locale, base, columns, rows, &[], true)
    }

    /// `pixels_known == false` leaves the PTY's reported pixel extents at zero —
    /// the legal "pixels unknown" report real hosts can produce.
    pub(super) fn spawn_geometry(
        fixture: &Fixture,
        locale: &str,
        base: &Path,
        columns: u16,
        rows: u16,
        pixels_known: bool,
    ) -> Self {
        Self::spawn_full(fixture, locale, base, columns, rows, &[], pixels_known)
    }

    /// A full-canvas client whose PTY reports zero pixel geometry — the
    /// pixel-less report real hosts can produce.
    pub(super) fn spawn_zeropixel(fixture: &Fixture, locale: &str) -> Self {
        Self::spawn_unknown_pixel(fixture, locale)
    }

    pub(super) fn spawn_args(fixture: &Fixture, locale: &str, args: &[&str]) -> Self {
        Self::spawn_full(
            fixture,
            locale,
            &fixture.base,
            COLUMNS as u16,
            ROWS as u16,
            args,
            true,
        )
    }

    /// A host whose PTY reports zero pixel geometry — real terminals that
    /// never fill `ws_xpixel`/`ws_ypixel`. Cell-addressed pointer input
    /// must still work: the encoder needs cells, never pixels.
    pub(super) fn spawn_unknown_pixel(fixture: &Fixture, locale: &str) -> Self {
        Self::spawn_full(
            fixture,
            locale,
            &fixture.base,
            COLUMNS as u16,
            ROWS as u16,
            &[],
            false,
        )
    }

    fn spawn_full(
        fixture: &Fixture,
        locale: &str,
        base: &Path,
        columns: u16,
        rows: u16,
        args: &[&str],
        pixels_known: bool,
    ) -> Self {
        fixture.register_runtime_base(base);
        let pty = open_outer_pty(columns, rows, pixels_known).unwrap();
        let original = tcgetattr(&pty.slave).unwrap();
        let mut child = crate::support::orchbee_command(
            fixture
                .cohort
                .join(crate::support::staged_artifact_name("orchbee")),
        )
        .args(args)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env("LC_ALL", locale)
        .env("TERM", "xterm-256color")
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::from(File::from(clone_cloexec(&pty.slave).unwrap())))
        .stdout(Stdio::from(File::from(clone_cloexec(&pty.slave).unwrap())))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
        let stderr = child.stderr.take().unwrap();
        fcntl(&stderr, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
        Self {
            child,
            master: File::from(pty.master),
            slave: pty.slave,
            original,
            stderr,
            stderr_bytes: Vec::new(),
            // The observer keeps a fixed declared canvas independent of the
            // spawn winsize, so a zero or undersized initial report still
            // stores every cell the product draws.
            screen: Screen::new(COLUMNS, ROWS),
            pixels: pixels_known,
        }
    }

    pub(super) fn collect(&mut self) {
        let mut bytes = [0; 16_384];
        loop {
            match self.master.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => self.screen.feed_all(&bytes[..count]).unwrap(),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) if error.raw_os_error() == Some(nix::libc::EIO) => break,
                Err(error) => panic!("PTY read: {error}"),
            }
        }
        loop {
            match self.stderr.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    self.stderr_bytes.extend_from_slice(&bytes[..count]);
                    assert!(
                        self.stderr_bytes.len() <= MAX_OUTPUT,
                        "Client stderr exceeded bound"
                    );
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) => panic!("Client stderr read: {error}"),
            }
        }
    }

    pub(super) fn contains(&self, text: &str) -> bool {
        if text.starts_with('\x1b') {
            self.screen.raw_contains(text.as_bytes())
        } else {
            self.screen
                .latest_frame()
                .is_some_and(|frame| frame.text().contains(text))
        }
    }

    /// Cumulative-raw search scoped to output written after `checkpoint` —
    /// used for mode bytes that legitimately appear earlier in the stream.
    pub(super) fn raw_contains_after(&self, checkpoint: usize, needle: &[u8]) -> bool {
        self.screen.raw_contains_after(checkpoint, needle)
    }

    /// Latest completed frame's cell position of `needle` — the same frame
    /// the published hit map was derived from, so clicks land deterministically.
    pub(super) fn cell_position(&mut self, needle: &str) -> (u16, u16) {
        self.collect();
        self.screen
            .latest_frame()
            .and_then(|frame| frame.cell_position(needle))
            .unwrap_or_else(|| panic!("no cell contains {needle:?}; {}", self.diagnostic(0)))
    }

    #[track_caller]
    pub(super) fn wait_any_text_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        description: &str,
        any_of: &[&str],
    ) {
        self.observe_after(checkpoint, deadline, description, |frame| {
            let text = frame.text();
            any_of
                .iter()
                .any(|needle| text.contains(needle))
                .then_some(())
        });
    }

    #[track_caller]
    pub(super) fn ready_before(&mut self, deadline: Instant) -> ReadyMode {
        self.observe_after(0, deadline, "first Tab frame", |frame| {
            frame.has_active_tab(1, "1").then_some(())
        });
        self.probe_status(deadline, &mut [], initial_ready_mode)
    }

    pub(super) fn checkpoint(&mut self) -> usize {
        self.collect();
        self.screen.checkpoint()
    }

    #[track_caller]
    pub(super) fn wait_frame_text_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        required: &[&str],
        forbidden: &[&str],
        exact_content_rows: &[&str],
        pumped_peers: &mut [&mut Client],
    ) {
        self.wait_frame_observation_after(
            checkpoint,
            deadline,
            FrameExpectation {
                header: None,
                required,
                forbidden,
                exact_content_rows,
            },
            pumped_peers,
        );
    }

    #[track_caller]
    pub(super) fn wait_native_frame_text_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        expectation: FrameExpectation<'_>,
        pumped_peers: &mut [&mut Client],
    ) {
        self.wait_frame_observation_after(checkpoint, deadline, expectation, pumped_peers);
    }

    fn wait_frame_observation_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        expectation: FrameExpectation<'_>,
        pumped_peers: &mut [&mut Client],
    ) {
        // Readiness is contextual chrome. First wait for the requested content and
        // modal dismissal, then explicitly open the prefix surface to inspect status.
        let needs_status = expectation.required.iter().any(|part| ready_status(part));
        let matches = |frame: &CompletedFrame, include_status: bool| {
            let rows = frame.displayed_rows();
            let text = rows.join("\n");
            (expectation
                .header
                .is_none_or(|header| frame.has_native_header(header))
                && expectation
                    .required
                    .iter()
                    .all(|part| (!include_status && ready_status(part)) || text.contains(part))
                && expectation
                    .forbidden
                    .iter()
                    .all(|part| !text.contains(part))
                && expectation.exact_content_rows.iter().all(|expected| {
                    rows.iter().any(|row| {
                        row.trim_matches(|character: char| {
                            character.is_whitespace() || matches!(character, '│' | '┃' | '║')
                        }) == *expected
                    })
                }))
            .then_some(())
        };
        self.observe_after_pumping(
            checkpoint,
            deadline,
            "complete expected UI content",
            pumped_peers,
            |frame| matches(frame, !needs_status),
        );
        if needs_status {
            self.probe_status(deadline, pumped_peers, |frame| matches(frame, true));
        }
    }

    // A status probe owns its prefix episode, including the completed cancellation
    // draw. Subsequent shell bytes and shortcuts never inherit the probe's capture.
    #[track_caller]
    fn probe_status<T>(
        &mut self,
        deadline: Instant,
        pumped_peers: &mut [&mut Client],
        mut predicate: impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> T {
        let checkpoint = self.checkpoint();
        self.send(b"\x02");
        let result = self.observe_after_pumping(
            checkpoint,
            deadline,
            "requested status overlay",
            pumped_peers,
            |frame| frame.prefix_active().then(|| predicate(frame)).flatten(),
        );
        let checkpoint = self.checkpoint();
        self.send(b"\x1b");
        self.observe_after_pumping(
            checkpoint,
            deadline,
            "dismissed status probe",
            pumped_peers,
            |frame| (!frame.prefix_active()).then_some(()),
        );
        result
    }

    #[track_caller]
    pub(super) fn wait_close_prompt_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        active_header: &str,
        content_rows: &[&str],
        pumped_peers: &mut [&mut Client],
    ) {
        let content_rows = content_rows
            .iter()
            .map(|row| ModalRow::exact(row))
            .collect::<Vec<_>>();
        self.wait_modal_prompt_after(
            checkpoint,
            deadline,
            WorkbenchHeader::active(active_header, 1, "1"),
            &content_rows,
            pumped_peers,
        );
    }

    #[track_caller]
    pub(super) fn wait_modal_prompt_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        header: WorkbenchHeader<'_>,
        content_rows: &[ModalRow<'_>],
        pumped_peers: &mut [&mut Client],
    ) {
        self.observe_after_pumping(
            checkpoint,
            deadline,
            "complete dialog with its exact heading edge and content inside its borders",
            pumped_peers,
            |frame| {
                (frame.has_native_header(header) && frame.has_bordered_modal_content(content_rows))
                    .then_some(())
            },
        );
    }

    #[track_caller]
    pub(super) fn wait_ready_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        tab_number: usize,
        safe_label: &str,
        mode: ReadyMode,
    ) {
        let footer = mode.footer(tab_number);
        self.observe_after(checkpoint, deadline, "active native Tab", |frame| {
            frame.has_active_tab(tab_number, safe_label).then_some(())
        });
        self.probe_status(deadline, &mut [], |frame| {
            (frame.has_active_tab(tab_number, safe_label) && frame.text().contains(&footer))
                .then_some(())
        });
    }

    #[track_caller]
    pub(super) fn wait_navigator_refreshed_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
    ) -> u64 {
        self.wait_ordered_navigator_after(checkpoint, deadline, &["1", "2"], 1)
    }

    #[track_caller]
    pub(super) fn wait_ordered_navigator_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        labels: &[&str],
        active_position: usize,
    ) -> u64 {
        self.observe_after(
            checkpoint,
            deadline,
            "refreshed Workbench navigator",
            |screen| {
                let rows = screen.rows();
                if !screen.has_ordered_tabs(labels, active_position)
                    || !screen.has_ordered_navigator_rows(labels, active_position)
                    || !rows.iter().any(|row| row.contains("Workbench navigator"))
                {
                    return None;
                }
                rows.iter().find_map(|row| {
                    let suffix = row.split_once("Tabs refreshed · revision ")?.1;
                    let digits = suffix
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>();
                    (!digits.is_empty())
                        .then(|| digits.parse::<u64>().ok())
                        .flatten()
                })
            },
        )
    }

    #[track_caller]
    pub(super) fn wait_ordered_ready_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        labels: &[&str],
        active_position: usize,
        mode: ReadyMode,
    ) {
        let footer = mode.footer(active_position);
        self.observe_after(checkpoint, deadline, "ordered Tabs", |screen| {
            screen
                .has_ordered_tabs(labels, active_position)
                .then_some(())
        });
        self.probe_status(deadline, &mut [], |screen| {
            (screen.has_ordered_tabs(labels, active_position) && screen.text().contains(&footer))
                .then_some(())
        });
    }

    #[track_caller]
    pub(super) fn wait_terminal_output_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        output: &str,
    ) {
        self.observe_after(checkpoint, deadline, "fresh shell output frame", |screen| {
            screen.text().contains(output).then_some(())
        });
    }

    // A completed frame strictly newer than `checkpoint`, evaluated by the caller's
    // predicate; peer Clients are not pumped (single-Client observation windows).
    #[track_caller]
    pub(super) fn wait_completed_frame_after<T>(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        description: &str,
        predicate: impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> T {
        self.observe_after_pumping(checkpoint, deadline, description, &mut [], predicate)
    }

    // Keep the observer's fixed 120x32 capacity; this witness only shrinks the viewport.
    pub(super) fn resize_rows(&mut self, rows: u16) {
        self.resize(COLUMNS as u16, rows)
    }

    /// Apply an explicit winsize on the slave. Any axis beyond the observer's
    /// fixed canvas is refused: a witness must never report a frame it could
    /// not have stored. Zero axes stay legal so callers can drive zero-cell
    /// transitions; recovery still has to name a real nonzero size. A client
    /// spawned without pixel geometry keeps reporting zero pixels afterwards.
    pub(super) fn resize(&mut self, columns: u16, rows: u16) {
        assert!(
            usize::from(columns) <= COLUMNS && usize::from(rows) <= ROWS,
            "resize {columns}x{rows} exceeds the observer canvas {COLUMNS}x{ROWS}"
        );
        let (ws_xpixel, ws_ypixel) = if self.pixels {
            (columns.saturating_mul(8), rows.saturating_mul(20))
        } else {
            (0, 0)
        };
        rustix::termios::tcsetwinsize(
            &self.slave,
            rustix::termios::Winsize {
                ws_col: columns,
                ws_row: rows,
                ws_xpixel,
                ws_ypixel,
            },
        )
        .unwrap();
    }

    #[track_caller]
    pub(super) fn wait_resized_ready_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        rows: u16,
        active_position: usize,
    ) {
        let footer = ReadyMode::Controlling.footer(active_position);
        self.observe_after(checkpoint, deadline, "resized frame", |_| Some(()));
        self.probe_status(deadline, &mut [], |screen| {
            screen
                .rows()
                .get(usize::from(rows) - 1)
                .is_some_and(|row| row.contains(&footer))
                .then_some(())
        });
    }

    #[track_caller]
    pub(super) fn wait_navigator_closed_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        tab_number: usize,
        safe_label: &str,
        mode: ReadyMode,
    ) {
        let footer = mode.footer(tab_number);
        self.observe_after(
            checkpoint,
            deadline,
            "closed Workbench navigator",
            |frame| {
                (frame.has_active_tab(tab_number, safe_label)
                    && !frame.text().contains("Workbench navigator")
                    && !frame.text().contains("Tabs refreshed · revision "))
                .then_some(())
            },
        );
        self.probe_status(deadline, &mut [], |frame| {
            (frame.has_active_tab(tab_number, safe_label)
                && !frame.text().contains("Workbench navigator")
                && frame.text().contains(&footer))
            .then_some(())
        });
    }

    pub(super) fn contains_native_header(&mut self, header: WorkbenchHeader<'_>) -> bool {
        self.collect();
        self.screen
            .latest_frame()
            .is_some_and(|frame| frame.has_native_header(header))
    }

    pub(super) fn current_has_ordered_tabs(
        &mut self,
        labels: &[&str],
        active_position: usize,
    ) -> bool {
        self.collect();
        self.screen
            .latest_frame()
            .is_some_and(|frame| frame.has_ordered_tabs(labels, active_position))
    }

    pub(super) fn current_frame_matches(
        &mut self,
        header: WorkbenchHeader<'_>,
        required: &[&str],
    ) -> bool {
        self.collect();
        self.screen.latest_frame().is_some_and(|frame| {
            let text = frame.text();
            frame.has_native_header(header) && required.iter().all(|part| text.contains(part))
        })
    }

    pub(super) fn send(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).unwrap();
    }

    #[track_caller]
    pub(super) fn exit_message_before(&mut self, deadline: Instant) -> String {
        loop {
            self.collect();
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    Instant::now() < deadline,
                    "Client exited after deadline; {}",
                    self.diagnostic(0)
                );
                assert_eq!(status.code(), Some(1));
                self.assert_termios_restored();
                assert!(!self.contains("\x1b[?1049h"));
                return String::from_utf8_lossy(&self.stderr_bytes).into_owned();
            }
            assert!(
                Instant::now() < deadline,
                "Client did not exit; {}",
                self.diagnostic(0)
            );
            thread::yield_now();
        }
    }

    #[track_caller]
    pub(super) fn close_before(&mut self, deadline: Instant) {
        self.send(b"\x02q");
        loop {
            self.collect();
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    Instant::now() < deadline,
                    "Client exited after deadline; {}",
                    self.diagnostic(0)
                );
                assert!(
                    status.success(),
                    "Client exited {status}; {}",
                    self.diagnostic(0)
                );
                self.assert_termios_restored();
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Client did not exit; {}",
                self.diagnostic(0)
            );
            thread::yield_now();
        }
    }

    fn observe_after<T>(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        description: &str,
        predicate: impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> T {
        self.observe_after_pumping(checkpoint, deadline, description, &mut [], predicate)
    }

    fn observe_after_pumping<T>(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        description: &str,
        pumped_peers: &mut [&mut Client],
        mut predicate: impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> T {
        loop {
            self.collect();
            for peer in &mut *pumped_peers {
                peer.collect();
            }
            if let Some(value) =
                self.screen
                    .observe_completed_before(checkpoint, deadline, &mut predicate)
            {
                return value;
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                panic!(
                    "Client exited {status} before {description}; {}",
                    self.diagnostic(checkpoint)
                );
            }
            assert!(
                Instant::now() < deadline,
                "deadline waiting for {description}; {}",
                self.diagnostic(checkpoint)
            );
            thread::yield_now();
        }
    }

    pub(super) fn diagnostic(&self, checkpoint: usize) -> String {
        format!(
            "latest completed screen:\n{}\nworking screen:\n{}\nstderr: {:?}\nraw since checkpoint: {}",
            self.screen
                .latest_frame()
                .map(CompletedFrame::text)
                .unwrap_or_default(),
            self.screen.working_text(),
            String::from_utf8_lossy(&self.stderr_bytes),
            self.screen.raw_since(checkpoint)
        )
    }

    fn assert_termios_restored(&self) {
        assert_eq!(tcgetattr(&self.slave).unwrap(), self.original);
    }

    fn status(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().unwrap()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.status().is_none() {
            let _result = self.child.kill();
        }
        let _result = self.child.wait();
    }
}

struct OuterPty {
    master: OwnedFd,
    slave: OwnedFd,
}

fn open_outer_pty(columns: u16, rows: u16, pixels_known: bool) -> io::Result<OuterPty> {
    let master =
        posix_openpt(OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK)
            .map_err(io::Error::from)?;
    grantpt(&master).map_err(io::Error::from)?;
    unlockpt(&master).map_err(io::Error::from)?;
    let slave_name = ptsname_r(&master).map_err(io::Error::from)?;
    let slave = open(
        Path::new(&slave_name),
        OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    // `pixels_known = false` models hosts that report zero pixel geometry —
    // the cell-addressed pointer path must not depend on them.
    let (ws_xpixel, ws_ypixel) = if pixels_known {
        (columns.saturating_mul(8), rows.saturating_mul(20))
    } else {
        (0, 0)
    };
    rustix::termios::tcsetwinsize(
        &slave,
        rustix::termios::Winsize {
            ws_col: columns,
            ws_row: rows,
            ws_xpixel,
            ws_ypixel,
        },
    )?;
    Ok(OuterPty {
        master: master.into(),
        slave,
    })
}

fn clone_cloexec(descriptor: &OwnedFd) -> io::Result<OwnedFd> {
    descriptor.as_fd().try_clone_to_owned()
}

pub(super) fn wait_file(path: &Path, clients: &mut [&mut Client], deadline: Instant) -> String {
    loop {
        for client in clients.iter_mut() {
            client.collect();
            if let Some(status) = client.status() {
                panic!(
                    "Client exited {status} before shell evidence {}; {}",
                    path.display(),
                    client.diagnostic(0)
                );
            }
        }
        if let Some(value) = nonempty_file_before(path, deadline) {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "missing shell evidence {}; {}",
            path.display(),
            clients
                .iter()
                .map(|client| client.diagnostic(0))
                .collect::<Vec<_>>()
                .join("\n")
        );
        thread::yield_now();
    }
}

fn nonempty_file_before(path: &Path, deadline: Instant) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    (!value.is_empty() && Instant::now() < deadline).then_some(value)
}

/// `wait_file` for a needle that is not the first bytes — pane-side recorders
/// accumulate, so witnesses wait on a marker substring, not on nonemptiness.
pub(super) fn wait_file_containing(
    path: &Path,
    needle: &str,
    clients: &mut [&mut Client],
    deadline: Instant,
) -> String {
    loop {
        for client in clients.iter_mut() {
            client.collect();
            if let Some(status) = client.status() {
                panic!(
                    "Client exited {status} before shell evidence {}; {}",
                    path.display(),
                    client.diagnostic(0)
                );
            }
        }
        if fs::read_to_string(path)
            .ok()
            .is_some_and(|value| value.contains(needle))
            && Instant::now() < deadline
        {
            return fs::read_to_string(path).unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "missing {needle:?} in shell evidence {}; {}",
            path.display(),
            clients
                .iter()
                .map(|client| client.diagnostic(0))
                .collect::<Vec<_>>()
                .join("\n")
        );
        thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{FcntlArg, FdFlag, OFlag, fcntl};

    #[test]
    fn expired_deadline_rejects_a_preloaded_file_observation() {
        let expired = Instant::now() - Duration::from_secs(1);
        assert!(nonempty_file_before(Path::new("/proc/self/cmdline"), expired).is_none());
    }

    #[test]
    fn outer_pty_and_stdio_clones_are_cloexec_from_creation() {
        let pty = open_outer_pty(COLUMNS as u16, ROWS as u16, true).unwrap();
        let stdin = clone_cloexec(&pty.slave).unwrap();
        let stdout = clone_cloexec(&pty.slave).unwrap();
        for descriptor in [&pty.master, &pty.slave, &stdin, &stdout] {
            let flags = FdFlag::from_bits_truncate(fcntl(descriptor, FcntlArg::F_GETFD).unwrap());
            assert!(flags.contains(FdFlag::FD_CLOEXEC));
        }
        let master_flags =
            OFlag::from_bits_truncate(fcntl(&pty.master, FcntlArg::F_GETFL).unwrap());
        assert!(master_flags.contains(OFlag::O_NONBLOCK));
        assert_eq!(
            rustix::termios::tcgetwinsize(&pty.slave).unwrap(),
            rustix::termios::Winsize {
                ws_col: COLUMNS as u16,
                ws_row: ROWS as u16,
                ws_xpixel: 960,
                ws_ypixel: 640,
            }
        );
    }
}
