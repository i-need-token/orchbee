//! Engine fixtures shared by the Windows install and channel tests: a private temporary root, a
//! console-interrupt context, a probe command, and an in-process completion that never touches a
//! user PATH.
use super::super::{
    Context, Error, complete,
    install::Completion,
    pointer::{Entry, Pointer},
};
use crate::{cli::console_interrupt::ArmedSignals, release_store::read_pointer};
use orchbee_local_runtime::PrivateCache;
use sha2::{Digest, Sha256};
use std::{
    fs,
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

/// Prints the staged Client's bytes, which this fixture writes as the Client's version line.
pub(super) fn print_probe(client: &Path) -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/d", "/c", "type"]).arg(client);
    command
}

/// Completes the install of a published version directory in process, as its Client's completion
/// helper would after verifying its own signature, so it fails unless it receives a published
/// release.
pub(super) fn in_process_completion(
    root: &Path,
    version: &Path,
    completion: Completion,
    _context: &mut Context,
) -> Result<(), Error> {
    let bytes = fs::read(version.join("release.json")).map_err(|_| Error::NotReleaseDirectory)?;
    let entry = Entry {
        product: version.file_name().unwrap().to_string_lossy().into_owned(),
        manifest: format!("{:x}", Sha256::digest(&bytes)),
    };
    complete::activate(root, &entry, completion)
}

/// The pointer under `root`, or `None` when it is missing or names no valid record.
pub(super) fn pointer(root: &Path) -> Option<Pointer> {
    read_pointer(root)
        .unwrap()
        .and_then(|bytes| Pointer::parse(&bytes))
}
