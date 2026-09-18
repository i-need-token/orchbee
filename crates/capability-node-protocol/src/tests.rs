use std::{
    io::{self, Cursor, Read, Write},
    num::{NonZeroU16, NonZeroU64},
};

use crate::{
    CapabilityNodeMessage, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, ProtocolError,
    ReadMessageError, WorkerId, decode_message, encode_message, read_message, write_message,
};

use super::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostCreationId, HostEpoch,
    HostLaunchAccepted, HostLaunchState, HostStartAccepted, HostTerminalId, HostTerminalIdentity,
    LeaseExpiresAtUnixMilliseconds, LeaseId, MAXIMUM_AUTHORIZED_CWD_BYTES, OperationId,
    StartTransition, TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId,
    TerminalLaunchCapabilityVersion, TerminalLaunchFailure, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection, TerminalStartResult,
    WorkerGeneration,
};

#[test]
fn round_trips_every_authorized_message_canonically() -> Result<(), Box<dyn std::error::Error>> {
    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let geometry = geometry()?;
    let identity = host_identity()?;
    let launch_rejections = [
        TerminalLaunchRejection::LifecycleActive,
        TerminalLaunchRejection::WrongWorker,
        TerminalLaunchRejection::StaleWorkerGeneration,
        TerminalLaunchRejection::LeaseExpired,
        TerminalLaunchRejection::StaleFence,
        TerminalLaunchRejection::ConflictingReplay,
        TerminalLaunchRejection::UnsupportedCapability,
        TerminalLaunchRejection::CapacityExceeded,
        TerminalLaunchRejection::HostRejected {
            host_epoch: identity.host_epoch(),
        },
    ];
    let start_rejections = [
        TerminalStartRejection::LifecycleActive,
        TerminalStartRejection::WrongWorker,
        TerminalStartRejection::StaleWorkerGeneration,
        TerminalStartRejection::LeaseExpired,
        TerminalStartRejection::StaleFence,
        TerminalStartRejection::ConflictingReplay,
        TerminalStartRejection::UnsupportedCapability,
        TerminalStartRejection::CapacityExceeded,
        TerminalStartRejection::HostRejected {
            host_epoch: identity.host_epoch(),
        },
        TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch: different_host_identity()?.host_epoch(),
        },
    ];
    let launch_failures = [
        TerminalLaunchFailure::HostUnavailableBeforeHandoff,
        TerminalLaunchFailure::HostReportedFailure {
            host_epoch: identity.host_epoch(),
        },
    ];
    let start_failures = [
        TerminalStartFailure::HostUnavailableBeforeHandoff,
        TerminalStartFailure::HostReportedFailure {
            host_epoch: identity.host_epoch(),
        },
    ];
    let mut messages = vec![
        CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization,
            Some(AbsoluteCwd::new("/tmp/orchbee")?),
            TerminalLaunchMode::Immediate(geometry),
        )),
        CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization,
            None,
            TerminalLaunchMode::Deferred,
        )),
        CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity,
                HostLaunchState::AwaitingStart,
            )),
        )?),
        CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity,
                HostLaunchState::Started(geometry),
            )),
        )?),
        CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::OutcomeUnknown {
                host_epoch: identity.host_epoch(),
            },
        )?),
        CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
            authorization,
            identity,
            geometry,
        )),
        CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
            authorization,
            identity,
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                identity,
                geometry,
                StartTransition::Started,
            )),
        )?),
        CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
            authorization,
            identity,
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                identity,
                geometry,
                StartTransition::AlreadyStarted,
            )),
        )?),
        CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
            authorization,
            identity,
            TerminalStartOutcome::OutcomeUnknown,
        )?),
    ];
    for rejection in launch_rejections {
        messages.push(CapabilityNodeMessage::TerminalLaunchResult(
            TerminalLaunchResult::new(authorization, TerminalLaunchOutcome::Rejected(rejection))?,
        ));
    }
    for rejection in start_rejections {
        messages.push(CapabilityNodeMessage::TerminalStartResult(
            TerminalStartResult::new(
                authorization,
                identity,
                TerminalStartOutcome::Rejected(rejection),
            )?,
        ));
    }
    for failure in launch_failures {
        messages.push(CapabilityNodeMessage::TerminalLaunchResult(
            TerminalLaunchResult::new(authorization, TerminalLaunchOutcome::Failed(failure))?,
        ));
    }
    for failure in start_failures {
        messages.push(CapabilityNodeMessage::TerminalStartResult(
            TerminalStartResult::new(
                authorization,
                identity,
                TerminalStartOutcome::Failed(failure),
            )?,
        ));
    }

    for expected in messages {
        let encoded = encode(&expected)?;
        let decoded = decode_message(&encoded)?;
        assert_eq!(decoded, expected);
        assert_eq!(encode(&decoded)?, encoded);
    }
    Ok(())
}

#[test]
fn lifecycle_active_rejections_use_distinct_documented_launch_and_start_tags()
-> Result<(), Box<dyn std::error::Error>> {
    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let messages = [
        (
            CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
                authorization,
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LifecycleActive),
            )?),
            9,
        ),
        (
            CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
                authorization,
                host_identity()?,
                TerminalStartOutcome::Rejected(TerminalStartRejection::LifecycleActive),
            )?),
            10,
        ),
    ];
    for (message, tag) in messages {
        let bytes = encode_message(&message)?;
        assert_eq!(bytes.last(), Some(&tag));
        assert_eq!(decode_message(&bytes)?, message);
    }
    Ok(())
}

#[test]
fn capability_two_capacity_rejections_have_exact_canonical_tags()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(TERMINAL_LAUNCH_CAPABILITY_VERSION.get(), 2);

    let current_authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let launch = CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
        current_authorization,
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
    )?);
    let encoded_launch = encode(&launch)?;
    assert_eq!(encoded_launch.len(), 101);
    assert_eq!(&encoded_launch[..8], &[b'O', b'B', b'C', b'N', 0, 1, 0, 93]);
    assert_eq!(&encoded_launch[97..99], &[0, 2]);
    assert_eq!(&encoded_launch[99..], &[2, 8]);
    assert_eq!(decode_message(&encoded_launch)?, launch);

    let target = host_identity()?;
    let start = CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
        current_authorization,
        target,
        TerminalStartOutcome::Rejected(TerminalStartRejection::CapacityExceeded),
    )?);
    let encoded_start = encode(&start)?;
    assert_eq!(encoded_start.len(), 149);
    assert_eq!(&encoded_start[..8], &[b'O', b'B', b'C', b'N', 0, 1, 0, 141]);
    assert_eq!(&encoded_start[97..99], &[0, 2]);
    assert_eq!(&encoded_start[147..], &[2, 9]);
    assert_eq!(decode_message(&encoded_start)?, start);

    let legacy_capability = TerminalLaunchCapabilityVersion::new(NonZeroU16::MIN);
    let future_capability = TerminalLaunchCapabilityVersion::new(
        NonZeroU16::new(3).ok_or("future capability must be nonzero")?,
    );
    for unsupported_capability in [legacy_capability, future_capability] {
        let unsupported_authorization = authorization(unsupported_capability)?;
        assert_eq!(
            TerminalLaunchResult::new(
                unsupported_authorization,
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
            ),
            Err(ProtocolError::InvalidPayload)
        );
        assert_eq!(
            TerminalStartResult::new(
                unsupported_authorization,
                target,
                TerminalStartOutcome::Rejected(TerminalStartRejection::CapacityExceeded),
            ),
            Err(ProtocolError::InvalidPayload)
        );

        let mut incompatible_launch = encoded_launch.clone();
        incompatible_launch[97..99].copy_from_slice(&unsupported_capability.get().to_be_bytes());
        assert_eq!(
            decode_message(&incompatible_launch),
            Err(ProtocolError::InvalidPayload)
        );

        let mut incompatible_start = encoded_start.clone();
        incompatible_start[97..99].copy_from_slice(&unsupported_capability.get().to_be_bytes());
        assert_eq!(
            decode_message(&incompatible_start),
            Err(ProtocolError::InvalidPayload)
        );
    }

    let legacy_request =
        CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization(legacy_capability)?,
            None,
            TerminalLaunchMode::Deferred,
        ));
    assert_eq!(decode_message(&encode(&legacy_request)?)?, legacy_request);
    Ok(())
}

#[test]
fn streaming_helpers_handle_partial_io_without_owning_stream_state()
-> Result<(), Box<dyn std::error::Error>> {
    let expected = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        Some(AbsoluteCwd::new("/tmp/orchbee")?),
        TerminalLaunchMode::Immediate(geometry()?),
    ));
    let canonical = encode(&expected)?;

    let mut reader = ChunkedReader::new(canonical.clone(), 3);
    assert_eq!(read_message(&mut reader)?, expected);

    let mut writer = ChunkedWriter::new(2);
    write_message(&mut writer, &expected)?;
    assert_eq!(writer.flush_calls, 0);
    assert_eq!(writer.bytes, canonical);
    Ok(())
}

#[test]
fn rejects_invalid_frame_headers_before_reading_or_allocating_payload()
-> Result<(), Box<dyn std::error::Error>> {
    let message = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        None,
        TerminalLaunchMode::Deferred,
    ));
    let canonical = encode(&message)?;

    let mut wrong_magic = canonical.clone();
    wrong_magic[0] = b'X';
    assert_eq!(
        decode_message(&wrong_magic),
        Err(ProtocolError::InvalidMagic)
    );

    let mut wrong_version = canonical.clone();
    wrong_version[4..6].copy_from_slice(&(PROTOCOL_VERSION + 1).to_be_bytes());
    assert_eq!(
        decode_message(&wrong_version),
        Err(ProtocolError::IncompatibleVersion {
            received: PROTOCOL_VERSION + 1,
        })
    );

    let mut unknown_kind = canonical.clone();
    unknown_kind[8] = u8::MAX;
    assert_eq!(
        decode_message(&unknown_kind),
        Err(ProtocolError::InvalidPayload)
    );

    let oversized_length =
        u16::try_from(MAXIMUM_FRAME_PAYLOAD_BYTES + 1).map_err(|_| "frame bound must fit u16")?;
    let mut oversized_header = Vec::from(*b"OBCN");
    oversized_header.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    oversized_header.extend_from_slice(&oversized_length.to_be_bytes());
    assert!(matches!(
        read_message(&mut Cursor::new(oversized_header)),
        Err(ReadMessageError::Protocol(ProtocolError::PayloadTooLarge {
            actual,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        })) if actual == MAXIMUM_FRAME_PAYLOAD_BYTES + 1
    ));
    Ok(())
}

#[test]
fn streaming_reader_reports_truncated_header_and_payload_as_io_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let message = CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        host_identity()?,
        geometry()?,
    ));
    let canonical = encode(&message)?;

    assert!(matches!(
        read_message(&mut Cursor::new(&canonical[..7])),
        Err(ReadMessageError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    assert!(matches!(
        read_message(&mut Cursor::new(&canonical[..canonical.len() - 1])),
        Err(ReadMessageError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    Ok(())
}

#[test]
fn unknown_start_result_retains_complete_target_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let target = host_identity()?;
    let message = CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
        authorization,
        target,
        TerminalStartOutcome::OutcomeUnknown,
    )?);

    let CapabilityNodeMessage::TerminalStartResult(decoded) = decode_message(&encode(&message)?)?
    else {
        return Err("wrong decoded result kind".into());
    };
    assert_eq!(decoded.authorization(), authorization);
    assert_eq!(decoded.target(), target);
    assert_eq!(decoded.outcome(), TerminalStartOutcome::OutcomeUnknown);
    Ok(())
}

#[test]
fn result_binding_echoes_every_stale_authority_guard() -> Result<(), Box<dyn std::error::Error>> {
    let capability = TerminalLaunchCapabilityVersion::new(
        NonZeroU16::new(9).ok_or("capability version must be nonzero")?,
    );
    let authorization = authorization(capability)?;
    let result = TerminalLaunchResult::new(
        authorization,
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::StaleFence),
    )?;
    let encoded = encode(&CapabilityNodeMessage::TerminalLaunchResult(result))?;
    let CapabilityNodeMessage::TerminalLaunchResult(decoded) = decode_message(&encoded)? else {
        return Err("wrong decoded result kind".into());
    };

    assert_eq!(
        decoded.authorization().terminal_id(),
        authorization.terminal_id()
    );
    assert_eq!(
        decoded.authorization().operation_id(),
        authorization.operation_id()
    );
    assert_eq!(
        decoded.authorization().worker_id(),
        authorization.worker_id()
    );
    assert_eq!(
        decoded.authorization().worker_generation(),
        authorization.worker_generation()
    );
    assert_eq!(decoded.authorization().fence(), authorization.fence());
    assert_eq!(decoded.authorization().lease(), authorization.lease());
    assert_eq!(decoded.authorization().capability_version(), capability);
    Ok(())
}

#[test]
fn result_echoes_lease_identity_and_expiry_byte_for_byte() -> Result<(), Box<dyn std::error::Error>>
{
    const LEASE_ID_RANGE: std::ops::Range<usize> = 73..89;
    const LEASE_EXPIRY_RANGE: std::ops::Range<usize> = 89..97;

    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let message = CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
        authorization,
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LeaseExpired),
    )?);
    let encoded = encode(&message)?;
    assert_eq!(
        &encoded[LEASE_ID_RANGE],
        &authorization.lease().id().as_bytes()
    );
    assert_eq!(
        &encoded[LEASE_EXPIRY_RANGE],
        &authorization.lease().expires_at().get().to_be_bytes()
    );

    let CapabilityNodeMessage::TerminalLaunchResult(decoded) = decode_message(&encoded)? else {
        return Err("wrong decoded result kind".into());
    };
    assert_eq!(decoded.authorization().lease(), authorization.lease());
    assert_eq!(
        encode(&CapabilityNodeMessage::TerminalLaunchResult(decoded))?,
        encoded
    );
    Ok(())
}

#[test]
fn enforces_cwd_and_geometry_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    let maximum_cwd = format!("/{}", "a".repeat(MAXIMUM_AUTHORIZED_CWD_BYTES - 1));
    assert_eq!(AbsoluteCwd::new(&maximum_cwd)?.as_str(), maximum_cwd);
    assert_eq!(
        AbsoluteCwd::new(&format!("{maximum_cwd}a")),
        Err(ProtocolError::InvalidCwd)
    );
    assert_eq!(
        AbsoluteCwd::new("relative/path"),
        Err(ProtocolError::InvalidCwd)
    );
    assert_eq!(
        AbsoluteCwd::new("/tmp\0secret"),
        Err(ProtocolError::InvalidCwd)
    );

    let maximum_grid = TerminalPixelGeometry::new(4096, 64, 15, 1023)?;
    assert_eq!(maximum_grid.columns(), 4096);
    assert_eq!(maximum_grid.rows(), 64);
    assert_eq!(maximum_grid.cell_pixel_width(), 15);
    assert_eq!(maximum_grid.cell_pixel_height(), 1023);
    let unknown_pixels = TerminalPixelGeometry::new(4096, 64, 0, 0)?;
    assert_eq!(unknown_pixels.cell_pixel_width(), 0);
    assert_eq!(unknown_pixels.cell_pixel_height(), 0);
    assert_eq!(
        TerminalPixelGeometry::new(4096, 65, 1, 1),
        Err(ProtocolError::InvalidGeometry)
    );
    assert_eq!(
        TerminalPixelGeometry::new(4096, 64, 16, 1),
        Err(ProtocolError::InvalidGeometry)
    );
    assert_eq!(
        TerminalPixelGeometry::new(0, 24, 8, 16),
        Err(ProtocolError::InvalidGeometry)
    );
    assert_eq!(
        TerminalPixelGeometry::new(80, 24, 0, 16),
        Err(ProtocolError::InvalidGeometry)
    );
    assert_eq!(
        TerminalPixelGeometry::new(80, 24, 8, 0),
        Err(ProtocolError::InvalidGeometry)
    );
    Ok(())
}

#[test]
fn rejects_zero_identity_generation_fence_expiry_and_capability()
-> Result<(), Box<dyn std::error::Error>> {
    let launch = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        None,
        TerminalLaunchMode::Deferred,
    ));
    let canonical = encode(&launch)?;
    for field_range in [
        9..25,
        25..41,
        41..57,
        57..65,
        65..73,
        73..89,
        89..97,
        97..99,
    ] {
        let mut malformed = canonical.clone();
        malformed[field_range].fill(0);
        assert!(matches!(
            decode_message(&malformed),
            Err(ProtocolError::InvalidIdentity | ProtocolError::InvalidPayload)
        ));
    }

    let start = CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        host_identity()?,
        geometry()?,
    ));
    let canonical_start = encode(&start)?;
    for identity_range in [99..115, 115..131, 131..147] {
        let mut malformed = canonical_start.clone();
        malformed[identity_range].fill(0);
        assert_eq!(
            decode_message(&malformed),
            Err(ProtocolError::InvalidIdentity)
        );
    }
    Ok(())
}

#[test]
fn rejects_relative_non_utf8_and_extra_cwd_fields() -> Result<(), Box<dyn std::error::Error>> {
    let launch = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        Some(AbsoluteCwd::new("/tmp")?),
        TerminalLaunchMode::Deferred,
    ));
    let canonical = encode(&launch)?;
    let cwd_offset = canonical
        .windows(4)
        .position(|window| window == b"/tmp")
        .ok_or("canonical cwd bytes must exist")?;

    let mut relative = canonical.clone();
    relative[cwd_offset] = b'.';
    assert!(matches!(
        decode_message(&relative),
        Err(ProtocolError::InvalidCwd)
    ));

    let mut non_utf8 = canonical.clone();
    non_utf8[cwd_offset] = 0xff;
    assert!(matches!(
        decode_message(&non_utf8),
        Err(ProtocolError::InvalidCwd)
    ));

    let mut extra = canonical;
    append_payload_bytes(&mut extra, &[1, b'X'])?;
    assert!(matches!(
        decode_message(&extra),
        Err(ProtocolError::InvalidPayload)
    ));
    Ok(())
}

#[test]
fn nonempty_environment_overlay_has_no_wire_representation()
-> Result<(), Box<dyn std::error::Error>> {
    let launch = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        None,
        TerminalLaunchMode::Deferred,
    ));
    let mut encoded = encode(&launch)?;

    // The closed launch payload ends after its start-mode byte. Bytes that could
    // encode even one environment entry are therefore unknown extra fields.
    append_payload_bytes(&mut encoded, &[1, 3, b'F', b'O', b'O', 3, b'b', b'a', b'r'])?;
    assert!(matches!(
        decode_message(&encoded),
        Err(ProtocolError::InvalidPayload)
    ));
    Ok(())
}

#[test]
fn maximum_launch_remains_inside_the_frame_bound() -> Result<(), Box<dyn std::error::Error>> {
    let maximum_cwd = AbsoluteCwd::new(&format!(
        "/{}",
        "z".repeat(MAXIMUM_AUTHORIZED_CWD_BYTES - 1)
    ))?;
    let message = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        Some(maximum_cwd),
        TerminalLaunchMode::Immediate(geometry()?),
    ));
    let encoded = encode(&message)?;
    let payload_length = usize::from(u16::from_be_bytes([encoded[6], encoded[7]]));
    assert!(payload_length <= MAXIMUM_FRAME_PAYLOAD_BYTES);
    assert_eq!(decode_message(&encoded)?, message);
    Ok(())
}

#[test]
fn rejects_unknown_outcomes_transitions_and_truncated_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;

    let mut unknown_launch_mode = encode(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
        AuthorizedTerminalLaunch::new(authorization, None, TerminalLaunchMode::Deferred),
    ))?;
    let launch_mode_offset = unknown_launch_mode.len() - 1;
    unknown_launch_mode[launch_mode_offset] = 99;
    assert_eq!(
        decode_message(&unknown_launch_mode),
        Err(ProtocolError::InvalidPayload)
    );

    let canonical_unknown = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            host_identity()?,
            TerminalStartOutcome::OutcomeUnknown,
        )?,
    ))?;
    let mut unknown_outcome = canonical_unknown.clone();
    let outcome_offset = unknown_outcome.len() - 1;
    unknown_outcome[outcome_offset] = 99;
    assert!(matches!(
        decode_message(&unknown_outcome),
        Err(ProtocolError::InvalidPayload)
    ));

    let mut unknown_launch_state = encode(&CapabilityNodeMessage::TerminalLaunchResult(
        TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                host_identity()?,
                HostLaunchState::AwaitingStart,
            )),
        )?,
    ))?;
    let launch_state_offset = unknown_launch_state.len() - 1;
    unknown_launch_state[launch_state_offset] = 99;
    assert_eq!(
        decode_message(&unknown_launch_state),
        Err(ProtocolError::InvalidPayload)
    );

    let target = host_identity()?;
    let mut unknown_start_transition = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                target,
                geometry()?,
                StartTransition::Started,
            )),
        )?,
    ))?;
    let transition_offset = unknown_start_transition.len() - 1;
    unknown_start_transition[transition_offset] = 99;
    assert_eq!(
        decode_message(&unknown_start_transition),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_rejection = encode(&CapabilityNodeMessage::TerminalLaunchResult(
        TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::WrongWorker),
        )?,
    ))?;
    let rejection_offset = unknown_rejection.len() - 1;
    unknown_rejection[rejection_offset] = 99;
    assert_eq!(
        decode_message(&unknown_rejection),
        Err(ProtocolError::InvalidPayload)
    );

    let mut start_only_launch_rejection = encode(&CapabilityNodeMessage::TerminalLaunchResult(
        TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                host_epoch: host_identity()?.host_epoch(),
            }),
        )?,
    ))?;
    let launch_rejection_tag_offset = start_only_launch_rejection.len() - 17;
    start_only_launch_rejection[launch_rejection_tag_offset] = 9;
    assert_eq!(
        decode_message(&start_only_launch_rejection),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_after_launch_lifecycle = encode(&CapabilityNodeMessage::TerminalLaunchResult(
        TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
        )?,
    ))?;
    let launch_lifecycle_tag_offset = unknown_after_launch_lifecycle.len() - 1;
    unknown_after_launch_lifecycle[launch_lifecycle_tag_offset] = 10;
    assert_eq!(
        decode_message(&unknown_after_launch_lifecycle),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_failure = encode(&CapabilityNodeMessage::TerminalLaunchResult(
        TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
        )?,
    ))?;
    let failure_offset = unknown_failure.len() - 1;
    unknown_failure[failure_offset] = 99;
    assert_eq!(
        decode_message(&unknown_failure),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_start_rejection = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            host_identity()?,
            TerminalStartOutcome::Rejected(TerminalStartRejection::WrongWorker),
        )?,
    ))?;
    let start_rejection_offset = unknown_start_rejection.len() - 1;
    unknown_start_rejection[start_rejection_offset] = 99;
    assert_eq!(
        decode_message(&unknown_start_rejection),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_after_start_lifecycle = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            host_identity()?,
            TerminalStartOutcome::Rejected(TerminalStartRejection::CapacityExceeded),
        )?,
    ))?;
    let start_lifecycle_tag_offset = unknown_after_start_lifecycle.len() - 1;
    unknown_after_start_lifecycle[start_lifecycle_tag_offset] = 11;
    assert_eq!(
        decode_message(&unknown_after_start_lifecycle),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_start_failure = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            host_identity()?,
            TerminalStartOutcome::Failed(TerminalStartFailure::HostUnavailableBeforeHandoff),
        )?,
    ))?;
    let start_failure_offset = unknown_start_failure.len() - 1;
    unknown_start_failure[start_failure_offset] = 99;
    assert_eq!(
        decode_message(&unknown_start_failure),
        Err(ProtocolError::InvalidPayload)
    );

    let mut truncated = encode(&CapabilityNodeMessage::AuthorizedTerminalStart(
        AuthorizedTerminalStart::new(authorization, host_identity()?, geometry()?),
    ))?;
    truncated.pop();
    let payload_length = u16::try_from(truncated.len() - 8)?;
    truncated[6..8].copy_from_slice(&payload_length.to_be_bytes());
    assert!(matches!(
        decode_message(&truncated),
        Err(ProtocolError::InvalidPayload)
    ));
    Ok(())
}

#[test]
fn rejects_contradictory_start_host_observations() -> Result<(), Box<dyn std::error::Error>> {
    const START_OBSERVED_IDENTITY_OFFSET: usize = 148;
    const START_OBSERVED_EPOCH_RANGE: std::ops::Range<usize> = 149..165;

    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let target = host_identity()?;
    let different = different_host_identity()?;
    let error = Err(ProtocolError::ContradictoryHostObservation);

    assert_eq!(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                different,
                geometry()?,
                StartTransition::Started,
            )),
        ),
        error
    );
    assert_eq!(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::Rejected(TerminalStartRejection::HostRejected {
                host_epoch: different.host_epoch(),
            }),
        ),
        error
    );
    assert_eq!(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::Failed(TerminalStartFailure::HostReportedFailure {
                host_epoch: different.host_epoch(),
            }),
        ),
        error
    );
    assert_eq!(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
                observed_host_epoch: target.host_epoch(),
            }),
        ),
        error
    );

    let mut accepted = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                target,
                geometry()?,
                StartTransition::Started,
            )),
        )?,
    ))?;
    accepted[START_OBSERVED_IDENTITY_OFFSET] ^= 1;
    assert_eq!(
        decode_message(&accepted),
        Err(ProtocolError::ContradictoryHostObservation)
    );

    let mut rejected = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::Rejected(TerminalStartRejection::HostRejected {
                host_epoch: target.host_epoch(),
            }),
        )?,
    ))?;
    rejected[START_OBSERVED_EPOCH_RANGE.start] ^= 1;
    assert_eq!(
        decode_message(&rejected),
        Err(ProtocolError::ContradictoryHostObservation)
    );

    let mut failed = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::Failed(TerminalStartFailure::HostReportedFailure {
                host_epoch: target.host_epoch(),
            }),
        )?,
    ))?;
    failed[START_OBSERVED_EPOCH_RANGE.start] ^= 1;
    assert_eq!(
        decode_message(&failed),
        Err(ProtocolError::ContradictoryHostObservation)
    );

    let mut identity_changed = encode(&CapabilityNodeMessage::TerminalStartResult(
        TerminalStartResult::new(
            authorization,
            target,
            TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
                observed_host_epoch: different.host_epoch(),
            }),
        )?,
    ))?;
    identity_changed[START_OBSERVED_EPOCH_RANGE].copy_from_slice(&target.host_epoch().as_bytes());
    assert_eq!(
        decode_message(&identity_changed),
        Err(ProtocolError::ContradictoryHostObservation)
    );
    Ok(())
}

fn authorization(
    capability_version: TerminalLaunchCapabilityVersion,
) -> Result<TerminalOperationAuthorization, ProtocolError> {
    let worker_generation = NonZeroU64::new(5).ok_or(ProtocolError::InvalidPayload)?;
    let fence = NonZeroU64::new(8).ok_or(ProtocolError::InvalidPayload)?;
    let expiry = NonZeroU64::new(1_800_000_000_000).ok_or(ProtocolError::InvalidPayload)?;
    Ok(TerminalOperationAuthorization::new(
        TerminalId::from_bytes([1; 16])?,
        OperationId::from_bytes([2; 16])?,
        WorkerId::from_bytes([3; 16])?,
        WorkerGeneration::new(worker_generation),
        TerminalFence::new(fence),
        TerminalOperationLease::new(
            LeaseId::from_bytes([4; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(expiry),
        ),
        capability_version,
    ))
}

fn host_identity() -> Result<HostTerminalIdentity, ProtocolError> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([5; 16])?,
        HostEpoch::from_bytes([6; 16])?,
        HostTerminalId::from_bytes([7; 16])?,
    ))
}

fn different_host_identity() -> Result<HostTerminalIdentity, ProtocolError> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([8; 16])?,
        HostEpoch::from_bytes([9; 16])?,
        HostTerminalId::from_bytes([10; 16])?,
    ))
}

fn geometry() -> Result<TerminalPixelGeometry, ProtocolError> {
    TerminalPixelGeometry::new(120, 40, 9, 18)
}

fn encode(message: &CapabilityNodeMessage) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    encode_message(message).map_err(Into::into)
}

fn append_payload_bytes(
    frame: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    frame.extend_from_slice(bytes);
    let payload_length = u16::try_from(frame.len() - 8)?;
    frame[4..6].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    frame[6..8].copy_from_slice(&payload_length.to_be_bytes());
    Ok(())
}

struct ChunkedReader {
    inner: Cursor<Vec<u8>>,
    maximum_chunk: usize,
}

impl ChunkedReader {
    fn new(bytes: Vec<u8>, maximum_chunk: usize) -> Self {
        Self {
            inner: Cursor::new(bytes),
            maximum_chunk,
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let chunk_length = buffer.len().min(self.maximum_chunk);
        self.inner.read(&mut buffer[..chunk_length])
    }
}

struct ChunkedWriter {
    bytes: Vec<u8>,
    flush_calls: usize,
    maximum_chunk: usize,
}

impl ChunkedWriter {
    fn new(maximum_chunk: usize) -> Self {
        Self {
            bytes: Vec::new(),
            flush_calls: 0,
            maximum_chunk,
        }
    }
}

impl Write for ChunkedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let chunk_length = buffer.len().min(self.maximum_chunk);
        self.bytes.extend_from_slice(&buffer[..chunk_length]);
        Ok(chunk_length)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_calls += 1;
        Ok(())
    }
}

fn paste_request(content: Vec<u8>) -> Result<CapabilityNodeMessage, Box<dyn std::error::Error>> {
    Ok(CapabilityNodeMessage::TerminalControlRequest(
        crate::TerminalControlRequest {
            request_id: OperationId::from_bytes([11; 16])?,
            worker_id: WorkerId::from_bytes([12; 16])?,
            worker_generation: WorkerGeneration::new(
                NonZeroU64::new(7).ok_or(ProtocolError::InvalidPayload)?,
            ),
            viewer_id: OperationId::from_bytes([13; 16])?,
            controller_id: OperationId::from_bytes([14; 16])?,
            action: crate::TerminalControlAction::Paste {
                sequence: NonZeroU64::new(41).ok_or(ProtocolError::InvalidPayload)?,
                content: crate::PasteContent::new(content)?,
            },
        },
    ))
}

#[test]
fn paste_record_round_trips_opaque_content_as_obnq() -> Result<(), Box<dyn std::error::Error>> {
    let mut content = Vec::new();
    for index in 0..8192_u32 {
        content.push((index % 251) as u8);
    }
    content.extend_from_slice(&[0x00, 0x02, 0x1b, 0x80, 0xff, b'\n']);
    let request = paste_request(content)?;

    let record = encode_message(&request)?;
    assert!(record.starts_with(b"OBNQ\0\x01\0\0"));
    assert_eq!(record.len(), 12 + 80 + 8192 + 6);
    assert_eq!(decode_message(&record)?, request);

    let mut reader = ChunkedReader::new(record.clone(), 5);
    assert_eq!(read_message(&mut reader)?, request);

    let mut adjacent = record.clone();
    adjacent.extend_from_slice(&record);
    let mut stream = ChunkedReader::new(adjacent, 7);
    assert_eq!(read_message(&mut stream)?, request);
    assert_eq!(read_message(&mut stream)?, request);
    Ok(())
}

#[test]
fn paste_record_rejects_malformed_headers_and_bodies() -> Result<(), Box<dyn std::error::Error>> {
    let request = paste_request(b"payload".to_vec())?;
    let record = encode_message(&request)?;

    let mut wrong_version = record.clone();
    wrong_version[4..6].copy_from_slice(&2_u16.to_be_bytes());
    assert_eq!(
        decode_message(&wrong_version),
        Err(ProtocolError::IncompatibleVersion { received: 2 })
    );

    let mut reserved = record.clone();
    reserved[6] = 1;
    assert_eq!(
        decode_message(&reserved),
        Err(ProtocolError::InvalidPayload)
    );

    let mut wrong_magic = record.clone();
    wrong_magic[3] = b'X';
    assert_eq!(
        decode_message(&wrong_magic),
        Err(ProtocolError::InvalidMagic)
    );

    for body_length in [0_usize, 79, 80] {
        let mut undersized = record.clone();
        undersized[8..12].copy_from_slice(&(body_length as u32).to_be_bytes());
        assert_eq!(
            decode_message(&undersized),
            Err(ProtocolError::InvalidPayload)
        );
    }

    let mut oversized = record.clone();
    oversized[8..12].copy_from_slice(&(262_225_u32).to_be_bytes());
    assert!(matches!(
        decode_message(&oversized),
        Err(ProtocolError::PayloadTooLarge {
            maximum: 262_224,
            ..
        })
    ));
    assert!(matches!(
        read_message(&mut Cursor::new(oversized)),
        Err(ReadMessageError::Protocol(ProtocolError::PayloadTooLarge {
            maximum: 262_224,
            ..
        }))
    ));

    let mut trailing = record.clone();
    trailing.push(0);
    assert_eq!(
        decode_message(&trailing),
        Err(ProtocolError::InvalidPayload)
    );

    let truncated = &record[..record.len() - 1];
    assert!(matches!(
        read_message(&mut Cursor::new(truncated)),
        Err(ReadMessageError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));

    let mut zero_sequence = record.clone();
    zero_sequence[12 + 72..12 + 80].copy_from_slice(&0_u64.to_be_bytes());
    assert_eq!(
        decode_message(&zero_sequence),
        Err(ProtocolError::InvalidPayload)
    );

    let mut zero_generation = record.clone();
    zero_generation[12 + 32..12 + 40].copy_from_slice(&0_u64.to_be_bytes());
    assert_eq!(
        decode_message(&zero_generation),
        Err(ProtocolError::InvalidPayload)
    );

    let mut zero_operation = record.clone();
    zero_operation[12..12 + 16].fill(0);
    assert_eq!(
        decode_message(&zero_operation),
        Err(ProtocolError::InvalidIdentity)
    );
    Ok(())
}

#[test]
fn paste_record_supports_maximum_content() -> Result<(), Box<dyn std::error::Error>> {
    let request = paste_request(vec![0xab; 262_144])?;
    let record = encode_message(&request)?;
    assert_eq!(record.len(), 262_236);
    assert_eq!(decode_message(&record)?, request);
    assert!(crate::PasteContent::new(vec![0; 262_145]).is_err());
    assert!(crate::PasteContent::new(Vec::new()).is_err());
    Ok(())
}

#[test]
fn pasted_result_and_paste_rejections_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let result = CapabilityNodeMessage::TerminalControlResult(crate::TerminalControlResult {
        request_id: OperationId::from_bytes([11; 16])?,
        viewer_id: OperationId::from_bytes([13; 16])?,
        controller_id: OperationId::from_bytes([14; 16])?,
        outcome: crate::TerminalControlOutcome::Pasted {
            accepted_bytes: crate::PasteAcceptedBytes::new(16_777)?,
        },
    });
    assert_eq!(decode_message(&encode(&result)?)?, result);
    assert!(crate::PasteAcceptedBytes::new(0).is_err());
    assert!(crate::PasteAcceptedBytes::new(1_048_576).is_ok());
    assert!(crate::PasteAcceptedBytes::new(1_048_577).is_err());

    for reason in [
        crate::ControlRejection::HostInputInvalid,
        crate::ControlRejection::HostInputLimitExceeded,
        crate::ControlRejection::HostInputBackpressure,
        crate::ControlRejection::HostInputUnavailable,
    ] {
        let rejected = CapabilityNodeMessage::TerminalControlResult(crate::TerminalControlResult {
            request_id: OperationId::from_bytes([11; 16])?,
            viewer_id: OperationId::from_bytes([13; 16])?,
            controller_id: OperationId::from_bytes([14; 16])?,
            outcome: crate::TerminalControlOutcome::Rejected(reason),
        });
        assert_eq!(decode_message(&encode(&rejected)?)?, rejected);
    }
    Ok(())
}
