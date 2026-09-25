use super::*;
use crate::agent::AgentTransition;
use crate::agent_service::AgentPolicy;
use crate::local_client_service::agent_harness;
use orchbee_agent_observation_protocol::{AgentChangesCursor, AgentSafeInteger, ObapRequestId};
use std::cell::Cell;

fn request(cursor: Option<AgentChangesCursor>, limit: u64) -> AgentChangesRequest {
    AgentChangesRequest::Read {
        request_id: ObapRequestId::parse("11111111111111111111111111111111").unwrap(),
        cursor,
        limit: AgentSafeInteger::new(limit),
    }
}

fn read(service: &AgentService, cursor: Option<AgentChangesCursor>, limit: u64) -> Outcome {
    let AgentChangesResult::Read { result, .. } = service
        .execute_changes_prepared(request(cursor, limit), || {
            service
                .snapshot()
                .map_err(|_| PreparedUnavailable)?
                .ok_or(PreparedUnavailable)
        })
        .unwrap();
    result.outcome
}

fn checkpoint(service: &AgentService) -> AgentChangesCursor {
    let Outcome::Current { cursor, .. } = read(service, None, 50) else {
        panic!("checkpoint expected");
    };
    cursor
}

fn seed(service: &AgentService, index: usize) {
    assert!(matches!(
        service
            .seed(&agent_harness::session_observed(&format!("s{index}"), 1.0))
            .unwrap(),
        AgentTransition::Applied { .. }
    ));
}

#[test]
fn changes_deliver_every_revision_and_require_reset_after_retention() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: false,
    })
    .unwrap();
    let start = checkpoint(&service);
    for index in 0..3 {
        seed(&service, index);
    }
    let Outcome::Changes {
        changes,
        cursor,
        observed_through_revision,
        runtime_revision,
        has_more,
    } = read(&service, Some(start.clone()), 2)
    else {
        panic!("changes expected")
    };
    assert_eq!(
        changes
            .iter()
            .map(|c| c.revision.value())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(changes[0].session.session_id.as_str(), "s0");
    assert_eq!(
        (
            observed_through_revision.value(),
            runtime_revision.value(),
            has_more
        ),
        (2, 3, true)
    );
    let Outcome::Changes {
        changes,
        cursor,
        has_more,
        ..
    } = read(&service, Some(cursor), 2)
    else {
        panic!("tail expected")
    };
    assert_eq!(changes[0].revision.value(), 3);
    assert!(!has_more);
    assert!(
        matches!(read(&service, Some(cursor), 2), Outcome::Current { revision, .. } if revision.value() == 3)
    );
    for index in 3..512 {
        seed(&service, index);
    }
    assert!(matches!(
        service
            .seed(&agent_harness::turn_started(
                "s0", "t1", "thinking", 1.0, 2.0
            ))
            .unwrap(),
        AgentTransition::Applied { .. }
    ));
    assert!(
        matches!(read(&service, Some(start), 100), Outcome::ResetRequired { revision, .. } if revision.value() == 513)
    );
}

#[test]
fn changes_boot_boundary_and_preparation_admission_are_explicit() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: false,
    })
    .unwrap();
    let old = checkpoint(&service);
    let restarted = AgentService::open(AgentPolicy {
        read: true,
        manage: false,
    })
    .unwrap();
    assert_eq!(
        read(&restarted, Some(old), 50),
        Outcome::Rejected {
            reason: Rejection::CursorInvalid
        }
    );
    let denied = AgentService::open(AgentPolicy {
        read: false,
        manage: true,
    })
    .unwrap();
    let calls = Cell::new(0);
    let AgentChangesResult::Read { result, .. } = denied
        .execute_changes_prepared(request(None, 50), || {
            calls.set(calls.get() + 1);
            panic!("denied provider must not run");
        })
        .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(
        result.outcome,
        Outcome::Rejected {
            reason: Rejection::ReadDenied
        }
    );
    let AgentChangesResult::Read { result, .. } = service
        .execute_changes_prepared(request(None, 50), || {
            calls.set(calls.get() + 1);
            panic!("preparation failed");
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(result.outcome, unavailable());
    assert!(matches!(read(&service, None, 50), Outcome::Current { .. }));
    service.shutdown();
    assert_eq!(read(&service, None, 50), unavailable());
}

#[test]
fn changes_preparation_runs_outside_cell_and_returns_coherent_latest_journal() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: false,
    })
    .unwrap();
    let cursor = checkpoint(&service);
    let calls = Cell::new(0);
    let AgentChangesResult::Read { result, .. } = service
        .execute_changes_prepared(request(Some(cursor), 50), || {
            calls.set(calls.get() + 1);
            let earlier = service.snapshot().unwrap().unwrap();
            seed(&service, 0);
            Ok(earlier)
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(result.outcome, Outcome::Changes { runtime_revision, has_more: false, .. } if runtime_revision.value() == 1)
    );
    let poisoned: Result<(), _> = service.with_state(|_| panic!("poison"));
    assert!(poisoned.is_err());
    let AgentChangesResult::Read { result, .. } = service
        .execute_changes_prepared(request(None, 50), || Ok(AgentRuntimeSnapshot::create()))
        .unwrap();
    assert_eq!(result.outcome, unavailable());
}
