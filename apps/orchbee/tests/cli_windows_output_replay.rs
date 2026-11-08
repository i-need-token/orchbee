//! Terminal output replay across real managed restarts: the CLI, Server, Worker and Zig Host under
//! test, and a default Session whose default program is cmd. With the preference on, each live
//! terminal's recent output is kept beside the checkpoint and shown again above the new prompt,
//! followed by the divider; a closed terminal's file goes at the next save; a file that belongs to
//! another Presentation is deleted and never replayed; turning the preference off removes every
//! kept file at the next save. The oracle is the Client text read and the files the runtime wrote;
//! nothing imports Server internals.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    PreferenceKey, UserPreferencesPatch, UserPreferencesRequest, UserPreferencesResponse,
};
use serde_json::{Value, json};
use session::{Fixture, system_program};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// Past the 30 s periodic save, so a capture that only time triggers has happened.
const SAVED: Duration = Duration::from_secs(45);
const STARTED: Duration = Duration::from_secs(30);
const DIVIDER: &str = "Output before restart (";
const CMD_PROFILE: &str = "cmd";
/// A kept file's checkpoint-style frame: magic, little-endian token and a SHA-256 digest.
const FRAME_MAGIC: &[u8] = b"OBCK\x01";
const FRAME_HEADER: usize = 45;

#[test]
fn kept_output_replays_only_into_its_own_terminal_and_never_outlives_its_reason() {
    let witness = Witness::new();
    let alpha = witness.fixture.workspace("alpha");
    let beta = witness.fixture.workspace("beta");
    witness.split(&alpha);
    let terminals = witness.terminals();
    let gamma = terminals
        .keys()
        .find(|panel| **panel != alpha && **panel != beta)
        .expect("the split Panel")
        .clone();
    for (panel, marker) in [
        (&alpha, "alpha-marker"),
        (&beta, "beta-marker"),
        (&gamma, "gamma-marker"),
    ] {
        witness.until_prompt(&terminals[panel]);
        witness.send(&terminals[panel], &format!("echo {marker}"));
    }
    witness.until("every terminal's output is kept", SAVED, || {
        let kept = witness.kept();
        ["alpha-marker", "beta-marker", "gamma-marker"]
            .iter()
            .all(|marker| kept.iter().any(|file| file.holds(marker)))
    });

    // A closed terminal's file goes at the next save; the checkpoint names exactly what is kept.
    let closed = witness
        .fixture
        .run(&["panel", "close", &gamma, "--confirm"]);
    assert_eq!(closed.code, Some(0), "{closed}");
    witness.until("the closed terminal's file is gone", SAVED, || {
        !witness.kept().iter().any(|file| file.holds("gamma-marker"))
    });
    witness.assert_named_exactly();

    // Each terminal shows its own earlier output, then the divider, then the new prompt.
    witness.stop();
    witness.assert_named_exactly();
    witness.start();
    let terminals = witness.restored();
    for (workspace, own, other) in [
        ("alpha", "alpha-marker", "beta-marker"),
        ("beta", "beta-marker", "alpha-marker"),
    ] {
        let terminal = &terminals[workspace];
        witness.until("the replay and a new prompt", STARTED, || {
            let screen = witness.screen(terminal);
            screen
                .find(DIVIDER)
                .is_some_and(|divider| screen[divider..].lines().skip(1).any(is_prompt))
        });
        let screen = witness.screen(terminal);
        let divider = screen.find(DIVIDER).unwrap();
        let marker = screen.find(own).unwrap_or_else(|| panic!("{screen}"));
        assert!(marker < divider, "{screen}");
        assert!(!screen.contains(other), "{screen}");
    }

    // A file holding another Presentation's output is deleted at restore and never replayed.
    witness.stop();
    let kept = witness.kept();
    let [first, second] = kept.as_slice() else {
        panic!("{kept:?}");
    };
    first.reframe(&second.payload);
    second.reframe(&first.payload);
    witness.start();
    let terminals = witness.restored();
    for terminal in terminals.values() {
        witness.until_prompt(terminal);
        let screen = witness.screen(terminal);
        assert!(!screen.contains(DIVIDER), "{screen}");
        assert!(!screen.contains("-marker"), "{screen}");
    }
    witness.until("the mismatched files are gone", STARTED, || {
        !first.path.exists() && !second.path.exists()
    });

    // Turning the preference off removes every kept file and the checkpoint names none.
    witness.until("restored terminals keep output again", SAVED, || {
        witness.kept().len() == 2
    });
    witness.set_preference(PreferenceKey::RestoreTerminalOutput, json!(false));
    witness.until("every kept file is gone", SAVED, || {
        witness.kept().is_empty() && !witness.newest_checkpoint().contains("\"output\"")
    });
}

/// One kept output file and its framed payload.
#[derive(Debug)]
struct Kept {
    path: PathBuf,
    token: u64,
    payload: Vec<u8>,
}

impl Kept {
    fn holds(&self, text: &str) -> bool {
        self.payload
            .windows(text.len())
            .any(|window| window == text.as_bytes())
    }

    /// Rewrites this file with `payload` under its own token, a valid frame for the wrong owner.
    fn reframe(&self, payload: &[u8]) {
        let mut bytes = FRAME_MAGIC.to_vec();
        bytes.extend_from_slice(&self.token.to_le_bytes());
        let mut digest = Sha256::new();
        digest.update(&bytes);
        digest.update(payload);
        bytes.extend_from_slice(&digest.finalize());
        bytes.extend_from_slice(payload);
        fs::write(&self.path, bytes).unwrap();
    }
}

struct Witness {
    fixture: Fixture,
}

impl Witness {
    fn new() -> Self {
        let witness = Self {
            fixture: Fixture::new("orchbee-output-replay-", &[]),
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
        witness.set_preference(PreferenceKey::RestoreTerminalOutput, json!(true));
        witness
    }

    fn until(&self, what: &str, within: Duration, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + within;
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            thread::sleep(Duration::from_millis(250));
        }
    }

    fn until_prompt(&self, terminal: &str) {
        self.until("a prompt", STARTED, || {
            self.screen(terminal).lines().any(is_prompt)
        });
    }

    fn split(&self, panel: &str) {
        let environment = self.fixture.terminal_environment();
        let mut words = vec!["panel", "split", "--direction=right", panel];
        words.extend(environment.iter().map(String::as_str));
        let created = self.fixture.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
    }

    /// The terminal of each Workspace by its label, once a restart bound both again. Panel ids
    /// are not kept across a restart; Workspace contexts are.
    fn restored(&self) -> HashMap<String, String> {
        let mut restored = HashMap::new();
        self.until("every Workspace's terminal is restored", STARTED, || {
            let listed = self.fixture.run(&["workspace", "list"]);
            let contexts: HashMap<String, String> = listed.document["result"]["workspaces"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|workspace| {
                    Some((
                        workspace["terminalContextId"].as_str()?.to_owned(),
                        workspace["label"].as_str()?.to_owned(),
                    ))
                })
                .collect();
            let listed = self.fixture.run(&["terminal", "list"]);
            restored = listed.document["result"]["terminals"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|terminal| {
                    let context = terminal["boundTo"]["terminalContextId"].as_str()?;
                    Some((contexts.get(context)?.clone(), handle(terminal)?))
                })
                .collect();
            ["alpha", "beta"]
                .iter()
                .all(|label| restored.contains_key(*label))
        });
        restored
    }

    /// The typed handle of each bound terminal by its Panel.
    fn terminals(&self) -> HashMap<String, String> {
        let listed = self.fixture.run(&["terminal", "list"]);
        listed.document["result"]["terminals"]
            .as_array()
            .unwrap_or_else(|| panic!("{listed}"))
            .iter()
            .filter_map(|terminal| {
                Some((
                    terminal["boundTo"]["panelId"].as_str()?.to_owned(),
                    handle(terminal)?,
                ))
            })
            .collect()
    }

    /// The recent unwrapped text of `terminal`, scrollback included. A read is a Client view.
    fn screen(&self, terminal: &str) -> String {
        let read =
            self.fixture
                .run(&["terminal", "read", terminal, "--source", "recent-unwrapped"]);
        read.document["result"]["content"]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{read}"))
            .to_owned()
    }

    fn send(&self, terminal: &str, text: &str) {
        let sent = self
            .fixture
            .run(&["terminal", "send", terminal, "--text", text, "--enter"]);
        assert_eq!(sent.code, Some(0), "{sent}");
    }

    fn stop(&self) {
        let stopped = self
            .fixture
            .run(&["server", "stop", "--confirm", "--session", "default"]);
        assert_eq!(stopped.code, Some(0), "{stopped}");
    }

    fn start(&self) {
        let started = self
            .fixture
            .run(&["server", "start", "--session", "default"]);
        assert_eq!(started.code, Some(0), "{started}");
    }

    /// Every file under the fixture root, depth first.
    fn files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut directories = vec![PathBuf::from(self.fixture.path(""))];
        while let Some(directory) = directories.pop() {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files
    }

    /// Every kept output file the runtime wrote, ordered by token.
    fn kept(&self) -> Vec<Kept> {
        let mut kept: Vec<Kept> = self
            .files()
            .into_iter()
            .filter(|path| {
                path.parent().and_then(Path::file_name) == Some("terminal-output".as_ref())
            })
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?;
                let token = name.strip_suffix(".bin")?.parse::<u64>().ok()?;
                let bytes = fs::read(&path).ok()?;
                (bytes.len() > FRAME_HEADER && bytes.starts_with(FRAME_MAGIC)).then(|| Kept {
                    payload: bytes[FRAME_HEADER..].to_vec(),
                    path,
                    token,
                })
            })
            .collect();
        kept.sort_by_key(|file| file.token);
        kept
    }

    /// The newest published checkpoint payload as text.
    fn newest_checkpoint(&self) -> String {
        self.files()
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("checkpoint") && name.ends_with(".json"))
            })
            .filter_map(|path| {
                let bytes = fs::read(&path).ok()?;
                (bytes.len() >= FRAME_HEADER && bytes.starts_with(FRAME_MAGIC)).then(|| {
                    (
                        u64::from_le_bytes(bytes[5..13].try_into().unwrap()),
                        String::from_utf8_lossy(&bytes[FRAME_HEADER..]).into_owned(),
                    )
                })
            })
            .max_by_key(|(generation, _)| *generation)
            .map(|(_, payload)| payload)
            .expect("a published checkpoint")
    }

    /// The newest checkpoint names every kept file and nothing else.
    fn assert_named_exactly(&self) {
        let checkpoint = self.newest_checkpoint();
        let kept = self.kept();
        assert_eq!(
            checkpoint.matches("\"output\":").count(),
            kept.len(),
            "{checkpoint}"
        );
        for file in &kept {
            assert!(
                checkpoint.contains(&format!("\"output\":\"{:016x}\"", file.token)),
                "{checkpoint}"
            );
        }
    }

    fn store(&self) -> UserPreferencesStore {
        UserPreferencesStore::new(
            UserPreferencesScope::from_environment(self.fixture.state_environment()).unwrap(),
        )
    }

    fn set_preference(&self, key: PreferenceKey, value: Value) {
        let source = match self.store().execute(&UserPreferencesRequest::Read {}) {
            UserPreferencesResponse::Snapshot { snapshot, .. } => snapshot.source,
            other => panic!("the preference record is unreadable: {other:?}"),
        };
        let response = self.store().execute(&UserPreferencesRequest::Update {
            source,
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

/// The typed handle the CLI accepts for a listed terminal.
fn handle(terminal: &Value) -> Option<String> {
    Some(format!(
        "@terminal:0x{}",
        terminal["terminalId"].as_str()?.trim_start_matches('0')
    ))
}

/// A cmd prompt: a drive path ending in `>`.
fn is_prompt(line: &str) -> bool {
    let line = line.trim_end();
    line.ends_with('>') && line.contains(":\\")
}
