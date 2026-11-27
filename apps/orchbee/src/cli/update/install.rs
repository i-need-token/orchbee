//! Complete cohort verification and immutable publication into `versions/<version>`, then the
//! published copy's own completion, which moves the pointer. The commands this owner runs come
//! from its caller, so tests publish and complete releases in process without starting a Client.
use super::{
    Context, Error, Report,
    acquire::Source,
    manifest::{Artifact, Release, own_platform},
    path_limit, platform,
    signature::{DOCUMENT_LIMIT, MANIFEST_LIMIT},
};
use crate::release_store::{ReleaseStore, open_release_file, read_pointer};
use orchbee_local_runtime::{
    Cohort, CohortArtifact, CommandFailure, automatic_updates_live, compare_product_versions,
    held_versions,
};
use orchbee_release_records::{CrossBuildName, Entry, Pointer};
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

/// The helper a published copy runs to complete its own install, in the spelling of that copy's
/// version. Its grammar never changes; a change adds a new helper name.
pub(super) const COMPLETION_HELPER: CrossBuildName = CrossBuildName {
    current: "--complete-install",
    earlier: "--complete-install-v1",
};

/// The cohort's programs, which a stage makes executable where the system asks that of a program,
/// as it does the `orchbee-files` a test release carries.
const PROGRAMS: [CohortArtifact; 4] = [
    CohortArtifact::Client,
    CohortArtifact::Server,
    CohortArtifact::Worker,
    CohortArtifact::TerminalHost,
];

/// Whether every program of `versions\<version>` under `root` fits the Windows program path limit.
/// A program too long to start would activate and then fail, so the update checks this first.
pub(super) fn programs_fit(root: &Path, version: &str) -> bool {
    let directory = root.join("versions").join(version);
    PROGRAMS
        .iter()
        .all(|program| path_limit::program_fits(&directory.join(program.file_name())))
}

/// Builds the version check of a verified staged Client.
pub(super) type Probe = fn(client: &Path) -> Command;
/// Completes the install of a published version directory under a store root.
pub(super) type Activation = fn(
    root: &Path,
    version: &Path,
    completion: Completion,
    context: &mut Context,
) -> Result<(), Error>;

/// What the completing Client is asked to do besides moving the pointer.
#[derive(Clone, Copy)]
pub(super) struct Completion {
    /// Whether it performs the PATH step; `--no-path` when false.
    pub(super) path: bool,
    /// Whether it activates a version lower than the active one; `--allow-downgrade` when true.
    pub(super) allow_downgrade: bool,
}

/// The staged Client's `--version`, without an inherited settings file.
pub(super) fn version_probe(client: &Path) -> Command {
    let mut command = Command::new(client);
    command.arg("--version").env_remove("ORCHBEE_CONFIG_FILE");
    command
}

/// The most of a failed helper's first standard-error line kept for the person.
const DIAGNOSTIC_LIMIT: usize = 1024;

/// Runs the published copy's completion helper, spelled as the copy's version reads it, so an
/// update can also move to a release up to 0.1.0-beta.24, which answers only the earlier spelling.
/// Its exit status `3` is the lower-version refusal, or `version-held` when the pointer, read
/// again, holds the version. Any other failure keeps the helper's first standard-error line, which
/// names the cause, such as a PATH step that could not finish. A helper stopped by a cancellation
/// or a deadline has ended and answers `cancelled` or `timeout`; `activate` reads the pointer
/// afterwards, since the helper may have moved it first. A helper not confirmed to have ended is
/// `activation-unknown`.
pub(super) fn run_completion(
    root: &Path,
    version: &Path,
    completion: Completion,
    context: &mut Context,
) -> Result<(), Error> {
    let name = version.file_name().and_then(|name| name.to_str());
    let mut command = Command::new(version.join(CohortArtifact::Client.file_name()));
    command.arg(COMPLETION_HELPER.for_release(name.unwrap_or_default()));
    if !completion.path {
        command.arg("--no-path");
    }
    if completion.allow_downgrade {
        command.arg("--allow-downgrade");
    }
    let result = platform::execute(command, context).map_err(|_| Error::Activation)?;
    match result.failure {
        None => Ok(()),
        Some(CommandFailure::Cancelled) => Err(Error::Cancelled),
        Some(CommandFailure::Timeout) => Err(Error::Timeout),
        Some(CommandFailure::PhysicalUnknown) => Err(Error::ActivationUnknown),
        Some(CommandFailure::ExitFailure) if result.exit_code == Some(3) => {
            let held = name.is_some_and(|name| held(root, name));
            Err(if held {
                Error::VersionHeld
            } else {
                Error::DowngradeRefused
            })
        }
        Some(_) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            context.completion_diagnostic = stderr
                .lines()
                .next()
                .map(|line| {
                    line.trim()
                        .chars()
                        .take(DIAGNOSTIC_LIMIT)
                        .collect::<String>()
                })
                .filter(|line| !line.is_empty());
            Err(Error::Activation)
        }
    }
}

/// The store's pointer, read without the writer lock; a missing root or an unreadable pointer is
/// none.
fn current_pointer(root: &Path) -> Option<Pointer> {
    read_pointer(root)
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
}

/// Whether the store holds `version` after a failed move, read without the writer lock. Only where
/// automatic updates run does any version count as held.
pub(super) fn held(root: &Path, version: &str) -> bool {
    automatic_updates_live()
        && current_pointer(root)
            .is_some_and(|pointer| held_versions(&pointer).iter().any(|held| held == version))
}

/// Where a release stands against the store's active version.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Standing {
    /// No version is active, or the active one orders at or below the release.
    Admitted,
    /// The pointer's active entry is this release and its version directory holds this manifest.
    Active,
    /// The active version orders above the release.
    Lower,
    /// The release's version fell back after a failed move and is held until a higher version is
    /// active, so nothing makes it active.
    Held,
}

/// Refuses the release `version`, whose `release.json` has the SHA-256 `identity`, before anything
/// is written when the store's active version is for another platform, because the store holds one
/// architecture; otherwise says where the release stands against the active version and whether
/// the store holds it. Records the active version it compared with. Reading the pointer and hashing
/// the stored manifest is the only file access, so an update decides from its signed feed alone.
pub(super) fn admit_store(
    root: &Path,
    version: &str,
    identity: &str,
    report: &mut Report,
) -> Result<Standing, Error> {
    let pointer = current_pointer(root);
    report.active_before = pointer
        .as_ref()
        .map(|pointer| pointer.active.product.clone());
    let Some(pointer) = pointer else {
        return Ok(Standing::Admitted);
    };
    let held =
        automatic_updates_live() && held_versions(&pointer).iter().any(|held| held == version);
    let active = pointer.active;
    let stored = stored_manifest(root, &active.product);
    let platform = stored
        .as_deref()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .and_then(|value| value.get("platform")?.as_str().map(str::to_owned));
    if platform.is_some_and(|platform| platform != own_platform()) {
        return Err(Error::OtherArchitecture);
    }
    Ok(
        if active.product == version
            && active.manifest == identity
            && stored.as_deref().map(sha256).as_deref() == Some(identity)
        {
            Standing::Active
        } else if held {
            Standing::Held
        } else if compare_product_versions(&active.product, version) == Ordering::Greater {
            Standing::Lower
        } else {
            Standing::Admitted
        },
    )
}

/// What a store's `versions\<version>` holds against the manifest identity an update was offered.
pub(super) enum Stored {
    Absent,
    /// The directory holds the offered manifest; these are its `release.json` and
    /// `release.json.sig`.
    Same {
        manifest: Vec<u8>,
        signature: Vec<u8>,
    },
    /// The directory holds another manifest under the same version.
    Other,
}

/// Reads `versions\<version>` of `root` without the writer lock and compares its `release.json`
/// with `identity`. A directory whose documents cannot be read is damaged.
///
/// # Errors
/// `cohort-invalid` for an unreadable directory, `storage` when its presence cannot be told.
pub(super) fn stored_version(
    root: &Path,
    version: &str,
    identity: &str,
    context: &mut Context,
) -> Result<Stored, Error> {
    let directory = root.join("versions").join(version);
    if !directory
        .try_exists()
        .map_err(|error| context.storage(&directory, &error))?
    {
        return Ok(Stored::Absent);
    }
    let manifest = stored_manifest(root, version).ok_or(Error::CohortInvalid)?;
    if sha256(&manifest) != identity {
        return Ok(Stored::Other);
    }
    let mut signature = Vec::new();
    open_release_file(&directory, "release.json.sig")
        .map_err(|_| Error::CohortInvalid)?
        .take(DOCUMENT_LIMIT as u64 + 1)
        .read_to_end(&mut signature)
        .map_err(|_| Error::CohortInvalid)?;
    Ok(Stored::Same {
        manifest,
        signature,
    })
}

fn sha256(bytes: &[u8]) -> String {
    crate::hex::encode(&Sha256::digest(bytes))
}

/// The `release.json` the version directory `version` holds, when it can be read. The pointer
/// admitted `version` as a version name.
fn stored_manifest(root: &Path, version: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    open_release_file(&root.join("versions").join(version), "release.json")
        .ok()?
        .take(MANIFEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

/// Verifies every file of `release` in a private stage, runs `probe` on the staged Client, and
/// publishes the stage, the signature document included, as `versions/<version>`. An existing
/// version directory holding the same manifest is verified and reused; one holding another
/// manifest is refused. Neither is ever overwritten.
pub(super) fn publish(
    root: &Path,
    source: &Source,
    release: &Release,
    signature: &[u8],
    probe: Probe,
    context: &mut Context,
    report: &mut Report,
) -> Result<PathBuf, Error> {
    context.check()?;
    let store = ReleaseStore::acquire(root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            Error::Busy
        } else {
            context.storage(root, &error)
        }
    })?;
    let destination = store
        .version_path(&release.version)
        .map_err(|error| context.storage(root, &error))?;
    if destination
        .try_exists()
        .map_err(|error| context.storage(&destination, &error))?
    {
        let mut stored = Vec::new();
        open_release_file(&destination, "release.json")
            .map_err(|_| Error::CohortInvalid)?
            .take(MANIFEST_LIMIT as u64 + 1)
            .read_to_end(&mut stored)
            .map_err(|error| context.storage(&destination, &error))?;
        // A version is built once, so another manifest under its name is a conflict.
        if stored != release.encoded {
            return Err(Error::VersionConflict);
        }
        verify(&destination, release, probe, context)?;
        report.published = Some(destination.clone());
        report.reused = true;
        return Ok(destination);
    }
    let mut stage = store
        .stage()
        .map_err(|error| context.storage(root, &error))?;
    let staged = stage.path().to_owned();
    let result = (|| {
        for (name, bytes) in [
            ("release.json", release.encoded.as_slice()),
            ("release.json.sig", signature),
        ] {
            let (path, mut file) = stage
                .create_file(name)
                .map_err(|error| context.storage(&staged.join(name), &error))?;
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| context.storage(&path, &error))?;
        }
        for artifact in &release.files {
            context.check()?;
            let (path, mut file) = stage
                .create_file(&artifact.path)
                .map_err(|error| context.storage(&staged.join(&artifact.path), &error))?;
            source
                .artifact(&artifact.url)?
                .copy(&path, &mut file, artifact.size, context)?;
            if PROGRAMS
                .iter()
                .any(|program| program.file_name() == artifact.path)
                || artifact.path == Cohort::files_file_name()
            {
                stage
                    .mark_executable(&file)
                    .map_err(|error| context.storage(&path, &error))?;
            }
        }
        report.phase = "verify";
        verify(stage.path(), release, probe, context)?;
        context.check()?;
        report.phase = "publish";
        let path = stage
            .publish(&release.version)
            .map_err(|error| context.storage(&destination, &error))?;
        report.published = Some(path.clone());
        Ok(path)
    })();
    if matches!(result, Err(Error::PhysicalUnknown)) || stage.cleanup().is_err() {
        report.cleanup_pending = Some(stage.retain_for_recovery());
    }
    result
}
fn verify(
    root: &Path,
    release: &Release,
    probe: Probe,
    context: &mut Context,
) -> Result<(), Error> {
    for artifact in &release.files {
        verify_file(root, artifact, context)?;
    }
    let cohort = Cohort::load(root, context.deadline).map_err(|_| Error::CohortInvalid)?;
    let sidecar = cohort.observation_adapters_sidecar();
    if !platform::verify_capsule(sidecar.directory(), sidecar.declared_payload_sha256())
        .map_err(|_| Error::CohortInvalid)?
    {
        return Err(Error::CohortInvalid);
    }
    // The Client is executed only after all manifest digests and cohort checks pass.
    let command = probe(&cohort.executable(CohortArtifact::Client));
    let result = platform::execute(command, context).map_err(|_| Error::SmokeFailed)?;
    context
        .command_result(&result)
        .map_err(|error| match error {
            Error::Download => Error::SmokeFailed,
            other => other,
        })?;
    let version = String::from_utf8(result.stdout).map_err(|_| Error::SmokeFailed)?;
    if version.trim() != format!("orchbee {}", release.version) {
        return Err(Error::SmokeFailed);
    }
    Ok(())
}
fn verify_file(root: &Path, artifact: &Artifact, context: &mut Context) -> Result<(), Error> {
    let mut file = open_release_file(root, &artifact.path).map_err(|_| Error::Checksum)?;
    if file.metadata().map_err(|_| Error::Checksum)?.len() != artifact.size {
        return Err(Error::Checksum);
    }
    let mut hash = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        context.check()?;
        let count = file.read(&mut bytes).map_err(|_| Error::Checksum)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    if crate::hex::encode(&hash.finalize()) != artifact.sha256 {
        return Err(Error::Checksum);
    }
    Ok(())
}

/// Whether the store's pointer names `entry` as the active version, read without the writer lock.
fn names_active(root: &Path, entry: &Entry) -> bool {
    current_pointer(root).is_some_and(|pointer| pointer.active == *entry)
}

/// Whether the store's pointer records an install made with `--no-path`, read without the writer
/// lock. An update keeps the PATH step off for such a store.
pub(super) fn records_no_path(root: &Path) -> bool {
    current_pointer(root).is_some_and(|pointer| pointer.no_path())
}

/// Records on the pointer whether the install that just activated ran without the PATH step: an
/// install made with `--no-path` sets the member that keeps later updates off the PATH step, and an
/// install with it clears the member. Nothing is written when the pointer already says so, so an
/// install without the member writes nothing new. The version is active by now, so a write that
/// fails is `activation-unknown`; a retry of the install writes the record.
pub(super) fn record_no_path(root: &Path, no_path: bool) -> Result<(), Error> {
    if records_no_path(root) == no_path {
        return Ok(());
    }
    let mut store = ReleaseStore::acquire(root).map_err(|_| Error::ActivationUnknown)?;
    let mut pointer = store
        .pointer()
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
        .ok_or(Error::ActivationUnknown)?;
    pointer.set_no_path(no_path);
    store
        .replace_pointer(&pointer.encode())
        .map_err(|_| Error::ActivationUnknown)
}

/// Runs `activation` for a published version, then reads the pointer to confirm that it names
/// that version. The completing Client takes the writer lock itself, so none is held here. A
/// completion that claims success while the pointer names another version is unconfirmed. A
/// failure, cancellation or deadline is reported as having moved nothing, which holds only while
/// the pointer does not name the version, so a completion that moved it and then failed or was
/// stopped is unconfirmed too.
pub(super) fn activate(
    root: &Path,
    version: &Path,
    release: &Release,
    activation: Activation,
    completion: Completion,
    context: &mut Context,
    report: &mut Report,
) -> Result<(), Error> {
    context.check()?;
    let entry = Entry {
        product: release.version.clone(),
        manifest: release.identity.clone(),
    };
    let was_active = names_active(root, &entry);
    if let Err(error) = activation(root, version, completion, context) {
        let moved = !was_active && names_active(root, &entry);
        let unconfirmed = matches!(error, Error::Activation | Error::Cancelled | Error::Timeout);
        return Err(if moved && unconfirmed {
            Error::ActivationUnknown
        } else {
            error
        });
    }
    report.activated = names_active(root, &entry);
    if report.activated {
        Ok(())
    } else {
        Err(Error::ActivationUnknown)
    }
}
