//! Windows update effects: declared inputs, no-follow source admission, owned process execution,
//! and the user `Path` value the PATH step edits.
use super::{Context, path_value::Edit};
use orchbee_local_runtime::{CommandOutput, EnvironmentKind, environment_kind};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};
use winsafe::{self as w, co};

/// The update feed a release build reads when `ORCHBEE_UPDATE_FEED` is unset, before `{channel}`
/// expansion; `{platform}` is this updater's own platform. Its signature document is the same
/// address with `.sig`. Frozen from the first version that carries it: what the address serves
/// changes by a website deploy, and it may redirect up to five times to another HTTPS host.
const RELEASE_FEED: &str = "https://orchbee.com/update/{channel}/{platform}/feed.json";
/// The public variable that names another feed: an absolute local path or an HTTPS address without
/// a query. Frozen with the address.
const FEED_VARIABLE: &str = "ORCHBEE_UPDATE_FEED";

/// The proxy variables curl reads for an HTTPS transfer, the only part of this process's
/// environment it receives. Windows looks environment names up without regard to case, so each
/// name also finds its lower-case spelling, which curl prefers on other systems (curl(1),
/// "ENVIRONMENT").
const PROXY_VARIABLES: [&str; 3] = ["HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"];

/// [`FEED_VARIABLE`]; when it is unset or empty, [`RELEASE_FEED`] for a release build and nothing
/// for a development or test build, so only a release build ever reads the public feed. A
/// non-Unicode value leaves the source unconfigured.
pub(super) fn feed_source() -> Option<String> {
    match std::env::var_os(FEED_VARIABLE) {
        Some(value) if !value.is_empty() => value.into_string().ok(),
        _ => orchbee_local_runtime::is_release()
            .then(|| RELEASE_FEED.replace("{platform}", &super::manifest::own_platform())),
    }
}

/// `ORCHBEE_INSTALL_DIR`, or `%LOCALAPPDATA%\Programs\Orchbee` for release and
/// `%LOCALAPPDATA%\Programs\<stem>` for a development or test build, so only release names the
/// release store. Session start reads the same root through the same owner.
pub(super) fn install_root() -> io::Result<PathBuf> {
    orchbee_local_runtime::release_store_root(|name| std::env::var_os(name))
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

/// Opens one file of a local release source to copy it: a regular file that is not a link. A
/// source needs no owner or access rule, unlike the store: nothing read from it is trusted until
/// its copy in the private stage matches the signed manifest, so a source an elevated npm unpacked
/// on another drive installs like one in the person's own folder.
pub(super) fn open_source_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(0x0020_0000)
        .open(path)?;
    let metadata = file.metadata()?;
    if metadata.is_file() && metadata.file_attributes() & 0x400 == 0 {
        Ok(file)
    } else {
        Err(io::ErrorKind::InvalidInput.into())
    }
}

pub(super) fn curl_command() -> io::Result<Command> {
    curl_command_from(&|name| std::env::var_os(name))
}

/// The system curl with a cleared environment holding only `SystemRoot` and the [`PROXY_VARIABLES`]
/// that `environment` sets, so no curlrc location, credential or other proxy setting reaches it.
pub(super) fn curl_command_from(
    environment: &dyn Fn(&str) -> Option<OsString>,
) -> io::Result<Command> {
    let system = winsafe::GetSystemDirectory().map_err(io::Error::other)?;
    let mut command = Command::new(PathBuf::from(&system).join("curl.exe"));
    command.env_clear().env(
        "SystemRoot",
        PathBuf::from(&system)
            .parent()
            .ok_or(io::ErrorKind::NotFound)?,
    );
    for name in PROXY_VARIABLES {
        if let Some(value) = environment(name) {
            command.env(name, value);
        }
    }
    Ok(command)
}

/// The system `tar.exe` with a cleared environment holding only `SystemRoot`. Its arguments and
/// working directory are the caller's.
pub(super) fn tar_command() -> io::Result<Command> {
    let system = winsafe::GetSystemDirectory().map_err(io::Error::other)?;
    let mut command = Command::new(PathBuf::from(&system).join("tar.exe"));
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

/// Like [`execute`] for a download or unpack child (`curl`, `tar`), contained: when this Client
/// ends, however abruptly, the child ends with it and never keeps writing into `.download`, so
/// the next update can remove what it left.
pub(super) fn execute_contained(
    command: Command,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    execute_capturing(command, OUTPUT_LIMIT, context)
}

/// Like [`execute_contained`], but captures up to `limit` bytes of standard output.
pub(super) fn execute_capturing(
    command: Command,
    limit: usize,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    Ok(orchbee_local_runtime::run_contained_command_exchange(
        command,
        None,
        limit,
        Some(context.deadline),
        || context.interrupted(),
    ))
}

/// Like [`execute_contained`] for the archive transfer, which ends at its own `deadline` rather
/// than the command's; Ctrl+C still ends it at once.
pub(super) fn execute_transfer(
    command: Command,
    deadline: Instant,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    Ok(orchbee_local_runtime::run_contained_command_exchange(
        command,
        None,
        OUTPUT_LIMIT,
        Some(deadline),
        || context.interrupted(),
    ))
}

/// The output [`execute_contained`] keeps, as [`execute`] does.
const OUTPUT_LIMIT: usize = 64 * 1024;

pub(super) fn verify_capsule(directory: &Path, expected: &str) -> io::Result<bool> {
    Ok(orchbee_agent_integration_writer::verify_distribution_capsule(directory, expected))
}

/// How many times the PATH step recomputes its edit when another writer changes the value between
/// its read and its write.
const PATH_ATTEMPTS: usize = 3;

/// The key under `HKEY_CURRENT_USER` that holds the user `Path`.
pub(super) const USER_ENVIRONMENT: &str = "Environment";

/// The variable a test build reads for a scratch registry key that stands in for
/// `HKEY_CURRENT_USER`, so that a rehearsal never touches the real `Path` or uninstall entries.
const TEST_REGISTRY_KEY: &str = "ORCHBEE_TEST_REGISTRY_KEY";
/// The only registry folder a test build's scratch key may name.
const TEST_REGISTRY_PREFIX: &str = "Software\\ob-test-scratch\\";

/// A test build's scratch key from [`TEST_REGISTRY_KEY`], whose `Environment` and `Uninstall`
/// subkeys stand in for the real ones; none when the variable is unset or the build is not a test
/// build. Any other value refuses rather than falling back to the real keys.
pub(super) fn scratch_registry_key() -> io::Result<Option<String>> {
    match (environment_kind(), std::env::var_os(TEST_REGISTRY_KEY)) {
        (EnvironmentKind::Test, Some(value)) => scratch_key(&value).map(Some),
        _ => Ok(None),
    }
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

/// The key under `HKEY_CURRENT_USER` whose `Path` value the PATH step edits: the user environment,
/// or a test build's scratch `Environment`.
pub(super) fn user_environment_key() -> io::Result<String> {
    Ok(match scratch_registry_key()? {
        Some(base) => format!("{base}\\Environment"),
        None => USER_ENVIRONMENT.to_owned(),
    })
}

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
            Some(&user_environment_key().ok()?),
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
