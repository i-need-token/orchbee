//! Catalog downloads run through the platform's owned process adapter. Builds without one refuse
//! before any process or network effect.
#[cfg(not(windows))]
use super::{CatalogUrl, Failure};
#[cfg(not(windows))]
use std::{sync::atomic::AtomicBool, time::Instant};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(in crate::agent_screen::updating) use windows::text;

/// No owned fetch process exists on this build, so the attempt fails as a process failure.
#[cfg(not(windows))]
pub(in crate::agent_screen::updating) fn text(
    _source: &CatalogUrl,
    _stop: &AtomicBool,
    _deadline: Instant,
) -> Result<String, Failure> {
    Err(Failure::Process)
}
