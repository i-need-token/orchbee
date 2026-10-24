use orchbee_capability_node_protocol::*;
use std::{io::Cursor, num::NonZeroU64};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn control_wire_preserves_scope_bytes_and_every_action_and_outcome() -> TestResult {
    let request = TerminalControlRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([2; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        viewer_id: OperationId::from_bytes([3; 16])?,
        controller_id: OperationId::from_bytes([4; 16])?,
        action: TerminalControlAction::Acquire,
    };
    let geometry = TerminalPixelGeometry::new(100, 31, 8, 16)?;
    for action in [
        TerminalControlAction::Acquire,
        TerminalControlAction::Release,
        TerminalControlAction::Takeover {
            previous_controller_id: OperationId::from_bytes([5; 16])?,
        },
        TerminalControlAction::Input {
            sequence: NonZeroU64::MAX,
            bytes: ControlInput::new(vec![0x1b; 4096])?,
        },
        TerminalControlAction::Resize {
            sequence: NonZeroU64::MIN,
            geometry,
        },
        TerminalControlAction::HostColors {
            sequence: NonZeroU64::MAX,
            colors: HostDefaultColors::new(0x00cc_cccc, 0x000c_0c0c)?,
        },
    ] {
        let message = CapabilityNodeMessage::TerminalControlRequest(TerminalControlRequest {
            action,
            ..request.clone()
        });
        let frame = encode_message(&message)?;
        assert_eq!(decode_message(&frame)?, message);
        assert_eq!(read_message(&mut Cursor::new(&frame))?, message);
        let mut bad = frame.clone();
        bad[8 + 1 + 16 + 16..8 + 1 + 16 + 16 + 8].fill(0);
        assert!(decode_message(&bad).is_err());
    }
    for outcome in [
        TerminalControlOutcome::Acquired,
        TerminalControlOutcome::Released,
        TerminalControlOutcome::InputAccepted,
        TerminalControlOutcome::Resized(geometry),
        TerminalControlOutcome::HostColorsAccepted,
    ]
    .into_iter()
    .chain(
        [
            ControlRejection::WrongWorker,
            ControlRejection::StaleWorkerGeneration,
            ControlRejection::ViewerUnavailable,
            ControlRejection::ControllerBusy,
            ControlRejection::NotController,
            ControlRejection::SequenceMismatch,
            ControlRejection::HostRejected,
        ]
        .map(TerminalControlOutcome::Rejected),
    ) {
        let message = CapabilityNodeMessage::TerminalControlResult(TerminalControlResult {
            request_id: request.request_id,
            viewer_id: request.viewer_id,
            controller_id: request.controller_id,
            outcome,
        });
        assert_eq!(decode_message(&encode_message(&message)?)?, message);
    }
    assert!(ControlInput::new(Vec::new()).is_err());
    assert!(ControlInput::new(vec![0; 4097]).is_err());
    assert!(HostDefaultColors::new(0x0100_0000, 0).is_err());
    assert!(HostDefaultColors::new(0, 0x0100_0000).is_err());
    let mut colors = encode_message(&CapabilityNodeMessage::TerminalControlRequest(
        TerminalControlRequest {
            action: TerminalControlAction::HostColors {
                sequence: NonZeroU64::MIN,
                colors: HostDefaultColors::new(0, 0)?,
            },
            ..request.clone()
        },
    ))?;
    let background = colors.len() - 4;
    colors[background] = 1;
    assert!(decode_message(&colors).is_err());
    Ok(())
}

#[test]
fn takeover_wire_requires_one_complete_nonzero_previous_controller() -> TestResult {
    let message = CapabilityNodeMessage::TerminalControlRequest(TerminalControlRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([2; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        viewer_id: OperationId::from_bytes([3; 16])?,
        controller_id: OperationId::from_bytes([4; 16])?,
        action: TerminalControlAction::Takeover {
            previous_controller_id: OperationId::from_bytes([5; 16])?,
        },
    });
    let mut bytes = encode_message(&message)?;
    assert!(decode_message(&bytes[..bytes.len() - 1]).is_err());
    let offset = bytes.len() - 16;
    bytes[offset..].fill(0);
    assert!(decode_message(&bytes).is_err());
    Ok(())
}
