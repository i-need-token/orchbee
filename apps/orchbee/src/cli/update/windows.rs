//! Windows update effects: declared inputs, no-follow source admission, owned process execution,
//! and the user `Path` value the PATH step edits.
use super::{Context, path_value::Edit};
use orchbee_local_runtime::CommandOutput;
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
};
use winsafe::{self as w, co};

/// `ORCHBEE_UPDATE_MANIFEST`; an absent or non-Unicode value leaves the source unconfigured.
pub(super) fn manifest_source() -> Option<String> {
    std::env::var("ORCHBEE_UPDATE_MANIFEST").ok()
}

/// `ORCHBEE_INSTALL_DIR`, or `%LOCALAPPDATA%\Programs\Orchbee` for release and
/// `%LOCALAPPDATA%\Programs\<stem>` for a development or test build, so only release names the
/// release store.
pub(super) fn install_root() -> io::Result<PathBuf> {
    let path = match std::env::var_os("ORCHBEE_INSTALL_DIR") {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(io::ErrorKind::NotFound)?)
            .join("Programs")
            .join(if orchbee_local_runtime::is_release() {
                "Orchbee"
            } else {
                orchbee_local_runtime::environment_stem()
            }),
    };
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|s| s.len() > 2048 || s.contains([';', '%', '\0', '\r', '\n', '"']))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(path)
}

pub(super) fn open_manifest(path: &Path) -> io::Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x0020_0000)
        .open(path)?;
    let metadata = file.metadata()?;
    Ok((metadata.is_file() && metadata.file_attributes() & 0x400 == 0).then_some(file))
}

pub(super) fn curl_command() -> io::Result<Command> {
    let system = winsafe::GetSystemDirectory().map_err(io::Error::other)?;
    let mut command = Command::new(PathBuf::from(&system).join("curl.exe"));
    command.env_clear().env(
        "SystemRoot",
        PathBuf::from(&system)
            .parent()
            .ok_or(io::ErrorKind::NotFound)?,
    );
    Ok(command)
}

pub(super) fn execute(command: Command, context: &mut Context) -> io::Result<CommandOutput> {
    Ok(orchbee_local_runtime::run_windows_command(
        command,
        Some(context.deadline),
        || context.interrupted(),
    ))
}

/// Like [`execute`], but captures up to `limit` bytes of standard output instead of 64 KiB.
pub(super) fn execute_capturing(
    command: Command,
    limit: usize,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    Ok(orchbee_local_runtime::run_windows_command_exchange(
        command,
        None,
        limit,
        Some(context.deadline),
        || context.interrupted(),
    ))
}

pub(super) fn verify_capsule(directory: &Path, expected: &str) -> io::Result<bool> {
    Ok(orchbee_agent_integration_writer::verify_distribution_capsule(directory, expected))
}

/// How many times the PATH step recomputes its edit when another writer changes the value between
/// its read and its write.
const PATH_ATTEMPTS: usize = 3;

/// The `Path` value of `HKEY_CURRENT_USER\<key>` with its registry type; an absent value reads as an
/// empty `REG_EXPAND_SZ`, the type Windows gives a new user `Path`.
#[derive(Eq, PartialEq)]
struct PathValue {
    expandable: bool,
    text: String,
}

/// Reads `Path` from an open key. Another type is refused, and so is text that could not be read
/// as UTF-16 without loss, because it could not be written back unchanged.
fn read_path(key: &w::HKEY) -> io::Result<PathValue> {
    let (expandable, text) = match key.RegQueryValueEx(Some("Path")) {
        Ok(w::RegistryValue::Sz(text)) => (false, text),
        Ok(w::RegistryValue::ExpandSz(text)) => (true, text),
        Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
        Err(error) if error == co::ERROR::FILE_NOT_FOUND => (true, String::new()),
        Err(error) => return Err(registry_error(error)),
    };
    if text.contains(char::REPLACEMENT_CHARACTER) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(PathValue { expandable, text })
}

fn registry_error(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

/// The edit the PATH step would make to the user `Path` for the store `root`, read without writing.
pub(super) fn planned_path_edit(root: &Path) -> Option<Edit> {
    let key = w::HKEY::CURRENT_USER
        .RegOpenKeyEx(
            Some(super::path_step::USER_ENVIRONMENT),
            co::REG_OPTION::default(),
            co::KEY::QUERY_VALUE,
        )
        .ok()?;
    let current = read_path(&key).ok()?;
    Some(super::path_value::edit(&current.text, root.to_str()?))
}

/// Applies `edit` to the `Path` value of `HKEY_CURRENT_USER\<key>` and writes the result with the
/// value's own registry type, then flushes the key and reads the value back. The key is a parameter
/// so that a rehearsal can name a scratch key. The value is read again just before the write and the
/// edit recomputed when another writer changed it. Nothing is broadcast.
pub(super) fn edit_user_path(key: &str, edit: &dyn Fn(&str) -> Edit) -> io::Result<()> {
    let (key, _) = w::HKEY::CURRENT_USER
        .RegCreateKeyEx(
            key,
            None,
            co::REG_OPTION::default(),
            co::KEY::QUERY_VALUE | co::KEY::SET_VALUE,
            None,
        )
        .map_err(registry_error)?;
    for _ in 0..PATH_ATTEMPTS {
        let before = read_path(&key)?;
        let value = match edit(&before.text) {
            Edit::Unchanged => return Ok(()),
            Edit::TooLong | Edit::UnnamedRoot => return Err(io::ErrorKind::InvalidInput.into()),
            Edit::Write { value, .. } => value,
        };
        if read_path(&key)? != before {
            continue;
        }
        // winsafe passes an empty string as a null pointer with a two-byte length, which
        // `RegSetValueExW` refuses with `ERROR_NOACCESS`. A lone NUL is written as exactly that
        // two-byte terminator, the bytes of an empty value, so removing the only entry leaves the
        // value empty with its type.
        let data = if value.is_empty() {
            "\0".to_owned()
        } else {
            value.clone()
        };
        let written = if before.expandable {
            w::RegistryValue::ExpandSz(data)
        } else {
            w::RegistryValue::Sz(data)
        };
        key.RegSetValueEx(Some("Path"), written)
            .map_err(registry_error)?;
        key.RegFlushKey().map_err(registry_error)?;
        let after = read_path(&key)?;
        return if after.text == value && after.expandable == before.expandable {
            Ok(())
        } else {
            Err(io::ErrorKind::Other.into())
        };
    }
    Err(io::ErrorKind::Interrupted.into())
}
