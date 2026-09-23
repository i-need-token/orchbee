use super::*;
use crate::agent_service::AgentPolicy;
use crate::local_client_service::agent_harness::{seeded, session_ended, session_observed};
use orchbee_agent_observation_protocol::ObapRequestId;

fn request_id() -> ObapRequestId {
    ObapRequestId::parse("11111111111111111111111111111111").unwrap()
}

fn id(value: &str) -> AgentSessionId {
    AgentSessionId::parse(value).unwrap()
}

fn name(value: &str) -> AgentName {
    AgentName::parse(value).unwrap()
}

fn target(value: &str) -> AgentSessionSelector {
    AgentSessionSelector::Auto { target: id(value) }
}

fn execute(service: &AgentService, request: AgentNamesRequest) -> AgentNamesResult {
    service.execute_names_prepared(request, || {
        service.snapshot().ok().flatten().ok_or(PreparedUnavailable)
    })
}

fn assign(
    service: &AgentService,
    target: AgentSessionSelector,
    value: &str,
) -> AgentNameAssignOutcome {
    let AgentNamesResult::NameAssign { result, .. } = execute(
        service,
        AgentNamesRequest::NameAssign {
            request_id: request_id(),
            target,
            name: name(value),
        },
    ) else {
        panic!("assignment result");
    };
    result.outcome
}

fn resolve(service: &AgentService, target: AgentSessionSelector) -> AgentResolveOutcome {
    let AgentNamesResult::Resolve { result, .. } = execute(
        service,
        AgentNamesRequest::Resolve {
            request_id: request_id(),
            target,
        },
    ) else {
        panic!("resolve result");
    };
    result.outcome
}

fn bindings(service: &AgentService) -> Vec<AgentNameBinding> {
    let AgentNamesResult::NamesList { result, .. } = execute(
        service,
        AgentNamesRequest::NamesList {
            request_id: request_id(),
        },
    ) else {
        panic!("list result");
    };
    let AgentNamesListOutcome::Listed { bindings } = result.outcome else {
        panic!("available names");
    };
    bindings
}

#[test]
fn rename_is_atomic_idempotent_and_collision_preserves_the_incumbent() {
    let service = seeded(&[session_observed("a", 1.0), session_observed("b", 1.0)]).unwrap();
    assert_eq!(
        assign(&service, target("a"), "alpha"),
        AgentNameAssignOutcome::Assigned {
            session_id: id("a"),
            name: name("alpha"),
            previous_name: None,
        }
    );
    assert_eq!(
        assign(&service, target("alpha"), "alpha"),
        AgentNameAssignOutcome::Unchanged {
            session_id: id("a"),
            name: name("alpha"),
        }
    );
    assert_eq!(
        assign(&service, target("b"), "alpha"),
        AgentNameAssignOutcome::Rejected {
            reason: AgentNameAssignRejectionReason::NameInUse,
            conflict_session_id: Some(id("a")),
        }
    );
    assert_eq!(
        assign(&service, target("alpha"), "renamed"),
        AgentNameAssignOutcome::Assigned {
            session_id: id("a"),
            name: name("renamed"),
            previous_name: Some(name("alpha")),
        }
    );
    assert_eq!(
        bindings(&service)
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["renamed"]
    );
    assert!(matches!(
        resolve(&service, target("alpha")),
        AgentResolveOutcome::Rejected {
            reason: AgentResolveRejectionReason::NotFound
        }
    ));
}

#[test]
fn exact_identity_wins_and_explicit_name_escapes_the_collision() {
    let service = seeded(&[session_observed("alpha", 1.0), session_observed("b", 1.0)]).unwrap();
    assign(&service, target("b"), "alpha");
    assert_eq!(
        resolve(&service, target("alpha")),
        AgentResolveOutcome::Resolved {
            session_id: id("alpha"),
            resolved_by: AgentResolvedBy::SessionId
        }
    );
    assert_eq!(
        resolve(
            &service,
            AgentSessionSelector::Name {
                name: name("alpha")
            }
        ),
        AgentResolveOutcome::Resolved {
            session_id: id("b"),
            resolved_by: AgentResolvedBy::Name
        }
    );
    service.seed(&session_ended("alpha", 1.0, 2.0)).unwrap();
    // Retained ended identities still win; they cannot acquire a live alias.
    assert_eq!(
        resolve(&service, target("alpha")),
        AgentResolveOutcome::Resolved {
            session_id: id("alpha"),
            resolved_by: AgentResolvedBy::SessionId
        }
    );
    assert!(matches!(
        assign(&service, target("alpha"), "ended"),
        AgentNameAssignOutcome::Rejected {
            reason: AgentNameAssignRejectionReason::SessionEnded,
            ..
        }
    ));
}

#[test]
fn lifecycle_fold_frees_names_and_release_distinguishes_missing_from_unnamed() {
    let service = seeded(&[session_observed("a", 1.0), session_observed("b", 1.0)]).unwrap();
    assign(&service, target("a"), "alpha");
    service.seed(&session_ended("a", 1.0, 2.0)).unwrap();
    assert!(bindings(&service).is_empty());
    assert!(matches!(
        assign(&service, target("b"), "alpha"),
        AgentNameAssignOutcome::Assigned { .. }
    ));
    let release = |target| {
        let AgentNamesResult::NameRelease { result, .. } = execute(
            &service,
            AgentNamesRequest::NameRelease {
                request_id: request_id(),
                target,
            },
        ) else {
            panic!("release");
        };
        result.outcome
    };
    assert_eq!(
        release(target("alpha")),
        AgentNameReleaseOutcome::Released {
            session_id: id("b"),
            name: name("alpha")
        }
    );
    assert_eq!(
        release(target("b")),
        AgentNameReleaseOutcome::Unchanged {
            session_id: id("b")
        }
    );
    assert_eq!(
        release(AgentSessionSelector::Name {
            name: name("alpha")
        }),
        AgentNameReleaseOutcome::Rejected {
            reason: AgentNameReleaseRejectionReason::NameNotFound
        }
    );
    assert_eq!(
        release(AgentSessionSelector::Session {
            session_id: id("absent")
        }),
        AgentNameReleaseOutcome::Rejected {
            reason: AgentNameReleaseRejectionReason::SessionNotFound
        }
    );
}

#[test]
fn policy_precedes_preparation_and_read_does_not_grant_manage() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: false,
    })
    .unwrap();
    let result = service.execute_names_prepared(
        AgentNamesRequest::NameAssign {
            request_id: request_id(),
            target: target("absent"),
            name: name("alpha"),
        },
        || panic!("denied manage must not prepare"),
    );
    assert!(matches!(
        result,
        AgentNamesResult::NameAssign {
            result: AgentNameAssignOutcomeEnvelope {
                outcome: AgentNameAssignOutcome::Rejected {
                    reason: AgentNameAssignRejectionReason::ManageDenied,
                    ..
                }
            },
            ..
        }
    ));
    assert!(bindings(&service).is_empty());
    let service = AgentService::open(AgentPolicy {
        read: false,
        manage: true,
    })
    .unwrap();
    let result = service.execute_names_prepared(
        AgentNamesRequest::NamesList {
            request_id: request_id(),
        },
        || panic!("denied read must not prepare"),
    );
    assert!(matches!(
        result,
        AgentNamesResult::NamesList {
            result: AgentNamesListOutcomeEnvelope {
                outcome: AgentNamesListOutcome::Rejected {
                    reason: AgentNamesListRejectionReason::ReadDenied
                }
            },
            ..
        }
    ));
}

#[test]
fn preparation_is_not_a_stale_liveness_authority_and_faults_never_fabricate_empty_names() {
    let service = seeded(&[session_observed("a", 1.0)]).unwrap();
    let stale = service.snapshot().unwrap().unwrap();
    let result = service.execute_names_prepared(
        AgentNamesRequest::NameAssign {
            request_id: request_id(),
            target: target("a"),
            name: name("alpha"),
        },
        || {
            service.seed(&session_ended("a", 1.0, 2.0)).unwrap();
            Ok(stale)
        },
    );
    assert!(matches!(
        result,
        AgentNamesResult::NameAssign {
            result: AgentNameAssignOutcomeEnvelope {
                outcome: AgentNameAssignOutcome::Rejected {
                    reason: AgentNameAssignRejectionReason::SessionEnded,
                    ..
                }
            },
            ..
        }
    ));
    let unavailable = |result| {
        assert!(matches!(
            result,
            AgentNamesResult::NamesList {
                result: AgentNamesListOutcomeEnvelope {
                    outcome: AgentNamesListOutcome::Unavailable { .. }
                },
                ..
            }
        ))
    };
    unavailable(service.execute_names_prepared(
        AgentNamesRequest::NamesList {
            request_id: request_id(),
        },
        || panic!("failed preparation"),
    ));
    service.shutdown();
    unavailable(execute(
        &service,
        AgentNamesRequest::NamesList {
            request_id: request_id(),
        },
    ));
    let poisoned = seeded(&[session_observed("a", 1.0)]).unwrap();
    assert!(
        poisoned
            .poison_state_after_mutation_for_test(&session_ended("a", 1.0, 2.0))
            .is_err()
    );
    unavailable(execute(
        &poisoned,
        AgentNamesRequest::NamesList {
            request_id: request_id(),
        },
    ));
}

#[test]
fn full_table_still_allows_atomic_rename_and_lists_in_name_order() {
    let events = (0..MAXIMUM_NAME_BINDINGS)
        .map(|n| session_observed(&format!("s{n:03}"), 1.0))
        .collect::<Vec<_>>();
    let service = seeded(&events).unwrap();
    for n in (0..MAXIMUM_NAME_BINDINGS).rev() {
        assert!(matches!(
            assign(&service, target(&format!("s{n:03}")), &format!("n{n:03}")),
            AgentNameAssignOutcome::Assigned { .. }
        ));
    }
    assert_eq!(bindings(&service).len(), MAXIMUM_NAME_BINDINGS);
    assert!(matches!(
        assign(&service, target("n000"), "renamed"),
        AgentNameAssignOutcome::Assigned {
            previous_name: Some(_),
            ..
        }
    ));
    let rows = bindings(&service);
    assert_eq!(rows.len(), MAXIMUM_NAME_BINDINGS);
    assert!(rows.windows(2).all(|pair| pair[0].name < pair[1].name));
}

#[test]
fn concurrent_claims_commit_exactly_one_owner() {
    let service = seeded(&[session_observed("a", 1.0), session_observed("b", 1.0)]).unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            assign(&service, target("a"), "shared")
        });
        let second = scope.spawn(|| {
            barrier.wait();
            assign(&service, target("b"), "shared")
        });
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(
            results
                .iter()
                .filter(|outcome| matches!(outcome, AgentNameAssignOutcome::Assigned { .. }))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    AgentNameAssignOutcome::Rejected {
                        reason: AgentNameAssignRejectionReason::NameInUse,
                        ..
                    }
                ))
                .count(),
            1
        );
    });
    assert_eq!(bindings(&service).len(), 1);
}

#[test]
fn presentation_selectors_require_one_live_navigable_session_and_mutate_atomically() {
    use crate::agent::{AgentObservationEvent, RawAttachment};
    use orchbee_agent_observation_protocol::AgentPresentationId;
    let presentation = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let selector = || AgentSessionSelector::Presentation {
        presentation_id: AgentPresentationId::parse(presentation).unwrap(),
    };
    let attached = |id: &str, navigable, visibility: &str| {
        let mut event = session_observed(id, 1.0);
        let AgentObservationEvent::SessionObserved {
            presentation_attachments,
            ..
        } = &mut event
        else {
            unreachable!()
        };
        presentation_attachments.push(RawAttachment {
            generation: "1".to_owned(),
            navigable,
            presentation_id: presentation.to_owned(),
            visibility: visibility.to_owned(),
        });
        event
    };
    let service = seeded(&[
        attached("a", true, "hidden"),
        attached("b", false, "visible"),
    ])
    .unwrap();
    assert_eq!(
        resolve(&service, selector()),
        AgentResolveOutcome::Resolved {
            session_id: id("a"),
            resolved_by: AgentResolvedBy::Presentation,
        }
    );
    assert!(matches!(
        assign(&service, selector(), "alpha"),
        AgentNameAssignOutcome::Assigned { .. }
    ));
    service.seed(&attached("c", true, "visible")).unwrap();
    assert_eq!(
        resolve(&service, selector()),
        AgentResolveOutcome::Rejected {
            reason: AgentResolveRejectionReason::AmbiguousTarget,
        }
    );
    assert_eq!(
        assign(&service, selector(), "renamed"),
        AgentNameAssignOutcome::Rejected {
            reason: AgentNameAssignRejectionReason::AmbiguousTarget,
            conflict_session_id: None,
        }
    );
    let AgentNamesResult::NameRelease { result, .. } = execute(
        &service,
        AgentNamesRequest::NameRelease {
            request_id: request_id(),
            target: selector(),
        },
    ) else {
        panic!("release result")
    };
    assert_eq!(
        result.outcome,
        AgentNameReleaseOutcome::Rejected {
            reason: AgentNameReleaseRejectionReason::AmbiguousTarget
        }
    );
    assert_eq!(bindings(&service)[0].name, name("alpha"));
    service.seed(&session_ended("c", 1.0, 2.0)).unwrap();
    assert!(
        matches!(resolve(&service, selector()), AgentResolveOutcome::Resolved { session_id, .. } if session_id == id("a"))
    );
    service.seed(&session_ended("a", 1.0, 2.0)).unwrap();
    assert_eq!(
        resolve(&service, selector()),
        AgentResolveOutcome::Rejected {
            reason: AgentResolveRejectionReason::NotFound
        }
    );
}
