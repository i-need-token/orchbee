//! Integration and conformance tests through this crate's public boundaries.

/// The fingerprint algorithm the build script runs, compiled again for the reference tests.
#[path = "../build/fingerprint.rs"]
mod algorithm;

mod admission;
mod conformance;
mod fingerprint;
mod registry;
mod support;
