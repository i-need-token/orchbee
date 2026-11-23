//! The command processor a configured `shell` command runs in. Windows runs `ComSpec` without
//! its AutoRun commands; macOS and Linux run the person's login shell, the shell a new terminal
//! starts, so the command sees the `PATH` and variables its login files set. The command text
//! reaches the processor as one literal argument.
use std::process::Command;

#[cfg(windows)]
pub(super) fn command(text: &str) -> Option<Command> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(shell()?);
    command.args(["/d", "/c"]).raw_arg(text);
    Some(command)
}

/// `ComSpec` when it is absolute, else the system's `cmd.exe`.
#[cfg(windows)]
fn shell() -> Option<std::ffi::OsString> {
    std::env::var_os("ComSpec")
        .filter(|path| std::path::Path::new(path).is_absolute())
        .or_else(|| {
            orchbee_local_runtime::windows_system_program("cmd.exe")
                .ok()
                .map(std::path::PathBuf::into_os_string)
        })
}

/// The processor a Terminal Host starts with `/d /c` and the command text as one raw argument.
#[cfg(windows)]
pub(super) fn processor() -> Option<String> {
    shell()?.into_string().ok()
}

/// Terminal Hosts run background programs only on Windows.
#[cfg(unix)]
pub(super) fn processor() -> Option<String> {
    None
}

/// `$SHELL`, else the account's shell, else `/bin/sh`, as for a new terminal, run as a login
/// shell, as VS Code runs a shell task. The caller's thread is the command's own owner
/// thread: a child keeps the signal mask of the thread that starts it, and the standard library
/// does not clear it, so this thread unblocks every signal first and the command and everything
/// it starts can be interrupted and stopped.
#[cfg(unix)]
pub(super) fn command(text: &str) -> Option<Command> {
    nix::sys::signal::SigSet::empty().thread_set_mask().ok()?;
    let shell = orchbee_local_runtime::resolve_default_interactive_launch().ok()?;
    let mut command = Command::new(&shell.executable);
    command.args(orchbee_local_runtime::login_shell_command(
        &shell.executable,
        text,
    ));
    Some(command)
}
