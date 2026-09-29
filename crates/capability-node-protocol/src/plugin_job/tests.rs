use super::*;
use crate::{CapabilityNodeMessage as Message, decode_message, encode_message, read_message};
use std::num::NonZeroU64;
fn request(action: PluginJobAction) -> PluginJobRequest {
    PluginJobRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action,
    }
}
fn intent() -> PluginCommandIntent {
    PluginCommandIntent {
        plugin_id: "sample".into(),
        command_id: "hello".into(),
        directory: "C:\\plugin".into(),
        argv: vec!["cmd.exe".into(), "/c".into(), "echo 中文".into()],
        environment: vec![("ORCHBEE_PLUGIN_ID".into(), "sample".into())],
    }
}
fn log() -> PluginCommandLog {
    PluginCommandLog {
        job_id: OperationId::from_bytes([1; 16]).unwrap(),
        sequence: 1,
        plugin_id: "sample".into(),
        command_id: "hello".into(),
        command: intent().argv,
        started_ms: 1,
        finished_ms: Some(2),
        exit_code: Some(0),
        failure: None,
        stdout: vec![0xff; MAXIMUM_PLUGIN_OUTPUT_BYTES],
        stderr: vec![0; MAXIMUM_PLUGIN_OUTPUT_BYTES],
        stdout_truncated: true,
        stderr_truncated: false,
    }
}
#[test]
fn plugin_records_roundtrip_full_outputs_and_strict_framing() {
    let req = request(PluginJobAction::Start(intent()));
    let result = PluginJobResult {
        operation_id: req.operation_id,
        worker_id: req.worker_id,
        worker_generation: req.worker_generation,
        outcome: PluginJobOutcome::Started {
            log: Box::new(log()),
            existing: false,
        },
    };
    assert!(result.answers(&req));
    for message in [
        Message::PluginJobRequest(req),
        Message::PluginJobResult(result),
    ] {
        let bytes = encode_message(&message).unwrap();
        assert_eq!(decode_message(&bytes).unwrap(), message);
        assert_eq!(read_message(&mut bytes.as_slice()).unwrap(), message);
        for cut in [0, 7, 11, bytes.len() - 1] {
            assert!(decode_message(&bytes[..cut]).is_err());
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode_message(&extra).is_err());
        let mut bad = bytes.clone();
        bad[7] = 1;
        assert!(decode_message(&bad).is_err());
        let mut bad = bytes;
        bad[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(read_message(&mut bad.as_slice()).is_err());
    }
}
#[test]
fn plugin_semantics_reject_invalid_intent_status_order_and_correlation() {
    let mut bad = intent();
    bad.environment.push(("PATH".into(), "untrusted".into()));
    assert!(bad.validate().is_err());
    let mut bad = intent();
    bad.environment.push(bad.environment[0].clone());
    assert!(bad.validate().is_err());
    let mut bad = intent();
    bad.directory = "relative".into();
    assert!(bad.validate().is_err());
    let mut bad = log();
    bad.finished_ms = None;
    assert!(bad.validate().is_err());
    let mut bad = log();
    bad.exit_code = Some(7);
    assert!(bad.validate().is_err());
    let mut bad = log();
    bad.stdout.pop();
    assert!(bad.validate().is_err());
    let req = request(PluginJobAction::List {
        plugin_id: Some("sample".into()),
        before: Some(2),
        limit: 1,
    });
    let mut result = PluginJobResult {
        operation_id: req.operation_id,
        worker_id: req.worker_id,
        worker_generation: req.worker_generation,
        outcome: PluginJobOutcome::Logs {
            logs: vec![log()],
            next_before: None,
        },
    };
    assert!(result.answers(&req));
    result.worker_generation = WorkerGeneration::new(NonZeroU64::new(2).unwrap());
    assert!(!result.answers(&req));
    result.worker_generation = req.worker_generation;
    result.outcome = PluginJobOutcome::Logs {
        logs: vec![log(), log()],
        next_before: Some(1),
    };
    assert!(result.validate().is_err());
}
#[test]
fn plugin_query_cancel_and_refusal_tags_roundtrip() {
    for action in [
        PluginJobAction::List {
            plugin_id: None,
            before: None,
            limit: 4,
        },
        PluginJobAction::List {
            plugin_id: Some("a".into()),
            before: Some(2),
            limit: 1,
        },
        PluginJobAction::Get {
            job_id: OperationId::from_bytes([4; 16]).unwrap(),
        },
        PluginJobAction::Cancel {
            job_id: OperationId::from_bytes([4; 16]).unwrap(),
        },
    ] {
        let m = Message::PluginJobRequest(request(action));
        assert_eq!(decode_message(&encode_message(&m).unwrap()).unwrap(), m);
    }
    for refusal in [
        PluginJobRefusal::RejectedFencing,
        PluginJobRefusal::InvalidIntent,
        PluginJobRefusal::Busy,
        PluginJobRefusal::Conflict,
        PluginJobRefusal::Unavailable,
        PluginJobRefusal::PhysicalUnknown,
        PluginJobRefusal::NotFound,
        PluginJobRefusal::Unsupported,
    ] {
        let r = request(PluginJobAction::Start(intent()));
        let m = Message::PluginJobResult(PluginJobResult {
            operation_id: r.operation_id,
            worker_id: r.worker_id,
            worker_generation: r.worker_generation,
            outcome: PluginJobOutcome::Refused(refusal),
        });
        assert_eq!(decode_message(&encode_message(&m).unwrap()).unwrap(), m);
    }
}
