use super::*;
use orchbee_workbench_protocol::{AgentCompletion, PanelAgentState};

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
fn only_a_change_that_hands_over_is_unseen_and_resumed_work_clears_an_unread_turn() {
    let mut owner = PhysicalAttentionState::default();
    let mut agent = state(1, 1, 1);
    publish(&mut owner, 1, std::slice::from_ref(&agent));
    let mut step = |owner: &mut PhysicalAttentionState, sequence: u64, next: PanelAgentState| {
        agent.sequence = WorkbenchRevision::new(sequence);
        agent.state = next;
        publish(owner, sequence, std::slice::from_ref(&agent));
        seen(owner, &agent).unwrap()
    };
    assert!(step(&mut owner, 2, PanelAgentState::Working));
    assert!(!step(&mut owner, 3, PanelAgentState::Idle));
    // An Agent working again carries no dot: its work ends in another hand-over that marks it.
    assert!(step(&mut owner, 4, PanelAgentState::Working));
    assert!(!step(&mut owner, 5, PanelAgentState::Idle));
    assert!(owner.acknowledge(&owner.paint(|_| true).unwrap(), Some(true), |_| true));
    // Resuming by itself after a read turn, as when a background command ends, is nothing new.
    assert!(step(&mut owner, 6, PanelAgentState::Working));
    assert!(step(&mut owner, 7, PanelAgentState::Unknown));
    assert!(!step(&mut owner, 8, PanelAgentState::Blocked));
}

#[test]
fn a_turn_that_ends_behind_an_unrecognised_screen_is_unseen_once() {
    let mut owner = PhysicalAttentionState::default();
    let mut agent = state(1, 1, 1);
    publish(&mut owner, 1, std::slice::from_ref(&agent));
    let mut step = |owner: &mut PhysicalAttentionState,
                    sequence: u64,
                    next: PanelAgentState,
                    completion: AgentCompletion| {
        agent.sequence = WorkbenchRevision::new(sequence);
        agent.state = next;
        agent.completion = completion;
        publish(owner, sequence, std::slice::from_ref(&agent));
        seen(owner, &agent).unwrap()
    };
    assert!(step(
        &mut owner,
        2,
        PanelAgentState::Working,
        AgentCompletion::None
    ));
    assert!(step(
        &mut owner,
        3,
        PanelAgentState::Unknown,
        AgentCompletion::None
    ));
    // The Server holds the finished turn once the screen is recognised again.
    assert!(!step(
        &mut owner,
        4,
        PanelAgentState::Idle,
        AgentCompletion::AwaitingInput
    ));
    assert!(owner.acknowledge(&owner.paint(|_| true).unwrap(), Some(true), |_| true));
    // A read Done that passes through an unrecognised screen is not handed over again.
    assert!(step(
        &mut owner,
        5,
        PanelAgentState::Unknown,
        AgentCompletion::AwaitingInput
    ));
    assert!(step(
        &mut owner,
        6,
        PanelAgentState::Idle,
        AgentCompletion::AwaitingInput
    ));
}

#[test]
fn stale_hidden_unfocused_and_failed_reads_never_acknowledge_physical_changes() {
    let mut owner = PhysicalAttentionState::default();
    publish(&mut owner, 0, &[]);
    let mut current = state(1, 1, 1);
    current.state = PanelAgentState::Done;
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
    let mut replacement = state(1, 2, 0);
    let mut child = state(2, 3, 0);
    // Only a state that hands the person something new starts unseen.
    replacement.state = PanelAgentState::Done;
    child.state = PanelAgentState::Done;
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
    terminal.state = PanelAgentState::Done;
    publish(&mut owner, 1, std::slice::from_ref(&terminal));
    assert_eq!(seen(&owner, &terminal), Some(false));
    assert_eq!(
        owner.unseen_in(terminal.binding.presentation_id.as_str()),
        0
    );
    let mut agent = state(1, 1, 1);
    agent.state = PanelAgentState::Done;
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
    let mut unversioned_completion = initial.clone();
    unversioned_completion.completion = AgentCompletion::AwaitingInput;
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
        PhysicalRound {
            report_revision: WorkbenchRevision::new(5),
            states: vec![unversioned_completion],
        },
    ] {
        assert!(owner.publish(&round).is_err());
        assert_eq!(seen(&owner, &initial), Some(true));
        assert_eq!(owner.entries.len(), 1);
        assert_eq!(owner.revision, Some(WorkbenchRevision::new(5)));
    }
}
