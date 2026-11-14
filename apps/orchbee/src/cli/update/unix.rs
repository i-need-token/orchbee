//! No update effects are implemented on Unix; every port refuses without observation or mutation.
use super::{Context, path_value::Edit};
use orchbee_local_runtime::CommandOutput;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn feed_source() -> Option<String> {
    None
}

pub(super) fn install_root() -> io::Result<PathBuf> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn open_manifest(_path: &Path) -> io::Result<Option<File>> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn open_source_file(_path: &Path) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn curl_command() -> io::Result<Command> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn tar_command() -> io::Result<Command> {
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

pub(super) fn planned_path_edit(_root: &Path) -> Option<Edit> {
    None
}

pub(super) fn user_environment_key() -> io::Result<String> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn edit_user_path(_key: &str, _edit: &dyn Fn(&str) -> Edit) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
