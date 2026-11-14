//! The completion helper `--complete-install-v1 [--no-path] [--allow-downgrade]`, which a published
//! copy runs to complete its own install. Under `install.lock` it refuses a version lower than the
//! active one unless allowed, performs the PATH step unless `--no-path`, and then moves the
//! pointer, so a failure leaves the pointer where it was. Its grammar never changes; help,
//! completion and the Agent guide leave it out.
use super::{
    Error, Output,
    install::Completion,
    manifest::{Release, own_platform},
    path_step, platform,
    signature::{self, DOCUMENT_LIMIT, MANIFEST_LIMIT, Refusal},
};
use crate::{
    cli_text::CliText,
    localization::Locale,
    release_store::{ReleaseStore, open_release_file, version_directory_parts},
};
use orchbee_local_runtime::{EnvironmentKind, compare_product_versions, environment_kind};
use orchbee_release_records::{Entry, Pointer};
use std::{cmp::Ordering, io::Read, path::Path};

/// Whether this build performs the PATH step. A build without it makes `install` behave as if
/// `--no-path` were given and say so in its result.
pub(super) const PATH_STEP_BUILT: bool = true;

/// Makes the published version `entry` names the active one under `root`, holding the writer
/// lock throughout. An unreadable pointer names no active version, so this repairs it.
pub(super) fn activate(root: &Path, entry: &Entry, completion: Completion) -> Result<(), Error> {
    let mut store = ReleaseStore::acquire(root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            Error::Busy
        } else {
            Error::Storage
        }
    })?;
    let current = store
        .pointer()
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes));
    if let Some(current) = &current
        && !completion.allow_downgrade
        && compare_product_versions(&current.active.product, &entry.product) == Ordering::Greater
    {
        return Err(Error::DowngradeRefused);
    }
    let directory = store
        .version_path(&entry.product)
        .map_err(|_| Error::Storage)?;
    if !directory.is_dir() {
        return Err(Error::NotReleaseDirectory);
    }
    if completion.path {
        let key = platform::user_environment_key().map_err(|_| Error::PathVariable)?;
        path_step::run(root, &entry.product, &key)?;
    }
    store
        .replace_pointer(&Pointer::activate(current, entry.clone()).encode())
        .map_err(|_| Error::Storage)
}

/// The signed release in `directory`, verified with this build's keys.
fn own_release(directory: &Path) -> Result<Release, Error> {
    let read = |name: &str, limit: usize| -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        open_release_file(directory, name)
            .map_err(|_| Error::NotReleaseDirectory)?
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Storage)?;
        Ok(bytes)
    };
    let manifest = read("release.json", MANIFEST_LIMIT)?;
    let document = read("release.json.sig", DOCUMENT_LIMIT)?;
    let trusted =
        signature::verify(&document, &manifest, signature::TRUSTED_KEYS).map_err(|refusal| {
            match refusal {
                Refusal::NoTrustedKeys => Error::SigningKeysUnavailable,
                Refusal::Oversized => Error::ManifestInvalid,
                Refusal::Untrusted => Error::SignatureUntrusted,
                Refusal::Malformed | Refusal::Invalid => Error::SignatureInvalid,
            }
        })?;
    Release::parse(trusted, &own_platform()).ok_or(Error::ManifestInvalid)
}

/// Completes the install of the version directory this executable sits in. Standard output stays
/// empty; the exit status is the result, and `3` is the lower-version refusal. Platform admission
/// ran before dispatch.
pub(crate) fn run(no_path: bool, allow_downgrade: bool, locale: Locale) -> Output {
    let result = (|| {
        if environment_kind() == EnvironmentKind::Development {
            return Err(Error::DevelopmentBuild);
        }
        let executable = std::env::current_exe().map_err(|_| Error::Internal)?;
        let (root, name) = executable
            .parent()
            .and_then(version_directory_parts)
            .ok_or(Error::NotReleaseDirectory)?;
        let release = own_release(executable.parent().ok_or(Error::Internal)?)?;
        if release.version != name || release.version != env!("CARGO_PKG_VERSION") {
            return Err(Error::NotReleaseDirectory);
        }
        let entry = Entry {
            product: release.version,
            manifest: release.identity,
        };
        activate(
            root,
            &entry,
            Completion {
                path: !no_path,
                allow_downgrade,
            },
        )
    })();
    Output {
        status: result.err().map_or(0, Error::status),
        stdout: Vec::new(),
        stderr: result.err().map_or_else(String::new, |error| {
            format!(
                "{}\n",
                CliText::new(locale).cli_update_message(error.code())
            )
        }),
        finalization: None,
    }
}
