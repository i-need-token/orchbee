//! Offline installed release provenance, bound by the store pointer to an independently admitted
//! native cohort.
use super::{
    manifest::{Release, own_platform},
    signature::{MANIFEST_LIMIT, TrustedManifest},
};
use crate::release_store::{open_release_file, read_pointer, version_directory_parts};
use orchbee_local_runtime::{Cohort, CohortArtifact, UpdateChannel};
use orchbee_release_records::{Entry, Pointer};
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
    // The store pointer, checked below, binds these bytes to the verified installation.
    let Some(release) = Release::parse(TrustedManifest::installed(&bytes), &own_platform()) else {
        return Err(());
    };
    let Some((store, directory)) = version_directory_parts(root) else {
        return Err(());
    };
    let entry = Entry {
        product: release.version.clone(),
        manifest: release.identity.clone(),
    };
    let bound = read_pointer(store)
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
        .is_some_and(|pointer| pointer.names(&entry));
    if !bound || directory != release.version || release.version != version {
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
