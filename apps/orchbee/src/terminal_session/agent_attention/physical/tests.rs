use super::*;
use orchbee_workbench_protocol::PanelAgentState;

fn state(id: u64, incarnation: u64, sequence: u64) -> TerminalAgentStateSnapshot {
    serde_json::from_value(serde_json::json!({
        "binding":{"presentationId":format!("{id:032x}"),"generation":format!("{incarnation:032x}"),
        "hostEpoch":"11111111111111111111111111111111","hostTerminalId":format!("{incarnation:032x}")},
        "sequence":sequence.to_string(),"agent":"pi","state":"idle"
    })).unwrap()
}
fn publish(
    owner: &mut PhysicalAttentionState,
    revision: u64,
    states: &[TerminalAgentStateSnapshot],
) {
    owner
        .publish(&PhysicalRound {
            report_revision: WorkbenchRevision::new(revision),
            states: states.to_vec(),
        })
        .unwrap();
}
fn seen(owner: &PhysicalAttentionState, state: &TerminalAgentStateSnapshot) -> Option<bool> {
    owner
        .snapshot(std::slice::from_ref(&state.binding), None)?
        .entries[0]
        .state
        .as_ref()
        .map(|s| s.seen)
}

#[test]
fn physical_clients_observe_independent_paints_and_preserve_quiet_updates() {
    let (mut a, mut b) = (
        PhysicalAttentionState::default(),
        PhysicalAttentionState::default(),
    );
    let initial = state(1, 1, 0);
    for client in [&mut a, &mut b] {
        publish(client, 0, std::slice::from_ref(&initial));
    }
    assert_eq!(seen(&a, &initial), Some(true));
    let changed = state(1, 1, 2); // A complete sample may span transient working -> idle.
    for client in [&mut a, &mut b] {
        publish(client, 2, std::slice::from_ref(&changed));
    }
    assert_eq!(seen(&a, &changed), Some(false));
    let proof = a.paint(|_| true).unwrap();
    assert!(a.acknowledge(&proof, None, |_| true));
    assert_eq!(seen(&a, &changed), Some(true));
    assert_eq!(seen(&b, &changed), Some(false));
    publish(&mut a, 3, std::slice::from_ref(&changed));
    publish(&mut b, 3, std::slice::from_ref(&changed));
    assert_eq!(seen(&a, &changed), Some(true));
    assert_eq!(seen(&b, &changed), Some(false));
}

#[test]
fn stale_hidden_unfocused_and_failed_reads_never_acknowledge_physical_changes() {
    let mut owner = PhysicalAttentionState::default();
    publish(&mut owner, 0, &[]);
    let current = state(1, 1, 1);
    publish(&mut owner, 1, std::slice::from_ref(&current));
    let proof = owner.paint(|_| true).unwrap();
    assert!(!owner.acknowledge(&proof, Some(false), |_| true));
    assert!(!owner.acknowledge(&proof, Some(true), |_| false));
    let hidden = owner.paint(|_| false).unwrap();
    assert!(!owner.acknowledge(&hidden, Some(true), |_| true));
    let next = state(1, 1, 2);
    publish(&mut owner, 2, std::slice::from_ref(&next));
    assert!(!owner.acknowledge(&proof, Some(true), |_| true));
    let proof = owner.paint(|_| true).unwrap();
    owner.unavailable();
    assert_eq!(seen(&owner, &next), None);
    assert!(!owner.acknowledge(&proof, None, |_| true));
    publish(&mut owner, 2, std::slice::from_ref(&next));
    assert!(!owner.acknowledge(&proof, None, |_| true));
    assert_eq!(seen(&owner, &next), Some(false));
    assert!(owner.acknowledge(&owner.paint(|_| true).unwrap(), Some(true), |_| true));
}

#[test]
fn physical_replacement_removal_and_child_entries_do_not_inherit_seen() {
    let mut owner = PhysicalAttentionState::default();
    let initial = state(1, 1, 5);
    publish(&mut owner, 5, std::slice::from_ref(&initial));
    let replacement = state(1, 2, 0);
    let child = state(2, 3, 0);
    publish(&mut owner, 6, &[replacement.clone(), child.clone()]);
    assert_eq!(seen(&owner, &initial), None);
    assert_eq!(seen(&owner, &replacement), Some(false));
    let proof = owner
        .paint(|binding| binding == &replacement.binding)
        .unwrap();
    assert!(owner.acknowledge(&proof, None, |_| true));
    assert_eq!(seen(&owner, &child), Some(false));
    publish(&mut owner, 7, std::slice::from_ref(&child));
    publish(&mut owner, 8, &[replacement.clone(), child]);
    assert_eq!(seen(&owner, &replacement), Some(false));
}

#[test]
fn ordinary_terminal_changes_do_not_create_agent_badges() {
    let mut owner = PhysicalAttentionState::default();
    publish(&mut owner, 0, &[]);
    let mut terminal = state(1, 1, 0);
    terminal.agent = None;
    terminal.state = PanelAgentState::Unknown;
    publish(&mut owner, 1, std::slice::from_ref(&terminal));
    assert_eq!(seen(&owner, &terminal), Some(false));
    assert_eq!(
        owner.unseen_in(terminal.binding.presentation_id.as_str()),
        0
    );
    let agent = state(1, 1, 1);
    publish(&mut owner, 2, std::slice::from_ref(&agent));
    assert_eq!(owner.unseen_in(agent.binding.presentation_id.as_str()), 1);
}

#[test]
fn malformed_rounds_are_atomic_and_cannot_regress_or_smuggle_state() {
    let mut owner = PhysicalAttentionState::default();
    let initial = state(1, 1, 5);
    publish(&mut owner, 5, std::slice::from_ref(&initial));
    let mut altered = initial.clone();
    altered.state = PanelAgentState::Working;
    for round in [
        PhysicalRound {
            report_revision: WorkbenchRevision::new(4),
            states: vec![initial.clone()],
        },
        PhysicalRound {
            report_revision: WorkbenchRevision::new(6),
            states: vec![state(2, 2, 0), state(1, 1, 4)],
        },
        PhysicalRound {
            report_revision: WorkbenchRevision::new(6),
            states: vec![altered],
        },
        PhysicalRound {
            report_revision: WorkbenchRevision::new(6),
            states: vec![initial.clone(), initial.clone()],
        },
    ] {
        assert!(owner.publish(&round).is_err());
        assert_eq!(seen(&owner, &initial), Some(true));
        assert_eq!(owner.entries.len(), 1);
        assert_eq!(owner.revision, Some(WorkbenchRevision::new(5)));
    }
}
