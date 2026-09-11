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
use runtime::{Process, Runtime};
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
    ))?;
    assert!(matches!(
        created.outcome(),
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { .. }
        }
    ));
    drop(client);
    let pty = openpty(
        Some(&Winsize {
            ws_col: 120,
            ws_row: 32,
            ws_xpixel: 0,
            ws_ypixel: 0,
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
        "while [ \"$(stty size)\" != \"21 68\" ]; do sleep 0.02; done; stty size > '{}'\n",
        dimensions_file.display()
    );
    master.write_all(command.as_bytes())?;
    pump_until(&mut master, &mut screen, |_| {
        fs::read_to_string(&dimensions_file).is_ok_and(|s| s.trim() == "21 68")
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
