//! Project the shared Windows literal path adapter into the Workbench wire type.
pub(crate) use orchbee_local_site_runtime::WindowsUserPathError as PathFailure;
#[cfg(test)]
use orchbee_workbench_protocol::MAXIMUM_DIRECTORY_PATH_BYTES;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use std::{ffi::OsString, path::PathBuf};

pub(crate) fn normalize(
    value: &str,
    environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<WorkbenchDirectoryPath, PathFailure> {
    let operand = orchbee_local_site_runtime::WindowsUserPathOperand::parse(value)?;
    resolve(&operand, environment, cwd)
}

pub(crate) fn resolve(
    operand: &orchbee_local_site_runtime::WindowsUserPathOperand,
    environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<WorkbenchDirectoryPath, PathFailure> {
    let path = operand.resolve(environment, cwd)?;
    WorkbenchDirectoryPath::parse(&path).map_err(|_| PathFailure::Input)
}

#[cfg(test)]
mod tests;
