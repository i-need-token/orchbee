//! Plugin popups through a real local Session: the CLI, Server, Worker and Zig Host under test, a
//! real TUI Client that runs in a terminal of a second Session, and a terminal that prints in
//! another Workspace. Each popup request submits ClientView controls that wake the TUI Client,
//! and the popup source's own terminal reads commit on the Server; whatever refresh that causes
//! must not refuse the popup. A Client held by copy mode or by a window too small for the popup
//! must name that cause.
#![cfg(windows)]

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Bounds the TUI Client's startup inside the second Session's terminal.
const STARTUP: Duration = Duration::from_secs(40);
/// Bounds each wait for an observation of the Client.
const OBSERVATION: Duration = Duration::from_secs(10);
/// A modal state is settled once the Client reports one phase on this many reads in a row,
/// 200 ms apart; a background refresh holds it busy only for moments.
const SETTLED_READS: usize = 4;
/// Opens per run, so that a pass does not depend on one quiet moment.
const OPENS: usize = 3;

#[test]
fn popups_open_on_a_live_client_and_busy_refusals_name_their_cause() {
    let fixture = Fixture::new();
    fixture.link_plugin();
    let tui = fixture.start_tui();
    fixture.background_output();
    fixture.settle_ready(&tui.client);

    for _ in 0..OPENS {
        let opened = fixture.popup(&tui.client);
        assert!(
            opened.code == Some(0)
                && opened.document["observation"]["reply"]["outcome"]["operation"]["status"]["kind"]
                    == "popup-opened",
            "{opened}"
        );
        let closed = fixture.run(&["popup", "close", "--client", &tui.client]);
        assert_eq!(closed.code, Some(0), "{closed}");
    }

    // Copy mode captures the Client's input.
    fixture.keys(&tui.terminal, &["ctrl+b", "U"]);
    fixture.settle_busy(&tui.client);
    assert_refused(&fixture.popup(&tui.client), "modal-open");
    fixture.keys(&tui.terminal, &["escape"]);
    fixture.settle_ready(&tui.client);

    // The action menu captures the Client's input while its view stays ready.
    fixture.keys(&tui.terminal, &["ctrl+b", "f2"]);
    fixture.settle_ready(&tui.client);
    assert_refused(&fixture.popup(&tui.client), "modal-open");
    fixture.keys(&tui.terminal, &["escape"]);
    fixture.settle_ready(&tui.client);

    // A window of 8 by 5 cells has no room for the popup.
    let small = Control::resize(&fixture, &tui.terminal, 8, 5);
    fixture.settle_busy(&tui.client);
    assert_refused(&fixture.popup(&tui.client), "insufficient-space");
    small.release();
}

fn assert_refused(run: &Run, reason: &str) {
    let details = &run.document["failure"]["details"];
    assert!(
        run.code == Some(3)
            && details["reason"] == "client-refused"
            && details["clientReason"] == reason,
        "{run}"
    );
}

/// One real local runtime with the default Session under observation and a `host` Session whose
/// terminal runs the TUI Client.
struct Fixture(PathBuf);

/// The TUI Client and the terminal of the `host` Session it runs in.
struct Tui {
    client: String,
    terminal: String,
}

/// One CLI invocation's observed exit, duration and machine output.
struct Run {
    words: String,
    code: Option<i32>,
    elapsed: Duration,
    document: Value,
    stderr: String,
}

impl Fixture {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-plugin-popup-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        for path in [
            "cohort", "runtime", "home", "local", "roaming", "work", "noise", "plugin",
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        let fixture = Self(root);
        fixture.stage();
        for session in ["default", "host"] {
            let started = fixture.run(&["server", "start", "--session", session]);
            assert_eq!(started.code, Some(0), "{started}");
        }
        fixture
    }

    /// Stages the CLI, Server and Worker under test with the real Host as one private cohort.
    fn stage(&self) {
        let built = std::path::Path::new(env!("CARGO_BIN_EXE_orchbee"))
            .parent()
            .unwrap();
        let mut artifacts = serde_json::Map::new();
        for (name, source) in [
            ("orchbee", built.join("orchbee.exe")),
            ("orchbee-server", built.join("orchbee-server.exe")),
            ("orchbee-worker", built.join("orchbee-worker.exe")),
            (
                "orchbee-terminal-host",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../terminal-host/zig-out/bin/orchbee-terminal-host.exe"),
            ),
        ] {
            let bytes = fs::read(&source).unwrap_or_else(|error| {
                panic!(
                    "{}: {error}; build orchbee-server, orchbee-worker and \
                     `pnpm --filter @orchbee/terminal-host build` first",
                    source.display()
                )
            });
            // Native cohort admission requires private regular files with one link.
            fs::write(self.0.join("cohort").join(format!("{name}.exe")), &bytes).unwrap();
            artifacts.insert(
                name.to_owned(),
                json!(format!("{:x}", Sha256::digest(&bytes))),
            );
        }
        artifacts.insert(
            "agent-observation-adapters".to_owned(),
            json!(format!("{:x}", Sha256::digest(b"unused adapters"))),
        );
        fs::write(
            self.0.join("cohort/cohort.json"),
            serde_json::to_vec(&json!({"schemaVersion": 1, "artifacts": artifacts})).unwrap(),
        )
        .unwrap();
    }

    /// The CLI under test in this fixture's isolated environment.
    fn command(&self, words: &[&str]) -> Command {
        let mut command = Command::new(self.0.join("cohort/orchbee.exe"));
        command
            .args(words)
            .env("ORCHBEE_RUNTIME_BASE", self.0.join("runtime"))
            .env_remove("ORCHBEE_RUNTIME_SCOPE")
            .env_remove("ORCHBEE_SESSION")
            .env_remove("ORCHBEE_CONFIG_FILE")
            .env_remove("ORCHBEE_TERMINAL_ID")
            .env_remove("ORCHBEE_WORKER_ID")
            .env("USERPROFILE", self.0.join("home"))
            .env("HOME", self.0.join("home"))
            .env("APPDATA", self.0.join("roaming"))
            .env("LOCALAPPDATA", self.0.join("local"))
            .env("LC_ALL", "en-US")
            .current_dir(&self.0)
            .stdin(Stdio::null());
        command
    }

    /// Runs one CLI verb with machine output.
    fn run(&self, words: &[&str]) -> Run {
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

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_str().unwrap().to_owned()
    }

    /// The isolated environment and a system-only `PATH` for terminals this fixture starts.
    fn terminal_environment(&self) -> Vec<String> {
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

    /// Links a popup plugin whose pane runs `cmd.exe` at half width and ten rows.
    fn link_plugin(&self) {
        let manifest = self.0.join("plugin/orchbee-plugin.toml");
        fs::write(
            &manifest,
            "id = 'audit'\nname = 'Audit tools'\nversion = '1'\n\n[[panes]]\nid = 'tools'\n\
             title = 'Tools'\nplacement = 'popup'\nwidth = '50%'\nheight = 10\n\
             command = ['cmd.exe']\n",
        )
        .unwrap();
        let linked = self.run(&["plugin", "link", manifest.to_str().unwrap()]);
        assert_eq!(linked.code, Some(0), "{linked}");
    }

    /// Starts a terminal in another Workspace that prints every second until the Session stops.
    fn background_output(&self) {
        let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        let ping = system.join("System32/PING.EXE");
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
            ping.to_str().unwrap(),
            "--arg",
            "-t",
            "--arg",
            "127.0.0.1",
        ];
        let environment = self.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
    }

    /// Opens a Workspace for the TUI to show, then runs `orchbee tui` in a terminal of the `host`
    /// Session. That terminal marks itself as an Orchbee terminal; the launcher clears those
    /// marks so the TUI attaches to the default Session as a top-level Client.
    fn start_tui(&self) -> Tui {
        let work = self.path("work");
        let mut words = vec![
            "workspace",
            "create",
            "--label",
            "work",
            "--cwd",
            &work,
            "--no-focus",
        ];
        let environment = self.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.run(&words);
        assert_eq!(created.code, Some(0), "{created}");

        let launcher = self.0.join("tui.cmd");
        fs::write(
            &launcher,
            format!(
                "@echo off\r\nset ORCHBEE_TERMINAL_ID=\r\nset ORCHBEE_WORKER_ID=\r\n\
                 set ORCHBEE_RUNTIME_SCOPE=\r\nset ORCHBEE_SESSION=\r\n\"{}\" tui\r\n",
                self.0.join("cohort/orchbee.exe").display()
            ),
        )
        .unwrap();
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
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
            "160",
            "--rows",
            "45",
            "--cell-width",
            "8",
            "--cell-height",
            "16",
            "--exec",
            shell.to_str().unwrap(),
            "--arg",
            "/d",
            "--arg",
            "/c",
            "--arg",
            launcher.to_str().unwrap(),
        ];
        words.extend(environment.iter().map(String::as_str));
        let created = self.run(&words);
        let panel = created.document["workspace"]["result"]["panelId"]
            .as_str()
            .unwrap_or_else(|| panic!("{created}"))
            .to_owned();

        let deadline = Instant::now() + STARTUP;
        let client = loop {
            if let Some(client) = self
                .clients()
                .into_iter()
                .find(|client| client["view"]["phase"] == "ready")
            {
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

    fn clients(&self) -> Vec<Value> {
        let listed = self.run(&["client", "list"]);
        listed.document["result"]["outcome"]["clients"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| panic!("{listed}"))
    }

    fn client(&self, id: &str) -> Value {
        self.clients()
            .into_iter()
            .find(|client| client["clientId"] == id)
            .unwrap_or_else(|| panic!("Client {id} left the Session"))
    }

    fn popup(&self, client: &str) -> Run {
        self.run(&[
            "plugin",
            "pane",
            "open",
            "--plugin",
            "audit",
            "--entrypoint",
            "tools",
            "--client",
            client,
        ])
    }

    /// Types keys into the TUI through its `host` terminal.
    fn keys(&self, terminal: &str, keys: &[&str]) {
        let mut words = vec!["terminal", "send", "--session", "host", terminal];
        for key in keys {
            words.extend(["--key", key]);
        }
        let sent = self.run(&words);
        assert_eq!(sent.code, Some(0), "{sent}");
    }

    /// Waits until the Client reports `phase` on consecutive reads: a mode holds it, not one
    /// background refresh.
    fn settle(&self, client: &str, phase: &str) {
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

    fn settle_busy(&self, client: &str) {
        self.settle(client, "busy");
    }

    fn settle_ready(&self, client: &str) {
        self.settle(client, "ready");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Stopping each Session ends its terminals, Host, Worker and Server before cleanup.
        for session in ["host", "default"] {
            let stopped = self.run(&["server", "stop", "--confirm", "--session", session]);
            if !thread::panicking() {
                assert_eq!(stopped.code, Some(0), "{stopped}");
            }
        }
        let target = fs::canonicalize(&self.0).unwrap();
        assert!(target.starts_with(fs::canonicalize(std::env::temp_dir()).unwrap()));
        assert!(
            target
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("orchbee-plugin-popup-"))
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

/// A controlling attachment that holds the TUI's terminal at one size until it is released.
struct Control(Child);

impl Control {
    fn resize(fixture: &Fixture, terminal: &str, columns: u16, rows: u16) -> Self {
        let columns = columns.to_string();
        let rows = rows.to_string();
        let child = fixture
            .command(&[
                "terminal",
                "session",
                "control",
                terminal,
                "--session",
                "host",
                "--takeover",
                "--cols",
                &columns,
                "--rows",
                &rows,
                "--timeout",
                "60000",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self(child)
    }

    /// Closes the attachment's input, which detaches it without stopping the terminal.
    fn release(mut self) {
        drop(self.0.stdin.take());
        let deadline = Instant::now() + OBSERVATION;
        while self.0.try_wait().unwrap().is_none() {
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
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
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
