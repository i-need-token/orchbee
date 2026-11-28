//! The rg program search runs: the file beside this process's executable, never one on `PATH`,
//! whose SHA-256 must equal the digest `ripgrep.json` pins for this build's target. It is hashed
//! before the process's first spawn and again before any spawn once its size, modification time
//! or, on macOS and Linux, its device and inode changed, so a file replaced while this process runs
//! is hashed again before its next spawn. A missing or different file leaves search unavailable
//! until the pinned bytes are back. The check catches a damaged, stale or wrong file, not a writer
//! that races a spawn: whoever can replace rg can replace this executable beside it.

use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, PoisonError},
    time::SystemTime,
};

use orchbee_local_runtime::{LogLevel, LogValue, log_event};
use sha2::{Digest, Sha256};

/// The pinned programs, by release platform.
const MANIFEST: &str = include_str!("../../ripgrep.json");

#[cfg(all(windows, target_arch = "x86_64"))]
const TARGET: Option<&str> = Some("windows-x86_64");
#[cfg(all(windows, target_arch = "aarch64"))]
const TARGET: Option<&str> = Some("windows-aarch64");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const TARGET: Option<&str> = Some("macos-aarch64");
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const TARGET: Option<&str> = Some("macos-x86_64");
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const TARGET: Option<&str> = Some("linux-x86_64");
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const TARGET: Option<&str> = Some("linux-aarch64");
#[cfg(not(any(
    all(windows, any(target_arch = "x86_64", target_arch = "aarch64")),
    all(
        any(target_os = "macos", target_os = "linux"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
const TARGET: Option<&str> = None;

const FILE_NAME: &str = if cfg!(windows) { "rg.exe" } else { "rg" };

/// The sibling rg and what its bytes were found to be.
pub(super) struct Program {
    /// Where rg lies and the digest it must have; none when this build pins no rg or this
    /// process's executable cannot be found.
    expected: OnceLock<Option<(PathBuf, [u8; 32])>>,
    /// The file as it was when last hashed, and whether its digest was the pinned one.
    checked: Mutex<Option<Checked>>,
}

struct Checked {
    identity: Identity,
    pinned: bool,
}

/// What tells one file from its replacement without reading it.
#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl Program {
    pub(super) const fn new() -> Self {
        Self {
            expected: OnceLock::new(),
            checked: Mutex::new(None),
        }
    }

    /// The program to spawn; `None` while the sibling rg is missing or not the pinned one.
    pub(super) fn verified(&self) -> Option<PathBuf> {
        let (path, expected) = self.expected.get_or_init(expected).as_ref()?;
        let identity = identity(path).ok()?;
        // Held while hashing, so concurrent spawns wait for one check instead of each reading the
        // file. The lock guards no invariant a panic could break.
        let mut checked = self.checked.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(previous) = checked
            .as_ref()
            .filter(|previous| previous.identity == identity)
        {
            return previous.pinned.then(|| path.clone());
        }
        // The identity read before hashing is kept, so a file that changes while it is read is
        // hashed again before the next spawn. A refusal is logged once for each state of the file.
        let pinned = match digest(path) {
            Ok(actual) if actual == *expected => true,
            Ok(_) => {
                log_event(LogLevel::Warning, "rg-refused", &[]);
                false
            }
            Err(error) => {
                log_event(
                    LogLevel::Warning,
                    "rg-refused",
                    &[("error", LogValue::from(error.kind()))],
                );
                false
            }
        };
        *checked = Some(Checked { identity, pinned });
        pinned.then(|| path.clone())
    }
}

/// The sibling rg and the digest `ripgrep.json` pins for this build's target.
fn expected() -> Option<(PathBuf, [u8; 32])> {
    let digest = TARGET.and_then(pinned)?;
    let executable = orchbee_local_runtime::current_executable().ok()?;
    Some((executable.parent()?.join(FILE_NAME), digest))
}

/// The digest `ripgrep.json` pins for `target`.
fn pinned(target: &str) -> Option<[u8; 32]> {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).ok()?;
    let hex = manifest
        .get("targets")?
        .get(target)?
        .get("sha256")?
        .as_str()?;
    let mut digest = [0_u8; 32];
    if hex.len() != 64 {
        return None;
    }
    for (byte, pair) in digest.iter_mut().zip(hex.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(digest)
}

fn identity(path: &Path) -> io::Result<Identity> {
    let metadata = std::fs::metadata(path)?;
    #[cfg(unix)]
    let (device, inode) = {
        use std::os::unix::fs::MetadataExt;
        (metadata.dev(), metadata.ino())
    };
    Ok(Identity {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        #[cfg(unix)]
        device,
        #[cfg(unix)]
        inode,
    })
}

/// The SHA-256 of the regular file at `path`.
fn digest(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().into())
}
