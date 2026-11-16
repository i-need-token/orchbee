//! The completion helper `--complete-install-v1 [--no-path] [--allow-downgrade]`, which a published
//! copy runs to complete its own install. Under `install.lock` it refuses a version lower than the
//! active one unless allowed, performs the PATH step unless `--no-path`, and then moves the
//! pointer, so a failure leaves the pointer where it was. Where automatic updates run, it first
//! refuses a version held after a failed move, with the lower-version refusal's exit status, and
//! the move also writes the pointer's fallback record: whether the new version may fall back to the
//! one it replaces, settled here in the new version's own code. It then removes the versions the
//! store no longer keeps, under the same lock; that cleanup never changes the result. Its grammar
//! never changes; help, completion and the Agent guide leave it out.
use super::{
    Error, Output,
    install::Completion,
    manifest::{Release, own_platform},
    path_step, retention,
    signature::{self, DOCUMENT_LIMIT, MANIFEST_LIMIT, Refusal, TrustedManifest},
};
use crate::{
    cli_text::CliText,
    localization::Locale,
    release_store::{ReleaseStore, open_release_file, version_directory_parts},
};
use orchbee_local_runtime::{
    EnvironmentKind, VersionHeld, automatic_updates_live, compare_product_versions, complete_move,
    environment_kind, held_versions,
};
use orchbee_release_records::{Entry, Pointer};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, io::Read, path::Path};

/// Whether this build performs the PATH step. A build without it makes `install` behave as if
/// `--no-path` were given and say so in its result.
pub(super) const PATH_STEP_BUILT: bool = true;

/// Makes the published version `entry` names the active one under `root`, holding the writer
/// lock throughout. An unreadable pointer names no active version, so this repairs it. Where
/// automatic updates run, a held version is refused first and the move writes the fallback record;
/// once the pointer has moved from a readable one, the versions it names neither active nor
/// previous leave the store when no process runs from them.
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
    // A build whose switch is off only ever activates itself, and only a build with it on records,
    // so only such a build can be held and only it refuses one.
    let live = automatic_updates_live();
    if live
        && current
            .as_ref()
            .is_some_and(|current| held_versions(current).contains(&entry.product))
    {
        return Err(Error::VersionHeld);
    }
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
        path_step::run(root, &entry.product)?;
    } else {
        path_step::refresh_present(root, &entry.product);
    }
    // Without a readable pointer before the move, no version is known to have been the previous
    // one, so nothing is removed.
    let readable = current.is_some();
    let pointer = if live {
        complete_move(current, entry.clone(), |replaced| {
            may_fall_back(&store, entry, replaced)
        })
        .map_err(|VersionHeld| Error::VersionHeld)?
    } else {
        Pointer::activate(current, entry.clone())
    };
    store
        .replace_pointer(&pointer.encode())
        .map_err(|_| Error::Storage)?;
    if live && readable {
        retention::remove_unused(&store, &pointer);
    }
    Ok(())
}

/// Whether the move from `replaced` to `target` may fall back: the replaced version's
/// `release.json` still hashes to its entry's manifest, and for every component `securityFloor` in
/// the target's own verified manifest, the replaced manifest's component with the same `id` has a
/// version at or above it. A component missing from either manifest imposes nothing. The target's
/// manifest is the one its entry binds, which the completion verified before it took the lock.
fn may_fall_back(store: &ReleaseStore, target: &Entry, replaced: &Entry) -> bool {
    let (Some(target), Some(replaced)) = (installed(store, target), installed(store, replaced))
    else {
        return false;
    };
    target.components.iter().all(|component| {
        component.security_floor.as_ref().is_none_or(|floor| {
            replaced
                .components
                .iter()
                .find(|other| other.id == component.id)
                .is_none_or(|other| {
                    compare_product_versions(&other.version, floor) != Ordering::Less
                })
        })
    })
}

/// The release a store version folder holds, when its `release.json` still hashes to the manifest
/// `entry` names.
fn installed(store: &ReleaseStore, entry: &Entry) -> Option<Release> {
    let directory = store.version_path(&entry.product).ok()?;
    let mut bytes = Vec::new();
    open_release_file(&directory, "release.json")
        .ok()?
        .take(MANIFEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if format!("{:x}", Sha256::digest(&bytes)) != entry.manifest {
        return None;
    }
    Release::parse(TrustedManifest::installed(&bytes), &own_platform())
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
/// empty; the exit status is the result, and `3` is the lower-version or held-version refusal, so a
/// caller before slice 1 reads a held version as a lower one. Platform admission ran before
/// dispatch.
pub(crate) fn run(no_path: bool, allow_downgrade: bool, locale: Locale) -> Output {
    let result = (|| {
        if environment_kind() == EnvironmentKind::Development {
            return Err(Error::DevelopmentBuild);
        }
        let executable =
            orchbee_local_runtime::current_executable().map_err(|_| Error::Internal)?;
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
