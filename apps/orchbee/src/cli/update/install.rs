//! Complete cohort verification and immutable publication before user PATH activation.
#[cfg(test)]
mod tests;
use super::{
    Context, Error, Report,
    acquire::Source,
    manifest::{Artifact, Release},
};
use crate::release_store::{ReleaseStore, open_release_file};
use base64::Engine;
use orchbee_local_runtime::{Cohort, run_windows_command};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn root() -> Result<PathBuf, Error> {
    let path = match std::env::var_os("ORCHBEE_INSTALL_DIR") {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(Error::InstallLocation)?)
            .join("Programs/Orchbee"),
    };
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|s| s.len() > 2048 || s.contains([';', '%', '\0', '\r', '\n', '"']))
    {
        return Err(Error::InstallLocation);
    }
    Ok(path)
}
pub(super) fn publish(
    root: &Path,
    source: &Source,
    release: &Release,
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
        .release_path(&release.identity)
        .map_err(|_| Error::Storage)?;
    if destination.try_exists().map_err(|_| Error::Storage)? {
        let mut stored = Vec::new();
        open_release_file(&destination, "release.json")
            .map_err(|_| Error::CohortInvalid)?
            .take(super::manifest::MANIFEST_LIMIT as u64 + 1)
            .read_to_end(&mut stored)
            .map_err(|_| Error::Storage)?;
        if stored != release.encoded {
            return Err(Error::CohortInvalid);
        }
        verify(&destination, release, context)?;
        report.published = Some(destination.clone());
        report.reused = true;
        return Ok(destination);
    }
    let mut stage = store.stage().map_err(|_| Error::Storage)?;
    let result = (|| {
        let (_, mut metadata) = stage
            .create_file("release.json")
            .map_err(|_| Error::Storage)?;
        metadata
            .write_all(&release.encoded)
            .and_then(|()| metadata.sync_all())
            .map_err(|_| Error::Storage)?;
        drop(metadata);
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
        verify(stage.path(), release, context)?;
        context.check()?;
        report.phase = "publish";
        let path = stage
            .publish(&release.identity)
            .map_err(|_| Error::Storage)?;
        report.published = Some(path.clone());
        Ok(path)
    })();
    if matches!(result, Err(Error::PhysicalUnknown)) || stage.cleanup().is_err() {
        report.cleanup_pending = Some(stage.retain_for_recovery());
    }
    result
}
fn verify(root: &Path, release: &Release, context: &mut Context) -> Result<(), Error> {
    for artifact in &release.files {
        verify_file(root, artifact, context)?;
    }
    let cohort = Cohort::load(root, context.deadline).map_err(|_| Error::CohortInvalid)?;
    let sidecar = cohort.observation_adapters_sidecar();
    if !crate::agent_integrations::verify_distribution_capsule(
        sidecar.directory(),
        sidecar.declared_payload_sha256(),
    ) {
        return Err(Error::CohortInvalid);
    }
    // This binary is executed only after all manifest digests and cohort checks pass.
    let mut command = Command::new(root.join("orchbee.exe"));
    command.arg("--version").env_remove("ORCHBEE_CONFIG_FILE");
    let result = run_windows_command(command, Some(context.deadline), || context.interrupted());
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

pub(super) fn activate(root: &Path, release: &Path, context: &mut Context) -> Result<(), Error> {
    // The root publisher lock also serializes cooperating PATH activations.
    let _store = ReleaseStore::acquire(root).map_err(|_| Error::Busy)?;
    context.check()?;
    let system = winsafe::GetSystemDirectory().map_err(|_| Error::Activation)?;
    let mut command =
        Command::new(PathBuf::from(system).join("WindowsPowerShell/v1.0/powershell.exe"));
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
        ])
        .arg(
            base64::engine::general_purpose::STANDARD.encode(
                include_str!("activate.ps1")
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
        )
        .env("ORCHBEE_INSTALL_ROOT", root)
        .env("ORCHBEE_INSTALL_RELEASE", release)
        .env_remove("PSModulePath");
    let result = run_windows_command(command, Some(context.deadline), || context.interrupted());
    if result.failure.is_some() {
        return Err(Error::ActivationUnknown);
    }
    Ok(())
}
