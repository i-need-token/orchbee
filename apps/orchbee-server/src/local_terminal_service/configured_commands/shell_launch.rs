//! Wraps one configured shell command in the platform launch the Worker starts. Builds without a
//! shell adapter refuse the launch before any Worker effect.
use orchbee_workbench_protocol::{ConfiguredCommandFailure as Failure, LaunchSelection};

/// Windows runs the command through `ComSpec` inside an encoded Windows PowerShell launch.
#[cfg(windows)]
pub(in crate::local_terminal_service) fn launch(
    command: &str,
    environment: &[(String, String)],
) -> Result<LaunchSelection, Failure> {
    use orchbee_local_runtime::windows_system_program;
    let shell = match std::env::var_os("ComSpec")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        Some(shell) => shell,
        None => windows_system_program("cmd.exe").map_err(|_| Failure::Unavailable)?,
    };
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
