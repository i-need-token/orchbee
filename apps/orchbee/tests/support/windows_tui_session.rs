//! One real local runtime for Windows process tests: the CLI, Server and Worker under test and a
//! Zig Host built from this checkout as one private cohort, the default Session under observation,
//! and a `host` Session whose terminal runs a real TUI Client. A test owns its root, Sessions,
//! controlling attachments and processes until they drop.

#[path = "inherited_environment.rs"]
mod inherited_environment;

use orchbee_local_runtime::CohortArtifact;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Bounds the TUI Client's startup inside the second Session's terminal.
const STARTUP: Duration = Duration::from_secs(40);
/// Bounds each wait for an observation of the Client.
pub const OBSERVATION: Duration = Duration::from_secs(10);
/// A modal state is settled once the Client reports one phase on this many reads in a row,
/// 200 ms apart; the Client's own transient work holds it busy only for moments.
const SETTLED_READS: usize = 4;
/// Bounds one controlling attachment, well past any test that holds one.
const CONTROL_TIMEOUT_MILLISECONDS: &str = "120000";

/// One real local runtime with the default Session under observation and a `host` Session whose
/// terminals run TUI Clients.
pub struct Fixture {
    root: PathBuf,
    prefix: &'static str,
}

/// The TUI Client and the terminal of the `host` Session it runs in.
pub struct Tui {
    pub client: String,
    pub terminal: String,
}

/// One CLI invocation's observed exit, duration and machine output.
pub struct Run {
    pub words: String,
    pub code: Option<i32>,
    pub elapsed: Duration,
    pub document: Value,
    pub stderr: String,
}

impl Fixture {
    /// Creates a private temporary root named after `prefix` with the shared directories and
    /// `directories`, stages the cohort under test and starts both Sessions.
    pub fn new(prefix: &'static str, directories: &[&str]) -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root =
            std::env::temp_dir().join(format!("{prefix}{:032x}", u128::from_le_bytes(nonce)));
        for path in [
            "cohort", "runtime", "home", "local", "roaming", "work", "noise",
        ]
        .iter()
        .chain(directories)
        {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        let fixture = Self { root, prefix };
        fixture.stage();
        for session in ["default", "host"] {
            let started = fixture.run(&["server", "start", "--session", session]);
            assert_eq!(started.code, Some(0), "{started}");
        }
        fixture
    }

    /// Stages the CLI, Server and Worker under test with a Host built from this checkout as one
    /// private cohort. `cargo test` builds only the CLI and `zig-out` keeps whichever Host was
    /// installed last, so after a rebase either peer or the Host could otherwise predate the code
    /// under test and fail as if that code were wrong.
    fn stage(&self) {
        let built = Path::new(env!("CARGO_BIN_EXE_orchbee")).parent().unwrap();
        let mut artifacts = serde_json::Map::new();
        for (name, artifact, source) in [
            ("orchbee", CohortArtifact::Client, built.join("orchbee.exe")),
            (
                "orchbee-server",
                CohortArtifact::Server,
                current_peer(built, "orchbee-server"),
            ),
            (
                "orchbee-worker",
                CohortArtifact::Worker,
                current_peer(built, "orchbee-worker"),
            ),
            (
                "orchbee-terminal-host",
                CohortArtifact::TerminalHost,
                self.build_host(),
            ),
        ] {
            let bytes =
                fs::read(&source).unwrap_or_else(|error| panic!("{}: {error}", source.display()));
            // Native cohort admission requires private regular files with one link, and each
            // carries its environment name rather than Cargo's release name.
            fs::write(self.root.join("cohort").join(artifact.file_name()), &bytes).unwrap();
            artifacts.insert(
                name.to_owned(),
                json!(format!("{:x}", Sha256::digest(&bytes))),
            );
        }
        // The product ships a pinned ConPTY beside its Host; a build without one uses the system
        // ConHost, which drops the private markers the shell's reader sends. Point this at a
        // verified bundle to witness the product's own console host too.
        if let Some(bundle) = std::env::var_os("ORCHBEE_TEST_CONPTY_DIR") {
            let target = self.root.join("cohort/conpty");
            fs::create_dir_all(&target).unwrap();
            for file in ["conpty.dll", "OpenConsole.exe"] {
                fs::copy(Path::new(&bundle).join(file), target.join(file)).unwrap();
            }
        }
        artifacts.insert(
            "agent-observation-adapters".to_owned(),
            json!(format!("{:x}", Sha256::digest(b"unused adapters"))),
        );
        fs::write(
            self.root.join("cohort/cohort.json"),
            serde_json::to_vec(&json!({"schemaVersion": 1, "artifacts": artifacts})).unwrap(),
        )
        .unwrap();
    }

    /// Builds the Host from this checkout into the fixture root, in the environment this test
    /// build and its peers share, as `pnpm --filter @orchbee/terminal-host build` does into
    /// `zig-out`. Zig's cache returns an unchanged build in well under a second.
    fn build_host(&self) -> PathBuf {
        let prefix = self.root.join("host");
        let environment = format!(
            "-Denvironment={}",
            orchbee_local_runtime::environment_stem()
        );
        let build = Command::new("zig")
            .args([
                "build",
                "-Doptimize=ReleaseSafe",
                environment.as_str(),
                "--prefix",
            ])
            .arg(&prefix)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../terminal-host"))
            .output()
            .unwrap_or_else(|error| panic!("start `zig build` for the Host: {error}"));
        assert!(
            build.status.success(),
            "the Host build failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        prefix.join("bin/orchbee-terminal-host.exe")
    }

    /// The staged CLI under test.
    fn staged_client(&self) -> PathBuf {
        self.root
            .join("cohort")
            .join(CohortArtifact::Client.file_name())
    }

    /// Answers the state variables of this fixture's isolated environment, so a test resolves the
    /// state locations the CLI under test uses.
    pub fn state_environment(&self) -> impl Fn(&str) -> Option<std::ffi::OsString> + use<> {
        let root = self.root.clone();
        move |name| {
            let directory = match name {
                "USERPROFILE" => "home",
                "APPDATA" => "roaming",
                "LOCALAPPDATA" => "local",
                _ => return None,
            };
            Some(root.join(directory).into_os_string())
        }
    }

    /// The CLI under test in this fixture's isolated environment.
    pub fn command(&self, words: &[&str]) -> Command {
        let mut command = inherited_environment::orchbee_command(self.staged_client());
        command
            .args(words)
            .env("ORCHBEE_RUNTIME_BASE", self.root.join("runtime"))
            .env("USERPROFILE", self.root.join("home"))
            .env("HOME", self.root.join("home"))
            .env("APPDATA", self.root.join("roaming"))
            .env("LOCALAPPDATA", self.root.join("local"))
            .env("LC_ALL", "en-US")
            .current_dir(&self.root)
            .stdin(Stdio::null());
        command
    }

    /// Runs one CLI verb with machine output.
    pub fn run(&self, words: &[&str]) -> Run {
        let started = Instant::now();
        let output = self.command(words).arg("--format=json").output().unwrap();
        Run {
            words: words.join(" "),
            code: output.status.code(),
            elapsed: started.elapsed(),
            document: serde_json::from_slice(&output.stdout).unwrap_or(Value::Null),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    pub fn path(&self, relative: &str) -> String {
        self.root.join(relative).to_str().unwrap().to_owned()
    }

    /// The isolated environment and a system-only `PATH` for terminals this fixture starts.
    pub fn terminal_environment(&self) -> Vec<String> {
        let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        let mut words = Vec::new();
        for (name, value) in [
            (
                "PATH",
                format!("{};{}", system.join("System32").display(), system.display()),
            ),
            ("USERPROFILE", self.path("home")),
            ("HOME", self.path("home")),
            ("APPDATA", self.path("roaming")),
            ("LOCALAPPDATA", self.path("local")),
            ("LC_ALL", "en-US".to_owned()),
        ] {
            words.push("--env".to_owned());
            words.push(format!("{name}={value}"));
        }
        words
    }

    /// Opens a Workspace in the default Session whose terminal starts in `work`, and returns its
    /// panel.
    pub fn workspace(&self, label: &str) -> String {
        let created = self.workspace_in(label, "work");
        created["panelId"]
            .as_str()
            .unwrap_or_else(|| panic!("{created}"))
            .to_owned()
    }

    /// Opens a Workspace in the default Session whose terminal starts in `directory`, one of the
    /// directories this fixture created, and returns its creation result.
    pub fn workspace_in(&self, label: &str, directory: &str) -> Value {
        let cwd = self.path(directory);
        let mut words = vec![
            "workspace",
            "create",
            "--label",
            label,
            "--cwd",
            &cwd,
            "--no-focus",
        ];
        let environment = self.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
        created.document["workspace"]["result"].clone()
    }

    /// Starts a terminal in another Workspace that prints every second until the Session stops,
    /// and returns its creation result.
    pub fn background_output(&self) -> Value {
        let ping = system_program("PING.EXE");
        let noise = self.path("noise");
        let mut words = vec![
            "workspace",
            "create",
            "--label",
            "noise",
            "--cwd",
            &noise,
            "--no-focus",
            "--exec",
            &ping,
            "--arg",
            "-t",
            "--arg",
            "127.0.0.1",
        ];
        let environment = self.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
        created.document["workspace"]["result"].clone()
    }

    /// Runs `orchbee tui` in a terminal of the `host` Session at `columns` by `rows`. That
    /// terminal marks itself as an Orchbee terminal; the launcher clears those marks so the TUI
    /// attaches to the default Session as a top-level Client.
    pub fn start_tui(&self, columns: u16, rows: u16) -> Tui {
        let existing: Vec<_> = self
            .clients()
            .into_iter()
            .filter_map(|client| client["clientId"].as_str().map(str::to_owned))
            .collect();
        let launcher = self.root.join("tui.cmd");
        fs::write(
            &launcher,
            format!(
                "@echo off\r\nset ORCHBEE_TERMINAL_ID=\r\nset ORCHBEE_WORKER_ID=\r\n\
                 set ORCHBEE_RUNTIME_SCOPE=\r\nset ORCHBEE_SESSION=\r\nset NO_COLOR=\r\nset COLORTERM=truecolor\r\n\"{}\" tui 2>\"{}\"\r\n",
                self.staged_client().display(),
                self.root.join(format!("tui-{}.stderr", existing.len())).display()
            ),
        )
        .unwrap();
        let shell = system_program("cmd.exe");
        let work = self.path("work");
        let columns = columns.to_string();
        let rows = rows.to_string();
        let mut words = vec![
            "workspace",
            "create",
            "--session",
            "host",
            "--label",
            "tui",
            "--cwd",
            &work,
            "--no-focus",
            "--cols",
            &columns,
            "--rows",
            &rows,
            "--cell-width",
            "8",
            "--cell-height",
            "16",
            "--exec",
            &shell,
            "--arg",
            "/d",
            "--arg",
            "/c",
            "--arg",
            launcher.to_str().unwrap(),
        ];
        let environment = self.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.run(&words);
        let panel = created.document["workspace"]["result"]["panelId"]
            .as_str()
            .unwrap_or_else(|| panic!("{created}"))
            .to_owned();

        let deadline = Instant::now() + STARTUP;
        let client = loop {
            if let Some(client) = self.clients().into_iter().find(|client| {
                client["view"]["phase"] == "ready"
                    && client["clientId"]
                        .as_str()
                        .is_some_and(|id| !existing.iter().any(|old| old == id))
            }) {
                break client["clientId"].as_str().unwrap().to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "the TUI Client never became ready"
            );
            thread::sleep(Duration::from_millis(250));
        };
        let listed = self.run(&["terminal", "list", "--session", "host"]);
        let terminal = listed.document["result"]["terminals"]
            .as_array()
            .and_then(|terminals| {
                terminals
                    .iter()
                    .find(|terminal| terminal["boundTo"]["panelId"] == panel.as_str())
            })
            .and_then(|terminal| terminal["terminalId"].as_str())
            .unwrap_or_else(|| panic!("{listed}"))
            .to_owned();
        Tui { client, terminal }
    }

    pub fn clients(&self) -> Vec<Value> {
        let listed = self.run(&["client", "list"]);
        listed.document["result"]["outcome"]["clients"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| panic!("{listed}"))
    }

    pub fn client(&self, id: &str) -> Value {
        self.clients()
            .into_iter()
            .find(|client| client["clientId"] == id)
            .unwrap_or_else(|| panic!("Client {id} left the Session"))
    }

    /// Types keys into the TUI through its `host` terminal.
    pub fn keys(&self, terminal: &str, keys: &[&str]) {
        let mut words = vec!["terminal", "send", "--session", "host", terminal];
        for key in keys {
            words.extend(["--key", key]);
        }
        let sent = self.run(&words);
        assert_eq!(sent.code, Some(0), "{sent}");
    }

    /// The actual foreground of the state cell immediately before `label`.
    /// Working and Done may share a glyph; SGR color remains observable through the real Host.
    pub fn state_color(&self, terminal: &str, label: &str) -> Option<Vec<u16>> {
        let read = self.run(&[
            "terminal",
            "read",
            "--session",
            "host",
            terminal,
            "--source",
            "visible",
            "--ansi",
        ]);
        assert_eq!(read.code, Some(0), "{read}");
        let line = read.document["result"]["content"]["text"]
            .as_str()?
            .lines()
            .find(|line| line.contains(label))?;
        let mut foreground = vec![39];
        let mut text = String::new();
        let mut colors = Vec::new();
        let mut chars = line.chars();
        while let Some(ch) = chars.next() {
            if ch != '\x1b' {
                text.push(ch);
                colors.push(foreground.clone());
                continue;
            }
            if chars.next() != Some('[') {
                return None;
            }
            let mut parameters = String::new();
            for ch in chars.by_ref() {
                if ch.is_ascii_alphabetic() {
                    break;
                }
                parameters.push(ch);
            }
            let codes: Vec<u16> = parameters
                .split(';')
                .map(|p| {
                    p.split(':')
                        .next()
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(0)
                })
                .collect();
            let mut index = 0;
            while let Some(&code) = codes.get(index) {
                match code {
                    0 | 39 => foreground = vec![39],
                    30..=37 | 90..=97 => foreground = vec![code],
                    38 | 48 => {
                        let count = match codes.get(index + 1) {
                            Some(2) => 5,
                            Some(5) => 3,
                            _ => return None,
                        };
                        let color = codes.get(index..index + count)?;
                        if code == 38 {
                            foreground = color.to_vec();
                        }
                        index += count;
                        continue;
                    }
                    _ => {}
                }
                index += 1;
            }
        }
        let before = text[..text.find(label)?].chars().count();
        Some(
            text.chars()
                .zip(colors)
                .take(before)
                .filter(|(ch, _)| !ch.is_whitespace())
                .last()?
                .1,
        )
    }

    /// The visible text of a `host` terminal, as its Host currently shows it.
    pub fn screen(&self, terminal: &str) -> String {
        let read = self.run(&[
            "terminal",
            "read",
            "--session",
            "host",
            terminal,
            "--source",
            "visible",
        ]);
        read.document["result"]["content"]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{read}"))
            .to_owned()
    }

    /// Waits until the Client reports `phase` on consecutive reads: a mode holds it, not one
    /// transient step.
    pub fn settle(&self, client: &str, phase: &str) {
        let deadline = Instant::now() + OBSERVATION;
        let mut reads = 0;
        while reads < SETTLED_READS {
            let current = self.client(client);
            assert!(
                Instant::now() < deadline,
                "the Client never settled {phase}: {current}"
            );
            reads = if current["view"]["phase"] == phase {
                reads + 1
            } else {
                0
            };
            thread::sleep(Duration::from_millis(200));
        }
    }

    pub fn settle_busy(&self, client: &str) {
        self.settle(client, "busy");
    }

    pub fn settle_ready(&self, client: &str) {
        self.settle(client, "ready");
    }
}

/// The Server or Worker that an earlier Cargo build left beside the CLI. One whose Cargo dep-info
/// lists a source changed after it was linked is refused: Cargo would rebuild it, so it predates
/// the code under test.
fn current_peer(built: &Path, name: &str) -> PathBuf {
    let executable = built.join(format!("{name}.exe"));
    let rebuild = "build them with `cargo build -p orchbee-server -p orchbee-worker` first";
    let modified = |path: &Path| fs::metadata(path).and_then(|metadata| metadata.modified());
    let linked = modified(&executable)
        .unwrap_or_else(|error| panic!("{}: {error}; {rebuild}", executable.display()));
    let info = fs::read_to_string(built.join(format!("{name}.d")))
        .unwrap_or_else(|error| panic!("{name}.d: {error}; {rebuild}"));
    let (_, sources) = info.split_once(": ").unwrap();
    // Dep-info escapes a space inside a path as `\ `; unescaped whitespace separates paths.
    for source in sources.replace("\\ ", "\0").split_whitespace() {
        let source = PathBuf::from(source.replace('\0', " "));
        if !modified(&source).is_ok_and(|changed| changed <= linked) {
            panic!(
                "{} is stale: {} changed after it was built; {rebuild}",
                executable.display(),
                source.display()
            );
        }
    }
    executable
}

/// A program in the Windows system directory.
pub fn system_program(name: &str) -> String {
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap());
    system
        .join("System32")
        .join(name)
        .to_str()
        .unwrap()
        .to_owned()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Stopping each Session ends its terminals, Host, Worker and Server before cleanup. Only the
        // staged CLI starts Sessions; after a failed staging, running the missing CLI would panic
        // again and abort the test process before the staging failure is reported.
        if self.staged_client().is_file() {
            if thread::panicking() {
                for entry in fs::read_dir(&self.root).into_iter().flatten().flatten() {
                    if entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "stderr")
                        && let Ok(text) = fs::read_to_string(entry.path())
                    {
                        eprintln!(
                            "TUI stderr {}:\n{}",
                            entry.path().display(),
                            text.chars().take(16000).collect::<String>()
                        );
                    }
                }
                let listed = self.run(&["terminal", "list", "--session", "host"]);
                if let Some(terminals) = listed.document["result"]["terminals"].as_array() {
                    for terminal in terminals
                        .iter()
                        .take(8)
                        .filter_map(|entry| entry["terminalId"].as_str())
                    {
                        eprintln!(
                            "Failed TUI screen: {}",
                            self.run(&[
                                "terminal",
                                "read",
                                "--session",
                                "host",
                                terminal,
                                "--source",
                                "visible",
                                "--ansi"
                            ])
                        );
                    }
                }
            }
            for session in ["host", "default"] {
                let stopped = self.run(&["server", "stop", "--confirm", "--session", session]);
                if !thread::panicking() {
                    assert_eq!(stopped.code, Some(0), "{stopped}");
                }
            }
        }
        let target = fs::canonicalize(&self.root).unwrap();
        assert!(target.starts_with(fs::canonicalize(std::env::temp_dir()).unwrap()));
        assert!(
            target
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(self.prefix))
        );
        let deadline = Instant::now() + OBSERVATION;
        while let Err(error) = fs::remove_dir_all(&target) {
            if Instant::now() >= deadline {
                if !thread::panicking() {
                    panic!("remove {}: {error}", target.display());
                }
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// A controlling attachment that holds a `host` terminal at one size and writes raw input to it
/// until it is released.
pub struct Control {
    child: Child,
    output: Option<thread::JoinHandle<()>>,
}

impl Control {
    /// Takes control of `terminal` at `columns` by `rows` and waits until the attachment opens.
    pub fn hold(fixture: &Fixture, terminal: &str, columns: u16, rows: u16) -> Self {
        Self::hold_in(fixture, "host", terminal, columns, rows)
    }

    /// Takes control of a terminal of `session`, as `hold` does in the `host` Session.
    pub fn hold_in(
        fixture: &Fixture,
        session: &str,
        terminal: &str,
        columns: u16,
        rows: u16,
    ) -> Self {
        let columns = columns.to_string();
        let rows = rows.to_string();
        let mut child = fixture
            .command(&[
                "terminal",
                "session",
                "control",
                terminal,
                "--session",
                session,
                "--takeover",
                "--cols",
                &columns,
                "--rows",
                &rows,
                "--timeout",
                CONTROL_TIMEOUT_MILLISECONDS,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Drain the terminal's output so the attachment never stalls on a full pipe.
        let stdout = child.stdout.take().unwrap();
        let (opened, open) = mpsc::channel();
        let output = thread::spawn(move || {
            let mut opened = Some(opened);
            let mut stdout = BufReader::new(stdout);
            let mut line = Vec::new();
            while stdout
                .read_until(b'\n', &mut line)
                .is_ok_and(|read| read > 0)
            {
                if opened.is_some()
                    && serde_json::from_slice::<Value>(&line)
                        .is_ok_and(|record| record["kind"] == "opened")
                    && let Some(opened) = opened.take()
                {
                    let _ = opened.send(());
                }
                line.clear();
            }
        });
        let mut control = Self {
            child,
            output: Some(output),
        };
        if open.recv_timeout(OBSERVATION).is_err() {
            panic!(
                "the control attachment never opened: {:?}",
                control.child.try_wait()
            );
        }
        control
    }

    /// Writes raw terminal input, such as SGR mouse reports, as the terminal's controller.
    pub fn input(&mut self, bytes: &[u8]) {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"type": "terminal.input", "bytes": encoded})
        )
        .unwrap();
        stdin.flush().unwrap();
    }

    /// Resizes the terminal as its controller, as a resize of the window that shows it would.
    pub fn resize(&mut self, columns: u16, rows: u16) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"type": "terminal.resize", "cols": columns, "rows": rows})
        )
        .unwrap();
        stdin.flush().unwrap();
    }

    /// Closes the attachment's input, which detaches it without stopping the terminal.
    pub fn release(mut self) {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + OBSERVATION;
        while self.child.try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "the control attachment did not detach"
            );
            thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(output) = self.output.take() {
            let _ = output.join();
        }
    }
}

impl fmt::Display for Run {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`orchbee {}` exited {:?} after {:?}: {}\n{}",
            self.words, self.code, self.elapsed, self.document, self.stderr
        )
    }
}
