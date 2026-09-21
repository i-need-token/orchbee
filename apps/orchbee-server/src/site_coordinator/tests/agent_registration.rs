use super::super::agent_registration::RegistrationFact;
use super::panel_split::{close_leaf, standalone_leaf};
use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion,
    pending_first_open,
};
use super::*;
use crate::terminal_coordinator::TerminalPhase;
use orchbee_capability_node_protocol::{
    HostTerminalIdentity, WorkerGeneration, WorkerId as CapabilityWorkerId,
};
use orchbee_server::WorkbenchTerminalInvariantError;
use orchbee_workbench_protocol::TerminalPresentationId;

fn complete_opened_member(coordinator: &mut SiteCoordinator) -> TerminalOperationAuthorization {
    let pending = pending_first_open(coordinator, 1, 101);
    let authorization = launch_authorization(&pending);
    let completion = immediate_completion(pending.command());
    coordinator
        .complete(pending, completion, Some(worker(1)))
        .unwrap();
    authorization
}

fn member_binding(site: &mut SiteCoordinator) -> (TerminalPresentationId, PresentationBinding) {
    let committed = snapshot(site);
    let mut members = Vec::new();
    for context in &committed.terminal_contexts {
        let membership = site
            .workbench
            .as_ref()
            .unwrap()
            .terminal_tree_membership(&context.terminal_context_id)
            .unwrap();
        for member in membership.members() {
            members.push((member.presentation_id().clone(), member.binding().clone()));
        }
    }
    assert_eq!(members.len(), 1);
    members.pop().unwrap()
}

#[test]
fn empty_and_missing_workbench_sites_have_no_registrations() {
    let coordinator = SiteCoordinator::new(None);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
    let coordinator = site();
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn pending_joint_creates_commit_membership_without_registration() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
    let _pending = dispatch_tab(&mut coordinator, command, 701);
    // The member is already committed while the retained create is still Creating.
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));

    let _standalone = pending_first_open(&mut coordinator, 2, 801);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn accepted_standalone_member_registers_one_typed_fact() {
    let mut coordinator = site();
    let authorization = complete_opened_member(&mut coordinator);
    let (presentation_id, binding) = member_binding(&mut coordinator);

    let facts = coordinator.agent_registration_snapshot().unwrap();
    let [fact] = facts.as_slice() else {
        panic!("exactly one registration expected: {facts:?}")
    };
    assert_eq!(fact.presentation_id(), &presentation_id);
    assert_eq!(fact.binding(), &binding);
    assert_eq!(fact.worker_id(), authorization.worker_id());
    assert_eq!(fact.worker_generation(), authorization.worker_generation());
    assert_eq!(fact.identity(), identity());
}

#[test]
fn accepted_workspace_member_registers_one_typed_fact() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
    let pending = dispatch_tab(&mut coordinator, command, 701);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch expected")
    };
    let authorization = launch.authorization();
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let (presentation_id, binding) = member_binding(&mut coordinator);

    let facts = coordinator.agent_registration_snapshot().unwrap();
    let [fact] = facts.as_slice() else {
        panic!("exactly one registration expected: {facts:?}")
    };
    assert_eq!(fact.presentation_id(), &presentation_id);
    assert_eq!(fact.binding(), &binding);
    assert_eq!(fact.worker_id(), authorization.worker_id());
    assert_eq!(fact.worker_generation(), authorization.worker_generation());
    assert_eq!(fact.identity(), identity());
}

#[test]
fn raw_terminal_creates_without_membership_never_register() {
    let mut coordinator = site();
    let pending = dispatch(coordinator.prepare(create(50), Some(worker(1)), Some(expiry())));
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch expected")
    };
    let completion = Completion::Launch(
        TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity(),
                HostLaunchState::AwaitingStart,
            )),
        )
        .unwrap(),
    );
    coordinator
        .complete(pending, completion, Some(worker(1)))
        .unwrap();
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn every_identity_retaining_phase_remains_eligible() {
    let mut coordinator = site();
    let _authorization = complete_opened_member(&mut coordinator);
    for (phase, expected) in [
        (TerminalPhase::AwaitingStart(identity()), 1),
        (TerminalPhase::Starting(identity()), 1),
        (
            TerminalPhase::Started {
                identity: identity(),
                geometry: geometry(),
            },
            1,
        ),
        (TerminalPhase::Unknown(Some(identity())), 1),
        (TerminalPhase::Unavailable(Some(identity())), 1),
        (TerminalPhase::Unknown(None), 0),
        (TerminalPhase::Unavailable(None), 0),
    ] {
        // Member bindings are immediate-only today, so joint transitions cannot reach
        // these retained phases; the predicate is exercised through the test setter on
        // the real committed member and retained binding. `Creating` is covered by the
        // pending-create test; forcing it here would instead violate the outcome/phase
        // invariant because this member already carries a create outcome.
        coordinator.terminals.set_phase_for_test(&id(101), phase);
        let facts = coordinator.agent_registration_snapshot().unwrap();
        assert_eq!(facts.len(), expected, "phase {phase:?}");
        if expected == 1 {
            assert_eq!(facts[0].identity(), identity());
        }
    }
}

#[test]
fn removed_membership_disappears_from_the_snapshot() {
    let mut coordinator = site();
    let _authorization = complete_opened_member(&mut coordinator);
    assert_eq!(coordinator.agent_registration_snapshot().unwrap().len(), 1);
    let (context, tab, panel) = standalone_leaf(&mut coordinator);
    let (_close, _closed) = close_leaf(&mut coordinator, context, tab, panel, 900);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn coordinator_invariant_failures_abort_the_whole_snapshot() {
    for corrupt in [
        (|coordinator: &mut SiteCoordinator| {
            // A completed member forced back to Creating violates the outcome/phase
            // invariant; the snapshot must not silently drop or keep either member.
            coordinator
                .terminals
                .set_phase_for_test(&id(701), TerminalPhase::Creating);
        }) as fn(&mut SiteCoordinator),
        |coordinator: &mut SiteCoordinator| {
            // A second retained create sharing the member's identity pair is ambiguous.
            let authorization = coordinator
                .terminals
                .retained_authorization(&id(701))
                .unwrap();
            coordinator
                .terminals
                .replace_authorization_for_test(&id(900), authorization);
        },
    ] {
        let mut coordinator = site();
        let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
        let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
        let pending = dispatch_tab(&mut coordinator, command, 701);
        let completion = immediate_completion(pending.command());
        coordinator
            .complete_terminal_tab_create(pending, completion, Some(worker(1)))
            .unwrap();
        let _raw = dispatch(coordinator.prepare(create(900), Some(worker(1)), Some(expiry())));
        corrupt(&mut coordinator);
        assert_eq!(
            coordinator.agent_registration_snapshot(),
            Err(WorkbenchTerminalInvariantError)
        );
    }
}

#[test]
fn registration_facts_convert_for_a_consumer_shaped_reader() {
    fn consumer_view(
        facts: &[RegistrationFact],
    ) -> Vec<(
        TerminalPresentationId,
        CapabilityWorkerId,
        WorkerGeneration,
        HostTerminalIdentity,
    )> {
        facts
            .iter()
            .map(|fact| {
                (
                    fact.presentation_id().clone(),
                    fact.worker_id(),
                    fact.worker_generation(),
                    fact.identity(),
                )
            })
            .collect()
    }

    let mut coordinator = site();
    let authorization = complete_opened_member(&mut coordinator);
    let facts = coordinator.agent_registration_snapshot().unwrap();
    let view = consumer_view(&facts);
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].1, authorization.worker_id());
    assert_eq!(view[0].2, authorization.worker_generation());
    assert_eq!(view[0].3, identity());
}
