use std::collections::BTreeSet;

use orchbee_connection_handshake::{
    AGENT_INPUT_AREA, CapabilityName, CapabilitySet, ConnectionKind, PROMPT_GATED_INPUT,
};

use super::{NEEDED_BY, SessionAction, serves};

/// This build's `client-server` offer without `missing`: what an older Session's answer lists.
fn answer_without(missing: &[&str]) -> CapabilitySet {
    CapabilitySet::new(
        ConnectionKind::ClientServer
            .capabilities()
            .iter()
            .filter(|name| !missing.contains(&name.as_str()))
            .cloned(),
    )
    .unwrap()
}

/// A capability added to the offer is classified here before it ships, and no row names one the
/// offer lacks, which no answer could then list.
#[test]
fn the_table_classifies_exactly_the_capabilities_the_offer_lists() {
    let offer = ConnectionKind::ClientServer.capabilities();
    let offered: BTreeSet<&str> = offer.iter().map(CapabilityName::as_str).collect();
    let classified: BTreeSet<&str> = NEEDED_BY.iter().map(|(name, _)| *name).collect();
    assert_eq!(classified, offered);
    assert_eq!(classified.len(), NEEDED_BY.len(), "one row per capability");
}

#[test]
fn a_missing_capability_disables_only_the_actions_that_need_it() {
    let action = SessionAction::StartInThisPanel;
    assert!(serves(Some(&answer_without(&[])), action));
    assert!(serves(None, action), "no answer knows of nothing missing");
    assert!(serves(Some(&answer_without(&[AGENT_INPUT_AREA])), action));
    assert!(!serves(
        Some(&answer_without(&[PROMPT_GATED_INPUT, AGENT_INPUT_AREA])),
        action
    ));
}
