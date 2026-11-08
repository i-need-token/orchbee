//! Windows state roots. Release keeps `%APPDATA%\Orchbee`, `%LOCALAPPDATA%\Orchbee` and the
//! temporary directory; a development or test environment keeps all three under `<home>\.<stem>`.
use crate::private_filesystem as permissions;
use std::{
    ffi::OsString,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

const RELEASE_STATE_DIRECTORY: &str = "Orchbee";
const RUNTIME_DIRECTORY: &str = "runtime";

/// The stem of this build's cohort file names and environment. Staging sets
/// `ORCHBEE_ARTIFACT_STEM` while it compiles the cohort, so each environment ships its own names
/// (ORC-382). A build without it keeps the shipped `orchbee` names.
pub(super) const ARTIFACT_STEM: &str = match option_env!("ORCHBEE_ARTIFACT_STEM") {
    Some(stem) => stem,
    None => super::RELEASE_STEM,
};
// A malformed stem would name files no staging produces, so it fails the build instead.
const _: () = assert!(
    valid_artifact_stem(ARTIFACT_STEM.as_bytes()),
    "ORCHBEE_ARTIFACT_STEM must be orchbee, ob-dev-<16 lowercase hex>, or ob-test-<16 lowercase hex>"
);
/// Mirrors `ARTIFACT_STEM_PATTERN` in `scripts/stage-native-windows.mjs`.
const fn valid_artifact_stem(stem: &[u8]) -> bool {
    let digest = match stem {
        b"orchbee" => return true,
        [b'o', b'b', b'-', b'd', b'e', b'v', b'-', digest @ ..]
        | [b'o', b'b', b'-', b't', b'e', b's', b't', b'-', digest @ ..] => digest,
        _ => return false,
    };
    if digest.len() != 16 {
        return false;
    }
    let mut index = 0;
    while index < digest.len() {
        if !matches!(digest[index], b'0'..=b'9' | b'a'..=b'f') {
            return false;
        }
        index += 1;
    }
    true
}

fn release() -> bool {
    super::is_release()
}

/// `<home>\.<stem>`. Home follows `USERPROFILE` first, as Node's `os.homedir()` does, so Desktop
/// derives the same directory.
fn data_root(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let home = env("USERPROFILE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(std::env::home_dir)?;
    Some(home.join(format!(".{}", super::environment_stem())))
}

/// The directory that holds this environment's roaming state, including `config.toml`. The path
/// is not validated: a relative or empty source yields a relative path for the caller to refuse.
pub fn roaming_state_root(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if release() {
        env("APPDATA").map(|value| PathBuf::from(value).join(RELEASE_STATE_DIRECTORY))
    } else {
        data_root(&env)
    }
}

/// The directory that holds this environment's local state, including Session catalogs and
/// checkpoints. Storage owners create it on first write; nothing here touches the filesystem.
pub fn local_state_root(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if release() {
        env("LOCALAPPDATA").map(|value| PathBuf::from(value).join(RELEASE_STATE_DIRECTORY))
    } else {
        data_root(&env)
    }
}

/// The variable that supplies the roaming root, as configuration diagnostics name it.
pub(crate) fn roaming_state_variable() -> &'static str {
    if release() { "APPDATA" } else { "USERPROFILE" }
}

pub(super) fn default_runtime_base(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if release() {
        Some(std::env::temp_dir())
    } else {
        data_root(env).map(|root| root.join(RUNTIME_DIRECTORY))
    }
}

/// Creates a development or test environment's default runtime base as a private directory below
/// its data root, so activation can admit it like any existing base. Release uses the existing
/// temporary directory and creates nothing.
pub fn prepare_default_runtime_base(env: impl Fn(&str) -> Option<OsString>) -> io::Result<()> {
    if release() {
        return Ok(());
    }
    let root = data_root(&env).ok_or(io::ErrorKind::NotFound)?;
    let _root = create_state_root(&root)?;
    let base = root.join(RUNTIME_DIRECTORY);
    let created = match fs::create_dir(&base) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    let mut directory = permissions::open_directory(&base, created)?;
    if created {
        permissions::protect_new(&mut directory)
    } else {
        permissions::require_private(&directory)
    }
}

/// Pins a state root and its ancestors and admits the root as a private directory. Only the root
/// must be private; its parent may be a home directory that other ACL entries share. A missing
/// root is absent, never created.
pub(crate) fn open_state_root(root: &Path) -> io::Result<Option<Vec<File>>> {
    let mut pins = permissions::open_ancestors(parent(root)?)?;
    let directory = match permissions::open_directory(root, false) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    permissions::require_private(&directory)?;
    pins.push(directory);
    Ok(Some(pins))
}

/// Like [`open_state_root`], but creates a missing root and protects it as private.
pub(crate) fn create_state_root(root: &Path) -> io::Result<Vec<File>> {
    let mut pins = permissions::open_ancestors(parent(root)?)?;
    let created = match fs::create_dir(root) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    let mut directory = permissions::open_directory(root, created)?;
    if created {
        permissions::protect_new(&mut directory)?;
    } else {
        permissions::require_private(&directory)?;
    }
    pins.push(directory);
    Ok(pins)
}

fn parent(root: &Path) -> io::Result<&Path> {
    permissions::require_exact_absolute(root)?;
    root.parent()
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
