//! Offline installed release provenance, bound to an independently admitted native cohort.
use super::manifest::{MANIFEST_LIMIT, Release};
use crate::release_store::open_release_file;
use orchbee_local_runtime::{Cohort, CohortArtifact, UpdateChannel};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::Path,
};

/// One installed release whose manifest matched the admitted native cohort.
pub(in crate::cli) struct VerifiedRelease {
    pub(in crate::cli) channel: UpdateChannel,
    pub(in crate::cli) version: String,
    pub(in crate::cli) build_id: String,
    pub(in crate::cli) manifest: String,
}

/// Reads no preferences and starts no process. Call only with the current cohort's admission result.
pub(in crate::cli) fn observe(
    executable: &Path,
    admitted: Option<&Cohort>,
    version: &str,
) -> Result<Option<VerifiedRelease>, ()> {
    let Some(admitted) = admitted else {
        return Err(());
    };
    let Some(root) = executable.parent() else {
        return Err(());
    };
    let file = match open_release_file(root, "release.json") {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let mut bytes = Vec::new();
    if file
        .take(MANIFEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Err(());
    }
    let Some(release) = Release::parse(&bytes, UpdateChannel::Stable)
        .or_else(|| Release::parse(&bytes, UpdateChannel::Preview))
    else {
        return Err(());
    };
    if root.file_name().and_then(|s| s.to_str()) != Some(release.identity.as_str())
        || release.version != version
    {
        return Err(());
    }
    for artifact in [
        CohortArtifact::Client,
        CohortArtifact::Server,
        CohortArtifact::Worker,
        CohortArtifact::TerminalHost,
    ] {
        if !release.files.iter().any(|file| {
            file.path.eq_ignore_ascii_case(&artifact.file_name())
                && file.sha256 == admitted.artifact_sha256(artifact)
        }) {
            return Err(());
        }
    }
    let Some(artifact) = release
        .files
        .iter()
        .find(|file| file.path.eq_ignore_ascii_case("cohort.json"))
    else {
        return Err(());
    };
    let mut cohort = Vec::new();
    let Ok(file) = open_release_file(root, &artifact.path) else {
        return Err(());
    };
    if file.take(16 * 1024 + 1).read_to_end(&mut cohort).is_err()
        || cohort.len() > 16 * 1024
        || cohort.len() as u64 != artifact.size
        || format!("{:x}", Sha256::digest(&cohort)) != artifact.sha256
    {
        return Err(());
    }
    Ok(Some(VerifiedRelease {
        channel: release.channel,
        version: release.version,
        build_id: release.build_id,
        manifest: release.identity,
    }))
}
