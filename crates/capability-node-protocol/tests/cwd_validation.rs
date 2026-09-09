use std::{io, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, CapabilityNodeMessage, OperationId, ProtocolError, TerminalCwdValidationOutcome,
    TerminalCwdValidationRequest, TerminalCwdValidationResult, WorkerGeneration, WorkerId,
    decode_message, encode_message,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn cwd_validation_round_trips_every_outcome_and_preserves_path_spelling() -> TestResult {
    let request = request("/tmp/repository/../workspace")?;
    let request_message = CapabilityNodeMessage::TerminalCwdValidationRequest(request.clone());
    let encoded_request = encode_message(&request_message)?;
    assert_eq!(encoded_request[8], 11);
    assert_eq!(decode_message(&encoded_request)?, request_message);

    for outcome in [
        TerminalCwdValidationOutcome::DirectoryAvailable,
        TerminalCwdValidationOutcome::DirectoryUnavailable,
        TerminalCwdValidationOutcome::WorkerUnavailable,
    ] {
        let message =
            CapabilityNodeMessage::TerminalCwdValidationResult(TerminalCwdValidationResult {
                request: request.clone(),
                outcome,
            });
        let encoded = encode_message(&message)?;
        assert_eq!(encoded[8], 12);
        assert_eq!(decode_message(&encoded)?, message);
        assert_eq!(encode_message(&decode_message(&encoded)?)?, encoded);
    }
    Ok(())
}

#[test]
fn cwd_validation_rejects_invalid_required_fields_unknown_outcome_and_truncation() -> TestResult {
    let canonical = encode_message(&CapabilityNodeMessage::TerminalCwdValidationResult(
        TerminalCwdValidationResult {
            request: request("/tmp/workspace")?,
            outcome: TerminalCwdValidationOutcome::DirectoryAvailable,
        },
    ))?;

    let mut zero_operation = canonical.clone();
    zero_operation[9..25].fill(0);
    assert_eq!(
        decode_message(&zero_operation),
        Err(ProtocolError::InvalidIdentity)
    );

    let mut zero_worker = canonical.clone();
    zero_worker[25..41].fill(0);
    assert_eq!(
        decode_message(&zero_worker),
        Err(ProtocolError::InvalidIdentity)
    );

    let mut zero_generation = canonical.clone();
    zero_generation[41..49].fill(0);
    assert_eq!(
        decode_message(&zero_generation),
        Err(ProtocolError::InvalidPayload)
    );

    let mut relative_cwd = canonical.clone();
    relative_cwd[51] = b'.';
    assert_eq!(
        decode_message(&relative_cwd),
        Err(ProtocolError::InvalidCwd)
    );

    let mut missing_cwd = canonical.clone();
    missing_cwd[49..51].fill(0);
    assert_eq!(decode_message(&missing_cwd), Err(ProtocolError::InvalidCwd));

    let mut invalid_utf8 = canonical.clone();
    invalid_utf8[51] = u8::MAX;
    assert_eq!(
        decode_message(&invalid_utf8),
        Err(ProtocolError::InvalidCwd)
    );

    let mut unknown_outcome = canonical.clone();
    *unknown_outcome
        .last_mut()
        .ok_or_else(|| io::Error::other("missing outcome"))? = u8::MAX;
    assert_eq!(
        decode_message(&unknown_outcome),
        Err(ProtocolError::InvalidPayload)
    );

    let mut truncated = canonical;
    truncated.pop();
    let payload_length = u16::try_from(truncated.len() - 8)?;
    truncated[6..8].copy_from_slice(&payload_length.to_be_bytes());
    assert_eq!(
        decode_message(&truncated),
        Err(ProtocolError::InvalidPayload)
    );

    let mut trailing = encode_message(&CapabilityNodeMessage::TerminalCwdValidationRequest(
        request("/tmp/workspace")?,
    ))?;
    trailing.push(0);
    let payload_length = u16::try_from(trailing.len() - 8)?;
    trailing[6..8].copy_from_slice(&payload_length.to_be_bytes());
    assert_eq!(
        decode_message(&trailing),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}

fn request(cwd: &str) -> TestResult<TerminalCwdValidationRequest> {
    Ok(TerminalCwdValidationRequest {
        operation_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([2; 16])?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(3).ok_or_else(|| io::Error::other("zero generation"))?,
        ),
        cwd: AbsoluteCwd::new(cwd)?,
    })
}
