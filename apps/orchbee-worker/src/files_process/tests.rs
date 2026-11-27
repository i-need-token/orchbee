use std::{
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    FilesEndOutcome as End, FilesEndpoint, FilesProcessAction as Action, FilesProcessIdentity,
    FilesProcessOutcome as Outcome, FilesProcessRequest, FilesStartOutcome as Start, FilesToken,
    OperationId, WorkerGeneration,
};
use orchbee_local_runtime::{Cohort, CohortArtifact, CommandChild};
use orchbee_local_worker_protocol::WorkerId;
use sha2::{Digest, Sha256};

use super::{FilesProcess, Held};
use crate::default_interactive_launch::SystemShell;

const WORKER: WorkerId = WorkerId::from_bytes([7; 16]);
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
const FIXTURE: &[u8] = b"native fixture\n";
const FIXTURE_SHA256: &str = "396cfec2d1c795ce71dce2d9c92f29cddec638632050c17861d771936a99282d";
static NEXT: AtomicU64 = AtomicU64::new(1);

/// A program that reads the token line, writes its first argument and the token where its second
/// argument, the endpoint, names, and keeps running, as `orchbee-files` keeps its link.
#[cfg(unix)]
const SIBLING: &[u8] =
    b"#!/bin/sh\nread -r token\nprintf '%s %s\\n' \"$1\" \"$token\" > \"$2\"\nexec sleep 30\n";

/// A cohort folder with the four artifacts and, when given, the `orchbee-files` digest its
/// manifest declares and the file's bytes, none when empty.
struct Fixture(PathBuf);

impl Fixture {
    fn new(sibling: Option<(&[u8], &str)>) -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "orchbee-worker-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        private(&path, 0o700);
        let mut entries = Vec::new();
        for (role, artifact) in [
            ("orchbee", CohortArtifact::Client),
            ("orchbee-server", CohortArtifact::Server),
            ("orchbee-worker", CohortArtifact::Worker),
            ("orchbee-terminal-host", CohortArtifact::TerminalHost),
        ] {
            let file = path.join(artifact.file_name());
            fs::write(&file, FIXTURE).unwrap();
            private(&file, 0o700);
            entries.push(format!("\"{role}\":\"{FIXTURE_SHA256}\""));
        }
        entries.push(format!(
            "\"agent-observation-adapters\":\"{FIXTURE_SHA256}\""
        ));
        if let Some((bytes, declared)) = sibling {
            if !bytes.is_empty() {
                let file = path.join(format!(
                    "{}-files{}",
                    orchbee_local_runtime::environment_stem(),
                    std::env::consts::EXE_SUFFIX
                ));
                fs::write(&file, bytes).unwrap();
                private(&file, 0o700);
            }
            entries.push(format!("\"orchbee-files\":\"{declared}\""));
        }
        let manifest = path.join("cohort.json");
        fs::write(
            &manifest,
            format!(
                "{{\"schemaVersion\":1,\"artifacts\":{{{}}}}}",
                entries.join(",")
            ),
        )
        .unwrap();
        private(&manifest, 0o600);
        Self(path)
    }

    fn cohort(&self) -> Cohort {
        Cohort::load(&self.0, Instant::now() + Duration::from_secs(5)).unwrap()
    }

    /// Where the started process dials: a file it writes on macOS and Linux, and on Windows a pipe
    /// name it only receives.
    fn endpoint(&self) -> PathBuf {
        #[cfg(unix)]
        return self.0.join("endpoint");
        #[cfg(windows)]
        return PathBuf::from(r"\\.\pipe").join(self.0.file_name().unwrap());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn private(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(windows)]
const fn private(_path: &Path, _mode: u32) {}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn execute(
    files: &FilesProcess<'_>,
    shell: &SystemShell,
    generation: NonZeroU64,
    action: Action,
) -> Outcome {
    let request = FilesProcessRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes(WORKER.as_bytes())
            .unwrap(),
        worker_generation: WorkerGeneration::new(generation),
        action,
    };
    files.execute(request, WORKER, GENERATION, shell).outcome
}

fn start(endpoint: &Path) -> Action {
    Action::Start {
        endpoint: FilesEndpoint::new(endpoint.to_str().unwrap().to_owned()).unwrap(),
        token: FilesToken::from_bytes([0x5a; 32]),
    }
}

#[test]
fn start_runs_only_the_sibling_its_cohort_declares_and_holds_one_process() {
    let shell = SystemShell::resolve();
    let other = NonZeroU64::new(2).unwrap();
    let absent = Fixture::new(None);
    let missing_file = Fixture::new(Some((b"", FIXTURE_SHA256)));
    #[cfg(unix)]
    let program = SIBLING.to_vec();
    // A copy of the system's command interpreter stands in: it runs the token line as a command,
    // which fails, and ends at the end of its input.
    #[cfg(windows)]
    let program =
        fs::read(PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join(r"System32\cmd.exe"))
            .unwrap();
    let tampered = Fixture::new(Some((&program, FIXTURE_SHA256)));
    let verified = Fixture::new(Some((&program, &sha256(&program))));
    let (absent_cohort, missing_cohort, tampered_cohort, verified_cohort) = (
        absent.cohort(),
        missing_file.cohort(),
        tampered.cohort(),
        verified.cohort(),
    );
    let target = verified.endpoint();

    // A request for another generation changes nothing, whatever it asks.
    let files = FilesProcess::new(Some(&verified_cohort));
    assert_eq!(
        execute(&files, &shell, other, start(&target)),
        Outcome::Start(Start::RejectedFencing)
    );
    let named = FilesProcessIdentity {
        process_id: 1,
        start: 1,
    };
    assert_eq!(
        execute(&files, &shell, other, Action::End { process: named }),
        Outcome::End(End::RejectedFencing)
    );
    assert!(files.held.borrow().is_none());

    for (cohort, expected) in [
        (None, Start::Missing),
        (Some(&absent_cohort), Start::Missing),
        (Some(&missing_cohort), Start::Missing),
        (Some(&tampered_cohort), Start::Unverified),
    ] {
        let refused = FilesProcess::new(cohort);
        assert_eq!(
            execute(&refused, &shell, GENERATION, start(&target)),
            Outcome::Start(expected)
        );
        assert!(refused.held.borrow().is_none());
    }

    let Outcome::Start(Start::Started(first)) = execute(&files, &shell, GENERATION, start(&target))
    else {
        panic!("the verified sibling did not start");
    };
    assert_eq!(
        files.held.borrow().as_ref().map(|held| held.identity),
        Some(first)
    );
    #[cfg(unix)]
    {
        // The process received the endpoint as its argument and the token line on its input.
        let deadline = Instant::now() + Duration::from_secs(10);
        let written = loop {
            match fs::read_to_string(&target) {
                Ok(written) if written.ends_with('\n') => break written,
                _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                other => panic!("the sibling wrote no token line: {other:?}"),
            }
        };
        assert_eq!(written, format!("--local-endpoint {}\n", "5a".repeat(32)));
    }
    // A second start ends the held process first, so a generation never runs two.
    let Outcome::Start(Start::Started(second)) =
        execute(&files, &shell, GENERATION, start(&target))
    else {
        panic!("the second start did not start");
    };
    assert_ne!(first, second);
    assert_eq!(
        execute(&files, &shell, GENERATION, Action::End { process: first }),
        Outcome::End(End::Unknown)
    );
    #[cfg(unix)]
    {
        assert_eq!(
            execute(&files, &shell, GENERATION, Action::End { process: second }),
            Outcome::End(End::Ended)
        );
        assert!(files.held.borrow().is_none());
    }
    // The interpreter ends at the end of its input, and a reap drops it unasked.
    #[cfg(windows)]
    wait_until_reaped(&files);
}

#[test]
fn end_settles_only_the_named_process_and_reap_drops_one_that_ended() {
    let shell = SystemShell::resolve();
    let files = FilesProcess::new(None);
    let child = running();
    let (process_id, start) = child.exact_identity().unwrap();
    let identity = FilesProcessIdentity { process_id, start };
    *files.held.borrow_mut() = Some(Held { identity, child });
    // The same process ID with another start names a different process.
    let other = FilesProcessIdentity {
        process_id,
        start: start.wrapping_add(1),
    };
    assert_eq!(
        execute(&files, &shell, GENERATION, Action::End { process: other }),
        Outcome::End(End::Unknown)
    );
    files.reap();
    assert!(
        files.held.borrow().is_some(),
        "a running process stays held"
    );
    assert_eq!(
        execute(
            &files,
            &shell,
            GENERATION,
            Action::End { process: identity }
        ),
        Outcome::End(End::Ended)
    );
    assert!(files.held.borrow().is_none());
    assert_eq!(
        execute(
            &files,
            &shell,
            GENERATION,
            Action::End { process: identity }
        ),
        Outcome::End(End::Unknown)
    );

    let child = ending();
    let (process_id, start) = child.exact_identity().unwrap();
    *files.held.borrow_mut() = Some(Held {
        identity: FilesProcessIdentity { process_id, start },
        child,
    });
    wait_until_reaped(&files);
}

fn wait_until_reaped(files: &FilesProcess<'_>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while files.held.borrow().is_some() {
        assert!(Instant::now() < deadline, "an ended process stayed held");
        thread::sleep(Duration::from_millis(20));
        files.reap();
    }
}

#[cfg(unix)]
fn shell_command(script: &str) -> CommandChild {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    CommandChild::spawn(command).unwrap()
}

#[cfg(unix)]
fn running() -> CommandChild {
    shell_command("exec sleep 30")
}

#[cfg(unix)]
fn ending() -> CommandChild {
    shell_command("exec sleep 1")
}

#[cfg(windows)]
fn system_program(name: &str, arguments: &[&str]) -> CommandChild {
    let mut command = Command::new(
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join(name),
    );
    command
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    CommandChild::spawn_contained(command).unwrap()
}

#[cfg(windows)]
fn running() -> CommandChild {
    system_program(
        r"WindowsPowerShell\v1.0\powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep 30",
        ],
    )
}

#[cfg(windows)]
fn ending() -> CommandChild {
    system_program("cmd.exe", &["/d", "/c", "exit 0"])
}
