use super::{
    MAXIMUM_SEND_RECORDS, MAXIMUM_SEND_TEXT_BYTES, SendAcceptedBytes, SendBytes, SendCleanupStatus,
    SendKey, SendProgress, SendRecord, SendRecords, SendTarget, SendText, SendTextInput,
    TerminalSendOutcome, TerminalSendRequest, TerminalSendResult,
};
use crate::{HostEpoch, HostTerminalId, RequestId, SessionId, TerminalId, WorkerId};
use std::num::NonZeroU64;

#[test]
fn send_stroke_mixed_records_preserve_boundaries_and_reject_malformed_payloads() {
    use crate::{SendKeyStroke, SendStrokeKey};
    let records = SendRecords::new(vec![
        SendRecord::Stroke(SendKeyStroke::new(SendStrokeKey::Character('😀'), 8).unwrap()),
        SendRecord::Stroke(
            crate::SendKeyStroke::new(crate::SendStrokeKey::Named(SendKey::Enter), 0).unwrap(),
        ),
    ])
    .unwrap();
    let request = TerminalSendRequest::new(
        RequestId::parse("11111111111111111111111111111111").unwrap(),
        SessionId::parse("22222222222222222222222222222222").unwrap(),
        WorkerId::parse("33333333333333333333333333333333").unwrap(),
        TerminalId::parse("44444444444444444444444444444444").unwrap(),
        records,
    );
    let wire = crate::send_record::encode(&request).unwrap();
    let at = 12 + 65;
    assert_eq!(&wire[at..], &[6, 8, 4, 0, 1, 0xf6, 0, 6, 0, 1, 0, 0, 0, 1]);
    assert_eq!(
        crate::send_record::decode(&wire).unwrap(),
        crate::ClientMessage::TerminalSend(request)
    );
    for end in 0..wire.len() {
        assert!(crate::send_record::decode(&wire[..end]).is_err());
    }
    for (offset, value) in [(0, 2), (0, 3), (0, 7), (1, 16), (2, 0), (3, 1)] {
        let mut invalid = wire.clone();
        invalid[at + offset] = value;
        assert!(crate::send_record::decode(&invalid).is_err());
    }
}

fn text(value: &str) -> SendText {
    SendText::new(value.to_string()).expect("valid send text")
}

fn target() -> SendTarget {
    SendTarget::new(
        NonZeroU64::new(7).expect("non-zero"),
        HostEpoch::parse("55555555555555555555555555555555").expect("valid epoch"),
        HostTerminalId::parse("66666666666666666666666666666666").expect("valid terminal"),
    )
}

#[test]
fn send_text_rejects_empty_and_excessive_bodies() {
    assert!(SendText::new(String::new()).is_err());
    assert!(SendText::new("x".repeat(MAXIMUM_SEND_TEXT_BYTES)).is_ok());
    assert!(SendText::new("x".repeat(MAXIMUM_SEND_TEXT_BYTES + 1)).is_err());
    assert!(SendText::new("工".repeat(MAXIMUM_SEND_TEXT_BYTES)).is_err());
    assert!(SendText::new("工".repeat(MAXIMUM_SEND_TEXT_BYTES / 3)).is_ok());
}

#[test]
fn folded_enter_shares_the_same_payload_bound() {
    let maximum = text(&"x".repeat(MAXIMUM_SEND_TEXT_BYTES));
    assert!(SendTextInput::new(maximum.clone(), false).is_ok());
    assert!(SendTextInput::new(maximum, true).is_err());
    let edge = text(&"x".repeat(MAXIMUM_SEND_TEXT_BYTES - 1));
    let input = SendTextInput::new(edge, true).expect("edge payload");
    assert_eq!(input.record_bytes(), MAXIMUM_SEND_TEXT_BYTES);
    assert!(input.enter());
}

#[test]
fn records_enforce_count_and_aggregate_decoded_bounds() {
    use super::{MAXIMUM_SEND_AGGREGATE_BYTES, MAXIMUM_SEND_BYTES_BYTES};
    let key = || {
        SendRecord::Stroke(
            crate::SendKeyStroke::new(crate::SendStrokeKey::Named(SendKey::Enter), 0).unwrap(),
        )
    };
    assert!(SendRecords::new(Vec::new()).is_err());
    assert!(SendRecords::new(vec![key(); MAXIMUM_SEND_RECORDS]).is_ok());
    assert!(SendRecords::new(vec![key(); MAXIMUM_SEND_RECORDS + 1]).is_err());
    // Key records carry no decoded bytes: the maximum count plus a 1MiB
    // bytes record is the exact aggregate ceiling.
    let mut records = vec![key(); MAXIMUM_SEND_RECORDS - 1];
    records.push(SendRecord::Bytes {
        bytes: SendBytes::new(vec![0; MAXIMUM_SEND_AGGREGATE_BYTES]).expect("bound"),
    });
    assert!(SendRecords::new(records).is_ok());
    // One extra decoded byte over the aggregate bound refuses.
    let mut over = vec![key(); MAXIMUM_SEND_RECORDS - 1];
    over.push(SendRecord::Bytes {
        bytes: SendBytes::new(vec![0; MAXIMUM_SEND_BYTES_BYTES]).expect("bound"),
    });
    over.insert(
        0,
        SendRecord::Bytes {
            bytes: SendBytes::new(vec![0; 1]).expect("one byte"),
        },
    );
    assert!(SendRecords::new(over).is_err());
}

#[test]
fn accepted_bytes_admits_zero_and_rejects_overflow() {
    assert_eq!(SendAcceptedBytes::new(0).expect("zero is legal").get(), 0);
    assert!(SendAcceptedBytes::new(SendAcceptedBytes::MAXIMUM).is_ok());
    assert!(SendAcceptedBytes::new(SendAcceptedBytes::MAXIMUM + 1).is_err());
}

#[test]
fn impossible_status_cleanup_pairs_are_rejected_at_construction_and_decode() {
    for outcome in [
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 0),
            cleanup: SendCleanupStatus::NotNeeded,
        },
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        },
        TerminalSendOutcome::Rejected {
            reason: super::SendRejectedReason::HostInputInvalid,
            progress: SendProgress::stopped(1, 0),
            cleanup: SendCleanupStatus::NotNeeded,
        },
    ] {
        assert!(
            TerminalSendResult::new(
                RequestId::parse("11111111111111111111111111111111").expect("id"),
                Some(target()),
                outcome.clone(),
            )
            .is_err()
        );
        let encoded = serde_json::to_string(&outcome).expect("encode");
        // The serde shape is permissive; the constructor is the authority.
        assert!(serde_json::from_str::<TerminalSendOutcome>(&encoded).is_ok());
    }
    for outcome in [
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 0),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
    ] {
        assert!(
            TerminalSendResult::new(
                RequestId::parse("11111111111111111111111111111111").expect("id"),
                Some(target()),
                outcome,
            )
            .is_ok()
        );
    }
}

#[test]
fn every_outcome_carries_the_cleanup_axis() -> Result<(), Box<dyn std::error::Error>> {
    for outcome in [
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 0),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::Rejected {
            reason: super::SendRejectedReason::HostInputInvalid,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        },
        TerminalSendOutcome::Unavailable {
            reason: super::SendUnavailableReason::HostProtocolFault,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
    ] {
        let encoded = serde_json::to_string(&outcome)?;
        assert!(encoded.contains("\"cleanup\":"), "{encoded}");
        assert_eq!(
            serde_json::from_str::<TerminalSendOutcome>(&encoded)?,
            outcome
        );
    }
    assert!(
        serde_json::from_str::<TerminalSendOutcome>(
            "{\"kind\":\"accepted\",\"cleanup\":\"confirmed\"}"
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<TerminalSendOutcome>(
            "{\"kind\":\"accepted\",\"progress\":{\"recordsAccepted\":1,\"firstStoppedRecord\":1,\"acceptedEncodedBytes\":0},\"cleanup\":\"future\"}"
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn request_exposes_the_flat_binding_triple_and_records() -> Result<(), Box<dyn std::error::Error>> {
    let request = TerminalSendRequest::new(
        RequestId::parse("11111111111111111111111111111111")?,
        SessionId::parse("22222222222222222222222222222222")?,
        WorkerId::parse("33333333333333333333333333333333")?,
        TerminalId::parse("44444444444444444444444444444444")?,
        SendRecords::new(vec![SendRecord::Stroke(
            crate::SendKeyStroke::new(crate::SendStrokeKey::Named(SendKey::Enter), 0).unwrap(),
        )])?,
    );
    assert_eq!(
        request.request_id().as_str(),
        "11111111111111111111111111111111"
    );
    assert_eq!(
        request.session_id().as_str(),
        "22222222222222222222222222222222"
    );
    assert_eq!(
        request.worker_id().as_str(),
        "33333333333333333333333333333333"
    );
    assert_eq!(
        request.terminal_id().as_str(),
        "44444444444444444444444444444444"
    );
    assert_eq!(
        request.records().as_slice(),
        &[SendRecord::Stroke(
            crate::SendKeyStroke::new(crate::SendStrokeKey::Named(SendKey::Enter), 0).unwrap()
        )]
    );
    Ok(())
}

#[test]
fn result_records_the_selected_physical_target_or_none() -> Result<(), Box<dyn std::error::Error>> {
    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    let result = TerminalSendResult::new(
        request_id.clone(),
        Some(target()),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 12),
            cleanup: SendCleanupStatus::Confirmed,
        },
    )?;
    let selected = result.target().expect("admitted send keeps its target");
    assert_eq!(selected.worker_generation().get(), 7);
    assert_eq!(
        selected.host_epoch().as_str(),
        "55555555555555555555555555555555"
    );
    assert_eq!(
        selected.host_terminal_id().as_str(),
        "66666666666666666666666666666666"
    );
    let encoded = wire_json(&result)?;
    assert!(encoded.contains("\"workerGeneration\":\"7\""), "{encoded}");
    assert!(encoded.contains("\"hostEpoch\":\"55555555555555555555555555555555\""));
    assert!(encoded.contains("\"hostTerminalId\":\"66666666666666666666666666666666\""));
    let bare = TerminalSendResult::new(
        request_id,
        None,
        TerminalSendOutcome::Rejected {
            reason: super::SendRejectedReason::TerminalUnavailable,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
    )?;
    assert!(bare.target().is_none());
    assert!(wire_json(&bare)?.contains("\"target\":null"));
    Ok(())
}

#[test]
fn absent_target_is_only_legal_for_pre_admission_refusals() -> Result<(), Box<dyn std::error::Error>>
{
    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    let accepted = || SendProgress::complete(1, 3);
    for outcome in [
        TerminalSendOutcome::Accepted {
            progress: accepted(),
            cleanup: SendCleanupStatus::Confirmed,
        },
        TerminalSendOutcome::Accepted {
            progress: accepted(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::Rejected {
            reason: super::SendRejectedReason::HostRejected,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        },
        TerminalSendOutcome::Unavailable {
            reason: super::SendUnavailableReason::HostUnavailable,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
    ] {
        assert!(
            TerminalSendResult::new(request_id.clone(), None, outcome.clone()).is_err(),
            "targetless {outcome:?} constructed"
        );
    }
    for outcome in [
        TerminalSendOutcome::Rejected {
            reason: super::SendRejectedReason::TerminalUnavailable,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
        TerminalSendOutcome::Unavailable {
            reason: super::SendUnavailableReason::ServerBusy,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        },
    ] {
        assert!(
            TerminalSendResult::new(request_id.clone(), None, outcome.clone()).is_ok(),
            "pre-admission {outcome:?} rejected"
        );
    }
    // A pinned target stays on the receipt even when no attachment obligation
    // arose; only the target-absent case is constrained.
    assert!(
        TerminalSendResult::new(
            request_id.clone(),
            Some(target()),
            TerminalSendOutcome::Rejected {
                reason: super::SendRejectedReason::StaleHostEpoch,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
        )
        .is_ok()
    );
    // The encode boundary revalidates the full invariant so a struct literal
    // cannot bypass `new`.
    let bypassed = TerminalSendResult {
        request_id: request_id.clone(),
        target: None,
        outcome: TerminalSendOutcome::Accepted {
            progress: accepted(),
            cleanup: SendCleanupStatus::Confirmed,
        },
    };
    assert!(wire_json(&bypassed).is_err());
    // The decode boundary applies the same rule, and a missing `target` field
    // is rejected rather than treated as `null`.
    for payload in [
        "{\"kind\":\"terminal.send.result\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"target\":null,\"outcome\":{\"kind\":\"accepted\",\"progress\":{\"recordsAccepted\":1,\"firstStoppedRecord\":1,\"acceptedEncodedBytes\":3},\"cleanup\":\"confirmed\"}}",
        "{\"kind\":\"terminal.send.result\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"target\":null,\"outcome\":{\"kind\":\"outcome-unknown\",\"progress\":{\"recordsAccepted\":0,\"firstStoppedRecord\":0,\"acceptedEncodedBytes\":0},\"cleanup\":\"not-needed\"}}",
        "{\"kind\":\"terminal.send.result\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"target\":null,\"outcome\":{\"kind\":\"rejected\",\"reason\":\"host-rejected\",\"progress\":{\"recordsAccepted\":0,\"firstStoppedRecord\":0,\"acceptedEncodedBytes\":0},\"cleanup\":\"confirmed\"}}",
        "{\"kind\":\"terminal.send.result\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"outcome\":{\"kind\":\"rejected\",\"reason\":\"terminal-unavailable\",\"progress\":{\"recordsAccepted\":0,\"firstStoppedRecord\":0,\"acceptedEncodedBytes\":0},\"cleanup\":\"not-needed\"}}",
    ] {
        assert!(
            crate::wire::decode_server_payload(payload.as_bytes()).is_err(),
            "payload {payload} decoded"
        );
    }
    assert!(
        crate::wire::decode_server_payload(
            b"{\"kind\":\"terminal.send.result\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"target\":null,\"outcome\":{\"kind\":\"rejected\",\"reason\":\"terminal-unavailable\",\"progress\":{\"recordsAccepted\":0,\"firstStoppedRecord\":0,\"acceptedEncodedBytes\":0},\"cleanup\":\"not-needed\"}}",
        )
        .is_ok()
    );
    Ok(())
}

fn wire_json(result: &TerminalSendResult) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = crate::wire::encode_server_payload(&crate::ServerMessage::TerminalSendResult(
        result.clone(),
    ))?;
    Ok(String::from_utf8(bytes)?)
}
