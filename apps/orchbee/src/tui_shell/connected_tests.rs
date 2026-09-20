use crate::{local_client::LocalClient, localization::Locale, terminal_session::TerminalIntent};
use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    pty::{Winsize, openpty},
    sys::termios::tcgetattr,
    unistd::dup,
};
use orchbee_client_server_protocol::{
    CreateAcceptedState, CreateOutcome, Geometry, RequestId, StartMode, TerminalCreateRequest,
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::fd::AsFd,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
mod runtime;
use runtime::{Process, Runtime, wait_for};
const CREATE_ID: &str = "11111111111111111111111111111111";
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "child entry used only by the real connected TUI process witness"]
fn child_runs_connected_tui() -> TestResult {
    let endpoint = std::env::var_os("ORCHBEE_TUI_TEST_CLIENT_ENDPOINT")
        .ok_or("parent test endpoint missing")?;
    let client = LocalClient::connect(std::path::Path::new(&endpoint), env!("CARGO_PKG_VERSION"))?;
    super::run_connected(
        Locale::EnUs,
        client,
        TerminalIntent::Attach(RequestId::parse(CREATE_ID)?),
        true,
    )?;
    Ok(())
}

#[test]
fn real_tui_renders_shell_resizes_and_restores_modes_while_terminal_is_retained() -> TestResult {
    let mut runtime = Runtime::start()?;
    let mut client = runtime.connect()?;
    let created = client.submit_terminal_create(TerminalCreateRequest::new(
        RequestId::parse(CREATE_ID)?,
        None,
        Some(orchbee_client_server_protocol::WorkingDirectory::parse(
            runtime.directory.to_str().ok_or("cwd")?,
        )?),
        StartMode::Immediate {
            geometry: Geometry::new(80, 24, 8, 16)?,
        },
        None,
    ))?;
    assert!(matches!(
        created.outcome(),
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { .. }
        }
    ));
    drop(client);
    // The PTY must declare valid cell-pixel geometry: the TUI derives cell width/height from
    // ws_xpixel/ws_col and ws_ypixel/ws_row, and a zero-pixel PTY is rejected by Geometry::new.
    let pty = openpty(
        Some(&Winsize {
            ws_col: 120,
            ws_row: 32,
            ws_xpixel: 120 * 8,
            ws_ypixel: 32 * 16,
        }),
        None,
    )?;
    let before = tcgetattr(&pty.slave)?;
    let mut child = Process(
        Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "tui_shell::connected_tests::child_runs_connected_tui",
                "--ignored",
                "--nocapture",
            ])
            .env("ORCHBEE_TUI_TEST_CLIENT_ENDPOINT", &runtime.client)
            .stdin(Stdio::from(File::from(dup(&pty.slave)?)))
            .stdout(Stdio::from(File::from(dup(&pty.slave)?)))
            .stderr(Stdio::piped())
            .spawn()?,
    );
    fcntl(pty.master.as_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    let mut master = File::from(pty.master);
    let mut screen = Screen::new(120, 32);
    pump_until(&mut master, &mut screen, |screen| {
        screen.text().contains("Terminal connected")
    })?;
    master.write_all(b"printf '\\164\\165\\151\\055\\146\\162\\141\\155\\145\\055\\067\\063\\071\\062\\066\\064\\n'\n")?;
    pump_until(&mut master, &mut screen, |screen| {
        screen.text().contains("tui-frame-739264")
    })?;
    rustix::termios::tcsetwinsize(
        &pty.slave,
        rustix::termios::Winsize {
            ws_col: 100,
            ws_row: 28,
            ws_xpixel: 800,
            ws_ypixel: 560,
        },
    )?;
    let dimensions_file = runtime.directory.join("dimensions");
    let command = format!(
        "while [ \"$(stty size)\" != \"24 68\" ]; do sleep 0.02; done; stty size > '{}'\n",
        dimensions_file.display()
    );
    master.write_all(command.as_bytes())?;
    pump_until(&mut master, &mut screen, |_| {
        fs::read_to_string(&dimensions_file).is_ok_and(|s| s.trim() == "24 68")
    })?;
    master.write_all(b"\x02q")?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        screen.drain(&mut master)?;
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("TUI did not exit".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut stderr = String::new();
    if let Some(mut pipe) = child.0.stderr.take() {
        pipe.read_to_string(&mut stderr)?;
    }
    assert!(status.success(), "child failed: {stderr}");
    let restored = tcgetattr(&pty.slave)?;
    assert_eq!(restored.input_flags, before.input_flags);
    assert_eq!(restored.output_flags, before.output_flags);
    assert_eq!(restored.control_flags, before.control_flags);
    // Match the existing PTY witness: PENDIN is kernel retype state, not an acquired mode.
    assert_eq!(
        restored.local_flags & !nix::sys::termios::LocalFlags::PENDIN,
        before.local_flags & !nix::sys::termios::LocalFlags::PENDIN
    );
    assert_eq!(restored.control_chars, before.control_chars);
    assert!(
        screen.alternate_entered && screen.alternate_left,
        "alternate screen was not restored"
    );
    assert_eq!(
        runtime
            .retained_and_stop()
            .map_err(|error| format!("retained inventory and cleanup: {error}"))?,
        1
    );
    Ok(())
}

/// Locates ASCII text on the observed screen, returning its 0-based
/// `(column, row)` — a cell guaranteed to be inside the pane viewport that
/// painted it.
fn find_text(screen: &Screen, needle: &[u8]) -> Option<(usize, usize)> {
    text_positions(screen, needle).into_iter().next()
}

fn text_positions(screen: &Screen, needle: &[u8]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (row, cells) in screen.cells.iter().enumerate() {
        for column in 0..cells.len().saturating_sub(needle.len().saturating_sub(1)) {
            if cells[column..].starts_with(needle) {
                found.push((column, row));
            }
        }
    }
    found
}

/// One SGR mouse report a pane recorder captured: `(button_code, x, y,
/// final)` — the raw bytes the pane program received after the client's
/// pane-local re-encoding.
fn sgr_reports(log: &[u8]) -> Vec<(u8, u16, u16, u8)> {
    let mut reports = Vec::new();
    let mut rest = log;
    while let Some(start) = rest.windows(3).position(|w| w == b"\x1b[<") {
        rest = &rest[start + 3..];
        let Some(end) = rest.iter().position(|b| *b == b'M' || *b == b'm') else {
            break;
        };
        let fields = String::from_utf8_lossy(&rest[..end]).to_string();
        let mut parts = fields.split(';');
        if let (Some(code), Some(x), Some(y)) = (
            parts.next().and_then(|v| v.parse::<u8>().ok()),
            parts.next().and_then(|v| v.parse::<u16>().ok()),
            parts.next().and_then(|v| v.parse::<u16>().ok()),
        ) {
            reports.push((code, x, y, rest[end]));
        }
        rest = &rest[end + 1..];
    }
    reports
}

/// An SGR host report for `column`/`row` in 0-based screen cells.
fn sgr_report(button: u8, column: usize, row: usize, press: bool) -> Vec<u8> {
    format!(
        "\x1b[<{button};{};{}{}",
        column + 1,
        row + 1,
        if press { 'M' } else { 'm' }
    )
    .into_bytes()
}

fn spawn_connected_tui(
    runtime: &Runtime,
    slave: &std::os::fd::OwnedFd,
) -> Result<Process, Box<dyn std::error::Error>> {
    Ok(Process(
        Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "tui_shell::connected_tests::child_runs_connected_tui",
                "--ignored",
                "--nocapture",
            ])
            .env("ORCHBEE_TUI_TEST_CLIENT_ENDPOINT", &runtime.client)
            .stdin(Stdio::from(File::from(dup(slave)?)))
            .stdout(Stdio::from(File::from(dup(slave)?)))
            .stderr(Stdio::piped())
            .spawn()?,
    ))
}

fn wait_child_exit(child: &mut Process, screen: &mut Screen, master: &mut File) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        screen.drain(master)?;
        if child.0.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("TUI did not exit".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes() -> TestResult {
    let mut runtime = Runtime::start()?;
    let mut client = runtime.connect()?;
    let created = client.submit_terminal_create(TerminalCreateRequest::new(
        RequestId::parse(CREATE_ID)?,
        None,
        Some(orchbee_client_server_protocol::WorkingDirectory::parse(
            runtime.directory.to_str().ok_or("cwd")?,
        )?),
        StartMode::Immediate {
            geometry: Geometry::new(80, 24, 8, 16)?,
        },
        None,
    ))?;
    assert!(matches!(
        created.outcome(),
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { .. }
        }
    ));
    drop(client);
    let window = Winsize {
        ws_col: 120,
        ws_row: 32,
        ws_xpixel: 120 * 8,
        ws_ypixel: 32 * 16,
    };
    // The controlling TUI on PTY A.
    let pty_a = openpty(Some(&window), None)?;
    let before_a = tcgetattr(&pty_a.slave)?;
    let mut child_a = spawn_connected_tui(&runtime, &pty_a.slave)?;
    fcntl(pty_a.master.as_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    let mut master_a = File::from(pty_a.master);
    let mut screen_a = Screen::new(120, 32);
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("Terminal connected")
    })?;
    // Pointer capture armed on the real wire before the first click.
    assert_eq!(screen_a.pointer_arms, 1, "pointer capture not armed");
    // A second TUI on PTY B attaches the same terminal and is read-only.
    let pty_b = openpty(Some(&window), None)?;
    let before_b = tcgetattr(&pty_b.slave)?;
    let mut child_b = spawn_connected_tui(&runtime, &pty_b.slave)?;
    fcntl(pty_b.master.as_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    let mut master_b = File::from(pty_b.master);
    let mut screen_b = Screen::new(120, 32);
    pump_until(&mut master_b, &mut screen_b, |screen| {
        screen.text().contains("Viewing only")
    })?;
    assert_eq!(
        screen_b.pointer_arms, 1,
        "read-only TUI did not arm capture"
    );

    // The synthetic mouse program requests button tracking plus SGR
    // *pixel* format first: cell-capable panes asking for `?1016` get a
    // bounded localized refusal, never fabricated cell coordinates.
    let pane_pixel = runtime.directory.join("pane-pixel.log");
    let pane_one = runtime.directory.join("pane-one.log");
    // The recorder must read the pane non-canonically: SGR reports carry
    // no newline, so a cooked line discipline would hold them forever.
    master_a.write_all(
        format!(
            "printf '\\033[?1002h\\033[?1006h\\033[?1016hPX1\\n'; stty -icanon -echo; cat > '{}'\n",
            pane_pixel.display()
        )
        .as_bytes(),
    )?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("PX1")
    })?;
    // The frame that painted the marker already carries the pane's
    // interaction contract, so the click adjudicates against pixels.
    let (px, py) = find_text(&screen_a, b"PX1").ok_or("pixel marker")?;
    master_a.write_all(&sgr_report(0, px, py, true))?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("pixel mouse reports")
    })?;
    assert!(
        sgr_reports(&fs::read(&pane_pixel)?).is_empty(),
        "pixel-mode pane received a fabricated cell report"
    );
    // The release closes the refused gesture before the recorder dies.
    master_a.write_all(&sgr_report(0, px, py, false))?;

    // Recovery: drop the pixel request, re-arm cell reporting, restart the
    // recorder — the same pane then receives ordinary cell reports.
    master_a.write_all(b"\x03")?;
    std::thread::sleep(Duration::from_millis(300));
    screen_a.drain(&mut master_a)?;
    master_a.write_all(b"printf 'P1BACK\\n'\n")?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("P1BACK")
    })?;
    master_a.write_all(
        format!(
            "printf '\\033[?1016l\\033[?1002h\\033[?1006hR1DY\\n'; cat > '{}'\n",
            pane_one.display()
        )
        .as_bytes(),
    )?;
    // termios already runs non-canonical from the first recorder.
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("R1DY")
    })?;
    let (col, row) = find_text(&screen_a, b"R1DY").ok_or("cell marker")?;
    master_a.write_all(&sgr_report(0, col, row, true))?;
    master_a.write_all(&sgr_report(0, col, row, false))?;
    wait_for(|| fs::read(&pane_one).is_ok_and(|log| sgr_reports(&log).len() >= 2))?;
    let reports = sgr_reports(&fs::read(&pane_one)?);
    assert_eq!(reports[0].0, 0, "press button code");
    assert_eq!(reports[0].3, b'M', "press terminator");
    assert_eq!(reports[1].3, b'm', "release terminator");
    assert_eq!(
        (reports[0].1, reports[0].2),
        (reports[1].1, reports[1].2),
        "press/release cell pair"
    );
    // A second click three cells right lands three cells right in
    // pane-local coordinates — a pure translation, never a screen cell.
    master_a.write_all(&sgr_report(0, col + 3, row, true))?;
    master_a.write_all(&sgr_report(0, col + 3, row, false))?;
    wait_for(|| fs::read(&pane_one).is_ok_and(|log| sgr_reports(&log).len() >= 4))?;
    let reports = sgr_reports(&fs::read(&pane_one)?);
    assert_eq!(
        (reports[2].1, reports[2].2),
        (reports[0].1 + 3, reports[0].2),
        "pane-local translation"
    );

    // The read-only client clicks the same pane: nothing reaches it and a
    // localized notice explains the drop.
    pump_until(&mut master_b, &mut screen_b, |screen| {
        screen.text().contains("R1DY")
    })?;
    let (bc, br) = find_text(&screen_b, b"R1DY").ok_or("marker on B")?;
    let before = sgr_reports(&fs::read(&pane_one)?).len();
    master_b.write_all(&sgr_report(0, bc, br, true))?;
    pump_until(&mut master_b, &mut screen_b, |screen| {
        screen.text().contains("Terminal input unavailable")
    })?;
    assert_eq!(sgr_reports(&fs::read(&pane_one)?).len(), before);

    // The recorder drops reporting and enters the alternate screen: a
    // wheel notch over it becomes bounded cursor arrows — three per
    // notch — because a non-reporting alternate-screen pane owns wheel
    // input only through the fallback.
    let pane_alt = runtime.directory.join("pane-alt.log");
    master_a.write_all(b"\x03")?;
    std::thread::sleep(Duration::from_millis(300));
    screen_a.drain(&mut master_a)?;
    master_a.write_all(b"printf 'P1ALT\\n'\n")?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("P1ALT")
    })?;
    master_a.write_all(
        format!(
            "printf '\\033[?1002l\\033[?1049hALTIN\\n'; stty -icanon -echo; cat > '{}'\n",
            pane_alt.display()
        )
        .as_bytes(),
    )?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.text().contains("ALTIN")
    })?;
    let (acol, arow) = find_text(&screen_a, b"ALTIN").ok_or("alt marker")?;
    master_a.write_all(&sgr_report(65, acol, arow, true))?;
    wait_for(|| {
        fs::read(&pane_alt).is_ok_and(|log| {
            log.windows(3)
                .filter(|w| *w == b"\x1b[B" || *w == b"\x1bOB")
                .count()
                >= 3
        })
    })?;

    // `Ctrl+B m` releases host capture on the wire and shows the
    // persistent released indicator; a second chord re-arms it.
    master_a.write_all(b"\x02m")?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.pointer_releases >= 1 && screen.text().contains("mouse released")
    })?;
    let before = fs::read(&pane_one)?.len();
    master_a.write_all(&sgr_report(0, col, row, true))?;
    std::thread::sleep(Duration::from_millis(300));
    screen_a.drain(&mut master_a)?;
    assert_eq!(fs::read(&pane_one)?.len(), before, "released capture input");
    master_a.write_all(b"\x02m")?;
    pump_until(&mut master_a, &mut screen_a, |screen| {
        screen.pointer_arms >= 2
    })?;

    // Exit: termios, the alternate screen, and pointer capture all restore.
    master_a.write_all(b"\x02q")?;
    wait_child_exit(&mut child_a, &mut screen_a, &mut master_a)?;
    let mut stderr = String::new();
    if let Some(mut pipe) = child_a.0.stderr.take() {
        pipe.read_to_string(&mut stderr)?;
    }
    assert!(stderr.is_empty(), "child A stderr: {stderr}");
    let restored = tcgetattr(&pty_a.slave)?;
    assert_eq!(restored.input_flags, before_a.input_flags);
    assert_eq!(restored.output_flags, before_a.output_flags);
    assert_eq!(restored.control_flags, before_a.control_flags);
    assert_eq!(
        restored.local_flags & !nix::sys::termios::LocalFlags::PENDIN,
        before_a.local_flags & !nix::sys::termios::LocalFlags::PENDIN
    );
    assert_eq!(restored.control_chars, before_a.control_chars);
    assert!(screen_a.alternate_entered && screen_a.alternate_left);
    assert!(
        screen_a.pointer_releases >= 2,
        "pointer capture not released on exit: {}",
        screen_a.pointer_releases
    );

    master_b.write_all(b"\x02q")?;
    wait_child_exit(&mut child_b, &mut screen_b, &mut master_b)?;
    let mut stderr_b = String::new();
    if let Some(mut pipe) = child_b.0.stderr.take() {
        pipe.read_to_string(&mut stderr_b)?;
    }
    assert!(stderr_b.is_empty(), "child B stderr: {stderr_b}");
    let restored_b = tcgetattr(&pty_b.slave)?;
    assert_eq!(restored_b.input_flags, before_b.input_flags);
    assert!(
        screen_b.pointer_releases >= 1,
        "read-only TUI did not release capture"
    );

    assert_eq!(
        runtime
            .retained_and_stop()
            .map_err(|error| format!("retained inventory and cleanup: {error}"))?,
        1
    );
    Ok(())
}

fn pump_until(
    master: &mut File,
    screen: &mut Screen,
    mut predicate: impl FnMut(&Screen) -> bool,
) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        screen.drain(master)?;
        if predicate(screen) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!("TUI display deadline; screen: {}", screen.text()).into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
// A test-only observer for this backend's absolute cursor writes and CSI modes, not a product VT.
struct Screen {
    cells: Vec<Vec<u8>>,
    x: usize,
    y: usize,
    escape: Option<Vec<u8>>,
    alternate_entered: bool,
    alternate_left: bool,
    /// `?1002`/`?1006` arm and release writes observed on the wire — the
    /// pointer-capture lease's real bytes, counted so a toggle pair is
    /// distinguishable from a single arm.
    pointer_arms: usize,
    pointer_releases: usize,
    bytes: usize,
}
impl Screen {
    fn new(columns: usize, rows: usize) -> Self {
        Self {
            cells: vec![vec![b' '; columns]; rows],
            x: 0,
            y: 0,
            escape: None,
            alternate_entered: false,
            alternate_left: false,
            pointer_arms: 0,
            pointer_releases: 0,
            bytes: 0,
        }
    }
    fn drain(&mut self, reader: &mut File) -> io::Result<()> {
        let mut bytes = [0; 8192];
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    for byte in &bytes[..count] {
                        self.feed(*byte)?;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }
    fn feed(&mut self, byte: u8) -> io::Result<()> {
        self.bytes += 1;
        if self.bytes > 8 * 1024 * 1024 {
            return Err(io::Error::other("TUI output exceeded witness bound"));
        }
        if let Some(escape) = self.escape.as_mut() {
            escape.push(byte);
            if escape.len() > 128 {
                return Err(io::Error::other("invalid backend escape"));
            }
            if escape.len() >= 2 && (0x40..=0x7e).contains(&byte) {
                let value = String::from_utf8_lossy(escape).to_string();
                self.escape = None;
                if value == "[?1049h" {
                    self.alternate_entered = true;
                }
                if value == "[?1049l" {
                    self.alternate_left = true;
                }
                // `?1002`/`?1006` are always written as a pair; counting the
                // tracking member suffices for both.
                if value == "[?1002h" {
                    self.pointer_arms += 1;
                }
                if value == "[?1002l" {
                    self.pointer_releases += 1;
                }
                if byte == b'H' {
                    let mut coordinates = value
                        .trim_start_matches('[')
                        .trim_end_matches('H')
                        .split(';');
                    self.y = coordinates
                        .next()
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .saturating_sub(1);
                    self.x = coordinates
                        .next()
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .saturating_sub(1);
                }
                if value == "[2J" {
                    for row in &mut self.cells {
                        row.fill(b' ');
                    }
                }
            }
        } else if byte == 27 {
            self.escape = Some(Vec::new());
        } else if byte == b'\r' {
            self.x = 0;
        } else if byte == b'\n' {
            self.y += 1;
        } else if byte.is_ascii_graphic() || byte == b' ' {
            if let Some(cell) = self
                .cells
                .get_mut(self.y)
                .and_then(|row| row.get_mut(self.x))
            {
                *cell = byte;
            }
            self.x += 1;
        }
        Ok(())
    }
    fn text(&self) -> String {
        self.cells
            .iter()
            .map(|row| String::from_utf8_lossy(row).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
