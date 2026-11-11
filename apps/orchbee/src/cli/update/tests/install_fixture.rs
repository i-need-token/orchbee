//! Engine fixtures shared by the Windows install and channel tests: a private temporary root, a
//! console-interrupt context, and probe and activation commands that never touch a user PATH.
use super::super::{Context, Report};
use crate::cli::console_interrupt::ArmedSignals;
use orchbee_local_runtime::PrivateCache;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// A private directory under the test process's temporary folder, removed on drop.
pub(super) struct Fixture(pub(super) PathBuf);

impl Fixture {
    pub(super) fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "orchbee-release-test-{}-{nanos}",
            std::process::id()
        ));
        PrivateCache::prepare(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-release-test-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}

/// A context with a minute's deadline. The console interrupt has one owner per process, so this
/// waits while another test holds it.
pub(super) fn context() -> Context {
    let give_up = Instant::now() + Duration::from_secs(30);
    loop {
        match ArmedSignals::arm_interrupts() {
            Ok(signals) => {
                return Context {
                    signals,
                    deadline: Instant::now() + Duration::from_secs(60),
                    curl_checked: false,
                };
            }
            Err(_) if Instant::now() < give_up => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => panic!("console interrupt: {error}"),
        }
    }
}

pub(super) fn report() -> Report {
    Report {
        phase: "configuration",
        channel: None,
        channel_saved: false,
        config_path: None,
        published: None,
        reused: false,
        activated: false,
        cleanup_pending: None,
        version: None,
        build_id: None,
        configuration_failure: None,
    }
}

/// Prints the staged Client's bytes, which this fixture writes as the Client's version line.
pub(super) fn print_probe(client: &Path) -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/d", "/c", "type"]).arg(client);
    command
}

/// Reads the published manifest, so activation fails unless it receives a published release.
pub(super) fn reading_activation(_root: &Path, release: &Path) -> io::Result<Command> {
    Ok(print_probe(&release.join("release.json")))
}
