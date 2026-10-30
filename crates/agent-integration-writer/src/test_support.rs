//! Fault-injection seams and the frozen provider fixture packs, shared by unit tests and the
//! owning application's CLI tests. The packs pin CLI exits too, so their runners take the
//! application's exit mapping instead of reaching into its command layer.

pub(super) mod fault_injection;
mod provider_conformance;
mod provider_effects;

pub use provider_conformance::provider_conformance64_replay;
pub use provider_effects::provider_effects52_conformance;

/// The owning application's exit status for one integration mutation report.
pub type CliStatus = fn(&super::MutationReport) -> u8;
