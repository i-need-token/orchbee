//! No update effects are implemented on Unix; every port refuses without observation or mutation.
use super::Context;
use orchbee_local_runtime::CommandOutput;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn manifest_source() -> Option<String> {
    None
}

pub(super) fn install_root() -> io::Result<PathBuf> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn open_manifest(_path: &Path) -> io::Result<Option<File>> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn curl_command() -> io::Result<Command> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn powershell_command() -> io::Result<Command> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn execute(_command: Command, _context: &mut Context) -> io::Result<CommandOutput> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn execute_capturing(
    _command: Command,
    _limit: usize,
    _context: &mut Context,
) -> io::Result<CommandOutput> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn verify_capsule(_directory: &Path, _expected: &str) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}
