//! Fault-injection seams and the frozen provider fixture packs, shared by unit tests and the
//! owning application's CLI tests. The packs pin CLI exits too, so their runners take the
//! application's exit mapping instead of reaching into its command layer. Only the application's
//! tests run the packs, so they compile with the `test-support` feature alone.

pub(super) mod fault_injection;
#[cfg(feature = "test-support")]
mod provider_conformance;
#[cfg(feature = "test-support")]
mod provider_effects;

#[cfg(feature = "test-support")]
pub use provider_conformance::provider_conformance64_replay;
#[cfg(feature = "test-support")]
pub use provider_effects::provider_effects52_conformance;

/// The owning application's exit status for one integration mutation report.
#[cfg(feature = "test-support")]
pub type CliStatus = fn(&super::MutationReport) -> u8;
