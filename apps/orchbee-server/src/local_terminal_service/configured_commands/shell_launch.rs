//! Wraps one configured shell command in the platform launch the Worker starts, and names the
//! program its Panel presents. Builds without a shell adapter refuse the launch before any Worker
//! effect.
use orchbee_workbench_protocol::{
    ConfiguredCommandFailure as Failure, LaunchSelection, PresentationLaunchProgram,
};

/// The Windows command processor: `ComSpec` when it names an absolute path, else the system
/// `cmd.exe`.
#[cfg(windows)]
fn command_processor() -> Result<std::path::PathBuf, Failure> {
    match std::env::var_os("ComSpec")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        Some(shell) => Ok(shell),
        None => orchbee_local_runtime::windows_system_program("cmd.exe")
            .map_err(|_| Failure::Unavailable),
    }
}

/// Windows runs the command through `ComSpec` inside an encoded Windows PowerShell launch.
#[cfg(windows)]
pub(in crate::local_terminal_service) fn launch(
    command: &str,
    environment: &[(String, String)],
) -> Result<LaunchSelection, Failure> {
    use orchbee_local_runtime::windows_system_program;
    let shell = command_processor()?;
    let program = windows_system_program("WindowsPowerShell/v1.0/powershell.exe")
        .map_err(|_| Failure::Unavailable)?;
    let encoded = orchbee_local_runtime::windows_raw_command_encoded(
        shell.to_str().ok_or(Failure::Unavailable)?,
        command,
        environment,
    )
    .ok_or(Failure::InvalidIntent)?;
    LaunchSelection::new(
        program.to_str().ok_or(Failure::Unavailable)?.into(),
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-EncodedCommand".into(),
            encoded,
        ],
    )
    .map_err(|_| Failure::InvalidIntent)
}

/// The program a Windows command's Panel presents, by the rule a macOS login-shell line follows
/// (`launch_capture::explicit_program`): its first word when that word is a plain file name or
/// path, else the command processor that runs the line. `cmd /c` sets no console title, so without
/// this the Panel would present the PowerShell adapter.
#[cfg(windows)]
pub(in crate::local_terminal_service) fn presented_program(
    command: &str,
) -> Option<PresentationLaunchProgram> {
    command_program(command)
        .and_then(PresentationLaunchProgram::from_executable)
        .or_else(|| PresentationLaunchProgram::from_executable(command_processor().ok()?.to_str()?))
}

/// A cmd line's first word when it names a program: a plain file name or path rather than a quoted
/// or expanded word, an `@` that hides the echo, a drive change or a label, or an internal command
/// that starts no program of its name. cmd ends an internal command's name at `.`, `\`, `/`, `:`
/// or `+`, so `cd..` is `cd`.
#[cfg(windows)]
fn command_program(command: &str) -> Option<&str> {
    const NOT_PROGRAMS: [&str; 18] = [
        "call", "cd", "chdir", "endlocal", "exit", "for", "goto", "if", "path", "popd", "prompt",
        "pushd", "rem", "set", "setlocal", "shift", "start", "title",
    ];
    let word = command.split_whitespace().next()?;
    let plain = word.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'~' | b'\\' | b'/' | b':')
    });
    let internal = NOT_PROGRAMS.iter().any(|name| {
        word.get(..name.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(name))
            && word[name.len()..]
                .chars()
                .next()
                .is_none_or(|next| matches!(next, '.' | '\\' | '/' | ':' | '+'))
    });
    // A leading `:` starts a label, a trailing `:` changes the drive, and a trailing separator
    // names a folder.
    let label_or_drive = word.starts_with(':') || word.ends_with([':', '\\', '/']);
    (plain && !internal && !label_or_drive).then_some(word)
}

/// A POSIX launch presents its program through `launch_capture::explicit_program`, which reads the
/// `/usr/bin/env` launch itself.
#[cfg(not(windows))]
pub(in crate::local_terminal_service) const fn presented_program(
    _command: &str,
) -> Option<PresentationLaunchProgram> {
    None
}

/// macOS and Linux run the command in the person's login shell, the shell a new terminal starts
/// (`$SHELL`, else the account's shell, else `/bin/sh`). `/usr/bin/env` sets the
/// command's environment first, so a value always arrives as written and no shell ever reads it.
#[cfg(unix)]
pub(in crate::local_terminal_service) fn launch(
    command: &str,
    environment: &[(String, String)],
) -> Result<LaunchSelection, Failure> {
    if command.trim().is_empty() || command.contains('\0') {
        return Err(Failure::InvalidIntent);
    }
    let shell = orchbee_local_runtime::resolve_default_interactive_launch()
        .map_err(|_| Failure::Unavailable)?
        .executable;
    // `env` takes its first operand without `=` as the program, so a shell path holding one would
    // be read as an assignment.
    if shell.contains('=') {
        return Err(Failure::Unavailable);
    }
    let mut arguments = Vec::with_capacity(environment.len() + 4);
    for (name, value) in environment {
        if !name.starts_with("ORCHBEE_")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
            || value.contains('\0')
        {
            return Err(Failure::InvalidIntent);
        }
        arguments.push(format!("{name}={value}"));
    }
    let login = orchbee_local_runtime::login_shell_command(&shell, command);
    arguments.push(shell);
    arguments.extend(login);
    LaunchSelection::new("/usr/bin/env".into(), arguments).map_err(|_| Failure::InvalidIntent)
}

/// No configured-command shell adapter exists on this build.
#[cfg(not(any(windows, unix)))]
pub(in crate::local_terminal_service) fn launch(
    _command: &str,
    _environment: &[(String, String)],
) -> Result<LaunchSelection, Failure> {
    Err(Failure::Unavailable)
}
