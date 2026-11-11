//! Windows update effects: declared inputs, no-follow source admission and owned process execution.
use super::Context;
use orchbee_local_runtime::CommandOutput;
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
};

/// `ORCHBEE_UPDATE_MANIFEST`; an absent or non-Unicode value leaves the source unconfigured.
pub(super) fn manifest_source() -> Option<String> {
    std::env::var("ORCHBEE_UPDATE_MANIFEST").ok()
}

pub(super) fn install_root() -> io::Result<PathBuf> {
    let path = match std::env::var_os("ORCHBEE_INSTALL_DIR") {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(io::ErrorKind::NotFound)?)
            .join("Programs/Orchbee"),
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

pub(super) fn powershell_command() -> io::Result<Command> {
    let system = winsafe::GetSystemDirectory().map_err(io::Error::other)?;
    Ok(Command::new(
        PathBuf::from(system).join("WindowsPowerShell/v1.0/powershell.exe"),
    ))
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
