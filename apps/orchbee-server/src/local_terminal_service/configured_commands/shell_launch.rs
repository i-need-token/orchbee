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

/// No configured-command shell adapter exists on this build.
#[cfg(not(windows))]
pub(in crate::local_terminal_service) fn launch(
    _command: &str,
    _environment: &[(String, String)],
) -> Result<LaunchSelection, Failure> {
    Err(Failure::Unavailable)
}
