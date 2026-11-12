//! The machine hub's endpoint name, derived from the data root's canonical path as a Session
//! endpoint is from its runtime folder. The spelling freezes at the first public release, because
//! an older hub and a newer interface meet after an update; `apps/orchbee/docs/machines.md` owns
//! the reasons.
use crate::DataRoot;
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};

/// The hub endpoint of `root`, which must exist: the name hashes the root's canonical path, so
/// every spelling of one folder reaches one hub, and only the root, never the stem, so the
/// endpoint and [`DataRoot::machine_hub_lock`] always name the same hub.
pub fn machine_hub_endpoint(root: &DataRoot) -> io::Result<PathBuf> {
    root.path()
        .canonicalize()
        .map(|canonical| endpoint(&canonical))
}

/// `\\.\pipe\orchbee-<id>-hub`, where `<id>` hashes the canonical path's UTF-16 code units with
/// ASCII letters, the drive letter included, lowercased.
#[cfg(windows)]
fn endpoint(canonical: &Path) -> PathBuf {
    use std::os::windows::ffi::OsStrExt;
    let mut hash = Sha256::new();
    hash.update(b"orchbee-windows-machine-hub-v1\0");
    for unit in canonical.as_os_str().encode_wide() {
        let unit = if (65..=90).contains(&unit) {
            unit + 32
        } else {
            unit
        };
        hash.update(unit.to_le_bytes());
    }
    format!(
        r"\\.\pipe\orchbee-{}-hub",
        &format!("{:x}", hash.finalize())[..32]
    )
    .into()
}

/// `<socket folder>/<id>-hub.sock` in this environment's socket folder for the effective user:
/// at most 90 bytes, inside the 103 that macOS allows.
#[cfg(unix)]
fn endpoint(canonical: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let mut hash = Sha256::new();
    hash.update(b"orchbee-posix-machine-hub-v1\0");
    hash.update(canonical.as_os_str().as_bytes());
    crate::ModeCheckedLocalSessionRendezvous::socket_root_path().join(format!(
        "{}-hub.sock",
        &format!("{:x}", hash.finalize())[..32]
    ))
}
