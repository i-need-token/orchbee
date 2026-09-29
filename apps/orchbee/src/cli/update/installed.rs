//! Offline installed release provenance, bound to an independently admitted native cohort.
use super::manifest::{MANIFEST_LIMIT, Release};
use orchbee_local_site_runtime::{
    NativeArtifact, NativeCohort, WindowsUpdateChannel, open_windows_release_file,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::Path,
};

/// Installation observation. Unavailable never becomes a guessed channel or development build.
pub(in crate::cli) enum Installation {
    Development,
    Unavailable,
    Verified {
        channel: WindowsUpdateChannel,
        version: String,
        build_id: String,
        manifest: String,
    },
}
impl Installation {
    pub(in crate::cli) fn delivery(&self) -> &'static str {
        match self {
            Self::Development => "development-cohort",
            Self::Unavailable => "unknown",
            Self::Verified { .. } => "native-release",
        }
    }
    pub(in crate::cli) fn channel(&self) -> Option<&'static str> {
        match self {
            Self::Verified { channel, .. } => Some(channel.as_str()),
            _ => None,
        }
    }
    pub(in crate::cli) fn json(&self) -> Value {
        match self {
            Self::Development => json!({"state":"development"}),
            Self::Unavailable => json!({"state":"unavailable"}),
            Self::Verified {
                channel,
                version,
                build_id,
                manifest,
            } => {
                json!({"state":"verified","channel":channel.as_str(),"version":version,"buildId":build_id,"manifestSha256":manifest})
            }
        }
    }
}
/// Reads no preferences and starts no process. Call only with the current cohort's admission result.
pub(in crate::cli) fn inspect(
    executable: &Path,
    admitted: Option<&NativeCohort>,
    version: &str,
) -> Installation {
    let Some(admitted) = admitted else {
        return Installation::Unavailable;
    };
    let Some(root) = executable.parent() else {
        return Installation::Unavailable;
    };
    let file = match open_windows_release_file(root, "release.json") {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Installation::Development,
        Err(_) => return Installation::Unavailable,
    };
    let mut bytes = Vec::new();
    if file
        .take(MANIFEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Installation::Unavailable;
    }
    let Some(release) = Release::parse(&bytes, WindowsUpdateChannel::Stable)
        .or_else(|| Release::parse(&bytes, WindowsUpdateChannel::Preview))
    else {
        return Installation::Unavailable;
    };
    if root.file_name().and_then(|s| s.to_str()) != Some(release.identity.as_str())
        || release.version != version
    {
        return Installation::Unavailable;
    }
    for artifact in [
        NativeArtifact::Client,
        NativeArtifact::Server,
        NativeArtifact::Worker,
        NativeArtifact::TerminalHost,
    ] {
        if !release.files.iter().any(|file| {
            file.path.eq_ignore_ascii_case(artifact.file_name())
                && file.sha256 == admitted.artifact_sha256(artifact)
        }) {
            return Installation::Unavailable;
        }
    }
    let Some(artifact) = release
        .files
        .iter()
        .find(|file| file.path.eq_ignore_ascii_case("cohort.json"))
    else {
        return Installation::Unavailable;
    };
    let mut cohort = Vec::new();
    let Ok(file) = open_windows_release_file(root, &artifact.path) else {
        return Installation::Unavailable;
    };
    if file.take(16 * 1024 + 1).read_to_end(&mut cohort).is_err()
        || cohort.len() > 16 * 1024
        || cohort.len() as u64 != artifact.size
        || format!("{:x}", Sha256::digest(&cohort)) != artifact.sha256
    {
        return Installation::Unavailable;
    }
    Installation::Verified {
        channel: release.channel,
        version: release.version,
        build_id: release.build_id,
        manifest: release.identity,
    }
}
