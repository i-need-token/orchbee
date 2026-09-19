use super::{
    MAXIMUM_SEND_TEXT_BYTES, SendAcceptedBytes, SendCleanupStatus, SendKey, SendTarget, SendText,
    SendTextInput, TerminalSendOutcome, TerminalSendPayload, TerminalSendRequest,
    TerminalSendResult,
};
use crate::{HostEpoch, HostTerminalId, RequestId, SiteId, TerminalId, WorkerId};
use std::num::NonZeroU64;

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
    assert_eq!(input.enter(), true);
}

#[test]
fn payload_decode_enforces_the_combined_bound() {
    let maximum = "x".repeat(MAXIMUM_SEND_TEXT_BYTES);
    let accepted = format!("{{\"kind\":\"text\",\"text\":\"{maximum}\",\"enter\":false}}");
    assert!(serde_json::from_str::<TerminalSendPayload>(&accepted).is_ok());
    let rejected = format!("{{\"kind\":\"text\",\"text\":\"{maximum}\",\"enter\":true}}");
    assert!(serde_json::from_str::<TerminalSendPayload>(&rejected).is_err());
}

#[test]
fn payload_wire_shape_is_closed_and_tagged() -> Result<(), Box<dyn std::error::Error>> {
    let payload = TerminalSendPayload::new_text(text("ls -la"), true)?;
    assert_eq!(
        serde_json::to_string(&payload)?,
        "{\"kind\":\"text\",\"text\":\"ls -la\",\"enter\":true}"
    );
    assert_eq!(
        serde_json::from_str::<TerminalSendPayload>("{\"kind\":\"key\",\"key\":\"ctrl-c\"}")?,
        TerminalSendPayload::Key {
            key: SendKey::CtrlC
        }
    );
    for rejected in [
        "{\"kind\":\"text\",\"text\":\"ls\",\"enter\":true,\"extra\":0}",
        "{\"kind\":\"key\",\"key\":\"ctrl-alt-delete\"}",
        "{\"kind\":\"text\",\"text\":\"\",\"enter\":false}",
        "{\"kind\":\"text\",\"enter\":true}",
        "{\"kind\":\"future\"}",
    ] {
        assert!(
            serde_json::from_str::<TerminalSendPayload>(rejected).is_err(),
            "payload {rejected} decoded"
        );
    }
    Ok(())
}

#[test]
fn send_key_set_serializes_to_the_fifteen_closed_names() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (SendKey::Enter, "enter"),
        (SendKey::Escape, "escape"),
        (SendKey::Tab, "tab"),
        (SendKey::Backspace, "backspace"),
        (SendKey::Delete, "delete"),
        (SendKey::Up, "up"),
        (SendKey::Down, "down"),
        (SendKey::Left, "left"),
        (SendKey::Right, "right"),
        (SendKey::Home, "home"),
        (SendKey::End, "end"),
        (SendKey::PageUp, "page-up"),
        (SendKey::PageDown, "page-down"),
        (SendKey::CtrlC, "ctrl-c"),
        (SendKey::CtrlD, "ctrl-d"),
    ];
    assert_eq!(cases.len(), 15);
    for (key, name) in cases {
        assert_eq!(serde_json::to_string(&key)?, format!("\"{name}\""));
        assert_eq!(
            serde_json::from_str::<SendKey>(&format!("\"{name}\""))?,
            key
        );
    }
    Ok(())
}

#[test]
fn accepted_bytes_admits_zero_and_rejects_overflow() {
    assert_eq!(SendAcceptedBytes::new(0).expect("zero is legal").get(), 0);
    assert!(SendAcceptedBytes::new(SendAcceptedBytes::MAXIMUM).is_ok());
    assert!(SendAcceptedBytes::new(SendAcceptedBytes::MAXIMUM + 1).is_err());
    assert!(serde_json::from_str::<SendAcceptedBytes>("0").is_ok());
    assert!(serde_json::from_str::<SendAcceptedBytes>("-1").is_err());
    assert!(serde_json::from_str::<SendAcceptedBytes>("1048577").is_err());
}

#[test]
fn impossible_status_cleanup_pairs_are_rejected_at_construction_and_decode() {
    for outcome in [
        TerminalSendOutcome::Accepted {
            accepted_bytes: SendAcceptedBytes::new(0).expect("zero is legal"),
            cleanup: SendCleanupStatus::NotNeeded,
        },
        TerminalSendOutcome::OutcomeUnknown {
            cleanup: SendCleanupStatus::Confirmed,
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
        assert!(serde_json::from_str::<TerminalSendOutcome>(&encoded).is_ok());
    }
    for outcome in [
        TerminalSendOutcome::Accepted {
            accepted_bytes: SendAcceptedBytes::new(0).expect("zero"),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
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
            accepted_bytes: SendAcceptedBytes::new(0)?,
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::Rejected {
            reason: super::SendRejectedReason::HostInputInvalid,
            cleanup: SendCleanupStatus::Confirmed,
        },
        TerminalSendOutcome::Unavailable {
            reason: super::SendUnavailableReason::HostProtocolFault,
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::OutcomeUnknown {
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
        serde_json::from_str::<TerminalSendOutcome>("{\"kind\":\"accepted\",\"acceptedBytes\":1}")
            .is_err()
    );
    assert!(
        serde_json::from_str::<TerminalSendOutcome>(
            "{\"kind\":\"accepted\",\"acceptedBytes\":1,\"cleanup\":\"future\"}"
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn request_exposes_the_flat_binding_triple_and_payload() -> Result<(), Box<dyn std::error::Error>> {
    let request = TerminalSendRequest::new(
        RequestId::parse("11111111111111111111111111111111")?,
        SiteId::parse("22222222222222222222222222222222")?,
        WorkerId::parse("33333333333333333333333333333333")?,
        TerminalId::parse("44444444444444444444444444444444")?,
        TerminalSendPayload::new_key(SendKey::Enter),
    );
    assert_eq!(
        request.request_id().as_str(),
        "11111111111111111111111111111111"
    );
    assert_eq!(
        request.site_id().as_str(),
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
        request.payload(),
        &TerminalSendPayload::Key {
            key: SendKey::Enter
        }
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
            accepted_bytes: SendAcceptedBytes::new(12)?,
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
            cleanup: SendCleanupStatus::NotNeeded,
        },
    )?;
    assert!(bare.target().is_none());
    assert!(wire_json(&bare)?.contains("\"target\":null"));
    Ok(())
}

fn wire_json(result: &TerminalSendResult) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = crate::wire::encode_server_payload(&crate::ServerMessage::TerminalSendResult(
        result.clone(),
    ))?;
    Ok(String::from_utf8(bytes)?)
}
