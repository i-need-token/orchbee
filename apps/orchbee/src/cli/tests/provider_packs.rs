//! Frozen provider fixture packs replayed against this application's integration exit mapping.

use crate::agent_integrations::test_support;
use crate::cli::integration::mutation_status;

#[test]
fn provider_effects52_conformance() {
    test_support::provider_effects52_conformance(mutation_status);
}

#[test]
fn provider_conformance64_replay() {
    test_support::provider_conformance64_replay(mutation_status);
}
