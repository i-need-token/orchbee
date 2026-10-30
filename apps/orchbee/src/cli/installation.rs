//! Interpret observed native release provenance without guessing a channel or development build.
use orchbee_local_runtime::{Cohort, UpdateChannel};
use serde_json::{Value, json};
use std::path::Path;

#[cfg(windows)]
#[path = "installation/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "installation/unix.rs"]
mod platform;

#[derive(Debug)]
pub(super) struct Unsupported;

/// Installation observation. Unavailable never becomes a guessed channel or development build.
pub(in crate::cli) enum Installation {
    Development,
    Unavailable,
    Verified {
        channel: UpdateChannel,
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

/// Only an observed missing release file means a development cohort.
/// A failed verification remains unavailable; an unsupported native port stays an error.
pub(super) fn inspect(
    executable: &Path,
    admitted: Option<&Cohort>,
    version: &str,
) -> Result<Installation, Unsupported> {
    platform::admit()?;
    Ok(
        match crate::cli::update::observe_installation(executable, admitted, version) {
            Ok(None) => Installation::Development,
            Ok(Some(crate::cli::update::VerifiedRelease {
                channel,
                version,
                build_id,
                manifest,
            })) => Installation::Verified {
                channel,
                version,
                build_id,
                manifest,
            },
            Err(()) => Installation::Unavailable,
        },
    )
}
