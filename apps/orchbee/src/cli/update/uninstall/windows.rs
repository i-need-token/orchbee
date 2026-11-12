//! Windows effects of uninstall outside the two roots: the user `Path` entry, the Start menu
//! shortcut and the uninstall registration; the processes running from the store; the npm carrier;
//! and the removal of the running program's own files after it exits.
//!
//! A running executable cannot be deleted on Windows, so the last files of the store, the Client's
//! own file, the launcher waiting for it and their folders, are removed by a detached `cmd.exe`
//! from the system folder after both exit. It inherits no handle, works in the system folder,
//! deletes only those exact files and folders without recursion, and ends once the store root is
//! gone or after about a minute. Interrupted, it leaves only those files and folders inside the
//! store root.
use super::super::path_value::{self, Edit};
use orchbee_local_runtime::{EnvironmentKind, environment_kind, environment_stem, is_release};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};
use winsafe::{self as w, co};

/// The variable a test build reads for a scratch registry key that stands in for
/// `HKEY_CURRENT_USER`, so that a rehearsal never touches the real `Path` or uninstall entries.
const TEST_REGISTRY_KEY: &str = "ORCHBEE_TEST_REGISTRY_KEY";
/// The only registry folder a test build's scratch key may name.
const TEST_REGISTRY_PREFIX: &str = "Software\\ob-test-scratch\\";
/// Where Windows lists per-user uninstall entries.
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
/// About one minute of removal attempts, one second apart.
const REMOVAL_ATTEMPTS: u32 = 60;

/// Where this environment's entries outside the roots are.
pub(super) struct Locations {
    /// The key below `HKEY_CURRENT_USER` whose `Path` value the PATH step edits.
    environment: String,
    /// The key below `HKEY_CURRENT_USER` that holds uninstall registrations.
    registrations: String,
    /// The installer's registration name, `com.orchbee.desktop_is1` in release.
    registration: String,
    /// The Start menu shortcut, `Orchbee.lnk` in release, when `APPDATA` names a folder.
    shortcut: Option<PathBuf>,
}

/// The store root this build installs into.
pub(super) fn store_root() -> io::Result<PathBuf> {
    super::super::platform::install_root()
}

/// The folders no root may equal or hold: the person's home, the profile's application-data
/// folders, the system and program folders, the per-user programs folder, and the temporary folder.
pub(super) fn protected_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = [
        "USERPROFILE",
        "HOME",
        "LOCALAPPDATA",
        "APPDATA",
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "PUBLIC",
    ]
    .into_iter()
    .filter_map(std::env::var_os)
    .map(PathBuf::from)
    .filter(|path| path.is_absolute())
    .collect();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        folders.push(local.join("Programs"));
    }
    folders.push(std::env::temp_dir());
    folders
}

/// This environment's names. A development or test build replaces `orchbee` and `Orchbee` by its
/// environment name, and a test build may move every key below a scratch key; any other value of
/// that variable refuses rather than falling back to the real keys.
pub(super) fn locations() -> io::Result<Locations> {
    let (name, shortcut) = if is_release() {
        ("orchbee".to_owned(), "Orchbee".to_owned())
    } else {
        (environment_stem().to_owned(), environment_stem().to_owned())
    };
    let base = match (environment_kind(), std::env::var_os(TEST_REGISTRY_KEY)) {
        (EnvironmentKind::Test, Some(value)) => Some(scratch_key(&value)?),
        _ => None,
    };
    let (environment, registrations) = match &base {
        Some(base) => (format!("{base}\\Environment"), format!("{base}\\Uninstall")),
        None => (
            super::super::path_step::USER_ENVIRONMENT.to_owned(),
            UNINSTALL_KEY.to_owned(),
        ),
    };
    Ok(Locations {
        environment,
        registrations,
        registration: format!("com.{name}.desktop_is1"),
        shortcut: std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| {
                path.join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs")
                    .join(format!("{shortcut}.lnk"))
            }),
    })
}

/// A scratch key below [`TEST_REGISTRY_PREFIX`] with one more plain name, or a refusal.
fn scratch_key(value: &OsString) -> io::Result<String> {
    let text = value.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let rest = text
        .get(..TEST_REGISTRY_PREFIX.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(TEST_REGISTRY_PREFIX))
        .map(|_| &text[TEST_REGISTRY_PREFIX.len()..])
        .ok_or(io::ErrorKind::InvalidInput)?;
    if rest.is_empty()
        || rest.len() > 64
        || !rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(text.to_owned())
}

impl Locations {
    /// The registry key whose `Path` value holds the entry, as people read it.
    pub(super) fn path_key(&self) -> String {
        format!("HKCU\\{}", self.environment)
    }

    /// The uninstall registration's key, as people read it.
    pub(super) fn registration_key(&self) -> String {
        format!("HKCU\\{}\\{}", self.registrations, self.registration)
    }

    /// The key below `HKEY_CURRENT_USER` whose `Path` value holds the entry.
    pub(super) fn environment(&self) -> &str {
        &self.environment
    }

    pub(super) fn shortcut(&self) -> Option<&Path> {
        self.shortcut.as_deref()
    }
}

/// Whether the user `Path` names the store's `bin` folder or another folder inside the store root.
pub(super) fn path_entry_present(locations: &Locations, root: &Path) -> io::Result<bool> {
    let key = match w::HKEY::CURRENT_USER.RegOpenKeyEx(
        Some(&locations.environment),
        co::REG_OPTION::default(),
        co::KEY::QUERY_VALUE,
    ) {
        Ok(key) => key,
        Err(error) if error == co::ERROR::FILE_NOT_FOUND => return Ok(false),
        Err(error) => return Err(registry_error(error)),
    };
    let current = match key.RegQueryValueEx(Some("Path")) {
        Ok(w::RegistryValue::Sz(text) | w::RegistryValue::ExpandSz(text)) => text,
        Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
        Err(error) if error == co::ERROR::FILE_NOT_FOUND => return Ok(false),
        Err(error) => return Err(registry_error(error)),
    };
    let root = root.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    Ok(matches!(
        path_value::removal(&current, root),
        Edit::Write { .. }
    ))
}

/// Whether the uninstall registration exists.
pub(super) fn registration_present(locations: &Locations) -> io::Result<bool> {
    match w::HKEY::CURRENT_USER.RegOpenKeyEx(
        Some(&format!(
            "{}\\{}",
            locations.registrations, locations.registration
        )),
        co::REG_OPTION::default(),
        co::KEY::QUERY_VALUE,
    ) {
        Ok(_) => Ok(true),
        Err(error) if error == co::ERROR::FILE_NOT_FOUND => Ok(false),
        Err(error) => Err(registry_error(error)),
    }
}

/// Removes the uninstall registration with its values; an absent one is already removed.
pub(super) fn remove_registration(locations: &Locations) -> io::Result<()> {
    let parent = match w::HKEY::CURRENT_USER.RegOpenKeyEx(
        Some(&locations.registrations),
        co::REG_OPTION::default(),
        // Deleting a key with its values needs the standard delete right, which only this set holds.
        co::KEY::ALL_ACCESS,
    ) {
        Ok(key) => key,
        Err(error) if error == co::ERROR::FILE_NOT_FOUND => return Ok(()),
        Err(error) => return Err(registry_error(error)),
    };
    match parent.RegDeleteTree(Some(&locations.registration)) {
        Ok(()) => Ok(()),
        Err(error) if error == co::ERROR::FILE_NOT_FOUND => Ok(()),
        Err(error) => Err(registry_error(error)),
    }
}

fn registry_error(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

/// One process whose program file is inside the store root.
pub(super) struct StoreProcess {
    pub(super) pid: u32,
    pub(super) image: PathBuf,
}

/// The processes whose program file is inside `root`, other than this one, those in `except` and
/// their descendants, such as the console host a Session's terminal runs from the store. A process
/// this user cannot query is not reported.
pub(super) fn store_processes(root: &Path, except: &[u32]) -> Vec<StoreProcess> {
    let own = std::process::id();
    let Ok(mut snapshot) = w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
    else {
        return Vec::new();
    };
    let parents: std::collections::HashMap<u32, u32> = snapshot
        .iter_processes()
        .filter_map(Result::ok)
        .map(|entry| (entry.th32ProcessID, entry.th32ParentProcessID))
        .collect();
    // A bounded walk up the parent links, so a cycle of reused identifiers ends.
    let excepted = |pid: u32| {
        let mut current = pid;
        for _ in 0..64 {
            if current == own || except.contains(&current) {
                return true;
            }
            match parents.get(&current) {
                Some(&parent) if parent != 0 && parent != current => current = parent,
                _ => return false,
            }
        }
        false
    };
    let pids: Vec<u32> = parents
        .keys()
        .copied()
        .filter(|pid| *pid != 0 && !excepted(*pid))
        .collect();
    pids.into_iter()
        .filter_map(|pid| {
            let image = image(pid)?;
            inside(&image, root).then_some(StoreProcess { pid, image })
        })
        .collect()
}

/// This process's parent, when it still runs and started before this process.
pub(super) fn parent() -> Option<StoreProcess> {
    let own = std::process::id();
    let mut snapshot =
        w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).ok()?;
    let pid = snapshot
        .iter_processes()
        .filter_map(Result::ok)
        .find(|entry| entry.th32ProcessID == own)
        .map(|entry| entry.th32ParentProcessID)?;
    let process =
        w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let (started, ..) = process.GetProcessTimes().ok()?;
    let (own_started, ..) = w::HPROCESS::GetCurrentProcess().GetProcessTimes().ok()?;
    if u64::from(started) > u64::from(own_started) {
        return None;
    }
    let image = process
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .ok()?;
    Some(StoreProcess {
        pid,
        image: PathBuf::from(image),
    })
}

fn image(pid: u32) -> Option<PathBuf> {
    let process =
        w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    process
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .ok()
        .map(PathBuf::from)
}

/// Whether `path` is strictly inside `root`, comparing names without letter case.
pub(super) fn inside(path: &Path, root: &Path) -> bool {
    let (path, root) = (fold(path), fold(root));
    path.strip_prefix(root.as_str())
        .is_some_and(|rest| rest.len() > 1 && rest.starts_with('\\'))
}

/// Whether two paths name the same file or folder, comparing names without letter case.
pub(super) fn same(left: &Path, right: &Path) -> bool {
    fold(left) == fold(right)
}

fn fold(path: &Path) -> String {
    path.to_string_lossy()
        .to_lowercase()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_owned()
}

/// Whether the npm carrier started this Client: its launcher runs the version's Client directly
/// under Node.js.
pub(super) fn npm_carrier(parent: Option<&StoreProcess>) -> bool {
    parent.is_some_and(|parent| {
        parent
            .image
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("node.exe"))
    })
}

/// Starts the detached removal of `files` and `folders`, the store's last entries, which this
/// process and its launcher hold until they exit; see the module documentation. `folders` are
/// deepest first and end with `root`.
pub(super) fn schedule_removal(
    files: &[PathBuf],
    folders: &[PathBuf],
    root: &Path,
) -> io::Result<()> {
    let quoted = |path: &Path| -> io::Result<String> {
        let text = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
        // The store root admits no quote or percent sign, so no name can end the quoting or expand.
        if text.contains(['"', '%', '\r', '\n', '\0']) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(format!("\"{text}\""))
    };
    let mut body = String::new();
    for file in files {
        body.push_str(&format!("del /f /q {} 2>nul & ", quoted(file)?));
    }
    for folder in folders {
        body.push_str(&format!("rd {} 2>nul & ", quoted(folder)?));
    }
    let root_text = root.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let script = format!(
        "for /l %i in (1,1,{REMOVAL_ATTEMPTS}) do @({body}(if not exist {} exit 0) & ping -n 2 127.0.0.1 >nul)",
        quoted(&PathBuf::from(format!(
            "{}\\",
            root_text.trim_end_matches('\\')
        )))?
    );
    let system = w::GetSystemDirectory().map_err(io::Error::other)?;
    let shell = format!("{system}\\cmd.exe");
    let command = format!("\"{shell}\" /d /q /v:off /s /c \"{script}\"");
    if command.encode_utf16().count() >= 32_767 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let start = |flags: co::CREATE| {
        w::CreateProcess(
            Some(&shell),
            Some(&command),
            None,
            None,
            false,
            co::CREATE::DETACHED_PROCESS | co::CREATE::NEW_PROCESS_GROUP | flags,
            &[],
            Some(&system),
            &mut w::STARTUPINFO::default(),
        )
    };
    // Leaving a Job keeps the removal running when a caller's Job ends with this process; a Job
    // that forbids leaving refuses the flag, and the launcher's Job lets children leave silently.
    start(co::CREATE::BREAKAWAY_FROM_JOB)
        .or_else(|_| start(co::CREATE::NoValue))
        .map(drop)
        .map_err(io::Error::other)
}
