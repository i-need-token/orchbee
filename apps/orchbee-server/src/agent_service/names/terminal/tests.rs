use super::*;
use crate::{
    agent::native_observation::CarrierIdentity,
    local_client_service::agent_harness::{seeded, session_observed},
};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use std::num::NonZeroU64;

fn fact(index: u128) -> TerminalNameFact {
    TerminalNameFact {
        binding: Binding {
            presentation: TerminalPresentationId::parse(&format!("{index:032x}")).unwrap(),
            carrier: CarrierIdentity {
                worker_id: WorkerId::from_bytes(index.to_be_bytes()).unwrap(),
                generation: WorkerGeneration::new(NonZeroU64::MIN),
            },
            host: HostTerminalIdentity::new(
                HostCreationId::from_bytes([4; 16]).unwrap(),
                HostEpoch::from_bytes([2; 16]).unwrap(),
                HostTerminalId::from_bytes(index.to_be_bytes()).unwrap(),
            ),
        },
        owner: TerminalAgentNameOwner {
            agent: Some("codex".into()),
            reference: None,
            clear: false,
        },
    }
}
fn selector(fact: &TerminalNameFact) -> AgentSessionSelector {
    AgentSessionSelector::Presentation {
        presentation_id: launch_binding(&fact.binding).presentation_id,
    }
}
fn name(value: &str) -> AgentName {
    AgentName::parse(value).unwrap()
}
fn named(value: &str) -> AgentSessionSelector {
    AgentSessionSelector::Name { name: name(value) }
}
fn reference(value: &str) -> orchbee_agent_observation_protocol::AgentProviderSessionReference {
    orchbee_agent_observation_protocol::AgentProviderSessionReference::id(value).unwrap()
}

#[test]
fn terminal_names_rename_resolve_release_without_canonical_sessions() {
    let service = seeded(&[]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let facts = [fact(1)];
    let target = terminal(&facts[0].binding);
    assert_eq!(
        names.assign_named(&snapshot, &selector(&facts[0]), &name("alpha"), &facts),
        Assign::AssignedTarget {
            target: target.clone(),
            name: name("alpha"),
            previous_name: None
        }
    );
    assert_eq!(
        names.assign_named(&snapshot, &named("alpha"), &name("alpha"), &facts),
        Assign::UnchangedTarget {
            target: target.clone(),
            name: name("alpha")
        }
    );
    assert_eq!(
        names.assign_named(&snapshot, &named("alpha"), &name("beta"), &facts),
        Assign::AssignedTarget {
            target: target.clone(),
            name: name("beta"),
            previous_name: Some(name("alpha"))
        }
    );
    assert!(matches!(
        names.resolve_named(&snapshot, &named("alpha"), &facts),
        AgentResolveOutcome::Rejected { .. }
    ));
    assert_eq!(
        names.resolve_named(&snapshot, &named("beta"), &facts),
        AgentResolveOutcome::ResolvedTarget {
            target: target.clone(),
            resolved_by: AgentResolvedBy::Name
        }
    );
    assert_eq!(
        names.release_named(&snapshot, &named("beta"), &facts),
        Release::ReleasedTarget {
            target: target.clone(),
            name: name("beta")
        }
    );
    assert_eq!(
        names.release_named(&snapshot, &selector(&facts[0]), &facts),
        Release::UnchangedTarget { target }
    );
}

#[test]
fn shared_namespace_refuses_collisions_in_both_directions_and_preserves_id_precedence() {
    let service = seeded(&[session_observed("canonical", 1.0)]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let facts = [fact(1), fact(2)];
    let canonical = AgentSessionSelector::Session {
        session_id: AgentSessionId::parse("canonical").unwrap(),
    };
    names.assign_named(&snapshot, &canonical, &name("hook"), &facts);
    names.assign_named(&snapshot, &selector(&facts[0]), &name("canonical"), &facts);
    assert!(matches!(
        names.assign_named(&snapshot, &canonical, &name("canonical"), &facts),
        Assign::RejectedTarget {
            conflict_target: Some(AgentNameTarget::Terminal { .. }),
            ..
        }
    ));
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[1]), &name("hook"), &facts),
        Assign::RejectedTarget {
            conflict_target: Some(AgentNameTarget::Session { .. }),
            ..
        }
    ));
    assert!(matches!(
        names.resolve_named(
            &snapshot,
            &AgentSessionSelector::Auto {
                target: AgentSessionId::parse("canonical").unwrap()
            },
            &facts
        ),
        AgentResolveOutcome::Resolved { .. }
    ));
    assert!(matches!(
        names.resolve_named(&snapshot, &named("canonical"), &facts),
        AgentResolveOutcome::ResolvedTarget { .. }
    ));
    let AgentNamesListOutcome::ListedTargets { bindings, .. } = names.names_list() else {
        panic!("mixed list")
    };
    assert_eq!(
        bindings.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
        vec!["canonical", "hook"]
    );
}

#[test]
fn unknown_observation_and_first_reference_preserve_but_reference_replacement_clears() {
    let service = seeded(&[]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let mut observed = fact(1);
    names.assign_named(
        &snapshot,
        &selector(&observed),
        &name("alpha"),
        std::slice::from_ref(&observed),
    );
    let mut unknown = observed.clone();
    unknown.owner.agent = None;
    names.observe_terminal_owner(&unknown);
    assert!(names.terminal_claim("alpha").is_some());
    observed.owner.reference = Some(reference("first"));
    names.observe_terminal_owner(&observed);
    names.observe_terminal_owner(&unknown);
    assert!(names.terminal_claim("alpha").is_some());
    observed.owner.reference = Some(reference("second"));
    names.observe_terminal_owner(&observed);
    assert!(names.terminal_claim("alpha").is_none());
}

#[test]
fn intermediate_provider_change_release_and_physical_replacement_cannot_resurrect_names() {
    let service = seeded(&[]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    for transition in 0..4 {
        let mut names = Names::default();
        let first = fact(1);
        let mut next = first.clone();
        names.assign_named(
            &snapshot,
            &selector(&first),
            &name("alpha"),
            std::slice::from_ref(&first),
        );
        match transition {
            0 => next.owner.agent = Some("claude".into()),
            1 => next.owner.clear = true,
            2 => {
                next.binding.host = HostTerminalIdentity::new(
                    HostCreationId::from_bytes([5; 16]).unwrap(),
                    next.binding.host.host_epoch(),
                    next.binding.host.terminal_id(),
                )
            }
            _ => {
                next.binding.carrier.generation = WorkerGeneration::new(NonZeroU64::new(2).unwrap())
            }
        }
        names.observe_terminal_owner(&next);
        names.observe_terminal_owner(&first);
        assert!(
            names.terminal_claim("alpha").is_none(),
            "transition {transition}"
        );
    }
}

#[test]
fn no_provider_and_stale_exact_binding_cannot_be_named_and_removed_membership_reclaims_capacity() {
    let service = seeded(&[]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let mut observed = fact(1);
    observed.owner.agent = None;
    assert!(matches!(
        names.assign_named(
            &snapshot,
            &selector(&observed),
            &name("alpha"),
            std::slice::from_ref(&observed)
        ),
        Assign::Rejected {
            reason: AssignReason::SessionNotFound,
            ..
        }
    ));
    observed.owner.agent = Some("custom".into());
    let mut stale = launch_binding(&observed.binding);
    stale.generation =
        orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"a".repeat(32))
            .unwrap();
    assert!(matches!(
        names.assign_named(
            &snapshot,
            &AgentSessionSelector::Terminal { binding: stale },
            &name("alpha"),
            std::slice::from_ref(&observed)
        ),
        Assign::Rejected { .. }
    ));
    names.assign_named(&snapshot, &selector(&observed), &name("alpha"), &[observed]);
    names.reconcile_terminals(&[]);
    assert_eq!(names.total_bindings(), 0);
}

#[test]
fn terminal_capacity_is_shared_with_canonical_names_and_renaming_at_capacity_is_allowed() {
    let service = seeded(&[session_observed("canonical", 1.0)]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let facts: Vec<_> = (1..=512).map(fact).collect();
    names.assign_named(
        &snapshot,
        &AgentSessionSelector::Session {
            session_id: AgentSessionId::parse("canonical").unwrap(),
        },
        &name("hook"),
        &facts,
    );
    for (i, fact) in facts.iter().take(511).enumerate() {
        assert!(matches!(
            names.assign_named(
                &snapshot,
                &selector(fact),
                &name(&format!("name{i:04}")),
                &facts
            ),
            Assign::AssignedTarget { .. }
        ));
    }
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[511]), &name("overflow"), &facts),
        Assign::Rejected {
            reason: AssignReason::CapacityExceeded,
            ..
        }
    ));
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[0]), &name("renamed"), &facts),
        Assign::AssignedTarget {
            previous_name: Some(_),
            ..
        }
    ));
    assert_eq!(names.total_bindings(), 512);
}

#[test]
fn pending_launch_and_terminal_name_cannot_claim_each_others_name_or_physical_target() {
    use orchbee_agent_observation_protocol::{
        AgentLaunchOutcome, AgentLaunchRequest, AgentProviderId, AgentSafeInteger, ObapRequestId,
    };
    let service = seeded(&[]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let facts = [fact(1), fact(2)];
    let binding = launch_binding(&facts[0].binding);
    let reserve = AgentLaunchRequest::Reserve {
        request_id: ObapRequestId::parse(&"a".repeat(32)).unwrap(),
        name: name("pending"),
        provider_id: AgentProviderId::parse("codex").unwrap(),
        presentation_id: binding.presentation_id,
        host_epoch: binding.host_epoch,
        host_terminal_id: binding.host_terminal_id,
        ttl_ms: AgentSafeInteger::new(10000),
    };
    let physical = facts.iter().map(|f| f.binding.clone()).collect::<Vec<_>>();
    assert!(matches!(
        names.execute_launch(&reserve, &snapshot, &physical, 0),
        AgentLaunchOutcome::Reserved { .. }
    ));
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[0]), &name("alpha"), &facts),
        Assign::Rejected {
            reason: AssignReason::LaunchPending,
            ..
        }
    ));
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[1]), &name("pending"), &facts),
        Assign::Rejected {
            reason: AssignReason::LaunchPending,
            ..
        }
    ));
    names.assign_named(&snapshot, &selector(&facts[1]), &name("alpha"), &facts);
    let mut next = reserve.clone();
    if let AgentLaunchRequest::Reserve { name, .. } = &mut next {
        *name = AgentName::parse("alpha").unwrap();
    }
    assert!(matches!(
        names.execute_launch(&next, &snapshot, &physical, 0),
        AgentLaunchOutcome::Rejected {
            reason: orchbee_agent_observation_protocol::AgentLaunchRejectionReason::NameInUse
        }
    ));
}

#[test]
fn physical_name_is_reclaimed_before_launch_without_a_names_read() {
    use orchbee_agent_observation_protocol::*;
    let service = seeded(&[]).unwrap();
    let snapshot = service.snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let old = fact(1);
    let new = fact(2);
    names.assign_named(
        &snapshot,
        &selector(&old),
        &name("reuse"),
        std::slice::from_ref(&old),
    );
    let binding = launch_binding(&new.binding);
    let reserve = AgentLaunchRequest::Reserve {
        request_id: ObapRequestId::parse(&"a".repeat(32)).unwrap(),
        name: name("reuse"),
        provider_id: AgentProviderId::parse("codex").unwrap(),
        presentation_id: binding.presentation_id,
        host_epoch: binding.host_epoch,
        host_terminal_id: binding.host_terminal_id,
        ttl_ms: AgentSafeInteger::new(10000),
    };
    assert!(matches!(
        names.execute_launch(&reserve, &snapshot, &[new.binding], 0),
        AgentLaunchOutcome::Reserved { .. }
    ));
    assert!(names.terminal_claim("reuse").is_none());
}

fn managed_seed(
    fact: &TerminalNameFact,
    label: &str,
) -> (
    orchbee_server::ManagedAgentRestart,
    orchbee_workbench_protocol::PanelAgentSession,
) {
    (orchbee_server::ManagedAgentRestart { presentation_id: fact.binding.presentation.clone(), name: name(label), provider_id: orchbee_agent_observation_protocol::AgentProviderId::parse("codex").unwrap() },
        serde_json::from_value(serde_json::json!({"source":"herdr:codex","agent":"codex","reference":{"kind":"id","value":"saved"}})).unwrap())
}
#[test]
fn restored_management_survives_reference_change_and_rename_but_not_provider_or_generation_change()
{
    use orchbee_agent_observation_protocol::{
        AgentLaunchOutcome, AgentLaunchRequest, AgentTerminalManagement, ObapRequestId,
    };
    let snapshot = seeded(&[]).unwrap().snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let mut observed = fact(1);
    observed.owner.reference = Some(reference("saved"));
    let seed = managed_seed(&observed, "restored");
    names
        .restore_managed(std::slice::from_ref(&seed), std::slice::from_ref(&observed))
        .unwrap();
    observed.owner.reference = Some(reference("next"));
    names.observe_terminal_owner(&observed);
    assert!(matches!(
        names.assign_named(
            &snapshot,
            &named("restored"),
            &name("renamed"),
            std::slice::from_ref(&observed)
        ),
        Assign::AssignedTarget { .. }
    ));
    let claims = names.managed_claims();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].name, name("renamed"));
    assert_eq!(
        claims[0].owned_session.as_ref().unwrap().reference.value,
        "saved"
    );
    let binding = launch_binding(&observed.binding);
    let request = AgentLaunchRequest::TerminalStatus {
        request_id: ObapRequestId::parse(&"1".repeat(32)).unwrap(),
        binding: binding.clone(),
    };
    assert!(matches!(
        names.execute_launch(&request, &snapshot, &[observed.binding.clone()], 0),
        AgentLaunchOutcome::TerminalStatus {
            management: AgentTerminalManagement::Active { .. },
            ..
        }
    ));
    let mut stale = binding;
    stale.generation =
        orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"9".repeat(32))
            .unwrap();
    let stale_request = AgentLaunchRequest::TerminalStatus {
        request_id: request.request_id().clone(),
        binding: stale,
    };
    assert!(matches!(
        names.execute_launch(&stale_request, &snapshot, &[observed.binding.clone()], 0),
        AgentLaunchOutcome::Rejected {
            reason: orchbee_agent_observation_protocol::AgentLaunchRejectionReason::BindingChanged
        }
    ));
    observed.owner.agent = Some("claude".into());
    names.observe_terminal_owner(&observed);
    assert!(names.managed_claims().is_empty());
    observed.owner.agent = Some("codex".into());
    names
        .restore_managed(&[seed], std::slice::from_ref(&observed))
        .unwrap();
    let mut replacement = observed.binding;
    replacement.host = HostTerminalIdentity::new(
        HostCreationId::from_bytes([9; 16]).unwrap(),
        replacement.host.host_epoch(),
        replacement.host.terminal_id(),
    );
    names.reconcile_terminal_bindings(&[replacement], false);
    assert!(names.managed_claims().is_empty());
}
#[test]
fn restored_names_validate_the_whole_namespace_and_release_removes_management() {
    let snapshot = seeded(&[]).unwrap().snapshot().unwrap().unwrap();
    let mut names = Names::default();
    let facts = [fact(1), fact(2)];
    let seeds = [
        managed_seed(&facts[0], "same"),
        managed_seed(&facts[1], "same"),
    ];
    assert!(names.restore_managed(&seeds, &facts).is_err());
    assert_eq!(names.total_bindings(), 0);
    names.restore_managed(&seeds[..1], &facts).unwrap();
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[1]), &name("same"), &facts),
        Assign::RejectedTarget { .. }
    ));
    assert!(matches!(
        names.release_named(&snapshot, &named("same"), &facts),
        Release::ReleasedTarget { .. }
    ));
    assert!(names.managed_claims().is_empty());
    assert!(matches!(
        names.assign_named(&snapshot, &selector(&facts[0]), &name("manual"), &facts),
        Assign::AssignedTarget { .. }
    ));
    assert!(names.managed_claims().is_empty());
}
