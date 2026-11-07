//! Agent session resume across real managed restarts: the CLI, Server, Worker and Zig Host under
//! test with the pinned ConPTY, and a default Session whose default program is cmd. Stub `claude`
//! and `codex` commands sit in each Panel's directory as npm-style shims to Node scripts at each
//! provider's package entry, so process identity reads them as those providers. A stub records how
//! it was started, reports its SessionStart through the shipped hook script to the Session's
//! managed observation endpoint, and stays alive like an interactive Agent.
//!
//! Capture, the checkpoint, restoration and the typed resume line are witnessed together. A stub
//! started in a Panel resumes once, with its own session id and the bare command, in that Panel's
//! retained directory after the first Client view, and again after the next restart. A Panel whose
//! directory was deleted, or that a Client typed into first, receives nothing, and the preference
//! off at startup discards every reference.
//!
//! A restored terminal carries no launch environment: the Host projects `PATH` from the registry,
//! where a real provider may be installed. cmd resolves a command from its current directory
//! first, and a resume is typed only in the retained directory, so the line always reaches the stub
//! there. PowerShell resolves only through `PATH`, so it is never this witness's shell. The hook's
//! correlation reaches the Host as a private APC, which only the pinned ConPTY carries, so the
//! witness needs `ORCHBEE_TEST_CONPTY_DIR` naming a manifest-verified bundle and runs only when
//! asked for.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    PreferenceKey, UserPreferencesPatch, UserPreferencesRequest, UserPreferencesResponse,
    UserPreferencesSnapshot,
};
use serde_json::{Value, json};
use session::{Control, Fixture, system_program};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// Bounds a shell's first prompt, a stub's start, its listing and a resume after the first view.
const OBSERVED: Duration = Duration::from_secs(30);
/// Well past the settle, readiness reads and spacing of any resume, so nothing typed by then
/// means nothing will be.
const QUIET: Duration = Duration::from_secs(6);
/// Controller and creation geometry, wide enough that no prompt in the long temporary path wraps.
const COLUMNS: u16 = 200;
const ROWS: u16 = 30;
/// The launch profile that makes cmd the default program before any terminal starts.
const CMD_PROFILE: &str = "cmd";

/// One stub provider: its command, its package entry, its hook adapter and the argument that
/// resumes a session.
struct Stub {
    program: &'static str,
    entry: &'static str,
    adapter: &'static str,
    resume: &'static str,
}

const CLAUDE: Stub = Stub {
    program: "claude",
    entry: "node_modules/@anthropic-ai/claude-code/cli.js",
    adapter: "claude-code",
    resume: "--resume",
};

const CODEX: Stub = Stub {
    program: "codex",
    entry: "node_modules/@openai/codex/bin/codex.js",
    adapter: "codex",
    resume: "resume",
};

/// A fresh run takes a new session id and a resumed one keeps its own. The hook runs as the
/// provider runs a configured hook: a child with the payload on standard input, inheriting the
/// environment and console.
const STUB: &str = r"
const { spawn } = require('node:child_process');
const { randomUUID } = require('node:crypto');
const { appendFileSync } = require('node:fs');
const argv = process.argv.slice(2);
const session = argv[0] === RESUME && argv.length === 2 ? argv[1] : randomUUID();
const hook = spawn(process.execPath, [HOOK, ADAPTER], { stdio: ['pipe', 'ignore', 'ignore'] });
hook.on('exit', () => {
  const record = { adapter: ADAPTER, script: __filename, argv, cwd: process.cwd(), session };
  appendFileSync(CALLS, JSON.stringify(record) + '\n');
  process.stdout.write(`${ADAPTER} stub running\n`);
});
hook.stdin.end(JSON.stringify({
  session_id: session,
  hook_event_name: 'SessionStart',
  source: argv.length === 0 ? 'startup' : 'resume',
  cwd: process.cwd(),
}));
setInterval(() => {}, 1 << 30);
";

#[test]
#[ignore = "requires ORCHBEE_TEST_CONPTY_DIR: only the pinned ConPTY carries the hook's correlation; run explicitly"]
fn retained_agents_resume_once_in_their_own_shells_and_never_in_a_moved_or_touched_one() {
    let witness = Witness::new(
        "orchbee-agent-resume-",
        &["work/claude", "work/codex", "work/gone", "work/touched"],
    );
    let claude_panel = witness.workspace("claude", "work/claude");
    let codex_panel = witness.workspace("codex", "work/codex");
    let gone_panel = witness.workspace("gone", "work/gone");
    let touched_panel = witness.workspace("touched", "work/touched");
    let claude = witness.start(&CLAUDE, &claude_panel, "work/claude");
    let codex = witness.start(&CODEX, &codex_panel, "work/codex");
    let gone = witness.start(&CLAUDE, &gone_panel, "work/gone");
    let touched = witness.start(&CODEX, &touched_panel, "work/touched");
    witness.until_listed(4);

    witness.stop();
    assert_eq!(
        witness.retained(),
        sorted([&claude, &codex, &gone, &touched]),
        "capture retains each live stub's own reference"
    );
    // A Panel whose directory is gone restores in home and is never resumed.
    witness.remove("work/gone");
    witness.start_session();
    // One Client opens the gate by taking control of a Panel and types into it within the settle;
    // it then views another Panel. The fourth stays unviewed, at its restored geometry.
    let mut touching = witness.control(&touched_panel);
    touching.input(b"x");
    let viewing = witness.control(&claude_panel);
    witness.until_resumed(&[(&CLAUDE, &claude, 1), (&CODEX, &codex, 1)]);
    thread::sleep(QUIET);
    witness.assert_resumed(&CLAUDE, &claude, "work/claude", 1);
    witness.assert_resumed(&CODEX, &codex, "work/codex", 1);
    witness.assert_resumed(&CLAUDE, &gone, "work/gone", 0);
    witness.assert_resumed(&CODEX, &touched, "work/touched", 0);
    let typed = witness.screen(&claude_panel);
    assert!(
        typed.contains(&format!("claude --resume {claude}")),
        "cmd did not receive the bare command:\n{typed}"
    );
    let touched_screen = witness.screen(&touched_panel);
    assert!(
        !touched_screen.contains(&touched),
        "a touched Panel received resume text:\n{touched_screen}"
    );
    let home = witness.directory("home").to_lowercase();
    let moved = witness.screen(&gone_panel);
    assert!(
        moved.to_lowercase().contains(&format!("{home}>")) && !moved.contains(&gone),
        "a Panel without its directory did not restore as a plain shell in home:\n{moved}"
    );
    touching.release();
    viewing.release();

    // Each resumed run is captured again, and resuming again keeps working.
    witness.until_listed(2);
    witness.stop();
    assert_eq!(witness.retained(), sorted([&claude, &codex]));
    witness.start_session();
    let viewing = witness.control(&claude_panel);
    witness.until_resumed(&[(&CLAUDE, &claude, 2), (&CODEX, &codex, 2)]);
    thread::sleep(QUIET);
    witness.assert_resumed(&CLAUDE, &claude, "work/claude", 2);
    witness.assert_resumed(&CODEX, &codex, "work/codex", 2);
    viewing.release();

    // With the preference off at startup, the retained references are discarded unresumed, and
    // the next capture records none.
    witness.until_listed(2);
    witness.stop();
    assert_eq!(witness.retained(), sorted([&claude, &codex]));
    witness.set_preference(PreferenceKey::ResumeAgentsOnRestore, json!(false));
    witness.start_session();
    let viewing = witness.control(&claude_panel);
    thread::sleep(QUIET);
    witness.assert_resumed(&CLAUDE, &claude, "work/claude", 2);
    witness.assert_resumed(&CODEX, &codex, "work/codex", 2);
    viewing.release();
    witness.stop();
    assert_eq!(witness.retained(), Vec::<String>::new());
    witness.start_session();
}

/// One real local runtime whose default program is cmd, with the stub providers in each of its
/// work directories.
struct Witness {
    fixture: Fixture,
}

impl Witness {
    fn new(prefix: &'static str, directories: &[&str]) -> Self {
        assert!(
            std::env::var_os("ORCHBEE_TEST_CONPTY_DIR").is_some(),
            "set ORCHBEE_TEST_CONPTY_DIR to a ConPTY bundle verified against \
             apps/terminal-host/windows-conpty.json"
        );
        // With this registry value cmd skips its current directory and could reach a real
        // provider on the registry `PATH` a restored terminal receives.
        for key in [
            r"HKCU\Environment",
            r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ] {
            let query = Command::new(system_program("reg.exe"))
                .args(["query", key, "/v", "NoDefaultCurrentDirectoryInExePath"])
                .output()
                .unwrap();
            assert!(
                !query.status.success(),
                "NoDefaultCurrentDirectoryInExePath is set in {key}; cmd would not reach the stubs"
            );
        }
        let witness = Self {
            fixture: Fixture::new(prefix, directories),
        };
        witness.set_preference(
            PreferenceKey::LaunchProfiles,
            json!({
                "version": 1,
                "defaultProfileId": CMD_PROFILE,
                "profiles": [
                    {"id": "system-default", "name": null, "command": {"kind": "system-shell", "login": false}},
                    {"id": CMD_PROFILE, "name": "Command Prompt", "command": {
                        "kind": "custom-executable",
                        "executable": system_program("cmd.exe"),
                        "arguments": ["/D"],
                    }},
                ],
            }),
        );
        // A version-manager shim on `PATH` would run inside the scratch profile, where it can
        // try to install tools; the stubs call the interpreter's own executable instead.
        let shim = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("node.exe"))
            .find(|path| path.is_file())
            .expect("node.exe on PATH");
        let resolved = Command::new(&shim)
            .args(["-p", "process.execPath"])
            .output()
            .unwrap();
        assert!(resolved.status.success(), "{}", shim.display());
        let node = PathBuf::from(String::from_utf8(resolved.stdout).unwrap().trim());
        assert!(node.is_file(), "{}", node.display());
        let hook = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../crates/agent-integration-writer/agent-observation-adapters/bundle/scripts/forward-hook.mjs",
        );
        assert!(hook.is_file(), "{}", hook.display());
        let literal = |value: &str| serde_json::to_string(value).unwrap();
        for stub in [&CLAUDE, &CODEX] {
            let entry = PathBuf::from(witness.directory("stubs")).join(stub.entry);
            fs::create_dir_all(entry.parent().unwrap()).unwrap();
            fs::write(
                &entry,
                format!(
                    "const RESUME = {};\nconst HOOK = {};\nconst ADAPTER = {};\nconst CALLS = {};\n{STUB}",
                    literal(stub.resume),
                    literal(hook.to_str().unwrap()),
                    literal(stub.adapter),
                    literal(&witness.directory("calls.jsonl")),
                ),
            )
            .unwrap();
            for directory in directories {
                fs::write(
                    PathBuf::from(witness.directory(directory))
                        .join(format!("{}.cmd", stub.program)),
                    format!("@\"{}\" \"{}\" %*\r\n", node.display(), entry.display()),
                )
                .unwrap();
            }
        }
        witness
    }

    /// A fixture path as Windows spells it.
    fn directory(&self, relative: &str) -> String {
        self.fixture.path(relative).replace('/', "\\")
    }

    /// Opens a Workspace whose default-program terminal starts in `directory` and returns its
    /// Panel.
    fn workspace(&self, label: &str, directory: &str) -> String {
        let cwd = self.directory(directory);
        let columns = COLUMNS.to_string();
        let rows = ROWS.to_string();
        let mut words = vec![
            "workspace",
            "create",
            "--label",
            label,
            "--cwd",
            &cwd,
            "--no-focus",
            "--cols",
            &columns,
            "--rows",
            &rows,
            "--cell-width",
            "8",
            "--cell-height",
            "16",
        ];
        let environment = self.fixture.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.fixture.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
        created.document["workspace"]["result"]["panelId"]
            .as_str()
            .unwrap_or_else(|| panic!("{created}"))
            .to_owned()
    }

    /// The typed handle of the terminal bound to `panel`; a bare id may also name a Panel.
    fn terminal(&self, panel: &str) -> String {
        let listed = self.fixture.run(&["terminal", "list"]);
        listed.document["result"]["terminals"]
            .as_array()
            .and_then(|terminals| {
                terminals
                    .iter()
                    .find(|terminal| terminal["boundTo"]["panelId"] == panel)
            })
            .and_then(|terminal| terminal["terminalId"].as_str())
            .map(|id| format!("@terminal:0x{}", id.trim_start_matches('0')))
            .unwrap_or_else(|| panic!("no terminal is bound to {panel}: {listed}"))
    }

    /// The recent unwrapped text of the terminal bound to `panel`. A read is a Client view.
    fn screen(&self, panel: &str) -> String {
        let terminal = self.terminal(panel);
        let read = self.fixture.run(&[
            "terminal",
            "read",
            &terminal,
            "--source",
            "recent-unwrapped",
        ]);
        read.document["result"]["content"]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{read}"))
            .to_owned()
    }

    /// A Client controlling and viewing the terminal bound to `panel`.
    fn control(&self, panel: &str) -> Control {
        Control::hold_in(
            &self.fixture,
            "default",
            &self.terminal(panel),
            COLUMNS,
            ROWS,
        )
    }

    /// Waits for cmd's prompt in `directory`, types `stub`'s command and returns the session id of
    /// the run that started there.
    fn start(&self, stub: &Stub, panel: &str, directory: &str) -> String {
        let prompt = format!("{}>", self.directory(directory).to_lowercase());
        let deadline = Instant::now() + OBSERVED;
        loop {
            let screen = self.screen(panel);
            if screen.to_lowercase().contains(&prompt) && !screen.contains("PS ") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "no cmd prompt in {directory}:\n{screen}"
            );
            thread::sleep(Duration::from_millis(250));
        }
        let terminal = self.terminal(panel);
        let sent = self.fixture.run(&[
            "terminal",
            "send",
            &terminal,
            "--text",
            stub.program,
            "--enter",
        ]);
        assert_eq!(sent.code, Some(0), "{sent}");
        loop {
            if let Some(call) = self.calls().into_iter().find(|call| {
                call["adapter"] == stub.adapter
                    && call["argv"] == json!([])
                    && self.same_directory(&call["cwd"], directory)
            }) {
                return call["session"].as_str().unwrap().to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "{} never started in {directory}:\n{}",
                stub.program,
                self.screen(panel)
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    /// Waits until the Session lists `count` Agents seen both through their hook and in their
    /// terminal's processes, as capture needs.
    fn until_listed(&self, count: usize) {
        let deadline = Instant::now() + OBSERVED;
        loop {
            let listed = self.fixture.run(&["agent", "list"]);
            let both = listed.document["result"]["items"]
                .as_array()
                .map_or(0, |items| {
                    items.iter().filter(|item| item["source"] == "both").count()
                });
            if both >= count {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the Session never listed {count} Agents: {listed}"
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    /// Every run the stubs recorded, in order. Each must be a stub this test placed.
    fn calls(&self) -> Vec<Value> {
        let root = self.directory("").to_lowercase();
        let calls: Vec<Value> = fs::read_to_string(self.directory("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for call in &calls {
            assert!(
                call["script"]
                    .as_str()
                    .is_some_and(|script| script.to_lowercase().starts_with(&root)),
                "a run outside this fixture: {call}"
            );
        }
        calls
    }

    /// The runs that resumed `session` with `stub`'s own command.
    fn resumes(&self, stub: &Stub, session: &str) -> Vec<Value> {
        self.calls()
            .into_iter()
            .filter(|call| {
                call["adapter"] == stub.adapter && call["argv"] == json!([stub.resume, session])
            })
            .collect()
    }

    fn until_resumed(&self, expected: &[(&Stub, &String, usize)]) {
        let deadline = Instant::now() + OBSERVED;
        while expected
            .iter()
            .any(|(stub, session, count)| self.resumes(stub, session).len() < *count)
        {
            assert!(
                Instant::now() < deadline,
                "the retained Agents never resumed: {:#?}",
                self.calls()
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    /// `session` resumed exactly `count` times, each in `directory`.
    fn assert_resumed(&self, stub: &Stub, session: &str, directory: &str, count: usize) {
        let resumes = self.resumes(stub, session);
        assert_eq!(
            resumes.len(),
            count,
            "{} resumed {session} {} times, not {count}: {:#?}",
            stub.program,
            resumes.len(),
            self.calls()
        );
        for resume in &resumes {
            assert!(
                self.same_directory(&resume["cwd"], directory),
                "{} resumed outside {directory}: {resume}",
                stub.program
            );
        }
    }

    fn same_directory(&self, actual: &Value, directory: &str) -> bool {
        let Some(actual) = actual.as_str() else {
            return false;
        };
        let expected = fs::canonicalize(self.directory(directory));
        expected.is_ok() && fs::canonicalize(actual).ok() == expected.ok()
    }

    fn stop(&self) {
        let stopped = self
            .fixture
            .run(&["server", "stop", "--confirm", "--session", "default"]);
        assert_eq!(stopped.code, Some(0), "{stopped}");
    }

    /// Starts the default Session again once its preference record still selects cmd: an
    /// unreadable record would restore PowerShell, which resolves only through `PATH`.
    fn start_session(&self) {
        let values = serde_json::to_value(&self.preferences().values).unwrap();
        assert_eq!(
            values["launchProfiles"]["defaultProfileId"], CMD_PROFILE,
            "{values}"
        );
        let started = self
            .fixture
            .run(&["server", "start", "--session", "default"]);
        assert_eq!(started.code, Some(0), "{started}");
    }

    /// Removes a fixture directory once the stopped Session's shells have released it.
    fn remove(&self, directory: &str) {
        let path = self.directory(directory);
        let deadline = Instant::now() + OBSERVED;
        while let Err(error) = fs::remove_dir_all(&path) {
            assert!(Instant::now() < deadline, "remove {path}: {error}");
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// The session ids the newest published checkpoint retains, sorted.
    fn retained(&self) -> Vec<String> {
        let checkpoint = self.checkpoint();
        let rows = checkpoint["agentSessions"]
            .as_array()
            .unwrap_or_else(|| panic!("no agentSessions in the checkpoint: {checkpoint}"));
        let mut sessions: Vec<_> = rows
            .iter()
            .map(|row| row["sessionId"].as_str().unwrap().to_owned())
            .collect();
        sessions.sort();
        sessions
    }

    /// The payload of the newest checkpoint frame the stopped Session published: a five-byte
    /// magic, a little-endian generation and a digest precede it.
    fn checkpoint(&self) -> Value {
        let mut newest: Option<(u64, Vec<u8>)> = None;
        let mut directories = vec![PathBuf::from(self.directory(""))];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if !(name.starts_with("checkpoint") && name.ends_with(".json")) {
                    continue;
                }
                let bytes = fs::read(&path).unwrap();
                if bytes.len() < 45 || &bytes[..5] != b"OBCK\x01" {
                    continue;
                }
                let generation = u64::from_le_bytes(bytes[5..13].try_into().unwrap());
                if newest
                    .as_ref()
                    .is_none_or(|(newest, _)| generation > *newest)
                {
                    newest = Some((generation, bytes[45..].to_vec()));
                }
            }
        }
        let (_, payload) = newest.expect("the Session published no checkpoint");
        serde_json::from_slice(&payload).unwrap()
    }

    fn store(&self) -> UserPreferencesStore {
        let roaming = self.fixture.path("roaming");
        UserPreferencesStore::new(
            UserPreferencesScope::from_environment(|name| {
                (name == "APPDATA").then(|| roaming.clone().into())
            })
            .unwrap(),
        )
    }

    fn preferences(&self) -> UserPreferencesSnapshot {
        match self.store().execute(&UserPreferencesRequest::Read {}) {
            UserPreferencesResponse::Snapshot { snapshot, .. }
                if snapshot.source != "unreadable" =>
            {
                *snapshot
            }
            other => panic!("the preference record is unreadable: {other:?}"),
        }
    }

    fn set_preference(&self, key: PreferenceKey, value: Value) {
        let response = self.store().execute(&UserPreferencesRequest::Update {
            source: self.preferences().source,
            changes: UserPreferencesPatch::set(key, value).unwrap(),
        });
        assert!(
            matches!(
                response,
                UserPreferencesResponse::Snapshot { changed: true, .. }
            ),
            "{response:?}"
        );
    }
}

fn sorted<const N: usize>(sessions: [&String; N]) -> Vec<String> {
    let mut sessions: Vec<_> = sessions.into_iter().cloned().collect();
    sessions.sort();
    sessions
}
