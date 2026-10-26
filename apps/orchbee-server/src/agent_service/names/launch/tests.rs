use super::*;
use crate::agent::native_observation::CarrierIdentity;
use crate::agent::{
    AgentPresentationAttachment, AgentPresentationVisibility, AgentTurnState, OpaqueId,
    PresentationId,
};
use crate::local_client_service::tests::agent_harness::{seeded, session_observed};
use orchbee_agent_observation_protocol::{
    AgentNameAssignOutcome, AgentNameAssignRejectionReason, AgentSessionSelector,
};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use std::num::NonZeroU64;

fn id(seed: u8) -> ObapRequestId {
    ObapRequestId::parse(&canonical_hex32([seed; 16])).unwrap()
}

fn binding(seed: u8) -> Binding {
    Binding {
        presentation: TerminalPresentationId::parse(&canonical_hex32([seed; 16])).unwrap(),
        carrier: CarrierIdentity {
            worker_id: WorkerId::from_bytes([1; 16]).unwrap(),
            generation: WorkerGeneration::new(NonZeroU64::MIN),
        },
        host: HostTerminalIdentity::new(
            HostCreationId::from_bytes([seed; 16]).unwrap(),
            HostEpoch::from_bytes([seed; 16]).unwrap(),
            HostTerminalId::from_bytes([seed; 16]).unwrap(),
        ),
    }
}

fn snapshot(binding: &Binding) -> AgentRuntimeSnapshot {
    let service = seeded(&[session_observed("s1", 1.0)]).unwrap();
    let mut snapshot = service.snapshot().unwrap().unwrap();
    let session = &mut snapshot.sessions[0];
    session.turn = AgentTurnState::Idle;
    session.provider_id = crate::agent::AgentProviderId::new("codex").unwrap();
    session.presentation_attachments = vec![AgentPresentationAttachment {
        generation: OpaqueId::new(&canonical_hex32(binding.host.creation_id().as_bytes())).unwrap(),
        presentation_id: PresentationId::new(binding.presentation.as_str()).unwrap(),
        navigable: true,
        visibility: AgentPresentationVisibility::Visible,
    }];
    snapshot
}

fn reserve(seed: u8, name: &str, binding: &Binding, ttl: u64) -> AgentLaunchRequest {
    AgentLaunchRequest::Reserve {
        request_id: id(seed),
        name: AgentName::parse(name).unwrap(),
        provider_id: AgentProviderId::parse("codex").unwrap(),
        presentation_id: orchbee_agent_observation_protocol::AgentPresentationId::parse(
            binding.presentation.as_str(),
        )
        .unwrap(),
        host_epoch: AgentHostEpoch::parse(&canonical_hex32(binding.host.host_epoch().as_bytes()))
            .unwrap(),
        host_terminal_id: AgentHostTerminalId::parse(&canonical_hex32(
            binding.host.terminal_id().as_bytes(),
        ))
        .unwrap(),
        ttl_ms: AgentSafeInteger::new(ttl),
    }
}

fn inspect(seed: u8) -> AgentLaunchRequest {
    AgentLaunchRequest::Inspect {
        request_id: id(90),
        reservation_id: id(seed),
    }
}
fn complete(seed: u8) -> AgentLaunchRequest {
    AgentLaunchRequest::Complete {
        request_id: id(91),
        reservation_id: id(seed),
        session_id: AgentSessionId::parse("s1").unwrap(),
    }
}

#[test]
fn reservation_excludes_stale_sessions_competing_claims_and_alias_interference_until_atomic_commit()
{
    let bindings = [binding(1), binding(2)];
    let mut snapshot = snapshot(&bindings[0]);
    let mut names = Names::default();
    assert!(matches!(
        names.execute_launch(
            &reserve(10, "alpha", &bindings[0], 10000),
            &snapshot,
            &bindings,
            100
        ),
        Outcome::Reserved { .. }
    ));
    assert_eq!(
        names.execute_launch(
            &reserve(11, "alpha", &bindings[1], 10000),
            &snapshot,
            &bindings,
            100
        ),
        reject(Rejection::NameInUse)
    );
    assert_eq!(
        names.execute_launch(
            &reserve(11, "beta", &bindings[0], 10000),
            &snapshot,
            &bindings,
            100
        ),
        reject(Rejection::PresentationBusy)
    );
    assert!(matches!(
        names.execute_launch(&inspect(10), &snapshot, &bindings, 100),
        Outcome::Pending {
            candidate_session_id: None,
            ..
        }
    ));
    let target = AgentSessionSelector::Session {
        session_id: AgentSessionId::parse("s1").unwrap(),
    };
    assert_eq!(
        names.assign(&snapshot, &target, &AgentName::parse("other").unwrap()),
        AgentNameAssignOutcome::Rejected {
            reason: AgentNameAssignRejectionReason::LaunchPending,
            conflict_session_id: None
        }
    );
    snapshot.sessions[0].observation.sequence += 1;
    assert_eq!(
        names.execute_launch(&complete(10), &snapshot, &bindings, 3099),
        reject(Rejection::NotReady)
    );
    assert!(matches!(
        names.execute_launch(&complete(10), &snapshot, &bindings, 3100),
        Outcome::Completed { .. }
    ));
    assert!(names.pending_names().is_empty());
    assert_eq!(names.by_name.get("alpha").unwrap().as_str(), "s1");
    assert_eq!(names.total_bindings(), 1);
    assert_eq!(
        names.execute_launch(&inspect(10), &snapshot, &bindings, 3100),
        reject(Rejection::ReservationNotFound)
    );
}

#[test]
fn expiry_replacement_and_old_cancellation_cannot_leak_or_release_a_new_claim() {
    let mut bindings = [binding(1)];
    let snapshot = snapshot(&bindings[0]);
    let mut names = Names::default();
    names.execute_launch(
        &reserve(10, "alpha", &bindings[0], 1000),
        &snapshot,
        &bindings,
        10,
    );
    assert_eq!(names.next_launch_deadline_ms(), Some(1010));
    assert!(matches!(
        names.execute_launch(
            &reserve(11, "alpha", &bindings[0], 1000),
            &snapshot,
            &bindings,
            1010
        ),
        Outcome::Reserved { .. }
    ));
    let cancel = AgentLaunchRequest::Cancel {
        request_id: id(90),
        reservation_id: id(10),
    };
    assert_eq!(
        names.execute_launch(&cancel, &snapshot, &bindings, 1011),
        reject(Rejection::ReservationNotFound)
    );
    assert_eq!(names.pending_names()[0].reservation_id, id(11));
    // A clock sample taken before an earlier contender acquired the lock
    // cannot move service time backwards or renew the reservation.
    names.reconcile_launches(&bindings, 1000, false);
    assert_eq!(names.pending_names()[0].remaining_ms.value(), 999);
    bindings[0].host = binding(2).host;
    names.reconcile_launches(&bindings, 1012, false);
    assert!(names.pending_names().is_empty());
    assert_eq!(names.next_launch_deadline_ms(), None);
    names.execute_launch(
        &reserve(12, "alpha", &bindings[0], 1000),
        &snapshot,
        &bindings,
        1012,
    );
    names.reconcile_launches(&bindings, 1013, true);
    assert!(names.pending_names().is_empty());
}

#[test]
fn candidate_ambiguity_staleness_and_attention_do_not_manufacture_readiness() {
    let bindings = [binding(1)];
    let mut snapshot = snapshot(&bindings[0]);
    let mut names = Names::default();
    names.execute_launch(
        &reserve(10, "alpha", &bindings[0], 10000),
        &snapshot,
        &bindings,
        0,
    );
    snapshot.sessions[0].observation.sequence += 1;
    snapshot.sessions[0].observation.freshness = AgentFreshness::Stale;
    assert_eq!(
        names.execute_launch(&complete(10), &snapshot, &bindings, 3000),
        reject(Rejection::NotReady)
    );
    snapshot.sessions[0].observation.freshness = AgentFreshness::Fresh;
    snapshot.sessions[0].turn = AgentTurnState::Unknown;
    assert_eq!(
        names.execute_launch(&complete(10), &snapshot, &bindings, 3000),
        reject(Rejection::NotReady)
    );
    snapshot.sessions[0].turn = AgentTurnState::Idle;
    snapshot.sessions[0]
        .pending_requests
        .push(crate::agent::AgentAttentionRequest {
            request_id: OpaqueId::new("approval").unwrap(),
            kind: crate::agent::AgentAttentionKind::Approval,
            turn_id: None,
        });
    assert_eq!(
        names.execute_launch(&complete(10), &snapshot, &bindings, 3000),
        reject(Rejection::NotReady)
    );
    snapshot.sessions[0].pending_requests.clear();
    let mut other = snapshot.sessions[0].clone();
    other.session_id = OpaqueId::new("s2").unwrap();
    snapshot.sessions.push(other);
    assert_eq!(
        names.execute_launch(&inspect(10), &snapshot, &bindings, 3000),
        reject(Rejection::CandidateAmbiguous)
    );
    snapshot.sessions[1].parent_session_id = Some(OpaqueId::new("s1").unwrap());
    assert!(matches!(
        names.execute_launch(&inspect(10), &snapshot, &bindings, 3000),
        Outcome::Pending {
            candidate_session_id: Some(_),
            ..
        }
    ));
}

#[test]
fn managed_readiness_requires_completed_launch_and_dies_with_its_ownership() {
    use orchbee_agent_observation_protocol::AgentManagement;
    for cause in ["clear", "replace", "provider", "ended", "detach"] {
        let mut bindings = [binding(1)];
        let mut snapshot = snapshot(&bindings[0]);
        let mut names = Names::default();
        let target = AgentSessionSelector::Session {
            session_id: AgentSessionId::parse("s1").unwrap(),
        };
        let status = AgentLaunchRequest::Status {
            request_id: id(90),
            session_id: AgentSessionId::parse("s1").unwrap(),
        };
        names.assign(&snapshot, &target, &AgentName::parse("manual").unwrap());
        assert!(matches!(
            names.execute_launch(&status, &snapshot, &bindings, 0),
            Outcome::Status {
                management: AgentManagement::Unmanaged {},
                ..
            }
        ));
        names.by_name.clear();
        names.by_session.clear();
        names.execute_launch(
            &reserve(10, "alpha", &bindings[0], 10000),
            &snapshot,
            &bindings,
            0,
        );
        assert!(matches!(
            names.execute_launch(&status, &snapshot, &bindings, 1),
            Outcome::Status {
                management: AgentManagement::Pending { blocked: false, .. },
                ..
            }
        ));
        snapshot.sessions[0].observation.sequence += 1;
        assert!(matches!(
            names.execute_launch(&complete(10), &snapshot, &bindings, 3000),
            Outcome::Completed { .. }
        ));
        names.assign(&snapshot, &target, &AgentName::parse("renamed").unwrap());
        assert!(matches!(
            names.execute_launch(&status, &snapshot, &bindings, 4000),
            Outcome::Status {
                management: AgentManagement::Active { .. },
                ..
            }
        ));
        match cause {
            "clear" => {
                names.by_name.clear();
                names.by_session.clear();
            }
            "replace" => bindings[0] = binding(2),
            "provider" => {
                snapshot.sessions[0].provider_id =
                    crate::agent::AgentProviderId::new("other").unwrap()
            }
            "ended" => snapshot.sessions[0].lifecycle = AgentLifecycle::Ended,
            "detach" => snapshot.sessions[0].presentation_attachments.clear(),
            _ => unreachable!(),
        }
        assert!(matches!(
            names.execute_launch(&status, &snapshot, &bindings, 4001),
            Outcome::Status {
                management: AgentManagement::Unmanaged {},
                ..
            }
        ));
        assert!(names.launches.active.is_empty());
        assert!(names.launches.active.is_empty());
    }
}
