//! Service-level coverage of the first-read: policy admission, canonical
//! ordering, pagination through verified boot-scoped cursors, loud rejection
//! and unavailability paths, and canonical-to-wire mapping fidelity. Seeds
//! arrive through real canonical event dispatch via the shared
//! `agent_harness` builders — never through a production seed mechanism.

use orchbee_agent_observation_protocol::{
    AgentActivity, AgentAttentionKind, AgentCapability, AgentExplainOutcome, AgentFreshnessMeaning,
    AgentGetOutcome, AgentLifecycle, AgentListCursor, AgentListOutcome, AgentListRejectionReason,
    AgentObservationFidelity, AgentObservationFreshness, AgentObservationSource,
    AgentPositiveInteger, AgentReadRequest, AgentReadResult, AgentSafeInteger, AgentSemanticStatus,
    AgentSessionId, AgentSessionRejectionReason, AgentStatusBasis, AgentTurnPhase, AgentTurnState,
    AgentUnavailableReason, ContentUnavailable, NavigateOnly, ObapRequestId,
};

use std::sync::Mutex;

use super::{AgentFoldUnavailable, AgentPolicy, AgentReadFault, AgentService, PreparedUnavailable};
use crate::local_client_service::tests::agent_harness;

fn request_id(seed: u8) -> ObapRequestId {
    ObapRequestId::parse(&format!("{seed:032x}")).unwrap()
}

fn session_id(value: &str) -> AgentSessionId {
    AgentSessionId::parse(value).unwrap()
}

fn list_request(
    seed: u8,
    include_ended: bool,
    limit: u64,
    cursor: Option<AgentListCursor>,
) -> AgentReadRequest {
    AgentReadRequest::List {
        request_id: request_id(seed),
        include_ended,
        limit: AgentSafeInteger::new(limit),
        cursor,
    }
}

fn get_request(seed: u8, session: &str) -> AgentReadRequest {
    AgentReadRequest::Get {
        request_id: request_id(seed),
        session_id: session_id(session),
    }
}

fn explain_request(seed: u8, session: &str) -> AgentReadRequest {
    AgentReadRequest::Explain {
        request_id: request_id(seed),
        session_id: session_id(session),
    }
}

fn listed(result: AgentReadResult) -> (Vec<String>, Option<AgentListCursor>, u64) {
    let AgentReadResult::List { result, .. } = result else {
        panic!("expected a list result");
    };
    let AgentListOutcome::Listed {
        items,
        next_cursor,
        revision,
    } = result.outcome
    else {
        panic!("expected a listed outcome");
    };
    (
        items
            .iter()
            .map(|item| item.session_id.as_str().to_owned())
            .collect(),
        next_cursor,
        revision.value(),
    )
}

#[test]
fn empty_runtime_lists_nothing_and_rejects_unknown_sessions() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let (items, next_cursor, revision) =
        listed(service.execute(list_request(1, false, 50, None)).unwrap());
    assert!(items.is_empty());
    assert!(next_cursor.is_none());
    assert_eq!(revision, 0);

    let AgentReadResult::Get {
        request_id: echoed,
        result,
    } = service.execute(get_request(2, "missing.session")).unwrap()
    else {
        panic!("expected a get result");
    };
    assert_eq!(echoed, request_id(2));
    assert_eq!(
        result.outcome,
        AgentGetOutcome::Rejected {
            reason: AgentSessionRejectionReason::NotFound,
        }
    );

    let AgentReadResult::Explain {
        request_id: echoed,
        result,
    } = service
        .execute(explain_request(3, "missing.session"))
        .unwrap()
    else {
        panic!("expected an explain result");
    };
    assert_eq!(echoed, request_id(3));
    assert_eq!(
        result.outcome,
        AgentExplainOutcome::Rejected {
            reason: AgentSessionRejectionReason::NotFound,
        }
    );
}

#[test]
fn denied_policy_rejects_every_operation() {
    let service = AgentService::open(AgentPolicy {
        read: false,
        manage: false,
    })
    .unwrap();
    let AgentReadResult::List { result, .. } =
        service.execute(list_request(1, false, 50, None)).unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Rejected {
            reason: AgentListRejectionReason::ReadDenied,
        }
    );
    let AgentReadResult::Get { result, .. } = service.execute(get_request(2, "s.a")).unwrap()
    else {
        panic!("expected a get result");
    };
    assert_eq!(
        result.outcome,
        AgentGetOutcome::Rejected {
            reason: AgentSessionRejectionReason::ReadDenied,
        }
    );
    let AgentReadResult::Explain { result, .. } =
        service.execute(explain_request(3, "s.a")).unwrap()
    else {
        panic!("expected an explain result");
    };
    assert_eq!(
        result.outcome,
        AgentExplainOutcome::Rejected {
            reason: AgentSessionRejectionReason::ReadDenied,
        }
    );
}

#[test]
fn lists_seeded_sessions_in_utf16_order_with_mapped_fields() {
    let service = agent_harness::seeded(&[
        agent_harness::session_observed("b.session", 1.0),
        agent_harness::session_observed("a.session", 1.0),
    ])
    .unwrap();
    let AgentReadResult::List {
        request_id: echoed,
        result,
    } = service.execute(list_request(1, false, 50, None)).unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(echoed, request_id(1));
    let AgentListOutcome::Listed {
        items,
        next_cursor,
        revision,
    } = result.outcome
    else {
        panic!("expected a listed outcome");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].session_id.as_str(), "a.session");
    assert_eq!(items[1].session_id.as_str(), "b.session");
    assert!(next_cursor.is_none());
    assert_eq!(revision.value(), 2);
    let first = &items[0];
    assert_eq!(first.provider_id.as_str(), "seed-provider");
    assert_eq!(
        first.execution_location.execution_location_id.as_str(),
        "loc-a.session"
    );
    assert_eq!(first.lifecycle, AgentLifecycle::Live);
    assert_eq!(first.pending_request_count.value(), 0);
    assert_eq!(first.observation.fidelity, AgentObservationFidelity::Rich);
    assert_eq!(
        first.observation.freshness,
        AgentObservationFreshness::Fresh
    );
    assert_eq!(first.observation.source, AgentObservationSource::Native);
    assert_eq!(first.observation.generation.value(), 1);
    assert!(first.presentation_attachments.is_empty());
    assert!(first.parent_session_id.is_none());
    assert_eq!(
        first.status,
        AgentSemanticStatus::Unknown {
            activity_hint: None,
        }
    );
}

#[test]
fn get_and_explain_map_canonical_detail_and_explanation() {
    let service = agent_harness::seeded(&[agent_harness::session_observed("sess.a", 7.0)]).unwrap();
    let AgentReadResult::Get { result, .. } = service.execute(get_request(1, "sess.a")).unwrap()
    else {
        panic!("expected a get result");
    };
    let AgentGetOutcome::Found { revision, session } = result.outcome else {
        panic!("expected a found outcome");
    };
    assert_eq!(revision.value(), 1);
    assert_eq!(session.snapshot.session_id.as_str(), "sess.a");
    assert_eq!(session.snapshot.turn, AgentTurnState::Unknown {});
    assert_eq!(
        session.status,
        AgentSemanticStatus::Unknown {
            activity_hint: None,
        }
    );
    assert!(session.snapshot.pending_requests.is_empty());
    assert!(session.snapshot.last_outcome.is_none());
    assert_eq!(session.snapshot.observation.generation.value(), 7);
    assert_eq!(
        session.snapshot.capabilities,
        vec![
            AgentCapability::Questions,
            AgentCapability::SessionLifecycle,
            AgentCapability::TurnLifecycle,
        ]
    );

    let AgentReadResult::Explain { result, .. } =
        service.execute(explain_request(2, "sess.a")).unwrap()
    else {
        panic!("expected an explain result");
    };
    let AgentExplainOutcome::Found {
        revision,
        session,
        explanation,
    } = result.outcome
    else {
        panic!("expected a found outcome");
    };
    assert_eq!(revision.value(), 1);
    assert_eq!(session.snapshot.session_id.as_str(), "sess.a");
    assert_eq!(explanation.action_path, NavigateOnly);
    assert_eq!(explanation.content_available, ContentUnavailable);
    assert_eq!(
        explanation.status_basis,
        AgentStatusBasis::UnobservedTurnState
    );
    assert_eq!(
        explanation.freshness_meaning,
        AgentFreshnessMeaning::CurrentObservation
    );
    assert_eq!(explanation.fidelity, AgentObservationFidelity::Rich);
    assert_eq!(explanation.freshness, AgentObservationFreshness::Fresh);
    assert_eq!(explanation.source, AgentObservationSource::Native);
    assert_eq!(explanation.generation.value(), 7);
    assert_eq!(explanation.sequence.value(), 7);
    assert!(
        explanation
            .capability_ceiling
            .contains(&AgentCapability::TurnLifecycle)
    );
}

#[test]
fn maps_attention_requests_and_active_turns() {
    let service = agent_harness::seeded(&[
        agent_harness::session_observed("sess.attention", 1.0),
        agent_harness::attention_requested("sess.attention", "req-1", "question", 1.0, 2.0),
        agent_harness::session_observed("sess.active", 3.0),
        agent_harness::turn_started("sess.active", "turn-1", "thinking", 3.0, 4.0),
    ])
    .unwrap();

    let AgentReadResult::Get { result, .. } =
        service.execute(get_request(1, "sess.attention")).unwrap()
    else {
        panic!("expected a get result");
    };
    let AgentGetOutcome::Found { session, .. } = result.outcome else {
        panic!("expected a found outcome");
    };
    assert_eq!(
        session.status,
        AgentSemanticStatus::NeedsAttention {
            count: AgentPositiveInteger::new(1).unwrap(),
        }
    );
    assert_eq!(session.snapshot.pending_requests.len(), 1);
    let request = &session.snapshot.pending_requests[0];
    assert_eq!(request.request_id.as_str(), "req-1");
    assert_eq!(request.kind, AgentAttentionKind::Question);
    assert_eq!(request.action_path, NavigateOnly);
    assert!(request.turn_id.is_none());

    let AgentReadResult::Get { result, .. } =
        service.execute(get_request(2, "sess.active")).unwrap()
    else {
        panic!("expected a get result");
    };
    let AgentGetOutcome::Found { session, .. } = result.outcome else {
        panic!("expected a found outcome");
    };
    assert_eq!(
        session.status,
        AgentSemanticStatus::Working {
            activity: Some(AgentActivity::Thinking),
            phase: AgentTurnPhase::Working,
        }
    );
    let AgentTurnState::Active {
        activity,
        phase,
        turn_id,
    } = session.snapshot.turn
    else {
        panic!("expected an active turn");
    };
    assert_eq!(activity, Some(AgentActivity::Thinking));
    assert_eq!(phase, AgentTurnPhase::Working);
    assert_eq!(turn_id.as_str(), "turn-1");
}

#[test]
fn paginates_through_verified_boot_scoped_cursors() {
    let service = agent_harness::seeded(&[
        agent_harness::session_observed("s.a", 1.0),
        agent_harness::session_observed("s.b", 2.0),
        agent_harness::session_observed("s.c", 3.0),
    ])
    .unwrap();
    let (items, next_cursor, revision) =
        listed(service.execute(list_request(1, false, 2, None)).unwrap());
    assert_eq!(items, vec!["s.a", "s.b"]);
    assert_eq!(revision, 3);
    let cursor = next_cursor.expect("a continuation cursor was not minted");
    let (items, next_cursor, revision) = listed(
        service
            .execute(list_request(2, false, 2, Some(cursor)))
            .unwrap(),
    );
    assert_eq!(items, vec!["s.c"]);
    assert!(next_cursor.is_none());
    assert_eq!(revision, 3);
}

#[test]
fn rejects_tampered_flag_mismatched_and_stale_cursors() {
    let service = agent_harness::seeded(&[
        agent_harness::session_observed("s.a", 1.0),
        agent_harness::session_observed("s.b", 2.0),
    ])
    .unwrap();
    let (items, next_cursor, _) = listed(service.execute(list_request(1, false, 1, None)).unwrap());
    assert_eq!(items, vec!["s.a"]);
    let cursor = next_cursor.expect("a continuation cursor was not minted");

    // A single-character edit inside the tag segment keeps the cursor shape
    // valid but always breaks the MAC.
    let mut bytes = cursor.as_str().as_bytes().to_vec();
    let tag_start = bytes.len() - 43;
    bytes[tag_start] = if bytes[tag_start] == b'A' { b'B' } else { b'A' };
    let tampered = AgentListCursor::parse(&String::from_utf8(bytes).unwrap())
        .expect("the tampered cursor no longer parses");
    let AgentReadResult::List { result, .. } = service
        .execute(list_request(2, false, 1, Some(tampered)))
        .unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Rejected {
            reason: AgentListRejectionReason::CursorInvalid,
        }
    );

    // The authentic cursor does not carry the `include_ended` flag.
    let AgentReadResult::List { result, .. } = service
        .execute(list_request(3, true, 1, Some(cursor.clone())))
        .unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Rejected {
            reason: AgentListRejectionReason::CursorInvalid,
        }
    );

    // The cursor was minted at revision 2; revision 3 makes it stale.
    assert!(matches!(
        service.seed(&agent_harness::session_observed("s.c", 3.0)),
        Ok(crate::agent::AgentTransition::Applied { .. })
    ));
    let AgentReadResult::List { result, .. } = service
        .execute(list_request(4, false, 1, Some(cursor)))
        .unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Rejected {
            reason: AgentListRejectionReason::CursorStale,
        }
    );
}

#[test]
fn filters_ended_sessions_by_include_ended() {
    let service = agent_harness::seeded(&[
        agent_harness::session_observed("s.live", 1.0),
        agent_harness::session_observed("s.gone", 2.0),
        agent_harness::session_ended("s.gone", 2.0, 3.0),
    ])
    .unwrap();
    let (items, _, _) = listed(service.execute(list_request(1, false, 50, None)).unwrap());
    assert_eq!(items, vec!["s.live"]);

    let AgentReadResult::List { result, .. } =
        service.execute(list_request(2, true, 50, None)).unwrap()
    else {
        panic!("expected a list result");
    };
    let AgentListOutcome::Listed { items, .. } = result.outcome else {
        panic!("expected a listed outcome");
    };
    assert_eq!(items.len(), 2);
    let ended = items
        .iter()
        .find(|item| item.session_id.as_str() == "s.gone")
        .expect("the ended session was filtered out");
    assert_eq!(ended.lifecycle, AgentLifecycle::Ended);
    assert_eq!(ended.status, AgentSemanticStatus::Ended {});
}

/// Semantic consumer proof: a real hook-corpus envelope admitted through
/// the bridge seam dispatches into the same `AgentState.runtime` the OBAP
/// read path clones — the session is readable through `execute`, and the
/// retire seam detaches the joined presentation on that same cell.
#[test]
fn bridge_dispatches_hook_envelope_into_the_read_path_runtime() {
    let service = AgentService::open_hook_fixture(
        AgentPolicy {
            read: true,
            manage: true,
        },
        "fixture.sock",
    )
    .unwrap();
    // Native wire profile: presentation ids are 32-hex, never the corpus's
    // `desktop:presentation:` fixture shape.
    let presentation_id = "aa".repeat(16);
    let creation_id = "01".repeat(16);

    // Registration seam emits the owned socket overlay and registers the
    // generation on the same cell.
    let overlay = service
        .hook_environment_overlay(presentation_id.clone(), creation_id.clone())
        .unwrap();
    assert_eq!(
        overlay,
        vec![(
            "ORCHBEE_AGENT_OBSERVATION_SOCKET".to_string(),
            "fixture.sock".to_string()
        )]
    );

    // A real corpus-shaped correlation hint, then the matching envelope.
    let hint = serde_json::json!({
        "correlationId": "correlation-0000000000000001",
        "eventId": "event-0000000000000001",
        "kind": "correlation",
        "version": 1,
    })
    .to_string();
    service
        .observe_hook_signal(&hint, creation_id, presentation_id.clone(), 0.0)
        .unwrap();
    let envelope = serde_json::json!({
        "adapterId": "codex",
        "correlations": [{
            "correlationId": "correlation-0000000000000001",
            "visibility": "hidden",
        }],
        "eventId": "event-0000000000000001",
        "observation": {
            "activity": null,
            "agentKey": null,
            "event": "session-started",
            "operationKey": null,
            "sessionKey": "session-1",
            "turnKey": null,
            "version": 1,
        },
        "version": 1,
    })
    .to_string();
    assert!(matches!(
        service.admit_hook_envelope(envelope.as_bytes(), 0.0),
        Ok(true)
    ));

    // The join fired synchronously: the read path sees the minted session
    // with the attached presentation on the same canonical store.
    let (items, _, revision) = listed(service.execute(list_request(1, false, 50, None)).unwrap());
    // SessionObserved + PresentationAttached: two dispatches, two revisions.
    assert_eq!(revision, 2);
    assert_eq!(items, vec!["desktop:agent:session:2"]);
    let AgentReadResult::Get { result, .. } = service
        .execute(get_request(2, "desktop:agent:session:2"))
        .unwrap()
    else {
        panic!("expected a get result");
    };
    let AgentGetOutcome::Found { session, .. } = result.outcome else {
        panic!("expected a found outcome");
    };
    assert_eq!(session.snapshot.provider_id.as_str(), "codex");
    assert_eq!(session.snapshot.presentation_attachments.len(), 1);
    assert_eq!(
        session.snapshot.presentation_attachments[0]
            .presentation_id
            .as_str(),
        presentation_id
    );
    assert!(session.snapshot.presentation_attachments[0].navigable);

    // Retire through the same seam: the read path reports the detachment.
    service.retire_hook_presentation(&presentation_id).unwrap();
    let AgentReadResult::Get { result, .. } = service
        .execute(get_request(3, "desktop:agent:session:2"))
        .unwrap()
    else {
        panic!("expected a get result");
    };
    let AgentGetOutcome::Found { session, .. } = result.outcome else {
        panic!("expected a found outcome");
    };
    assert!(!session.snapshot.presentation_attachments[0].navigable);
}

#[test]
fn reports_runtime_unavailable_after_disposal() {
    let service = agent_harness::seeded(&[agent_harness::session_observed("s.a", 1.0)]).unwrap();
    service.shutdown();
    let AgentReadResult::List { result, .. } =
        service.execute(list_request(1, false, 50, None)).unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
    let AgentReadResult::Get { result, .. } = service.execute(get_request(2, "s.a")).unwrap()
    else {
        panic!("expected a get result");
    };
    assert_eq!(
        result.outcome,
        AgentGetOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
    let AgentReadResult::Explain { result, .. } =
        service.execute(explain_request(3, "s.a")).unwrap()
    else {
        panic!("expected an explain result");
    };
    assert_eq!(
        result.outcome,
        AgentExplainOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
}

/// Failure-boundary witness on the real `with_state` helper: a fold panic
/// after a committed real mutation poisons the one canonical cell
/// permanently. The panic is contained inside the caller's held outer
/// guards, every later read reports `RuntimeUnavailable`, every write
/// seam returns `Err` (never `Ok(false)` or an empty overlay), and
/// shutdown stays an idempotent no-op — the cell is never unpoisoned and
/// the pre-fault mutation is never observable.
#[test]
fn a_panicked_fold_permanently_poisons_the_single_state_cell() {
    let service = AgentService::open_hook_fixture(
        AgentPolicy {
            read: true,
            manage: true,
        },
        "fixture.sock",
    )
    .unwrap();
    let presentation_id = "aa".repeat(16);
    let creation_id = "01".repeat(16);
    service
        .hook_environment_overlay(presentation_id.clone(), creation_id.clone())
        .unwrap();
    let hint = serde_json::json!({
        "correlationId": "correlation-0000000000000001",
        "eventId": "event-0000000000000001",
        "kind": "correlation",
        "version": 1,
    })
    .to_string();
    service
        .observe_hook_signal(&hint, creation_id, presentation_id.clone(), 0.0)
        .unwrap();
    let envelope = serde_json::json!({
        "adapterId": "codex",
        "correlations": [{
            "correlationId": "correlation-0000000000000001",
            "visibility": "hidden",
        }],
        "eventId": "event-0000000000000001",
        "observation": {
            "activity": null,
            "agentKey": null,
            "event": "session-started",
            "operationKey": null,
            "sessionKey": "session-1",
            "turnKey": null,
            "version": 1,
        },
        "version": 1,
    })
    .to_string();
    assert!(matches!(
        service.admit_hook_envelope(envelope.as_bytes(), 0.0),
        Ok(true)
    ));

    // The real bridge read BEFORE the fault: the minted session is found.
    let AgentReadResult::Get { result, .. } = service
        .execute(get_request(1, "desktop:agent:session:2"))
        .unwrap()
    else {
        panic!("expected a get result");
    };
    assert!(matches!(result.outcome, AgentGetOutcome::Found { .. }));

    // Inject the fault through the real helper while the caller holds two
    // dummy outer guards: the contained panic must leave both healthy.
    let outer_registry = Mutex::new(());
    let outer_coordinator = Mutex::new(());
    let registry_guard = outer_registry.lock().unwrap();
    let coordinator_guard = outer_coordinator.lock().unwrap();
    let outcome = service
        .poison_state_after_mutation_for_test(&agent_harness::session_observed("s.fault", 7.0));
    assert!(matches!(outcome, Err(AgentFoldUnavailable)));
    assert!(!outer_registry.is_poisoned());
    assert!(!outer_coordinator.is_poisoned());
    drop(coordinator_guard);
    drop(registry_guard);

    // The cell is permanently poisoned; the dispatch that committed
    // before the panic is never observable — a Get for it reports
    // Unavailable, not Found.
    assert!(service.cell_poisoned_for_test());
    let AgentReadResult::List { result, .. } =
        service.execute(list_request(2, false, 50, None)).unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
    let AgentReadResult::Get { result, .. } = service.execute(get_request(3, "s.fault")).unwrap()
    else {
        panic!("expected a get result");
    };
    assert_eq!(
        result.outcome,
        AgentGetOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
    let AgentReadResult::Explain { result, .. } =
        service.execute(explain_request(4, "s.fault")).unwrap()
    else {
        panic!("expected an explain result");
    };
    assert_eq!(
        result.outcome,
        AgentExplainOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );

    // Every write seam reports Err — unavailability is never confused
    // with a rejected payload or an empty overlay.
    assert!(
        service
            .admit_hook_envelope(envelope.as_bytes(), 1.0)
            .is_err()
    );
    assert!(
        service
            .observe_hook_signal(&hint, "02".repeat(16), presentation_id.clone(), 1.0)
            .is_err()
    );
    assert!(
        service
            .hook_environment_overlay("bb".repeat(16), "03".repeat(16))
            .is_err()
    );
    assert!(service.retire_hook_presentation(&presentation_id).is_err());
    assert!(service.advance_hook_settlement(10.0, 1.0).is_err());
    assert!(
        service
            .seed(&agent_harness::session_observed("s.more", 8.0))
            .is_err()
    );

    // Shutdown is a documented no-op on the poisoned cell — idempotent,
    // and it never unpoisons: reads stay Unavailable afterwards.
    service.shutdown();
    service.shutdown();
    assert!(service.cell_poisoned_for_test());
    let AgentReadResult::Get { result, .. } = service
        .execute(get_request(5, "desktop:agent:session:2"))
        .unwrap()
    else {
        panic!("expected a get result");
    };
    assert_eq!(
        result.outcome,
        AgentGetOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
}

/// A healthy-cell shutdown is idempotent and does NOT poison the cell —
/// disposal is the clean `Ok(None)` arm, distinct from fold poison.
#[test]
fn shutdown_is_idempotent_without_poisoning_a_healthy_cell() {
    let service = agent_harness::seeded(&[agent_harness::session_observed("s.a", 1.0)]).unwrap();
    service.shutdown();
    service.shutdown();
    assert!(!service.cell_poisoned_for_test());
    let AgentReadResult::List { result, .. } =
        service.execute(list_request(1, false, 50, None)).unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
}

// ---------------------------------------------------------------------------
// Staged native observation leaf — real-service bridge coverage.
//
// These drive the `native_*` seams through the same `with_state` cell
// the read path clones: held hints replay into the canonical runtime on
// binding commit, carrier retire consumes RetirePresentation effects
// atomically, expiry at late binding never replays, and a fault inside
// the combined leaf+effect fold poisons the whole cell. The pure-leaf
// classification coverage lives in `agent/native_observation/tests.rs`.

mod native_leaf {
    use std::num::NonZeroU64;
    use std::time::{Duration, Instant};

    use orchbee_capability_node_protocol::{
        HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration, WorkerId,
    };
    use orchbee_workbench_protocol::TerminalPresentationId;
    use serde_json::json;

    use super::*;
    use crate::agent::native_observation::{Binding, CarrierIdentity, NativeClass, NativeEvent};
    use orchbee_agent_observation_protocol::AgentPresentationVisibility;

    fn carrier_id(worker_seed: u8, generation: u64) -> CarrierIdentity {
        CarrierIdentity {
            worker_id: WorkerId::from_bytes([worker_seed; 16]).unwrap(),
            generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
        }
    }

    fn host(seed: u8) -> HostTerminalIdentity {
        HostTerminalIdentity::new(
            HostCreationId::from_bytes([seed; 16]).unwrap(),
            HostEpoch::from_bytes([seed.wrapping_add(16); 16]).unwrap(),
            HostTerminalId::from_bytes([seed.wrapping_add(32); 16]).unwrap(),
        )
    }

    fn presentation(seed: u8) -> TerminalPresentationId {
        TerminalPresentationId::parse(&format!("{seed:032x}")).unwrap()
    }

    fn binding(seed: u8, carrier: CarrierIdentity) -> Binding {
        Binding {
            presentation: presentation(seed),
            carrier,
            host: host(seed),
        }
    }

    fn hint_json(correlation: u64, event: u64) -> String {
        format!(
            "{{\"correlationId\":\"correlation-{correlation:016}\",\
             \"eventId\":\"event-{event:016}\",\
             \"kind\":\"correlation\",\"version\":1}}"
        )
    }

    fn event(
        carrier: CarrierIdentity,
        sequence: u64,
        host: HostTerminalIdentity,
        raw: impl Into<Box<str>>,
    ) -> NativeEvent {
        NativeEvent::new(carrier, NonZeroU64::new(sequence).unwrap(), host, raw).unwrap()
    }

    fn envelope(correlation: u64, event: u64) -> Vec<u8> {
        json!({
            "adapterId": "codex",
            "correlations": [{
                "correlationId": format!("correlation-{correlation:016}"),
                "visibility": "hidden",
            }],
            "eventId": format!("event-{event:016}"),
            "observation": {
                "activity": null,
                "agentKey": null,
                "event": "session-started",
                "operationKey": null,
                "sessionKey": "session-1",
                "turnKey": null,
                "version": 1,
            },
            "version": 1,
        })
        .to_string()
        .into_bytes()
    }
    /// The partial-match shape: correlation c1 can carry a live hint
    /// while c2 never matches — the only envelope form that produces
    /// a settlement timer (join.rs :529-541).
    fn envelope2(c1: u64, c2: u64, event: u64) -> Vec<u8> {
        json!({
            "adapterId": "codex",
            "correlations": [
                {
                    "correlationId": format!("correlation-{c1:016}"),
                    "visibility": "visible",
                },
                {
                    "correlationId": format!("correlation-{c2:016}"),
                    "visibility": "visible",
                },
            ],
            "eventId": format!("event-{event:016}"),
            "observation": {
                "activity": null,
                "agentKey": null,
                "event": "session-started",
                "operationKey": null,
                "sessionKey": "session-1",
                "turnKey": null,
                "version": 1,
            },
            "version": 1,
        })
        .to_string()
        .into_bytes()
    }

    fn listed_sessions(service: &AgentService) -> Vec<String> {
        let (items, _cursor, _revision) =
            listed(service.execute(list_request(1, false, 50, None)).unwrap());
        items
    }

    /// T1 — the real bridge per the delivery oracle (fixture material
    /// `input41-same-runtime-native-leaf-oracle.md`, literals adopted
    /// byte-exact): admit carrier -> native hint HELD before membership
    /// -> complete typed binding reconciliation (registers through the
    /// None-overlay `register_presentation_binding` seam and replays the
    /// held record exactly once) -> matching rich envelope -> join ->
    /// settle -> SAME canonical runtime Get/List, plus the ended-history
    /// negatives. The second canonical dispatch is `session-idle-observed`
    /// (revision 2), not a separate first-attach event.
    #[test]
    fn held_hint_replays_into_canonical_runtime_on_binding_commit() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(0xaa, 1);
        // Oracle literals: C1 = "01"*16 (join generation), P1 = "aa"*16.
        let host1 = HostTerminalIdentity::new(
            HostCreationId::from_bytes([0x01; 16]).unwrap(),
            HostEpoch::from_bytes([0x11; 16]).unwrap(),
            HostTerminalId::from_bytes([0x21; 16]).unwrap(),
        );
        let p1 = TerminalPresentationId::parse(&"aa".repeat(16)).unwrap();
        let h1 = "{\"correlationId\":\"correlation-0000000000000001\",            \"eventId\":\"event-0000000000000001\",            \"kind\":\"correlation\",\"version\":1}";
        let ev1 = json!({
            "adapterId": "codex",
            "correlations": [{
                "correlationId": "correlation-0000000000000001",
                "visibility": "visible",
            }],
            "eventId": "event-0000000000000001",
            "observation": {
                "activity": null,
                "agentKey": null,
                "event": "session-started",
                "operationKey": null,
                "sessionKey": "session-1",
                "turnKey": null,
                "version": 1,
            },
            "version": 1,
        })
        .to_string()
        .into_bytes();

        // Event before membership: the leaf holds it (the raw
        // translator would have dropped it at the generation fence).
        service.native_admit_carrier(carrier, t0).unwrap().unwrap();
        let held = service
            .native_apply_event(event(carrier, 1, host1, h1), t0)
            .unwrap();
        assert_eq!((held.class, held.ack_w), (NativeClass::Held, Some(1)));
        assert_eq!(service.native_held_count().unwrap(), 1);
        assert!(listed_sessions(&service).is_empty());

        // Complete typed reconciliation: register (None-overlay seam)
        // + replay the held record exactly once.
        service
            .native_reconcile_bindings(
                vec![Binding {
                    presentation: p1.clone(),
                    carrier,
                    host: host1,
                }],
                t0,
            )
            .unwrap()
            .unwrap();
        assert_eq!(service.native_held_count().unwrap(), 0);

        // Matching rich envelope -> join_correlation -> settle ->
        // SessionObserved + SessionIdleObserved in the SAME runtime.
        assert!(service.admit_hook_envelope(&ev1, 1000.0).unwrap());
        service.advance_hook_settlement(50.0, 1050.0).unwrap();

        // Canonical Get: session:2 (execution:1 minted first), one
        // attachment {P1, C1, navigable, visible}, rich/hook/fresh
        // observation at sequence 2, revision 2.
        let AgentReadResult::Get { result, .. } = service
            .execute(get_request(2, "desktop:agent:session:2"))
            .unwrap()
        else {
            panic!("expected a get result");
        };
        let AgentGetOutcome::Found { revision, session } = result.outcome else {
            panic!("expected the session to be found");
        };
        assert_eq!(revision.value(), 2);
        let snapshot = session.snapshot;
        assert_eq!(snapshot.session_id.as_str(), "desktop:agent:session:2");
        assert_eq!(snapshot.provider_id.as_str(), "codex");
        assert_eq!(snapshot.lifecycle, AgentLifecycle::Live);
        assert_eq!(snapshot.presentation_attachments.len(), 1);
        let attachment = &snapshot.presentation_attachments[0];
        assert_eq!(attachment.presentation_id.as_str(), "aa".repeat(16));
        assert_eq!(attachment.generation.as_str(), "01".repeat(16));
        assert!(attachment.navigable);
        assert_eq!(attachment.visibility, AgentPresentationVisibility::Visible);
        assert_eq!(
            snapshot.observation.fidelity,
            AgentObservationFidelity::Rich
        );
        assert_eq!(
            snapshot.observation.freshness,
            AgentObservationFreshness::Fresh
        );
        assert_eq!(snapshot.observation.source, AgentObservationSource::Hook);
        assert_eq!(snapshot.observation.sequence.value(), 2);
        let (items, _cursor, list_revision) =
            listed(service.execute(list_request(3, false, 50, None)).unwrap());
        assert_eq!(items, vec!["desktop:agent:session:2".to_owned()]);
        assert_eq!(list_revision, 2);

        // Ended-history negatives (oracle N3): H7 routes immediately on
        // the live binding; EV5 session-ended applies to session-1's
        // session; Get still finds it ended, List filters on
        // include_ended, and reconcile-removal flips navigable without
        // erasing history.
        let h7 = "{\"correlationId\":\"correlation-0000000000000007\",            \"eventId\":\"event-0000000000000007\",            \"kind\":\"correlation\",\"version\":1}";
        let routed = service
            .native_apply_event(event(carrier, 2, host1, h7), t0)
            .unwrap();
        assert_eq!((routed.class, routed.ack_w), (NativeClass::Routed, Some(2)));
        let ev5 = json!({
            "adapterId": "codex",
            "correlations": [{
                "correlationId": "correlation-0000000000000007",
                "visibility": "visible",
            }],
            "eventId": "event-0000000000000007",
            "observation": {
                "activity": null,
                "agentKey": null,
                "event": "session-ended",
                "operationKey": null,
                "sessionKey": "session-1",
                "turnKey": null,
                "version": 1,
            },
            "version": 1,
        })
        .to_string()
        .into_bytes();
        assert!(service.admit_hook_envelope(&ev5, 1500.0).unwrap());
        service.advance_hook_settlement(50.0, 1550.0).unwrap();

        let AgentReadResult::Get { result, .. } = service
            .execute(get_request(4, "desktop:agent:session:2"))
            .unwrap()
        else {
            panic!("expected a get result");
        };
        let AgentGetOutcome::Found { session, .. } = result.outcome else {
            panic!("ended history must persist");
        };
        assert_eq!(session.snapshot.lifecycle, AgentLifecycle::Ended);
        let (live_items, ..) = listed(service.execute(list_request(5, false, 50, None)).unwrap());
        assert!(live_items.is_empty());
        let (all_items, ..) = listed(service.execute(list_request(6, true, 50, None)).unwrap());
        assert_eq!(all_items, vec!["desktop:agent:session:2".to_owned()]);

        // Retire the only binding: the attachment flips navigable but
        // the session record remains Found — no history disappearance.
        service
            .native_reconcile_bindings(Vec::new(), t0)
            .unwrap()
            .unwrap();
        let AgentReadResult::Get { result, .. } = service
            .execute(get_request(7, "desktop:agent:session:2"))
            .unwrap()
        else {
            panic!("expected a get result");
        };
        let AgentGetOutcome::Found { session, .. } = result.outcome else {
            panic!("retired presentation must not erase history");
        };
        assert!(!session.snapshot.presentation_attachments[0].navigable);
    }

    /// T2 — carrier retirement consumes its `RetirePresentation`
    /// effects inside the same fold: after retire, a replayed hint on
    /// the dead presentation is fenced by the join (generation
    /// registration is gone), so nothing reaches the runtime.
    #[test]
    fn carrier_retire_drops_join_fences_atomically() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(1, 1);

        service.native_admit_carrier(carrier, t0).unwrap().unwrap();
        service
            .native_reconcile_bindings(vec![binding(1, carrier)], t0)
            .unwrap()
            .unwrap();
        service.native_retire_carrier(carrier, t0).unwrap();

        // A raw translator hint on the retired presentation is fenced:
        // registration was dropped in the same fold that retired the
        // carrier.
        assert!(service.admit_hook_envelope(&envelope(7, 7), 0.0).unwrap());
        service
            .observe_hook_signal(
                &hint_json(7, 7),
                format!("{:032x}", 1u8),
                presentation(1).as_str().to_owned(),
                0.0,
            )
            .unwrap();
        service.advance_hook_settlement(1_000.0, 1_000.0).unwrap();
        assert!(listed_sessions(&service).is_empty());
    }

    /// T4 — expiry at late binding: the held record is swept at the
    /// head of reconcile and can never replay onto a late binding.
    #[test]
    fn expired_held_record_never_replays_on_late_binding() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(1, 1);
        service.native_admit_carrier(carrier, t0).unwrap().unwrap();
        assert!(service.admit_hook_envelope(&envelope(5, 5), 0.0).unwrap());
        let held = service
            .native_apply_event(event(carrier, 1, host(1), hint_json(5, 5)), t0)
            .unwrap();
        assert_eq!(held.class, NativeClass::Held);

        let late = t0 + Duration::from_secs(11);
        service
            .native_reconcile_bindings(vec![binding(1, carrier)], late)
            .unwrap()
            .unwrap();
        assert_eq!(service.native_held_count().unwrap(), 0);
        service.advance_hook_settlement(1_000.0, 1_000.0).unwrap();
        assert!(listed_sessions(&service).is_empty());
    }

    /// Gap and sweep seams round-trip through the same `with_state`
    /// boundary: gap cover ACKs as data values, and an explicit sweep
    /// expels an aged held record without any other entry.
    #[test]
    fn native_gap_and_sweep_seams() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(1, 1);
        service.native_admit_carrier(carrier, t0).unwrap().unwrap();

        let held = service
            .native_apply_event(event(carrier, 1, host(1), hint_json(8, 8)), t0)
            .unwrap();
        assert_eq!(held.class, NativeClass::Held);
        let covered = service.native_apply_gap(carrier, 2, 4, t0).unwrap();
        assert_eq!(covered.class, NativeClass::Gap);
        assert_eq!(covered.ack_w, Some(4));

        let late = t0 + Duration::from_secs(11);
        service.native_sweep(late).unwrap();
        assert_eq!(service.native_held_count().unwrap(), 0);
    }

    /// T8 — the shared poison boundary: a panic inside the combined
    /// leaf+effect fold poisons the whole cell; every subsequent seam —
    /// leaf, translator, and the canonical read path — reports the same
    /// unavailability.
    #[test]
    fn combined_fold_fault_poisons_the_whole_cell() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(1, 1);

        let fault = service.native_poison_after_mutation_for_test(carrier, t0);
        assert!(fault.is_err());
        assert!(service.cell_poisoned_for_test());

        assert!(service.native_admit_carrier(carrier, t0).is_err());
        assert!(
            service
                .native_apply_event(event(carrier, 1, host(1), hint_json(1, 1)), t0)
                .is_err()
        );
        assert!(service.native_held_count().is_err());
        assert!(service.admit_hook_envelope(&envelope(1, 1), 0.0).is_err());
        // The canonical read path maps the poisoned fold to the
        // existing RuntimeUnavailable wire arm — the same
        // unavailability axis as the seam Errs.
        let AgentReadResult::List { result, .. } =
            service.execute(list_request(2, false, 50, None)).unwrap()
        else {
            panic!("expected a list result");
        };
        assert_eq!(
            result.outcome,
            AgentListOutcome::Unavailable {
                reason: AgentUnavailableReason::RuntimeUnavailable,
            }
        );
    }

    /// U1 — deterministic schedule value through the production-shaped
    /// Instant seams (delivery report-44): a PARTIAL match — two
    /// correlations, one live hint — is the only envelope shape that
    /// produces a settlement timer; its deadline lands at the admit
    /// instant + the source 25ms settle delay, mapped back through the
    /// service clock origin. Single-threaded: no endpoint exists.
    #[test]
    fn admit_seam_schedules_partial_match_on_the_call_instant() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(0xcc, 1);
        service.native_admit_carrier(carrier, t0).unwrap().unwrap();
        service
            .native_apply_event(event(carrier, 1, host(0xcc), hint_json(1, 1)), t0)
            .unwrap();
        service
            .native_reconcile_bindings(vec![binding(0xcc, carrier)], t0)
            .unwrap()
            .unwrap();
        let t_a = Instant::now();
        assert!(service.admit_envelope(&envelope2(1, 2, 1), t_a).unwrap());
        let deadline = service
            .next_deadline()
            .unwrap()
            .expect("a partial match must schedule a settlement");
        let delay = deadline.duration_since(t_a);
        assert!(
            delay > Duration::from_millis(20) && delay < Duration::from_millis(30),
            "settlement must land at admit + 25ms, got {delay:?}"
        );
    }

    /// U2 — the funnel-sync discriminator: a settlement timer created
    /// by the native Instant funnel (RouteHint inside
    /// `apply_native_effects`, event-before-hint ordering) must land
    /// on the HINT instant — without the head sync it would be born on
    /// the earlier admit's stale base, already in the past.
    #[test]
    fn native_funnel_sync_schedules_on_the_hint_instant() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(0xcd, 1);
        service.native_admit_carrier(carrier, t0).unwrap().unwrap();
        service
            .native_reconcile_bindings(vec![binding(0xcd, carrier)], t0)
            .unwrap()
            .unwrap();
        let t_a = Instant::now();
        // Both correlations unmatched -> pending event, no timer.
        assert!(service.admit_envelope(&envelope2(1, 2, 1), t_a).unwrap());
        assert_eq!(service.next_deadline().unwrap(), None);
        let t_h = t_a + Duration::from_millis(100);
        let routed = service
            .native_apply_event(event(carrier, 1, host(0xcd), hint_json(1, 1)), t_h)
            .unwrap();
        assert_eq!(routed.class, NativeClass::Routed);
        let deadline = service
            .next_deadline()
            .unwrap()
            .expect("the RouteHint partial match must schedule");
        let delay = deadline
            .checked_duration_since(t_h)
            .expect("without funnel sync the deadline is born in the past");
        assert!(
            delay > Duration::from_millis(20) && delay < Duration::from_millis(30),
            "funnel sync must schedule at hint + 25ms, got {delay:?}"
        );
    }

    /// U3 — the settle drain through the same Instant seam: after the
    /// native-funnel timer exists, `tick` fires it and the pending
    /// event applies — the unit-level drain the endpoint's own tick
    /// performs in the real-thread cases.
    #[test]
    fn tick_seam_drains_the_scheduled_settlement() {
        let service = AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap();
        let t0 = Instant::now();
        let carrier = carrier_id(0xce, 1);
        service.native_admit_carrier(carrier, t0).unwrap().unwrap();
        service
            .native_reconcile_bindings(vec![binding(0xce, carrier)], t0)
            .unwrap()
            .unwrap();
        let t_a = Instant::now();
        assert!(service.admit_envelope(&envelope2(1, 2, 1), t_a).unwrap());
        let t_h = t_a + Duration::from_millis(100);
        service
            .native_apply_event(event(carrier, 1, host(0xce), hint_json(1, 1)), t_h)
            .unwrap();
        service.tick(t_h + Duration::from_millis(30)).unwrap();
        assert_eq!(service.next_deadline().unwrap(), None);
        let sessions = listed_sessions(&service);
        assert_eq!(sessions.len(), 1);
        let AgentReadResult::Get { result, .. } =
            service.execute(get_request(3, &sessions[0])).unwrap()
        else {
            panic!("expected a get result");
        };
        assert!(matches!(result.outcome, AgentGetOutcome::Found { .. }));
    }

    /// Prepared-read boundary (Root25): the shared executor owns the
    /// policy gate, request correlation, projection, and the provider
    /// panic boundary; the caller supplies a REQUIRED snapshot provider.
    /// These tests pin the ordering (deny-before-prepare), the
    /// exactly-once invocation contract, the correlated-unavailable
    /// mapping, and the staged single-fold `native_reconcile_and_snapshot`
    /// helper feeding real reads off the same canonical runtime the
    /// leaf writes — never a second fold, never a reinjected seam.
    mod prepared_read {
        use std::cell::Cell;

        use super::*;
        use crate::agent::AgentRuntimeSnapshot;

        fn unavailable_provider() -> Result<AgentRuntimeSnapshot, PreparedUnavailable> {
            Err(PreparedUnavailable)
        }

        fn exploding_provider() -> Result<AgentRuntimeSnapshot, PreparedUnavailable> {
            panic!("the provider must never run here");
        }

        /// Denied List/Get/Explain are rejected BEFORE preparation: a
        /// provider that panics on any invocation still yields a
        /// correlated `ReadDenied`, proving zero invocations and the
        /// load-bearing gate order.
        #[test]
        fn denied_requests_reject_without_invoking_the_provider() {
            let service = AgentService::open(AgentPolicy {
                read: false,
                manage: false,
            })
            .unwrap();

            let AgentReadResult::List {
                request_id: echoed,
                result,
            } = service
                .execute_prepared_for_test(list_request(1, false, 50, None), exploding_provider)
                .unwrap()
            else {
                panic!("expected a list result");
            };
            assert_eq!(echoed, request_id(1));
            assert_eq!(
                result.outcome,
                AgentListOutcome::Rejected {
                    reason: AgentListRejectionReason::ReadDenied,
                }
            );

            let AgentReadResult::Get {
                request_id: echoed,
                result,
            } = service
                .execute_prepared_for_test(get_request(2, "any.session"), exploding_provider)
                .unwrap()
            else {
                panic!("expected a get result");
            };
            assert_eq!(echoed, request_id(2));
            assert_eq!(
                result.outcome,
                AgentGetOutcome::Rejected {
                    reason: AgentSessionRejectionReason::ReadDenied,
                }
            );

            let AgentReadResult::Explain {
                request_id: echoed,
                result,
            } = service
                .execute_prepared_for_test(explain_request(3, "any.session"), exploding_provider)
                .unwrap()
            else {
                panic!("expected an explain result");
            };
            assert_eq!(echoed, request_id(3));
            assert_eq!(
                result.outcome,
                AgentExplainOutcome::Rejected {
                    reason: AgentSessionRejectionReason::ReadDenied,
                }
            );
        }

        /// An eligible read invokes the provider exactly once and the
        /// executor projects off THAT snapshot — no hidden second
        /// `snapshot()` fold.
        #[test]
        fn eligible_reads_invoke_the_provider_exactly_once() {
            let service = agent_harness::seeded(&[
                agent_harness::session_observed("sess.a", 1.0),
                agent_harness::session_observed("sess.b", 2.0),
            ])
            .unwrap();
            let calls = Cell::new(0u32);
            let provider = || {
                calls.set(calls.get() + 1);
                match service.snapshot() {
                    Ok(Some(snapshot)) => Ok(snapshot),
                    _ => Err(PreparedUnavailable),
                }
            };

            let (items, ..) = listed(
                service
                    .execute_prepared_for_test(list_request(1, false, 50, None), provider)
                    .unwrap(),
            );
            assert_eq!(items, vec!["sess.a".to_owned(), "sess.b".to_owned()]);
            assert_eq!(calls.get(), 1);

            let AgentReadResult::Get { result, .. } = service
                .execute_prepared_for_test(get_request(2, "sess.b"), provider)
                .unwrap()
            else {
                panic!("expected a get result");
            };
            assert!(matches!(result.outcome, AgentGetOutcome::Found { .. }));
            assert_eq!(calls.get(), 2);

            let AgentReadResult::Explain { result, .. } = service
                .execute_prepared_for_test(explain_request(3, "sess.a"), provider)
                .unwrap()
            else {
                panic!("expected an explain result");
            };
            assert!(matches!(result.outcome, AgentExplainOutcome::Found { .. }));
            assert_eq!(calls.get(), 3);
        }

        /// A declining provider over a NONEMPTY canonical state yields
        /// the correlated `RuntimeUnavailable` arm on every operation —
        /// never an empty list, never last-good state — and a subsequent
        /// valid provider serves again.
        #[test]
        fn unavailable_provider_never_serves_last_good_state() {
            let service = agent_harness::seeded(&[
                agent_harness::session_observed("sess.a", 1.0),
                agent_harness::session_observed("sess.b", 2.0),
            ])
            .unwrap();

            let AgentReadResult::List {
                request_id: echoed,
                result,
            } = service
                .execute_prepared_for_test(list_request(1, false, 50, None), unavailable_provider)
                .unwrap()
            else {
                panic!("expected a list result");
            };
            assert_eq!(echoed, request_id(1));
            assert_eq!(
                result.outcome,
                AgentListOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                }
            );

            let AgentReadResult::Get { result, .. } = service
                .execute_prepared_for_test(get_request(2, "sess.a"), unavailable_provider)
                .unwrap()
            else {
                panic!("expected a get result");
            };
            assert_eq!(
                result.outcome,
                AgentGetOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                }
            );

            let AgentReadResult::Explain { result, .. } = service
                .execute_prepared_for_test(explain_request(3, "sess.a"), unavailable_provider)
                .unwrap()
            else {
                panic!("expected an explain result");
            };
            assert_eq!(
                result.outcome,
                AgentExplainOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                }
            );

            // Healing: a subsequent valid provider serves the same
            // nonempty state — unavailability is per-request evidence,
            // not a latched cache.
            let (items, ..) = listed(
                service
                    .execute_prepared_for_test(list_request(4, false, 50, None), || {
                        match service.snapshot() {
                            Ok(Some(snapshot)) => Ok(snapshot),
                            _ => Err(PreparedUnavailable),
                        }
                    })
                    .unwrap(),
            );
            assert_eq!(items, vec!["sess.a".to_owned(), "sess.b".to_owned()]);
        }

        /// A provider panic is caught at the executor boundary and
        /// mapped to the correlated `RuntimeUnavailable` arm — it never
        /// propagates into the session loop and never faults the cell.
        #[test]
        fn provider_panic_maps_to_correlated_unavailable() {
            let service =
                agent_harness::seeded(&[agent_harness::session_observed("sess.a", 1.0)]).unwrap();

            let AgentReadResult::List {
                request_id: echoed,
                result,
            } = service
                .execute_prepared_for_test(list_request(9, false, 50, None), || {
                    panic!("provider-side fault")
                })
                .unwrap()
            else {
                panic!("expected a list result");
            };
            assert_eq!(echoed, request_id(9));
            assert_eq!(
                result.outcome,
                AgentListOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                }
            );

            // The service is unharmed: the legacy path still serves.
            let (items, ..) = listed(service.execute(list_request(10, false, 50, None)).unwrap());
            assert_eq!(items, vec!["sess.a".to_owned()]);
            assert!(!service.cell_poisoned_for_test());
        }

        /// Ambiguous complete facts are rejected atomically INSIDE the
        /// combined fold: no partial binding change, no partial
        /// translator effect — the held record is still held and a
        /// subsequent valid snapshot reconciles and clones in one fold.
        #[test]
        fn reconcile_and_snapshot_rejects_ambiguous_facts_atomically() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let held = service
                .native_apply_event(event(carrier, 1, host(1), hint_json(1, 1)), t0)
                .unwrap();
            assert_eq!(held.class, NativeClass::Held);

            // Ambiguous: two bindings claim the same presentation.
            let ambiguous = vec![
                binding(1, carrier),
                Binding {
                    presentation: presentation(1),
                    carrier,
                    host: host(2),
                },
            ];
            let inner = service
                .native_reconcile_and_snapshot(ambiguous, t0)
                .unwrap();
            assert!(inner.is_err());
            // Atomic rejection: the held record was not replayed and no
            // fence was dropped.
            assert_eq!(service.native_held_count().unwrap(), 1);

            // Valid facts heal: ONE fold reconciles + replays + clones.
            let snapshot = service
                .native_reconcile_and_snapshot(vec![binding(1, carrier)], t0)
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(service.native_held_count().unwrap(), 0);
            // The replayed hint is pending inside the join, not yet a
            // canonical session — the clone reflects exactly that.
            assert!(snapshot.sessions.is_empty());
        }

        /// The same-runtime prepared read: held hint -> complete typed
        /// reconciliation inside the provider's ONE fold (fence +
        /// replay consumed there) -> rich envelope through the existing
        /// hook seam -> canonical session -> prepared Get/List served
        /// from the SAME `state.runtime`. Nothing is reinjected through
        /// an old seam — the provider IS the future owner's shape.
        #[test]
        fn prepared_read_serves_the_runtime_the_leaf_reconciles() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(0xcc, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let held = service
                .native_apply_event(event(carrier, 1, host(1), hint_json(3, 3)), t0)
                .unwrap();
            assert_eq!(held.class, NativeClass::Held);

            let calls = Cell::new(0u32);
            let facts = vec![binding(1, carrier)];
            let provider = || {
                calls.set(calls.get() + 1);
                match service.native_reconcile_and_snapshot(facts.clone(), Instant::now()) {
                    Ok(Ok(Some(snapshot))) => Ok(snapshot),
                    _ => Err(PreparedUnavailable),
                }
            };

            // First prepared List: the fold reconciles, registers the
            // fence, replays the held record exactly once — the clone
            // still shows no canonical session (the hint only parks
            // inside the join).
            let (items, ..) = listed(
                service
                    .execute_prepared_for_test(list_request(1, false, 50, None), provider)
                    .unwrap(),
            );
            assert!(items.is_empty());
            assert_eq!(calls.get(), 1);
            assert_eq!(service.native_held_count().unwrap(), 0);

            // The matching rich envelope joins through the existing
            // hook seam into the SAME runtime (execution:1 minted
            // first -> session:2, revision 2).
            assert!(service.admit_hook_envelope(&envelope(3, 3), 0.0).unwrap());
            service.advance_hook_settlement(50.0, 50.0).unwrap();

            // Second prepared read: unchanged facts emit no effects and
            // the clone carries the canonical session.
            let AgentReadResult::Get { result, .. } = service
                .execute_prepared_for_test(get_request(2, "desktop:agent:session:2"), provider)
                .unwrap()
            else {
                panic!("expected a get result");
            };
            let AgentGetOutcome::Found { revision, session } = result.outcome else {
                panic!("expected the session to be found");
            };
            assert_eq!(revision.value(), 2);
            assert_eq!(
                session.snapshot.session_id.as_str(),
                "desktop:agent:session:2"
            );
            assert_eq!(session.snapshot.presentation_attachments.len(), 1);
            let attachment = &session.snapshot.presentation_attachments[0];
            assert_eq!(
                attachment.presentation_id.as_str(),
                presentation(1).as_str()
            );
            assert_eq!(attachment.generation.as_str(), "01".repeat(16));
            assert!(attachment.navigable);
            assert_eq!(calls.get(), 2);
        }

        /// Ended history under valid empty bindings: the retire effect
        /// drops the fence inside the same fold that clones, the
        /// session record stays retrievable, and no history disappears.
        #[test]
        fn ended_history_survives_empty_reconcile_through_prepared_read() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(0xdd, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let routed = service
                .native_apply_event(event(carrier, 1, host(1), hint_json(4, 4)), t0)
                .unwrap();
            // No binding yet -> Held; bind, replay, then the session.
            assert_eq!(routed.class, NativeClass::Held);
            service
                .native_reconcile_bindings(vec![binding(1, carrier)], t0)
                .unwrap()
                .unwrap();
            assert!(service.admit_hook_envelope(&envelope(4, 4), 0.0).unwrap());
            service.advance_hook_settlement(50.0, 50.0).unwrap();

            // session-ended on a second routed hint applies to the same
            // session-1 binding (oracle N3 shape).
            let routed = service
                .native_apply_event(event(carrier, 2, host(1), hint_json(5, 5)), t0)
                .unwrap();
            assert_eq!(routed.class, NativeClass::Routed);
            let ended = json!({
                "adapterId": "codex",
                "correlations": [{
                    "correlationId": format!("correlation-{:016}", 5u64),
                    "visibility": "visible",
                }],
                "eventId": format!("event-{:016}", 5u64),
                "observation": {
                    "activity": null,
                    "agentKey": null,
                    "event": "session-ended",
                    "operationKey": null,
                    "sessionKey": "session-1",
                    "turnKey": null,
                    "version": 1,
                },
                "version": 1,
            })
            .to_string()
            .into_bytes();
            assert!(service.admit_hook_envelope(&ended, 500.0).unwrap());
            service.advance_hook_settlement(50.0, 550.0).unwrap();

            // Valid EMPTY bindings through the combined fold: retire
            // the fence and clone the post-effect truth in one fold.
            let provider =
                || match service.native_reconcile_and_snapshot(Vec::new(), Instant::now()) {
                    Ok(Ok(Some(snapshot))) => Ok(snapshot),
                    _ => Err(PreparedUnavailable),
                };
            let AgentReadResult::Get { result, .. } = service
                .execute_prepared_for_test(get_request(3, "desktop:agent:session:2"), provider)
                .unwrap()
            else {
                panic!("expected a get result");
            };
            let AgentGetOutcome::Found { session, .. } = result.outcome else {
                panic!("ended history must persist");
            };
            assert_eq!(session.snapshot.lifecycle, AgentLifecycle::Ended);
            assert!(!session.snapshot.presentation_attachments[0].navigable);

            let (items, ..) = listed(
                service
                    .execute_prepared_for_test(list_request(4, true, 50, None), provider)
                    .unwrap(),
            );
            assert_eq!(items, vec!["desktop:agent:session:2".to_owned()]);
        }

        /// Cursor discipline through prepared reads: unchanged facts
        /// leave the revision stable so a minted cursor still pages;
        /// a real canonical change then makes that cursor stale — a
        /// typed `Rejected`, never an `AgentReadFault`.
        #[test]
        fn unchanged_facts_keep_revision_stable_and_real_changes_stale_cursors() {
            let service = agent_harness::seeded(&[
                agent_harness::session_observed("sess.a", 1.0),
                agent_harness::session_observed("sess.b", 2.0),
            ])
            .unwrap();
            let empty_facts =
                || match service.native_reconcile_and_snapshot(Vec::new(), Instant::now()) {
                    Ok(Ok(Some(snapshot))) => Ok(snapshot),
                    _ => Err(PreparedUnavailable),
                };

            let (_items, cursor, revision) = listed(
                service
                    .execute_prepared_for_test(list_request(1, false, 1, None), empty_facts)
                    .unwrap(),
            );
            assert_eq!(revision, 2);
            let cursor = cursor.expect("a continuation cursor was not minted");

            // Unchanged facts: the reconcile is a no-op, the revision
            // does not move, and the cursor still verifies.
            let (items, next, revision) = listed(
                service
                    .execute_prepared_for_test(
                        list_request(2, false, 1, Some(cursor.clone())),
                        empty_facts,
                    )
                    .unwrap(),
            );
            assert_eq!(items, vec!["sess.b".to_owned()]);
            assert!(next.is_none());
            assert_eq!(revision, 2);

            // A real canonical change bumps the revision; the same
            // cursor is now stale — a typed outcome, not a fault.
            service
                .seed(&agent_harness::session_observed("sess.c", 3.0))
                .unwrap();
            let AgentReadResult::List { result, .. } = service
                .execute_prepared_for_test(list_request(3, false, 1, Some(cursor)), empty_facts)
                .unwrap()
            else {
                panic!("expected a list result");
            };
            assert_eq!(
                result.outcome,
                AgentListOutcome::Rejected {
                    reason: AgentListRejectionReason::CursorStale,
                }
            );
        }

        /// The real poison boundary through the new path: a panicked
        /// combined leaf+effect fold permanently poisons the cell, the
        /// provider's helper call surfaces the fold `Err`, and the
        /// executor maps it to `RuntimeUnavailable` — the same axis as
        /// the legacy read.
        #[test]
        fn poisoned_cell_surfaces_unavailable_through_the_prepared_path() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            assert!(
                service
                    .native_poison_after_mutation_for_test(carrier, t0)
                    .is_err()
            );
            assert!(service.cell_poisoned_for_test());

            let provider =
                || match service.native_reconcile_and_snapshot(Vec::new(), Instant::now()) {
                    Ok(Ok(Some(snapshot))) => Ok(snapshot),
                    _ => Err(PreparedUnavailable),
                };
            let AgentReadResult::List {
                request_id: echoed,
                result,
            } = service
                .execute_prepared_for_test(list_request(1, false, 50, None), provider)
                .unwrap()
            else {
                panic!("expected a list result");
            };
            assert_eq!(echoed, request_id(1));
            assert_eq!(
                result.outcome,
                AgentListOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                }
            );

            let AgentReadResult::List { result, .. } =
                service.execute(list_request(2, false, 50, None)).unwrap()
            else {
                panic!("expected a list result");
            };
            assert_eq!(
                result.outcome,
                AgentListOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                }
            );
        }
    }

    /// Drain page/final-fence slice (Root28): the typed reply page is
    /// lifted to domain facts before the fold, applied whole under one
    /// full-identity guard, and the final clone runs behind the same
    /// live identity+watermark fence — the T9-T11/E1-E8 deterministic
    /// expectations against the real service and leaf folds. There is
    /// no public route to hand arbitrary facts to either seam: the only
    /// page input is the validated `AgentDrainReply` itself.
    mod drain_fence {
        use orchbee_capability_node_protocol::{AgentDrainEntry, AgentDrainReply};

        use super::*;
        use crate::agent::AgentRuntimeSnapshot;
        use crate::agent_service::{DrainExpectation, DrainPageOutcome, FinalFence};

        fn drain_event(sequence: u64, seed: u8) -> AgentDrainEntry {
            AgentDrainEntry::Event {
                sequence: NonZeroU64::new(sequence).unwrap(),
                identity: host(seed),
                payload: hint_json(sequence, sequence),
            }
        }

        fn drain_gap(low: u64, high: u64) -> AgentDrainEntry {
            AgentDrainEntry::Gap {
                low: NonZeroU64::new(low).unwrap(),
                high,
            }
        }

        fn reply(tail: u64, entries: Vec<AgentDrainEntry>) -> AgentDrainReply {
            AgentDrainReply::new(NonZeroU64::new(1).unwrap(), 0, tail, entries).unwrap()
        }

        fn fenced(
            service: &AgentService,
            carrier: CarrierIdentity,
            barrier: u64,
        ) -> Result<Result<Option<AgentRuntimeSnapshot>, FinalFence>, AgentFoldUnavailable>
        {
            service.native_fenced_final_snapshot(
                DrainExpectation::Carrier {
                    req_carrier: carrier,
                    barrier,
                },
                Vec::new(),
                Instant::now(),
            )
        }

        /// E1/T9 — a valid EMPTY page is a real capture: zero applies
        /// still return the live W, and the fenced final serves.
        #[test]
        fn empty_page_captures_the_live_watermark() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 7);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();

            let outcome = service
                .native_apply_drain_page(carrier, &reply(0, Vec::new()), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 0 });

            let snapshot = fenced(&service, carrier, 0).unwrap().unwrap();
            assert!(snapshot.is_some());
        }

        /// E2 — an all-dedup page (coverage at or below the live W,
        /// including a stale tail below it) still returns the live W.
        #[test]
        fn all_dedup_and_stale_pages_return_the_live_watermark() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let outcome = service
                .native_apply_drain_page(
                    carrier,
                    &reply(4, vec![drain_event(1, 1), drain_gap(2, 4)]),
                    t0,
                )
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 4 });

            // Fully deduped coverage (tail == W): Applied, W unmoved.
            let outcome = service
                .native_apply_drain_page(carrier, &reply(4, vec![drain_gap(1, 4)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 4 });
            // A stale tail below W is still a legal capture, not a rotation.
            let outcome = service
                .native_apply_drain_page(carrier, &reply(2, vec![drain_gap(1, 2)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 4 });
        }

        /// E3 — the fold-level fence is a VALUE, not an inference from
        /// per-entry discards: an absent or different carrier refuses
        /// before any apply, with zero mutation.
        #[test]
        fn mismatched_or_absent_carrier_applies_nothing() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();

            // No carrier installed at all.
            let outcome = service
                .native_apply_drain_page(carrier_id(1, 1), &reply(1, vec![drain_event(1, 1)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::CarrierMismatch);

            // A different full identity while (A,1) is installed.
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let outcome = service
                .native_apply_drain_page(carrier_id(2, 1), &reply(1, vec![drain_event(1, 1)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::CarrierMismatch);
            assert_eq!(service.native_held_count().unwrap(), 0);
            // State was untouched: a valid apply under A sees fresh W.
            let outcome = service
                .native_apply_drain_page(carrier, &reply(1, vec![drain_event(1, 1)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 1 });
        }

        /// E4/T10 — whole-page atomicity: mixed event+gap coverage
        /// lands whole inside one guard, and a mismatched page leaves
        /// zero effects and zero held records — retirement needs this
        /// same guard, so there is no mid-page interleave to observe.
        #[test]
        fn page_applies_whole_or_not_at_all() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();

            let outcome = service
                .native_apply_drain_page(
                    carrier,
                    &reply(
                        4,
                        vec![drain_event(1, 1), drain_event(2, 1), drain_gap(3, 4)],
                    ),
                    t0,
                )
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 4 });
            // Both event payloads held pending binding — the held count
            // is the zero-mutation witness for the mismatched page.
            assert_eq!(service.native_held_count().unwrap(), 2);
            assert!(listed_sessions(&service).is_empty());

            let outcome = service
                .native_apply_drain_page(
                    carrier_id(2, 1),
                    &reply(9, vec![drain_event(5, 2), drain_gap(6, 9)]),
                    t0,
                )
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::CarrierMismatch);
            assert_eq!(service.native_held_count().unwrap(), 2);
            let outcome = service
                .native_apply_drain_page(carrier, &reply(5, vec![drain_event(5, 1)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 5 });
        }

        /// MAX-entries page: 480 contiguous GAP entries (17 bytes each,
        /// 8160 <= 8165) apply whole — the legal page bound is real.
        #[test]
        fn maximum_legal_page_applies_whole() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();

            let entries: Vec<AgentDrainEntry> = (1..=480).map(|i| drain_gap(i, i)).collect();
            let outcome = service
                .native_apply_drain_page(carrier, &reply(480, entries), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 480 });
        }

        /// E5/T11 — replacement between apply and final: the in-fold
        /// fence compares the live tuple, so a stale barrier under the
        /// old identity can never justify the new carrier's snapshot.
        /// Covers same-generation different-worker replacement.
        #[test]
        fn replaced_carrier_fences_the_final_snapshot() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let old = carrier_id(1, 7);
            service.native_admit_carrier(old, t0).unwrap().unwrap();
            let outcome = service
                .native_apply_drain_page(old, &reply(8, vec![drain_gap(1, 8)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 8 });

            service.native_retire_carrier(old, t0).unwrap();
            let new = carrier_id(2, 7); // same generation, different worker
            service.native_admit_carrier(new, t0).unwrap().unwrap();

            let result = fenced(&service, old, 8).unwrap();
            assert!(matches!(result, Err(FinalFence::CarrierMismatch)));
            // The new identity's own expectation serves normally.
            let snapshot = fenced(&service, new, 0).unwrap().unwrap();
            assert!(snapshot.is_some());
        }

        /// E6 — NoCarrier is the read's true-none only while the leaf
        /// is empty: a carrier admitted before the final fold surfaces
        /// CarrierAppeared, never a silent stale serve.
        #[test]
        fn appeared_carrier_fences_the_nocarrier_expectation() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();

            let snapshot = service
                .native_fenced_final_snapshot(DrainExpectation::NoCarrier, Vec::new(), t0)
                .unwrap()
                .unwrap();
            assert!(snapshot.is_some());

            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let result = service
                .native_fenced_final_snapshot(DrainExpectation::NoCarrier, Vec::new(), t0)
                .unwrap();
            assert!(matches!(result, Err(FinalFence::CarrierAppeared)));
        }

        /// E7 — the barrier is a typed gate, not a serve condition:
        /// met serves; unmet refuses with the live values.
        #[test]
        fn barrier_met_serves_and_unmet_refuses() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();
            let outcome = service
                .native_apply_drain_page(carrier, &reply(8, vec![drain_gap(1, 8)]), t0)
                .unwrap()
                .unwrap();
            assert_eq!(outcome, DrainPageOutcome::Applied { w: 8 });

            let snapshot = fenced(&service, carrier, 8).unwrap().unwrap();
            assert!(snapshot.is_some());
            let result = fenced(&service, carrier, 9).unwrap();
            assert!(matches!(
                result,
                Err(FinalFence::BarrierUnmet { w: 8, barrier: 9 })
            ));
        }

        /// E8 — two distinct refusal axes stay separate: an ambiguous
        /// facts snapshot is Rejected atomically with the leaf
        /// untouched, while a fold panic poisons the whole cell —
        /// permanently and for every seam.
        #[test]
        fn atomic_rejection_and_fold_poison_are_distinct_axes() {
            let service = AgentService::open(AgentPolicy {
                read: true,
                manage: true,
            })
            .unwrap();
            let t0 = Instant::now();
            let carrier = carrier_id(1, 1);
            service.native_admit_carrier(carrier, t0).unwrap().unwrap();

            // Duplicate presentation makes the snapshot ambiguous.
            let result = service
                .native_fenced_final_snapshot(
                    DrainExpectation::Carrier {
                        req_carrier: carrier,
                        barrier: 0,
                    },
                    vec![binding(1, carrier), binding(1, carrier)],
                    t0,
                )
                .unwrap();
            match result {
                Err(FinalFence::Rejected(error)) => {
                    assert_eq!(error.0, "binding snapshot is not one-to-one");
                }
                _ => panic!("expected a typed rejection"),
            }
            // Leaf untouched: a valid final still serves afterwards.
            let snapshot = fenced(&service, carrier, 0).unwrap().unwrap();
            assert!(snapshot.is_some());

            // A fold panic poisons the whole cell — both new seams and
            // the pre-existing ones report the same unavailability.
            let fault = service.native_poison_after_mutation_for_test(carrier, t0);
            assert!(fault.is_err());
            assert!(
                service
                    .native_apply_drain_page(carrier, &reply(0, Vec::new()), t0)
                    .is_err()
            );
            assert!(fenced(&service, carrier, 0).is_err());
            assert!(service.native_held_count().is_err());
        }
    }
}

impl AgentService {
    pub(crate) fn execute(
        &self,
        request: AgentReadRequest,
    ) -> Result<AgentReadResult, AgentReadFault> {
        self.execute_prepared(request, || match self.snapshot() {
            Ok(Some(snapshot)) => Ok(snapshot),
            Ok(None) | Err(_) => Err(PreparedUnavailable),
        })
    }
}

use crate::agent::native_observation::{CarrierIdentity, NativeClass, NativeEvent};
use std::time::Instant;
pub(crate) struct NativeOutcome {
    class: NativeClass,
    ack_w: Option<u64>,
}
impl NativeOutcome {
    #[cfg(unix)]
    pub(crate) fn class(&self) -> NativeClass {
        self.class
    }
}
impl AgentService {
    pub(crate) fn native_apply_event(
        &self,
        event: NativeEvent,
        now: Instant,
    ) -> Result<NativeOutcome, AgentFoldUnavailable> {
        self.with_state(|state| {
            let transition = state.native.apply_event_observed(event, now);
            state.apply_native_effects(transition.effects, now);
            NativeOutcome {
                class: transition.class,
                ack_w: transition.ack_w,
            }
        })
    }
    pub(crate) fn native_apply_gap(
        &self,
        carrier: CarrierIdentity,
        lo: u64,
        hi: u64,
        now: Instant,
    ) -> Result<NativeOutcome, AgentFoldUnavailable> {
        self.with_state(|state| {
            let transition = state.native.apply_gap_observed(&carrier, lo, hi, now);
            state.apply_native_effects(transition.effects, now);
            NativeOutcome {
                class: transition.class,
                ack_w: transition.ack_w,
            }
        })
    }
}
