use std::io::Cursor;

use crate::*;

// The session-read fixtures below exercise the Read arm of the expanded OBAP
// union. Keep their round trips typed as reads while checking the public wire
// entry points used by both reads and names.
fn parse_agent_request(encoded: &[u8]) -> Result<AgentReadRequest, AgentContractError> {
    match crate::parse_agent_request(encoded)? {
        AgentRequest::Read(request) => Ok(request),
        AgentRequest::Names(_) => Err(AgentContractError::InvalidDocument),
    }
}

fn serialize_agent_request(request: &AgentReadRequest) -> Result<Vec<u8>, AgentContractError> {
    crate::serialize_agent_request(&request.clone().into())
}

fn parse_agent_result(encoded: &[u8]) -> Result<AgentReadResult, AgentContractError> {
    match crate::parse_agent_result(encoded)? {
        AgentResult::Read(result) => Ok(result),
        AgentResult::Names(_) => Err(AgentContractError::InvalidDocument),
    }
}

fn serialize_agent_result(result: &AgentReadResult) -> Result<Vec<u8>, AgentContractError> {
    crate::serialize_agent_result(&result.clone().into())
}

fn read_agent_request(reader: &mut impl std::io::Read) -> Result<AgentReadRequest, AgentWireError> {
    match crate::read_agent_request(reader)? {
        AgentRequest::Read(request) => Ok(request),
        AgentRequest::Names(_) => Err(AgentWireError::InvalidHeader),
    }
}

fn write_agent_request(
    writer: &mut impl std::io::Write,
    request: &AgentReadRequest,
) -> Result<(), AgentWireError> {
    crate::write_agent_request(writer, &request.clone().into())
}

fn read_agent_result(reader: &mut impl std::io::Read) -> Result<AgentReadResult, AgentWireError> {
    match crate::read_agent_result(reader)? {
        AgentResult::Read(result) => Ok(result),
        AgentResult::Names(_) => Err(AgentWireError::InvalidHeader),
    }
}

fn write_agent_result(
    writer: &mut impl std::io::Write,
    result: &AgentReadResult,
) -> Result<(), AgentWireError> {
    crate::write_agent_result(writer, &result.clone().into())
}

fn request_id(text: &str) -> ObapRequestId {
    ObapRequestId::parse(text).unwrap()
}

const RID: &str = "0123456789abcdef0123456789abcdef";
const PID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CURSOR_TEXT: &str = "v1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn summary(session: &str) -> AgentSessionSummary {
    AgentSessionSummary {
        execution_location: AgentExecutionLocation {
            execution_location_id: AgentExecutionLocationId::parse("local").unwrap(),
        },
        lifecycle: AgentLifecycle::Live,
        observation: AgentObservation {
            fidelity: AgentObservationFidelity::Rich,
            freshness: AgentObservationFreshness::Fresh,
            generation: AgentSafeInteger::new(7),
            sequence: AgentSafeInteger::new(3),
            source: AgentObservationSource::Native,
        },
        parent_session_id: None,
        pending_request_count: AgentSafeInteger::new(0),
        presentation_attachments: vec![AgentPresentationAttachment {
            generation: AgentAttachmentGeneration::parse("gen-1").unwrap(),
            navigable: true,
            presentation_id: AgentPresentationId::parse(PID).unwrap(),
            visibility: AgentPresentationVisibility::Visible,
        }],
        provider_id: AgentProviderId::parse("devin").unwrap(),
        session_id: AgentSessionId::parse(session).unwrap(),
        status: AgentSemanticStatus::Idle {},
    }
}

fn snapshot(session: &str) -> AgentSessionSnapshot {
    AgentSessionSnapshot {
        activity_hint: None,
        capabilities: vec![AgentCapability::SessionLifecycle],
        execution_location: AgentExecutionLocation {
            execution_location_id: AgentExecutionLocationId::parse("local").unwrap(),
        },
        last_outcome: None,
        lifecycle: AgentLifecycle::Live,
        observation: AgentObservation {
            fidelity: AgentObservationFidelity::Basic,
            freshness: AgentObservationFreshness::Stale,
            generation: AgentSafeInteger::new(2),
            sequence: AgentSafeInteger::new(9),
            source: AgentObservationSource::Hook,
        },
        parent_session_id: None,
        pending_requests: vec![],
        presentation_attachments: vec![],
        provider_id: AgentProviderId::parse("devin").unwrap(),
        session_id: AgentSessionId::parse(session).unwrap(),
        turn: AgentTurnState::Idle {},
    }
}

fn detail(session: &str) -> AgentSessionDetail {
    AgentSessionDetail {
        snapshot: snapshot(session),
        status: AgentSemanticStatus::Idle {},
    }
}

fn explanation() -> AgentSessionExplanation {
    AgentSessionExplanation {
        action_path: NavigateOnly,
        capability_ceiling: vec![AgentCapability::SessionLifecycle],
        content_available: ContentUnavailable,
        fidelity: AgentObservationFidelity::Basic,
        freshness: AgentObservationFreshness::Stale,
        freshness_meaning: AgentFreshnessMeaning::LastObservationMayBeOutdated,
        generation: AgentSafeInteger::new(2),
        sequence: AgentSafeInteger::new(9),
        source: AgentObservationSource::Hook,
        status: AgentSemanticStatus::Idle {},
        status_basis: AgentStatusBasis::AffirmativeIdle,
    }
}

#[test]
fn typed_requests_round_trip() {
    let list = AgentReadRequest::List {
        request_id: request_id(RID),
        include_ended: true,
        limit: AgentSafeInteger::new(25),
        cursor: Some(AgentListCursor::parse(CURSOR_TEXT).unwrap()),
    };
    let encoded = serialize_agent_request(&list).unwrap();
    assert_eq!(parse_agent_request(&encoded).unwrap(), list);

    let get = AgentReadRequest::Get {
        request_id: request_id(RID),
        session_id: AgentSessionId::parse("session-1").unwrap(),
    };
    let encoded = serialize_agent_request(&get).unwrap();
    assert_eq!(
        String::from_utf8(encoded.clone()).unwrap(),
        "{\"kind\":\"agent.session.get.request\",\"requestId\":\"0123456789abcdef0123456789abcdef\",\"sessionId\":\"session-1\"}"
    );
    assert_eq!(parse_agent_request(&encoded).unwrap(), get);
}

#[test]
fn list_request_defaults_apply() {
    let parsed = parse_agent_request(
        br#"{"kind":"agent.session.list.request","requestId":"0123456789abcdef0123456789abcdef"}"#,
    )
    .unwrap();
    let AgentReadRequest::List {
        include_ended,
        limit,
        cursor,
        ..
    } = parsed
    else {
        panic!("expected list request")
    };
    assert!(!include_ended);
    assert_eq!(limit.value(), DEFAULT_LIST_LIMIT);
    assert!(cursor.is_none());
}

/// Schema-vs-binding regression: the list `cursor` key is absent-or-string.
/// The default first-page request must serialize without the key, an explicit
/// `null` must be rejected, and a string cursor must round-trip.
#[test]
fn list_cursor_is_absent_or_string_never_null() {
    let first_page = AgentReadRequest::List {
        request_id: request_id(RID),
        include_ended: false,
        limit: AgentSafeInteger::new(DEFAULT_LIST_LIMIT),
        cursor: None,
    };
    let encoded = serialize_agent_request(&first_page).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert!(!document.as_object().unwrap().contains_key("cursor"));
    assert_eq!(parse_agent_request(&encoded).unwrap(), first_page);

    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.list.request","requestId":"0123456789abcdef0123456789abcdef","cursor":null}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.list.request","requestId":"0123456789abcdef0123456789abcdef","cursor":7}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );

    let with_cursor = format!(
        "{{\"kind\":\"agent.session.list.request\",\"requestId\":\"{RID}\",\"cursor\":\"{CURSOR_TEXT}\"}}"
    );
    let parsed = parse_agent_request(with_cursor.as_bytes()).unwrap();
    let AgentReadRequest::List {
        cursor: Some(_), ..
    } = parsed
    else {
        panic!("expected list request carrying a cursor")
    };
    let reencoded = serialize_agent_request(&parsed).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&reencoded).unwrap();
    assert_eq!(
        document.as_object().unwrap()["cursor"],
        serde_json::Value::String(CURSOR_TEXT.to_owned())
    );
    assert_eq!(parse_agent_request(&reencoded).unwrap(), parsed);
}

/// Schema-vs-binding regression: `working.activity` and
/// `unknown.activityHint` are required-but-nullable — a missing key is a
/// contract violation while explicit `null` is the only accepted absence.
#[test]
fn semantic_status_required_nullable_keys() {
    let result = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(AgentSessionDetail {
                    snapshot: snapshot("s"),
                    status: AgentSemanticStatus::Working {
                        activity: None,
                        phase: AgentTurnPhase::Queued,
                    },
                }),
            },
        },
    };
    let encoded = serialize_agent_result(&result).unwrap();

    // The producer emits the required key as explicit null.
    let document: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    let status = document
        .pointer("/result/outcome/session/status")
        .unwrap()
        .as_object()
        .unwrap();
    assert_eq!(status["activity"], serde_json::Value::Null);

    // Dropping the key is a schema violation, not a silent None.
    let mut broken = document.clone();
    broken
        .pointer_mut("/result/outcome/session/status")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("activity");
    assert_eq!(
        parse_agent_result(&serde_json::to_vec(&broken).unwrap()),
        Err(AgentContractError::InvalidDocument)
    );

    // `unknown` without `activityHint` is rejected; with explicit null it
    // decodes.
    let mut unknown = document.clone();
    *unknown
        .pointer_mut("/result/outcome/session/status")
        .unwrap() = serde_json::json!({ "kind": "unknown" });
    assert_eq!(
        parse_agent_result(&serde_json::to_vec(&unknown).unwrap()),
        Err(AgentContractError::InvalidDocument)
    );
    unknown
        .pointer_mut("/result/outcome/session/status")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("activityHint".to_owned(), serde_json::Value::Null);
    assert!(parse_agent_result(&serde_json::to_vec(&unknown).unwrap()).is_ok());
}

#[test]
fn typed_results_round_trip_all_outcomes() {
    let results = [
        AgentReadResult::List {
            request_id: request_id(RID),
            result: AgentListOutcomeEnvelope {
                outcome: AgentListOutcome::Listed {
                    items: vec![summary("s-1"), summary("s-2")],
                    next_cursor: Some(AgentListCursor::parse(CURSOR_TEXT).unwrap()),
                    revision: AgentSafeInteger::new(41),
                },
            },
        },
        AgentReadResult::List {
            request_id: request_id(RID),
            result: AgentListOutcomeEnvelope {
                outcome: AgentListOutcome::Rejected {
                    reason: AgentListRejectionReason::CursorStale,
                },
            },
        },
        AgentReadResult::List {
            request_id: request_id(RID),
            result: AgentListOutcomeEnvelope {
                outcome: AgentListOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                },
            },
        },
        AgentReadResult::Get {
            request_id: request_id(RID),
            result: AgentGetOutcomeEnvelope {
                outcome: AgentGetOutcome::Found {
                    revision: AgentSafeInteger::new(9),
                    session: Box::new(detail("s-1")),
                },
            },
        },
        AgentReadResult::Get {
            request_id: request_id(RID),
            result: AgentGetOutcomeEnvelope {
                outcome: AgentGetOutcome::Rejected {
                    reason: AgentSessionRejectionReason::NotFound,
                },
            },
        },
        AgentReadResult::Explain {
            request_id: request_id(RID),
            result: AgentExplainOutcomeEnvelope {
                outcome: AgentExplainOutcome::Found {
                    revision: AgentSafeInteger::new(9),
                    session: Box::new(detail("s-1")),
                    explanation: explanation(),
                },
            },
        },
        AgentReadResult::Explain {
            request_id: request_id(RID),
            result: AgentExplainOutcomeEnvelope {
                outcome: AgentExplainOutcome::Rejected {
                    reason: AgentSessionRejectionReason::ReadDenied,
                },
            },
        },
    ];
    for result in results {
        let encoded = serialize_agent_result(&result).unwrap();
        assert_eq!(parse_agent_result(&encoded).unwrap(), result);
        let mut frame = Vec::new();
        write_agent_result(&mut frame, &result).unwrap();
        let mut cursor = Cursor::new(frame);
        assert_eq!(read_agent_result(&mut cursor).unwrap(), result);
    }
}

#[test]
fn frame_round_trip_preserves_kind() {
    let request = AgentReadRequest::Explain {
        request_id: request_id(RID),
        session_id: AgentSessionId::parse("s-9").unwrap(),
    };
    let mut frame = Vec::new();
    write_agent_request(&mut frame, &request).unwrap();
    assert_eq!(&frame[..4], b"OBAP");
    assert_eq!(u16::from_be_bytes([frame[4], frame[5]]), 1);
    assert_eq!(u16::from_be_bytes([frame[6], frame[7]]), 5);
    let mut cursor = Cursor::new(frame);
    assert_eq!(read_agent_request(&mut cursor).unwrap(), request);
}

#[test]
fn frame_header_rejections() {
    let request = AgentReadRequest::Get {
        request_id: request_id(RID),
        session_id: AgentSessionId::parse("s-1").unwrap(),
    };
    let mut frame = Vec::new();
    write_agent_request(&mut frame, &request).unwrap();

    // Wrong magic.
    let mut bad = frame.clone();
    bad[0] = b'X';
    assert!(matches!(
        read_agent_request(&mut Cursor::new(bad)),
        Err(AgentWireError::InvalidHeader)
    ));
    // Unsupported version.
    let mut bad = frame.clone();
    bad[5] = 9;
    assert!(matches!(
        read_agent_request(&mut Cursor::new(bad)),
        Err(AgentWireError::InvalidHeader)
    ));
    // A request frame is not a result.
    assert!(matches!(
        read_agent_result(&mut Cursor::new(frame.clone())),
        Err(AgentWireError::InvalidHeader)
    ));
    // Header kind mismatched against the body kind.
    let mut bad = frame.clone();
    bad[7] = 5; // explain request kind, get request body
    assert!(matches!(
        read_agent_request(&mut Cursor::new(bad)),
        Err(AgentWireError::InvalidHeader)
    ));
    // Declared length above the request ceiling is refused before the body.
    let mut bad = frame[..12].to_vec();
    bad[8..12].copy_from_slice(&(MAXIMUM_AGENT_REQUEST_BYTES as u32 + 1).to_be_bytes());
    bad.extend_from_slice(&[0; 4]);
    assert!(matches!(
        read_agent_request(&mut Cursor::new(bad)),
        Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge { .. }
        ))
    ));
    // A get/explain result frame is capped below the list ceiling.
    let mut bad = Vec::new();
    bad.extend_from_slice(b"OBAP");
    bad.extend_from_slice(&1_u16.to_be_bytes());
    bad.extend_from_slice(&4_u16.to_be_bytes());
    bad.extend_from_slice(&(MAXIMUM_AGENT_SESSION_RESULT_BYTES as u32 + 1).to_be_bytes());
    assert!(matches!(
        read_agent_result(&mut Cursor::new(bad)),
        Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge { .. }
        ))
    ));
    // A list result frame admits the larger ceiling but no more.
    let mut bad = Vec::new();
    bad.extend_from_slice(b"OBAP");
    bad.extend_from_slice(&1_u16.to_be_bytes());
    bad.extend_from_slice(&2_u16.to_be_bytes());
    bad.extend_from_slice(&(MAXIMUM_AGENT_LIST_RESULT_BYTES as u32 + 1).to_be_bytes());
    assert!(matches!(
        read_agent_result(&mut Cursor::new(bad)),
        Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge { .. }
        ))
    ));
}

#[test]
fn strict_json_rejections() {
    // Unknown field.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.get.request","requestId":"0123456789abcdef0123456789abcdef","sessionId":"s","extra":1}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // Unknown kind literal.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.delete.request","requestId":"0123456789abcdef0123456789abcdef","sessionId":"s"}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // Missing required field.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.get.request","requestId":"0123456789abcdef0123456789abcdef"}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // Zero request identity.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.get.request","requestId":"00000000000000000000000000000000","sessionId":"s"}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // Out-of-range limit and non-integral limit spelling tolerated only as an integer.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.list.request","requestId":"0123456789abcdef0123456789abcdef","limit":0}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.list.request","requestId":"0123456789abcdef0123456789abcdef","limit":101}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.list.request","requestId":"0123456789abcdef0123456789abcdef","limit":1.5}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // An outcome kind outside the closed union.
    assert_eq!(
        parse_agent_result(
            br#"{"kind":"agent.session.get.result","requestId":"0123456789abcdef0123456789abcdef","result":{"outcome":{"kind":"mystery"}}}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // A wait/changes request kind has no variant in this slice.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.wait.request","requestId":"0123456789abcdef0123456789abcdef","sessionId":"s"}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
    // Session id outside the opaque alphabet.
    assert_eq!(
        parse_agent_request(
            br#"{"kind":"agent.session.get.request","requestId":"0123456789abcdef0123456789abcdef","sessionId":"bad id"}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
}

#[test]
fn semantic_invariants_enforced() {
    // Duplicate capabilities.
    let mut session = snapshot("s");
    session.capabilities = vec![AgentCapability::Activity, AgentCapability::Activity];
    let result = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(AgentSessionDetail {
                    snapshot: session.clone(),
                    status: AgentSemanticStatus::Idle {},
                }),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );

    // Two navigable generations on one presentation.
    let mut session = snapshot("s");
    session.presentation_attachments = vec![
        AgentPresentationAttachment {
            generation: AgentAttachmentGeneration::parse("g1").unwrap(),
            navigable: true,
            presentation_id: AgentPresentationId::parse(PID).unwrap(),
            visibility: AgentPresentationVisibility::Visible,
        },
        AgentPresentationAttachment {
            generation: AgentAttachmentGeneration::parse("g2").unwrap(),
            navigable: true,
            presentation_id: AgentPresentationId::parse(PID).unwrap(),
            visibility: AgentPresentationVisibility::Hidden,
        },
    ];
    let result = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(AgentSessionDetail {
                    snapshot: session.clone(),
                    status: AgentSemanticStatus::Idle {},
                }),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );

    // An activity hint without the terminal-scoped activity capability.
    let mut session = snapshot("s");
    session.activity_hint = Some(AgentActivity::Thinking);
    let result = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(AgentSessionDetail {
                    snapshot: session.clone(),
                    status: AgentSemanticStatus::Idle {},
                }),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );

    // An ended session retaining pending work.
    let mut session = snapshot("s");
    session.lifecycle = AgentLifecycle::Ended;
    session.pending_requests = vec![AgentAttentionRequest {
        action_path: NavigateOnly,
        kind: AgentAttentionKind::Question,
        request_id: AgentAttentionRequestId::parse("r-1").unwrap(),
        turn_id: None,
    }];
    let result = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(AgentSessionDetail {
                    snapshot: session,
                    status: AgentSemanticStatus::Ended {},
                }),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );

    // A turn-scoped request that does not name the active turn.
    let mut session = snapshot("s");
    session.turn = AgentTurnState::Active {
        activity: None,
        phase: AgentTurnPhase::Working,
        turn_id: AgentTurnId::parse("turn-a").unwrap(),
    };
    session.pending_requests = vec![AgentAttentionRequest {
        action_path: NavigateOnly,
        kind: AgentAttentionKind::Approval,
        request_id: AgentAttentionRequestId::parse("r-1").unwrap(),
        turn_id: Some(AgentTurnId::parse("turn-b").unwrap()),
    }];
    let result = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(AgentSessionDetail {
                    snapshot: session,
                    status: AgentSemanticStatus::Working {
                        activity: None,
                        phase: AgentTurnPhase::Working,
                    },
                }),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );
}

#[test]
fn collection_caps_enforced() {
    // 101 listed items.
    let result = AgentReadResult::List {
        request_id: request_id(RID),
        result: AgentListOutcomeEnvelope {
            outcome: AgentListOutcome::Listed {
                items: (0..101)
                    .map(|index| summary(&format!("s-{index}")))
                    .collect(),
                next_cursor: None,
                revision: AgentSafeInteger::new(1),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );
    // 33 attachments on one summary.
    let mut row = summary("s");
    row.presentation_attachments = (0..33)
        .map(|index| AgentPresentationAttachment {
            generation: AgentAttachmentGeneration::parse(&format!("g-{index}")).unwrap(),
            navigable: false,
            presentation_id: AgentPresentationId::parse(PID).unwrap(),
            visibility: AgentPresentationVisibility::Unknown,
        })
        .collect();
    let result = AgentReadResult::List {
        request_id: request_id(RID),
        result: AgentListOutcomeEnvelope {
            outcome: AgentListOutcome::Listed {
                items: vec![row],
                next_cursor: None,
                revision: AgentSafeInteger::new(1),
            },
        },
    };
    assert_eq!(
        serialize_agent_result(&result),
        Err(AgentContractError::InvalidDocument)
    );
}

#[test]
fn safe_integer_bounds() {
    let at_max = format!(
        "{{\"kind\":\"agent.session.list.result\",\"requestId\":\"{RID}\",\"result\":{{\"outcome\":{{\"kind\":\"listed\",\"items\":[],\"nextCursor\":null,\"revision\":{}}}}}}}",
        MAXIMUM_SAFE_INTEGER
    );
    assert!(parse_agent_result(at_max.as_bytes()).is_ok());
    let above_max = format!(
        "{{\"kind\":\"agent.session.list.result\",\"requestId\":\"{RID}\",\"result\":{{\"outcome\":{{\"kind\":\"listed\",\"items\":[],\"nextCursor\":null,\"revision\":{}}}}}}}",
        MAXIMUM_SAFE_INTEGER + 1
    );
    assert_eq!(
        parse_agent_result(above_max.as_bytes()),
        Err(AgentContractError::InvalidDocument)
    );
    // Integral float spellings normalize like the canonical decoded-number contract.
    assert!(parse_agent_result(
        br#"{"kind":"agent.session.list.result","requestId":"0123456789abcdef0123456789abcdef","result":{"outcome":{"kind":"listed","items":[],"nextCursor":null,"revision":1.0}}}"#
    )
    .is_ok());
    assert_eq!(
        parse_agent_result(
            br#"{"kind":"agent.session.list.result","requestId":"0123456789abcdef0123456789abcdef","result":{"outcome":{"kind":"listed","items":[],"nextCursor":null,"revision":-1}}}"#
        ),
        Err(AgentContractError::InvalidDocument)
    );
}

#[test]
fn cursor_shape_and_payload_layout() {
    let cursor = AgentListCursor::parse(CURSOR_TEXT).unwrap();
    assert_eq!(cursor.payload_segment().len(), 59);
    assert_eq!(cursor.tag_segment().len(), 43);
    for bad in [
        "v1.AAA.AAA",
        "x1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "v1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "v1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA+AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
        assert_eq!(
            AgentListCursor::parse(bad),
            Err(AgentCursorError::InvalidShape),
            "{bad}"
        );
    }

    let payload = AgentCursorPayload {
        version: AgentCursorPayload::VERSION,
        operation: AgentCursorPayload::LIST_OPERATION,
        flags: AgentCursorPayload::FLAG_INCLUDE_ENDED,
        instance: 0xdead_beef_cafe_f00d,
        target: 0,
        revision: 123_456,
        position: 7,
    };
    let encoded = payload.encode();
    assert_eq!(encoded[0], 1);
    assert_eq!(encoded[1], 1);
    assert_eq!(&encoded[2..4], &1_u16.to_be_bytes());
    assert_eq!(AgentCursorPayload::decode(&encoded), Some(payload));
    assert_eq!(AgentCursorPayload::decode(&encoded[..43]), None);
}

#[test]
fn encoded_bounds_are_derived_and_asserted() {
    // Worst-case listed page: 100 summaries each carrying 32 attachments with
    // maximum-width identities — must stay under the 1 MiB result ceiling.
    let max_attachment = AgentPresentationAttachment {
        generation: AgentAttachmentGeneration::parse(&"g".repeat(128)).unwrap(),
        navigable: true,
        presentation_id: AgentPresentationId::parse(PID).unwrap(),
        visibility: AgentPresentationVisibility::Unknown,
    };
    let worst_summary = AgentSessionSummary {
        execution_location: AgentExecutionLocation {
            execution_location_id: AgentExecutionLocationId::parse(&"l".repeat(128)).unwrap(),
        },
        lifecycle: AgentLifecycle::Live,
        observation: AgentObservation {
            fidelity: AgentObservationFidelity::Heuristic,
            freshness: AgentObservationFreshness::Disconnected,
            generation: AgentSafeInteger::new(MAXIMUM_SAFE_INTEGER),
            sequence: AgentSafeInteger::new(MAXIMUM_SAFE_INTEGER),
            source: AgentObservationSource::Heuristic,
        },
        parent_session_id: Some(AgentSessionId::parse(&"p".repeat(128)).unwrap()),
        pending_request_count: AgentSafeInteger::new(MAXIMUM_PENDING_REQUESTS as u64),
        presentation_attachments: (0..MAXIMUM_PRESENTATION_ATTACHMENTS)
            .map(|index| {
                let mut attachment = max_attachment.clone();
                attachment.navigable = index == 0;
                attachment.generation =
                    AgentAttachmentGeneration::parse(&format!("g-{index:03}")).unwrap();
                attachment
            })
            .collect(),
        provider_id: AgentProviderId::parse(&"p".repeat(64)).unwrap(),
        session_id: AgentSessionId::parse(&"s".repeat(128)).unwrap(),
        status: AgentSemanticStatus::NeedsAttention {
            count: AgentPositiveInteger::new(MAXIMUM_SAFE_INTEGER).unwrap(),
        },
    };
    let worst_page = AgentReadResult::List {
        request_id: request_id(RID),
        result: AgentListOutcomeEnvelope {
            outcome: AgentListOutcome::Listed {
                items: (0..MAXIMUM_LIST_ITEMS)
                    .map(|index| {
                        let mut row = worst_summary.clone();
                        row.session_id =
                            AgentSessionId::parse(&format!("s-{index:03}-{}", "x".repeat(120)))
                                .unwrap();
                        row.presentation_attachments = (0..MAXIMUM_PRESENTATION_ATTACHMENTS)
                            .map(|slot| {
                                let mut attachment = max_attachment.clone();
                                attachment.navigable = false;
                                attachment.generation = AgentAttachmentGeneration::parse(&format!(
                                    "g-{index:03}-{slot:03}"
                                ))
                                .unwrap();
                                attachment
                            })
                            .collect();
                        row
                    })
                    .collect(),
                next_cursor: Some(AgentListCursor::parse(CURSOR_TEXT).unwrap()),
                revision: AgentSafeInteger::new(MAXIMUM_SAFE_INTEGER),
            },
        },
    };
    let encoded = serialize_agent_result(&worst_page).unwrap();
    assert!(
        encoded.len() < MAXIMUM_AGENT_LIST_RESULT_BYTES,
        "worst listed page {} bytes must stay under {}",
        encoded.len(),
        MAXIMUM_AGENT_LIST_RESULT_BYTES
    );
    let mut frame = Vec::new();
    write_agent_result(&mut frame, &worst_page).unwrap();

    // A saturated detail/explain body must stay under the session ceiling.
    let mut saturated = snapshot(&"s".repeat(128));
    saturated.presentation_attachments = (0..MAXIMUM_PRESENTATION_ATTACHMENTS)
        .map(|slot| AgentPresentationAttachment {
            generation: AgentAttachmentGeneration::parse(&format!("g-{slot:03}")).unwrap(),
            navigable: false,
            presentation_id: AgentPresentationId::parse(PID).unwrap(),
            visibility: AgentPresentationVisibility::Unknown,
        })
        .collect();
    saturated.pending_requests = (0..MAXIMUM_PENDING_REQUESTS)
        .map(|index| AgentAttentionRequest {
            action_path: NavigateOnly,
            kind: AgentAttentionKind::Unclassified,
            request_id: AgentAttentionRequestId::parse(&format!("r-{index:03}")).unwrap(),
            turn_id: None,
        })
        .collect();
    saturated.turn = AgentTurnState::Active {
        activity: Some(AgentActivity::Compacting),
        phase: AgentTurnPhase::WaitingExternal,
        turn_id: AgentTurnId::parse(&"t".repeat(128)).unwrap(),
    };
    let worst_get = AgentReadResult::Get {
        request_id: request_id(RID),
        result: AgentGetOutcomeEnvelope {
            outcome: AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(MAXIMUM_SAFE_INTEGER),
                session: Box::new(AgentSessionDetail {
                    snapshot: saturated,
                    status: AgentSemanticStatus::Working {
                        activity: Some(AgentActivity::Compacting),
                        phase: AgentTurnPhase::WaitingExternal,
                    },
                }),
            },
        },
    };
    let encoded = serialize_agent_result(&worst_get).unwrap();
    assert!(
        encoded.len() < MAXIMUM_AGENT_SESSION_RESULT_BYTES,
        "worst detail body {} bytes must stay under {}",
        encoded.len(),
        MAXIMUM_AGENT_SESSION_RESULT_BYTES
    );

    // Small outcomes are far below their dedicated bound.
    for outcome_result in [
        AgentReadResult::List {
            request_id: request_id(RID),
            result: AgentListOutcomeEnvelope {
                outcome: AgentListOutcome::Rejected {
                    reason: AgentListRejectionReason::CursorInvalid,
                },
            },
        },
        AgentReadResult::Get {
            request_id: request_id(RID),
            result: AgentGetOutcomeEnvelope {
                outcome: AgentGetOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                },
            },
        },
    ] {
        let encoded = serialize_agent_result(&outcome_result).unwrap();
        assert!(encoded.len() < MAXIMUM_AGENT_SMALL_OUTCOME_BYTES);
    }
}
