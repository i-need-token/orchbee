use std::{error::Error, num::NonZeroU64};

use orchbee_terminal_protocol::{
    CreationId, Envelope, TerminalId, TerminalIdentity, TerminalMetadataChanged,
    encode_lifecycle_control, encode_terminal_effect_event, encode_transport_envelope,
};

use super::*;

#[test]
fn only_strict_identity_scoped_lifecycle_and_metadata_events_are_drained()
-> Result<(), Box<dyn Error>> {
    let epoch = HostEpoch::from_bytes([1; 16])?;
    let other_epoch = HostEpoch::from_bytes([2; 16])?;
    let identity = TerminalIdentity {
        creation_id: CreationId::from_bytes([3; 16])?,
        host_epoch: epoch,
        terminal_id: TerminalId::from_bytes([4; 16])?,
    };
    let started = encode_lifecycle_control(&LifecycleControl::TerminalStarted(identity))?;
    let record = |kind, role, correlation, host_epoch, optional_message, payload: &[u8]| {
        encode_transport_envelope(&Envelope {
            correlation_id: correlation,
            host_epoch,
            kind,
            optional_message,
            payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role,
        })
    };
    let valid = record(
        started.kind,
        MessageRole::Event,
        None,
        Some(epoch),
        false,
        &started.payload,
    )?;
    validate_event(&valid, epoch)?;
    let metadata = encode_terminal_effect_event(&TerminalEffectEvent::MetadataChanged(
        TerminalMetadataChanged {
            current_directory: Some("/tmp".to_owned()),
            identity,
            revision: NonZeroU64::MIN,
            title: Some("shell".to_owned()),
        },
    ))?;
    validate_event(
        &record(
            metadata.kind,
            metadata.role,
            None,
            Some(epoch),
            false,
            &metadata.payload,
        )?,
        epoch,
    )?;
    let wrong_identity =
        encode_lifecycle_control(&LifecycleControl::TerminalStarted(TerminalIdentity {
            host_epoch: other_epoch,
            ..identity
        }))?;
    for bytes in [
        record(
            started.kind,
            MessageRole::Event,
            None,
            Some(other_epoch),
            false,
            &started.payload,
        )?,
        record(
            started.kind,
            MessageRole::Event,
            None,
            Some(epoch),
            true,
            &started.payload,
        )?,
        record(
            started.kind,
            MessageRole::Event,
            None,
            Some(epoch),
            false,
            &wrong_identity.payload,
        )?,
        record(
            started.kind,
            MessageRole::Event,
            None,
            Some(epoch),
            false,
            b"{}",
        )?,
        valid[..valid.len() - 1].to_vec(),
    ] {
        assert!(validate_event(&bytes, epoch).is_err());
    }
    for (offset, value) in [(24, 5), (6, 2), (16, 255)] {
        let mut malformed = valid.clone();
        malformed[offset] = value;
        assert!(validate_event(&malformed, epoch).is_err());
    }
    Ok(())
}
