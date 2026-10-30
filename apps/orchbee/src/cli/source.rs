//! Shared declared-input materialization and finish projection over the original native guard.
//! Native ports retain source descriptors, signal policy and the owed emit finalization.
use super::SourceFinalization;

mod input;
pub(crate) use input::{InputSource, SourceFailure, materialize, source_details};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
use unsupported as platform;

pub(crate) use platform::SignalWatch;
use platform::{acquire_file, acquire_stdin};
#[cfg(windows)]
pub(in crate::cli) use windows::acquire_document;

pub(crate) enum Finish {
    Clear,
    Cancelled {
        signo: i32,
        finalization: SourceFinalization,
    },
}

pub(crate) struct ArmedSignals(platform::ArmedSignals);

impl ArmedSignals {
    pub(crate) fn arm() -> Result<Self, platform::ArmError> {
        platform::ArmedSignals::arm().map(Self)
    }

    pub(crate) fn finish(self) -> Result<Finish, platform::SignalError> {
        platform::finish(self.0).map(|cancelled| match cancelled {
            None => Finish::Clear,
            Some((signo, finalization)) => Finish::Cancelled {
                signo,
                finalization,
            },
        })
    }

    pub(crate) fn into_finalization(self) -> SourceFinalization {
        self.0.into_finalization()
    }
}

/// The declared form's acquisition bound: the literal `bytes` record bound
/// plus one spare byte to detect oversize, or the `paste` content bound.
/// The folded `--enter` CR counts toward the `bytes` record bound after
/// acquisition, so acquisition reads at most the plain bound.
fn limit_for(paste: bool) -> usize {
    if paste {
        crate::invocation::MAXIMUM_SEND_PASTE_BYTES
    } else {
        crate::invocation::MAXIMUM_SEND_BYTES_BYTES
    }
}
