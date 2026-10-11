//! One source handle and exclusive new-file publication; failure retains only the known created leaf.
use super::{PreparedWindowsHerdrImport, WindowsSettingsError, valid_windows_herdr_file_operand};
use crate::windows_permissions as permissions;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

/// A writer failure distinguishes a refused creation from a new file requiring inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsHerdrImportWriteFailure {
    /// No destination was created by this invocation.
    BeforeCreate {
        stage: &'static str,
        reason: &'static str,
    },
    /// The invocation created its target; inspect the explicitly selected path before recovery.
    AfterCreate {
        stage: &'static str,
        reason: &'static str,
    },
}
impl WindowsHerdrImportWriteFailure {
    /// Closed workflow stage without platform prose.
    pub const fn stage(self) -> &'static str {
        match self {
            Self::BeforeCreate { stage, .. } | Self::AfterCreate { stage, .. } => stage,
        }
    }
    /// Canonical cause without a configuration value or OS exception.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::BeforeCreate { reason, .. } | Self::AfterCreate { reason, .. } => reason,
        }
    }
    /// Whether the caller must inspect this invocation's new candidate residue.
    pub const fn created(self) -> bool {
        matches!(self, Self::AfterCreate { .. })
    }
}

/// Read one regular source snapshot with write/delete sharing disabled and a 64 KiB bound.
///
/// # Errors
/// Refuses nonordinary operands, reparse/nonregular/multilink leaves and invalid source bytes.
pub fn read_windows_herdr_source(path: &Path) -> Result<String, WindowsSettingsError> {
    let error = |reason| WindowsSettingsError::new("source", reason);
    if !path.is_absolute() || !path.to_str().is_some_and(valid_windows_herdr_file_operand) {
        return Err(error("invalid-path"));
    }
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x0020_0000)
        .open(path)
        .map_err(|_| error("unavailable"))?;
    let metadata = file.metadata().map_err(|_| error("io-error"))?;
    if !metadata.is_file()
        || metadata.file_attributes() & 0x400 != 0
        || winapi_util::file::information(&file)
            .map_err(|_| error("io-error"))?
            .number_of_links()
            != 1
    {
        return Err(error("not-file"));
    }
    if metadata.len() > super::super::LIMIT {
        return Err(error("too-large"));
    }
    let mut bytes = Vec::new();
    file.take(super::super::LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("io-error"))?;
    if bytes.len() > 64 * 1024 {
        return Err(error("too-large"));
    }
    String::from_utf8(bytes).map_err(|_| error("invalid-utf8"))
}

/// Publish a prepared candidate once to an exclusively created ordinary local file.
///
/// # Errors
/// Before-create failures preserve existing files; after-create failures retain the exact new
/// path for caller-owned recovery. No failure deletes, retries or reloads anything.
pub fn write_windows_herdr_import(
    destination: &Path,
    prepared: &PreparedWindowsHerdrImport,
) -> Result<(), WindowsHerdrImportWriteFailure> {
    let before = |stage, reason| WindowsHerdrImportWriteFailure::BeforeCreate { stage, reason };
    if !destination.is_absolute()
        || !destination
            .to_str()
            .is_some_and(valid_windows_herdr_file_operand)
    {
        return Err(before("destination", "invalid-path"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| before("destination", "invalid-path"))?;
    let pins =
        permissions::open_tree(parent).map_err(|_| before("destination", "unsafe-parent"))?;
    // GENERIC_READ | GENERIC_WRITE | WRITE_DAC; protect_new uses this same exclusive handle.
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .access_mode(0xC004_0000)
        .create_new(true)
        .share_mode(0)
        .custom_flags(0x0020_0000)
        .open(destination)
        .map_err(|error| {
            before(
                "create",
                if error.kind() == io::ErrorKind::AlreadyExists {
                    "already-exists"
                } else {
                    "create-failed"
                },
            )
        })?;
    let result = finish_new(
        &mut file,
        prepared.candidate_toml().as_bytes(),
        |file| {
            let metadata = file.metadata()?;
            if !metadata.is_file()
                || metadata.file_attributes() & 0x400 != 0
                || winapi_util::file::information(&*file)?.number_of_links() != 1
            {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            permissions::protect_new(file)
        },
        |file, bytes| file.write_all(bytes),
        |file| file.sync_all(),
    );
    // Pins outlive the exclusive leaf so no path is reopened or redirected during settlement.
    drop(file);
    drop(pins);
    result
}

fn finish_new(
    file: &mut File,
    bytes: &[u8],
    protect: impl FnOnce(&mut File) -> io::Result<()>,
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    synchronize: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), WindowsHerdrImportWriteFailure> {
    protect(file).map_err(|_| WindowsHerdrImportWriteFailure::AfterCreate {
        stage: "protect",
        reason: "protect-failed",
    })?;
    write(file, bytes).map_err(|_| WindowsHerdrImportWriteFailure::AfterCreate {
        stage: "write",
        reason: "write-failed",
    })?;
    synchronize(file).map_err(|_| WindowsHerdrImportWriteFailure::AfterCreate {
        stage: "synchronize",
        reason: "sync-failed",
    })
}

#[cfg(test)]
mod tests;
