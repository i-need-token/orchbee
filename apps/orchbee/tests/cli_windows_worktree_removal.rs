//! Failed Worktree removals through a real local Session: the CLI, Server, Worker and Zig Host
//! under test, a real Git repository, a directory held open the way another program's working
//! directory holds it, another Workspace where a terminal prints and tabs open and close, and
//! real elapsed time beyond the Server's 30-second Close attempt budget before the user resumes,
//! reads or closes.
#![cfg(windows)]

use orchbee_local_runtime::CohortArtifact;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Longer than the Server's 30-second Close attempt budget, with margin for its maintenance turn.
const HOLD: Duration = Duration::from_secs(35);
/// Bounds a resume or close that must not wait on a parked Close; waiting on one takes 30 seconds
/// for an explicit close and 120 seconds for a resume.
const PROMPT_MS: &str = "20000";

#[test]
fn removals_held_past_the_close_budget_resume_and_close_without_close_resume() {
    let fixture = Fixture::new();
    let repository = fixture.repository();
    let elsewhere = fixture.background_output();
    let mut locked = fixture.failed_removal(&repository, "verify-locked");
    let mut unlocked = fixture.failed_removal(&repository, "verify-unlocked");
    let mut kept = fixture.failed_removal(&repository, "verify-kept");
    unlocked.lock = None;
    // Content in the residue makes the removal unprovable: the Workspace stays held until the
    // explicit close that the failure names.
    fs::write(kept.path.join("keep.txt"), b"keep\n").unwrap();
    kept.lock = None;
    let refused = fixture.resume(&kept);
    let status = operation_status(&refused);
    assert!(
        refused.code == Some(3)
            && status["reason"]["error"] == "checkout-not-found"
            && status["resumable"] == false,
        "{refused}"
    );

    // Each failed removal holds its Workspace Close past the attempt budget while the user keeps
    // working in another Workspace: its terminal prints, and each tab closed there runs Session
    // close maintenance over every retained Close.
    let held_until = Instant::now() + HOLD;
    while Instant::now() < held_until {
        fixture.open_and_close_tab(&elsewhere);
        thread::sleep(Duration::from_secs(3));
    }

    let busy = fixture.resume(&locked);
    let status = operation_status(&busy);
    assert!(
        busy.code == Some(3)
            && status["kind"] == "failed"
            && status["reason"]["error"] == "io"
            && status["resumable"] == true,
        "{busy}"
    );
    let removed = fixture.resume(&unlocked);
    assert!(
        removed.code == Some(0) && operation_status(&removed)["kind"] == "removed",
        "{removed}"
    );
    assert!(!unlocked.path.exists());
    fixture.git(&[
        "show-ref",
        "--verify",
        "--quiet",
        "refs/heads/verify-unlocked",
    ]);
    // A held Close waits for the Worktree decision rather than parking, and `workspace
    // close-resume` cannot end the hold, so it is refused instead of renewing a budget.
    let held = fixture.run(&["workspace", "close-status", "--workspace", &kept.workspace]);
    let close = &held.document["result"];
    assert!(
        held.code == Some(0) && close["status"]["resumable"] == false,
        "{held}"
    );
    let renewed = fixture.run(&[
        "workspace",
        "close-resume",
        "--workspace",
        &kept.workspace,
        "--request",
        close["closeRequestId"].as_str().unwrap(),
        "--generation",
        close["attemptGeneration"].as_str().unwrap(),
        "--confirm",
    ]);
    assert_eq!(
        renewed.document["failure"]["details"]["reason"], "not-resumable",
        "{renewed}"
    );
    // Explicit close also finishes a resumable failure whose directory was released, without Git.
    locked.lock = None;
    for removal in [&kept, &locked] {
        let closed = fixture.run(&[
            "workspace",
            "close",
            &removal.workspace,
            "--confirm",
            "--timeout",
            PROMPT_MS,
        ]);
        // Exit 0 reports that the Workspace disappeared within the wait; expiry exits 7.
        assert!(
            closed.code == Some(0) && closed.document["result"]["action"] == "workspace.close",
            "{closed}"
        );
    }
    // Closing never runs Git: the emptied checkout directory and the residue's content remain.
    assert!(locked.path.is_dir());
    assert!(kept.path.join("keep.txt").is_file());
    let listed = fixture.run(&["workspace", "list"]);
    let remaining = listed.document["result"]["workspaces"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"));
    for removal in [&locked, &unlocked, &kept] {
        assert!(
            remaining
                .iter()
                .all(|workspace| workspace["workspaceId"] != removal.workspace.as_str()),
            "{listed}"
        );
    }
}

/// The fixture root holding the private cohort, isolated profile, runtime base and repository.
struct Fixture(PathBuf);

/// A Worktree whose removal failed on its held directory.
struct Removal {
    operation: String,
    attempt: u64,
    workspace: String,
    path: PathBuf,
    lock: Option<fs::File>,
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
            "orchbee-worktree-removal-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        for path in [
            "cohort", "runtime", "home", "local", "roaming", "noise", "repo",
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        let fixture = Self(root);
        fixture.stage();
        let started = fixture.run(&["server", "start"]);
        assert_eq!(started.code, Some(0), "{started}");
        fixture
    }

    /// Stages the CLI, Server and Worker under test with the real Host as one private cohort.
    fn stage(&self) {
        let built = Path::new(env!("CARGO_BIN_EXE_orchbee")).parent().unwrap();
        let mut artifacts = serde_json::Map::new();
        for (name, artifact, source) in [
            ("orchbee", CohortArtifact::Client, built.join("orchbee.exe")),
            (
                "orchbee-server",
                CohortArtifact::Server,
                built.join("orchbee-server.exe"),
            ),
            (
                "orchbee-worker",
                CohortArtifact::Worker,
                built.join("orchbee-worker.exe"),
            ),
            (
                "orchbee-terminal-host",
                CohortArtifact::TerminalHost,
                Path::new(env!("CARGO_MANIFEST_DIR"))
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
            // Native cohort admission requires private regular files with one link, and each
            // carries its environment name rather than Cargo's release name.
            fs::write(self.0.join("cohort").join(artifact.file_name()), &bytes).unwrap();
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

    /// Runs one CLI verb against this fixture's Session with machine output.
    fn run(&self, words: &[&str]) -> Run {
        let started = Instant::now();
        let output = Command::new(
            self.0
                .join("cohort")
                .join(CohortArtifact::Client.file_name()),
        )
        .args(words)
        .arg("--format=json")
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
        .stdin(Stdio::null())
        .output()
        .unwrap();
        Run {
            words: words.join(" "),
            code: output.status.code(),
            elapsed: started.elapsed(),
            document: serde_json::from_slice(&output.stdout).unwrap_or(Value::Null),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn git(&self, words: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.0.join("repo"))
            .args(words)
            .env("HOME", self.0.join("home"))
            .env("USERPROFILE", self.0.join("home"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {words:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn repository(&self) -> String {
        self.git(&["init", "--quiet", "--initial-branch=main"]);
        self.git(&["config", "user.name", "Orchbee Fixture"]);
        self.git(&["config", "user.email", "fixture@example.invalid"]);
        self.git(&["config", "commit.gpgsign", "false"]);
        fs::write(self.0.join("repo/seed.txt"), b"seed\n").unwrap();
        self.git(&["add", "seed.txt"]);
        self.git(&["commit", "--quiet", "-m", "seed"]);
        self.0.join("repo").to_str().unwrap().to_owned()
    }

    /// Starts a terminal in another Workspace that prints every second until the Session stops,
    /// and returns that Workspace.
    fn background_output(&self) -> String {
        let created = self.run(&[
            "workspace",
            "create",
            "--cwd",
            self.0.join("noise").to_str().unwrap(),
            "--label",
            "noise",
        ]);
        let workspace = created.document["workspace"]["result"]["workspaceId"]
            .as_str()
            .unwrap_or_else(|| panic!("{created}"))
            .to_owned();
        let printing = self.run(&[
            "tab",
            "create",
            "--workspace",
            &workspace,
            "--label",
            "ping",
            "--exec",
            "ping",
            "--arg",
            "-t",
            "--arg",
            "127.0.0.1",
        ]);
        assert_eq!(printing.code, Some(0), "{printing}");
        workspace
    }

    /// Opens a default-shell tab in `workspace` and closes it again.
    fn open_and_close_tab(&self, workspace: &str) {
        let opened = self.run(&[
            "tab",
            "create",
            "--workspace",
            workspace,
            "--label",
            "scratch",
        ]);
        let tab = opened.document["result"]["tabId"]
            .as_str()
            .unwrap_or_else(|| panic!("{opened}"));
        let closed = self.run(&["tab", "close", tab, "--confirm", "--timeout", PROMPT_MS]);
        assert_eq!(closed.code, Some(0), "{closed}");
    }

    /// Creates a Worktree with its first terminal, holds its directory, and removes it.
    fn failed_removal(&self, repository: &str, branch: &str) -> Removal {
        let created = self.run(&[
            "worktree",
            "create",
            "--cwd",
            repository,
            "--branch",
            branch,
            "--timeout",
            "60000",
        ]);
        let status = operation_status(&created);
        assert!(
            created.code == Some(0)
                && status["kind"] == "opened"
                && status["terminalOutcome"]["kind"] == "started",
            "{created}"
        );
        let workspace = status["workspaceId"].as_str().unwrap().to_owned();
        let path = PathBuf::from(status["checkout"]["path"].as_str().unwrap());
        let lock = Some(hold(&path));
        let removed = self.run(&[
            "worktree",
            "remove",
            "--workspace",
            &workspace,
            "--confirm",
            "--timeout",
            "60000",
        ]);
        let status = operation_status(&removed);
        // Git deletes the registration and contents, then fails on the held directory itself.
        assert!(
            removed.code == Some(3)
                && status["kind"] == "failed"
                && status["reason"]["error"] == "git-failed"
                && status["resumable"] == true,
            "{removed}"
        );
        let reply = &removed.document["result"]["observation"]["reply"];
        Removal {
            operation: reply["operationId"].as_str().unwrap().to_owned(),
            attempt: reply["outcome"]["attempt"].as_u64().unwrap(),
            workspace,
            path,
            lock,
        }
    }

    fn resume(&self, removal: &Removal) -> Run {
        self.run(&[
            "worktree",
            "resume",
            &removal.operation,
            "--generation",
            &removal.attempt.to_string(),
            "--confirm",
            "--timeout",
            PROMPT_MS,
        ])
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Stopping the Session ends its terminals, Host, Worker and Server before cleanup.
        let stopped = self.run(&["server", "stop", "--confirm"]);
        if !thread::panicking() {
            assert_eq!(stopped.code, Some(0), "{stopped}");
        }
        let target = fs::canonicalize(&self.0).unwrap();
        assert!(target.starts_with(fs::canonicalize(std::env::temp_dir()).unwrap()));
        assert!(
            target
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("orchbee-worktree-removal-"))
        );
        let deadline = Instant::now() + Duration::from_secs(10);
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

impl fmt::Display for Run {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`orchbee {}` exited {:?} after {:?}: {}\n{}",
            self.words, self.code, self.elapsed, self.document, self.stderr
        )
    }
}

fn operation_status(run: &Run) -> &Value {
    &run.document["result"]["observation"]["reply"]["outcome"]["status"]
}

/// Opens a directory without delete sharing, as another program's working directory does.
fn hold(directory: &Path) -> fs::File {
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(directory)
        .unwrap()
}
