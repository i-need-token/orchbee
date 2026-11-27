//! The command each Git run starts from. Its environment is this process's own, which the Worker
//! gave it from the terminal launch environment, without the variables that would point Git at
//! another repository, with English messages, no pager and no terminal prompt, and for a command
//! that may ask for credentials or a message, the relay's variables.

use orchbee_git_process::{GIT_PROGRAM, read_program};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
    sync::OnceLock,
};

/// Variables that would make Git act on another repository than the one the command names.
const REDIRECTING: [&str; 8] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

/// The relay variables a helper started by Git reads.
pub(super) const RELAY_VARIABLE: &str = "ORCHBEE_FILES_RELAY";
pub(super) const RELAY_TOKEN_VARIABLE: &str = "ORCHBEE_FILES_RELAY_TOKEN";
/// The argument Git's editor call passes before the message file.
pub(super) const EDITOR_OPTION: &str = "--git-editor";

/// What a command may do, which picks its program and its locks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Access {
    /// A read that starts no helper and takes no optional lock, so it never blocks the user's own
    /// Git. On Windows it starts the installation's Git without the launcher.
    Read,
    /// A command that changes the repository or may start hooks, credential helpers or ssh, which
    /// on Windows need the variables the Git for Windows launcher sets.
    Write,
}

/// The relay a command's helpers reach: this executable, its endpoint and the operation's token.
#[derive(Clone, Debug)]
pub(super) struct RelayVariables {
    pub(super) executable: OsString,
    pub(super) endpoint: OsString,
    pub(super) token: String,
}

/// A Git command in `directory`.
pub(super) fn git(directory: &Path, access: Access, relay: Option<&RelayVariables>) -> Command {
    let program = match access {
        Access::Read => read_git().clone(),
        Access::Write => OsString::from(GIT_PROGRAM),
    };
    let mut command = Command::new(program);
    command.current_dir(directory);
    for name in REDIRECTING {
        command.env_remove(name);
    }
    command
        .env("LANGUAGE", "en")
        .env("LC_ALL", "en_US.UTF-8")
        .env("LANG", "en_US.UTF-8")
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0");
    if access == Access::Read {
        command.env("GIT_OPTIONAL_LOCKS", "0");
    }
    if let Some(relay) = relay {
        command
            .env("GIT_ASKPASS", &relay.executable)
            .env("SSH_ASKPASS", &relay.executable)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("GIT_EDITOR", editor(&relay.executable))
            .env(RELAY_VARIABLE, &relay.endpoint)
            .env(RELAY_TOKEN_VARIABLE, &relay.token);
    }
    command
}

/// The Git a read starts, resolved once per process.
fn read_git() -> &'static OsString {
    static PROGRAM: OnceLock<OsString> = OnceLock::new();
    PROGRAM.get_or_init(read_program)
}

/// Git runs its editor through a POSIX shell, so the executable is quoted as one word.
fn editor(executable: &OsStr) -> OsString {
    let mut quoted = OsString::from("'");
    let text = executable.to_string_lossy();
    quoted.push(text.replace('\'', r"'\''"));
    quoted.push("' ");
    quoted.push(EDITOR_OPTION);
    quoted
}

/// The version `git --version` printed, as `(major, minor, patch)`.
pub(super) fn version(stdout: &[u8]) -> Option<(u32, u32, u32)> {
    let text = std::str::from_utf8(stdout).ok()?;
    let version = text.trim().strip_prefix("git version ")?;
    let mut parts = version
        .split(['.', ' '])
        .map(|part| part.parse::<u32>().ok());
    let major = parts.next()??;
    let minor = parts.next()??;
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

/// Whether `name` is a full object name: 40 hexadecimal digits, or 64 in a SHA-256 repository.
pub(super) fn is_object_name(name: &str) -> bool {
    matches!(name.len(), 40 | 64) && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}
