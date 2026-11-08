//! The CLI under test as tests start it. On Windows it is a copy named by `CohortArtifact`, so no
//! test process carries a release image name (ORC-382); POSIX cohorts keep the shipped names until
//! ORC-328 and run Cargo's binary.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub use crate::inherited_environment::orchbee_command;

/// The CLI executable this test process starts.
pub fn client() -> &'static Path {
    static CLIENT: OnceLock<PathBuf> = OnceLock::new();
    CLIENT.get_or_init(stage)
}

/// The CLI under test without any inherited `ORCHBEE_*` variable, for a test that gives it the
/// environment its fixture owns.
pub fn isolated() -> Command {
    orchbee_command(client())
}

/// Starts the CLI under test for an invocation that owns no fixture. On Windows it receives the
/// shared offline profile, since a development or test build keeps all of its state under
/// `<USERPROFILE>\.<stem>`; a test with a fixture of its own gives the CLI that fixture's profile.
pub fn command() -> Command {
    #[cfg(windows)]
    {
        let mut command = isolated();
        command.env("USERPROFILE", profile());
        command
    }
    #[cfg(not(windows))]
    {
        isolated()
    }
}

/// A profile beside the staged copy, so an invocation without a fixture never reads or writes the
/// person's own state. Such invocations share it, so none of them may leave Orchbee state there.
#[cfg(windows)]
pub fn profile() -> &'static Path {
    static PROFILE: OnceLock<PathBuf> = OnceLock::new();
    PROFILE.get_or_init(|| {
        let profile = client().with_file_name("offline-profile");
        std::fs::create_dir_all(&profile).unwrap();
        profile
    })
}

#[cfg(not(windows))]
fn stage() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchbee"))
}

/// Every test process of one build shares one copy under Cargo's test temporary directory; a later
/// build removes the copies of earlier builds that nothing runs.
#[cfg(windows)]
fn stage() -> PathBuf {
    use orchbee_local_runtime::CohortArtifact;
    use std::{fs, time::UNIX_EPOCH};

    let built = Path::new(env!("CARGO_BIN_EXE_orchbee"));
    let metadata = fs::metadata(built).unwrap();
    let modified = metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let build = format!("staged-client-{:x}-{modified:x}", metadata.len());
    let directory = root.join(&build);
    let client = directory.join(CohortArtifact::Client.file_name());
    if client.is_file() {
        return client;
    }
    fs::create_dir_all(&directory).unwrap();
    let partial = directory.join(format!("partial-{}", std::process::id()));
    fs::copy(built, &partial).unwrap();
    // Another test process of this build may publish first; either complete copy serves.
    if fs::rename(&partial, &client).is_err() {
        let _ = fs::remove_file(&partial);
        assert!(client.is_file(), "stage {}", client.display());
    }
    for entry in fs::read_dir(root).unwrap().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("staged-client-") && name != build {
            // A copy that a running test still holds stays until a later build.
            let _ = fs::remove_dir_all(entry.path());
        }
    }
    client
}
