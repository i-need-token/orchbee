use super::*;
use crate::agent::native_observation::{Binding, CarrierIdentity};
use crate::agent_service::{AgentPolicy, DrainExpectation};
use orchbee_agent_observation_protocol::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use std::{num::NonZeroU64, sync::Barrier, time::Duration};

fn request(seed: char) -> AgentLaunchRequest {
    AgentLaunchRequest::Reserve {
        request_id: ObapRequestId::parse(&seed.to_string().repeat(32)).unwrap(),
        name: AgentName::parse("alpha").unwrap(),
        provider_id: AgentProviderId::parse("codex").unwrap(),
        presentation_id: AgentPresentationId::parse(&"1".repeat(32)).unwrap(),
        host_epoch: AgentHostEpoch::parse(&"2".repeat(32)).unwrap(),
        host_terminal_id: AgentHostTerminalId::parse(&"3".repeat(32)).unwrap(),
        ttl_ms: AgentSafeInteger::new(10000),
    }
}

#[test]
fn launch_policy_rejects_before_preparation_and_failed_preparation_never_reserves() {
    for (request, read, manage, reason) in [
        (
            AgentLaunchRequest::Status {
                request_id: request('4').request_id().clone(),
                session_id: AgentSessionId::parse("s1").unwrap(),
            },
            false,
            true,
            AgentLaunchRejectionReason::ReadDenied,
        ),
        (
            request('4'),
            true,
            false,
            AgentLaunchRejectionReason::ManageDenied,
        ),
        (
            AgentLaunchRequest::Inspect {
                request_id: request('4').request_id().clone(),
                reservation_id: request('5').request_id().clone(),
            },
            false,
            true,
            AgentLaunchRejectionReason::ReadDenied,
        ),
    ] {
        let service = AgentService::open(AgentPolicy { read, manage }).unwrap();
        let result = service
            .execute_launch_prepared(request, || panic!("denied capability must not prepare"));
        assert_eq!(result.outcome(), &AgentLaunchOutcome::Rejected { reason });
        assert!(service.next_deadline().unwrap().is_none());
    }
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let result = service.execute_launch_prepared(request('4'), || Err(PreparedUnavailable));
    assert!(matches!(
        result.outcome(),
        AgentLaunchOutcome::Unavailable { .. }
    ));
    assert!(service.next_deadline().unwrap().is_none());
}

#[test]
fn competing_launches_commit_one_claim_and_service_timer_releases_it() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let binding = Binding {
        presentation: TerminalPresentationId::parse(&"1".repeat(32)).unwrap(),
        carrier: CarrierIdentity {
            worker_id: WorkerId::from_bytes([1; 16]).unwrap(),
            generation: WorkerGeneration::new(NonZeroU64::MIN),
        },
        host: HostTerminalIdentity::new(
            HostCreationId::from_bytes([0x44; 16]).unwrap(),
            HostEpoch::from_bytes([0x22; 16]).unwrap(),
            HostTerminalId::from_bytes([0x33; 16]).unwrap(),
        ),
    };
    service
        .native_admit_carrier(binding.carrier, Instant::now())
        .unwrap()
        .unwrap();
    service
        .native_fenced_final_snapshot(
            DrainExpectation::Carrier {
                req_carrier: binding.carrier,
                barrier: 0,
            },
            vec![binding],
            Instant::now(),
        )
        .unwrap()
        .unwrap()
        .unwrap();
    let barrier = Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let jobs: Vec<_> = ['5', '6']
            .into_iter()
            .map(|seed| {
                let service = &service;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    service.execute_launch_prepared(request(seed), || {
                        service
                            .snapshot()
                            .map_err(|_| PreparedUnavailable)?
                            .ok_or(PreparedUnavailable)
                    })
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result.outcome(), AgentLaunchOutcome::Reserved { .. }))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result.outcome(),
                AgentLaunchOutcome::Rejected {
                    reason: AgentLaunchRejectionReason::NameInUse
                }
            ))
            .count(),
        1
    );
    let deadline = service.next_deadline().unwrap().unwrap();
    assert!(deadline <= Instant::now() + Duration::from_secs(10));
    service.tick(deadline).unwrap();
    assert!(service.next_deadline().unwrap().is_none());
    let listed = service.execute_names_prepared(
        AgentNamesRequest::NamesList {
            request_id: request('7').request_id().clone(),
        },
        || {
            service
                .snapshot()
                .map_err(|_| PreparedUnavailable)?
                .ok_or(PreparedUnavailable)
        },
    );
    assert!(
        matches!(listed, AgentNamesResult::NamesList { result:AgentNamesListOutcomeEnvelope { outcome:AgentNamesListOutcome::Listed { pending, .. } }, .. } if pending.is_empty())
    );
}

#[test]
fn terminal_management_policy_precedes_guarded_preparation_and_unavailable_never_becomes_unmanaged()
{
    let binding = AgentLaunchBinding {
        presentation_id: AgentPresentationId::parse(&"1".repeat(32)).unwrap(),
        generation: AgentAttachmentGeneration::parse(&"2".repeat(32)).unwrap(),
        host_epoch: AgentHostEpoch::parse(&"3".repeat(32)).unwrap(),
        host_terminal_id: AgentHostTerminalId::parse(&"4".repeat(32)).unwrap(),
    };
    let request = AgentLaunchRequest::TerminalStatus {
        request_id: ObapRequestId::parse(&"5".repeat(32)).unwrap(),
        binding,
    };
    let service = AgentService::open(AgentPolicy {
        read: false,
        manage: true,
    })
    .unwrap();
    assert!(matches!(
        service
            .execute_launch_guarded(request.clone(), |_| panic!("denied before preparation"))
            .outcome(),
        AgentLaunchOutcome::Rejected {
            reason: AgentLaunchRejectionReason::ReadDenied
        }
    ));
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: false,
    })
    .unwrap();
    assert!(matches!(
        service
            .execute_launch_guarded(request.clone(), |_| Err(PreparedUnavailable))
            .outcome(),
        AgentLaunchOutcome::Unavailable { .. }
    ));
    assert!(matches!(
        service
            .execute_launch_guarded(request, |request| service.fold_launch(request))
            .outcome(),
        AgentLaunchOutcome::Rejected {
            reason: AgentLaunchRejectionReason::BindingChanged
        }
    ));
}
