//! Complete cohort verification and immutable publication into `versions/<version>`, then the
//! published copy's own completion, which moves the pointer. The commands this owner runs come
//! from its caller, so tests publish and complete releases in process without starting a Client.
use super::{
    Context, Error, Report,
    acquire::Source,
    manifest::{Artifact, Release, own_platform},
    platform,
    signature::MANIFEST_LIMIT,
};
use crate::release_store::{ReleaseStore, open_release_file, read_pointer};
use orchbee_local_runtime::{Cohort, CohortArtifact, CommandFailure, compare_product_versions};
use orchbee_release_records::{Entry, Pointer};
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

/// The helper a published copy runs to complete its own install. Its grammar never changes; a
/// change adds a new helper name.
pub(super) const COMPLETION_HELPER: &str = "--complete-install-v1";

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

/// Runs the published copy's completion helper; its exit status `3` is the lower-version refusal.
/// Any other failure keeps the helper's first standard-error line, which names the cause, such as
/// a PATH step that could not finish.
pub(super) fn run_completion(
    _root: &Path,
    version: &Path,
    completion: Completion,
    context: &mut Context,
) -> Result<(), Error> {
    let mut command = Command::new(version.join(CohortArtifact::Client.file_name()));
    command.arg(COMPLETION_HELPER);
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
            Err(Error::DowngradeRefused)
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

/// The active version a store's pointer names, read without the writer lock; a missing root or an
/// unreadable pointer names none.
pub(super) fn active_version(root: &Path) -> Option<String> {
    read_pointer(root)
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
        .map(|pointer| pointer.active.product)
}

/// Refuses `release` before anything is written when the store's active version is for another
/// platform, because the store holds one architecture, or when it orders above `release` and the
/// person did not allow a downgrade. Records the active version it compared with.
pub(super) fn admit_store(
    root: &Path,
    release: &Release,
    allow_downgrade: bool,
    report: &mut Report,
) -> Result<(), Error> {
    report.active_before = active_version(root);
    let Some(active) = &report.active_before else {
        return Ok(());
    };
    if active_platform(root, active).is_some_and(|platform| platform != own_platform()) {
        return Err(Error::OtherArchitecture);
    }
    if !allow_downgrade && compare_product_versions(active, &release.version) == Ordering::Greater {
        return Err(Error::DowngradeRefused);
    }
    Ok(())
}

/// The platform the active version's manifest names, when it can be read. The pointer admitted
/// `active` as a version name.
fn active_platform(root: &Path, active: &str) -> Option<String> {
    let mut bytes = Vec::new();
    open_release_file(&root.join("versions").join(active), "release.json")
        .ok()?
        .take(MANIFEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("platform")?.as_str().map(str::to_owned)
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
    let store = ReleaseStore::acquire(root).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            Error::Busy
        } else {
            Error::Storage
        }
    })?;
    let destination = store
        .version_path(&release.version)
        .map_err(|_| Error::Storage)?;
    if destination.try_exists().map_err(|_| Error::Storage)? {
        let mut stored = Vec::new();
        open_release_file(&destination, "release.json")
            .map_err(|_| Error::CohortInvalid)?
            .take(MANIFEST_LIMIT as u64 + 1)
            .read_to_end(&mut stored)
            .map_err(|_| Error::Storage)?;
        // A version is built once, so another manifest under its name is a conflict.
        if stored != release.encoded {
            return Err(Error::VersionConflict);
        }
        verify(&destination, release, probe, context)?;
        report.published = Some(destination.clone());
        report.reused = true;
        return Ok(destination);
    }
    let mut stage = store.stage().map_err(|_| Error::Storage)?;
    let result = (|| {
        for (name, bytes) in [
            ("release.json", release.encoded.as_slice()),
            ("release.json.sig", signature),
        ] {
            let (_, mut file) = stage.create_file(name).map_err(|_| Error::Storage)?;
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| Error::Storage)?;
        }
        for artifact in &release.files {
            context.check()?;
            let (path, mut file) = stage
                .create_file(&artifact.path)
                .map_err(|_| Error::Storage)?;
            source
                .artifact(&artifact.url)?
                .copy(&path, &mut file, artifact.size, context)?;
        }
        report.phase = "verify";
        verify(stage.path(), release, probe, context)?;
        context.check()?;
        report.phase = "publish";
        let path = stage
            .publish(&release.version)
            .map_err(|_| Error::Storage)?;
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
    if format!("{:x}", hash.finalize()) != artifact.sha256 {
        return Err(Error::Checksum);
    }
    Ok(())
}

/// Runs `activation` for a published version, then reads the pointer to confirm that it names
/// that version. The completing Client takes the writer lock itself, so none is held here. A
/// completion that claims success while the pointer names another version is unconfirmed.
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
    activation(root, version, completion, context)?;
    let entry = Entry {
        product: release.version.clone(),
        manifest: release.identity.clone(),
    };
    report.activated = read_pointer(root)
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
        .is_some_and(|pointer| pointer.active == entry);
    if report.activated {
        Ok(())
    } else {
        Err(Error::ActivationUnknown)
    }
}
