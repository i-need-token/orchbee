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
    LaunchSelection, LeaseExpiresAtUnixMilliseconds, LeaseId, MAXIMUM_AUTHORIZED_CWD_BYTES,
    MAXIMUM_LAUNCH_SELECTION_BYTES, OperationId, StartTransition,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchCapabilityVersion,
    TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection,
    TerminalLaunchResult, TerminalOperationAuthorization, TerminalOperationLease,
    TerminalPixelGeometry, TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection,
    TerminalStartResult, WorkerGeneration,
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
            None,
        )),
        CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization,
            None,
            TerminalLaunchMode::Deferred,
            None,
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
    assert_eq!(PROTOCOL_VERSION, 2);
    assert_eq!(TERMINAL_LAUNCH_CAPABILITY_VERSION.get(), 2);

    let current_authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let launch = CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
        current_authorization,
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
    )?);
    let encoded_launch = encode(&launch)?;
    assert_eq!(encoded_launch.len(), 101);
    assert_eq!(&encoded_launch[..8], &[b'O', b'B', b'C', b'N', 0, 2, 0, 93]);
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
    assert_eq!(&encoded_start[..8], &[b'O', b'B', b'C', b'N', 0, 2, 0, 141]);
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
            None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
fn launch_selection_tail_round_trips_and_decodes_closed() -> Result<(), Box<dyn std::error::Error>>
{
    let authorization = authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?;
    let selection = LaunchSelection::new(
        "/bin/sh".to_owned(),
        vec![
            "-c".to_owned(),
            String::new(),
            "--".to_owned(),
            "--flag=1".to_owned(),
            "漢字".to_owned(),
        ],
    )?;
    let launch = CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
        authorization,
        Some(AbsoluteCwd::new("/tmp")?),
        TerminalLaunchMode::Immediate(geometry()?),
        Some(selection),
    ));
    let canonical = encode(&launch)?;
    assert_eq!(decode_message(&canonical)?, launch);

    // A payload that ends at the mode block decodes to the default launch —
    // the pre-extension wire shape stays a legal same-kind frame.
    let base = encode(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
        AuthorizedTerminalLaunch::new(
            authorization,
            Some(AbsoluteCwd::new("/tmp")?),
            TerminalLaunchMode::Immediate(geometry()?),
            None,
        ),
    ))?;
    assert_eq!(canonical[8..base.len()], base[8..]);
    let decoded_default = match decode_message(&base)? {
        CapabilityNodeMessage::AuthorizedTerminalLaunch(value) => value,
        other => return Err(format!("decoded {other:?}").into()),
    };
    assert_eq!(decoded_default.launch(), None);

    // A truncated selection block is rejected at every cut point, and a
    // garbage tail after a complete selection is rejected by finish().
    for cut in 1..(canonical.len() - base.len()) {
        let mut truncated = canonical[..base.len() + cut].to_vec();
        let length = u16::try_from(truncated.len() - 8)?;
        truncated[6..8].copy_from_slice(&length.to_be_bytes());
        assert!(
            decode_message(&truncated).is_err(),
            "tail truncated at {cut} decoded"
        );
    }
    let mut trailing = canonical;
    append_payload_bytes(&mut trailing, &[0])?;
    assert_eq!(
        decode_message(&trailing),
        Err(ProtocolError::InvalidPayload)
    );

    // A syntactically complete but lexically invalid selection — a relative
    // executable — is rejected by the closed decoder contract.
    let mut relative = encode(&launch)?;
    let exec_offset = relative
        .windows(7)
        .position(|window| window == b"/bin/sh")
        .ok_or("canonical exec bytes must exist")?;
    relative[exec_offset] = b'.';
    assert_eq!(
        decode_message(&relative),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}

#[test]
fn maximum_launch_remains_inside_the_frame_bound() -> Result<(), Box<dyn std::error::Error>> {
    let maximum_cwd = AbsoluteCwd::new(&format!(
        "/{}",
        "z".repeat(MAXIMUM_AUTHORIZED_CWD_BYTES - 1)
    ))?;
    // The worst-case payload pairs the maximum cwd with a maximum-size launch
    // selection: executable plus argument bytes total exactly the 30-KiB bound.
    let maximum_selection = LaunchSelection::new("/bin/sh".to_owned(), {
        let mut args = vec![String::new(); 128];
        args[0] = "x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES - "/bin/sh".len());
        args
    })?;
    let message = CapabilityNodeMessage::AuthorizedTerminalLaunch(
        AuthorizedTerminalLaunch::new(
            authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
            Some(maximum_cwd),
            TerminalLaunchMode::Immediate(geometry()?),
            Some(maximum_selection),
        )
        .with_environment({
            let mut pairs: Vec<_> = (0..32).map(|n| (format!("K{n}"), String::new())).collect();
            let names: usize = pairs.iter().map(|(name, _)| name.len()).sum();
            pairs[0].1 = "x".repeat(crate::MAXIMUM_LAUNCH_ENVIRONMENT_BYTES - names);
            crate::LaunchEnvironment::new(pairs)?
        }),
    );
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
        AuthorizedTerminalLaunch::new(authorization, None, TerminalLaunchMode::Deferred, None),
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

fn send_request(records: crate::SendRecords) -> crate::TerminalSendRequest {
    crate::TerminalSendRequest {
        request_id: OperationId::from_bytes([21; 16]).expect("id"),
        worker_id: WorkerId::from_bytes([22; 16]).expect("id"),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(9).expect("nonzero")),
        target: HostTerminalIdentity::new(
            HostCreationId::from_bytes([23; 16]).expect("id"),
            HostEpoch::from_bytes([24; 16]).expect("id"),
            HostTerminalId::from_bytes([25; 16]).expect("id"),
        ),
        attachment_id: OperationId::from_bytes([26; 16]).expect("id"),
        records,
    }
}

#[test]
fn send_stroke_frame_preserves_next_record_and_rejects_malformed_values() {
    use crate::{SendKeyStroke, SendRecord, SendRecords, SendStrokeKey};
    let records = SendRecords::new(vec![
        SendRecord::Stroke(SendKeyStroke::new(SendStrokeKey::Character('😀'), 8).unwrap()),
        SendRecord::Key {
            key: crate::SendKey::Enter,
        },
    ])
    .unwrap();
    let request = CapabilityNodeMessage::TerminalSendRequest(send_request(records));
    let wire = encode_message(&request).unwrap();
    let at = 12 + 105;
    assert_eq!(&wire[at..], &[6, 8, 4, 0, 1, 0xf6, 0, 2, 1]);
    assert_eq!(decode_message(&wire).unwrap(), request);
    for end in 0..wire.len() {
        assert!(decode_message(&wire[..end]).is_err());
    }
    for (offset, value) in [(0, 7), (1, 16), (2, 0), (3, 1)] {
        let mut invalid = wire.clone();
        invalid[at + offset] = value;
        assert!(decode_message(&invalid).is_err());
    }
}

/// One-record convenience: every typed constructor re-validates.
fn one_record(record: crate::SendRecord) -> crate::SendRecords {
    crate::SendRecords::new(vec![record]).expect("one record is always in bounds")
}

fn text_record(text: &str, enter: bool) -> crate::SendRecord {
    crate::SendRecord::Text(
        crate::SendTextInput::new(crate::SendText::new(text.to_owned()).expect("text"), enter)
            .expect("text input"),
    )
}

#[test]
fn send_request_record_encodes_the_golden_obns_frame() -> Result<(), Box<dyn std::error::Error>> {
    let request = CapabilityNodeMessage::TerminalSendRequest(send_request(one_record(
        text_record("ls -la", true),
    )));
    let mut body = Vec::new();
    body.extend_from_slice(&[21; 16]);
    body.extend_from_slice(&[22; 16]);
    body.extend_from_slice(&9_u64.to_be_bytes());
    body.extend_from_slice(&[23; 16]);
    body.extend_from_slice(&[24; 16]);
    body.extend_from_slice(&[25; 16]);
    body.extend_from_slice(&[26; 16]);
    body.push(1);
    body.push(1);
    body.extend_from_slice(&6_u16.to_be_bytes());
    body.extend_from_slice(b"ls -la");
    body.push(1);
    let mut expected = Vec::from(*b"OBNS");
    expected.extend_from_slice(&1_u16.to_be_bytes());
    expected.extend_from_slice(&0_u16.to_be_bytes());
    expected.extend_from_slice(&(body.len() as u32).to_be_bytes());
    expected.extend_from_slice(&body);
    assert_eq!(encode_message(&request)?, expected);
    assert_eq!(expected.len(), 12 + 105 + 10);
    assert_eq!(decode_message(&expected)?, request);
    let mut stream = ChunkedReader::new(expected.clone(), 7);
    assert_eq!(read_message(&mut stream)?, request);
    Ok(())
}

#[test]
fn send_record_sequence_preserves_order_and_every_form() -> Result<(), Box<dyn std::error::Error>> {
    let request =
        CapabilityNodeMessage::TerminalSendRequest(send_request(crate::SendRecords::new(vec![
            text_record("echo hi", false),
            crate::SendRecord::Key {
                key: crate::SendKey::CtrlC,
            },
            crate::SendRecord::Combo {
                modifiers: crate::SendModifiers::new(true, true, false)?,
                base: crate::SendComboBase::new_letter(b'p')?,
            },
            crate::SendRecord::Combo {
                modifiers: crate::SendModifiers::new(false, false, true)?,
                base: crate::SendComboBase::new_named(crate::SendKey::Tab)?,
            },
            crate::SendRecord::Bytes {
                bytes: crate::SendBytes::new(vec![0x00, 0xff, 0x1b])?,
            },
            crate::SendRecord::Paste {
                content: crate::SendPasteContent::new(b"pasted\ncontent".to_vec())?,
            },
            crate::SendRecord::Key {
                key: crate::SendKey::Enter,
            },
        ])?));
    let encoded = encode_message(&request)?;
    assert!(encoded.starts_with(b"OBNS\0\x01\0\0"));
    assert_eq!(decode_message(&encoded)?, request);
    Ok(())
}

#[test]
fn send_request_decode_enforces_every_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let request = CapabilityNodeMessage::TerminalSendRequest(send_request(one_record(
        text_record("hi", false),
    )));
    let encoded = encode_message(&request)?;
    // Header: 12 bytes; body prefix 105 bytes; text record starts at 12+105.
    let record_at = 12 + 105;

    let mut wrong_magic = encoded.clone();
    wrong_magic[3] = b'X';
    // A wrong record magic falls through to the standard frame decoder,
    // which rejects the unknown magic identically.
    assert_eq!(
        decode_message(&wrong_magic),
        Err(ProtocolError::InvalidMagic)
    );

    let mut wrong_version = encoded.clone();
    wrong_version[4..6].copy_from_slice(&2_u16.to_be_bytes());
    assert_eq!(
        decode_message(&wrong_version),
        Err(ProtocolError::IncompatibleVersion { received: 1 })
    );

    let mut reserved = encoded.clone();
    reserved[6] = 1;
    assert_eq!(
        decode_message(&reserved),
        Err(ProtocolError::InvalidPayload)
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode_message(&trailing),
        Err(ProtocolError::InvalidPayload)
    );

    // The declared body length must exceed the identity prefix.
    let mut short_body = encoded.clone();
    short_body[8..12].copy_from_slice(&105_u32.to_be_bytes());
    assert!(matches!(
        decode_message(&short_body),
        Err(ProtocolError::PayloadTooLarge { .. })
    ));

    let mut oversized = encoded.clone();
    oversized[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        decode_message(&oversized),
        Err(ProtocolError::PayloadTooLarge { .. })
    ));

    // Record count zero is invalid; count exceeding capacity rejects at
    // sequence construction.
    let mut zero_count = encoded.clone();
    zero_count[12 + 104] = 0;
    zero_count.truncate(12 + 105);
    zero_count[8..12].copy_from_slice(&105_u32.to_be_bytes());
    // body == prefix exactly is below the minimum, so the length rule fires.
    assert!(matches!(
        decode_message(&zero_count),
        Err(ProtocolError::PayloadTooLarge { .. })
    ));

    // An unknown record kind refuses.
    let mut bad_kind = encoded.clone();
    bad_kind[record_at] = 9;
    assert_eq!(
        decode_message(&bad_kind),
        Err(ProtocolError::InvalidPayload)
    );

    // An unknown key code refuses.
    let mut bad_key = encoded.clone();
    bad_key[record_at] = 2;
    bad_key[record_at + 1] = 16;
    let mut bad_key_frame = bad_key[..record_at + 2].to_vec();
    let bad_key_len = u32::try_from(bad_key_frame.len() - 12)?;
    bad_key_frame[8..12].copy_from_slice(&bad_key_len.to_be_bytes());
    assert_eq!(
        decode_message(&bad_key_frame),
        Err(ProtocolError::InvalidPayload)
    );

    // Invalid UTF-8 inside a text record refuses.
    let mut bad_utf8 = encoded.clone();
    bad_utf8[record_at + 3] = 0x80;
    assert_eq!(
        decode_message(&bad_utf8),
        Err(ProtocolError::InvalidPayload)
    );

    // A non-0/1 enter flag refuses.
    let mut bad_enter = encoded.clone();
    bad_enter[record_at + 5] = 2;
    assert_eq!(
        decode_message(&bad_enter),
        Err(ProtocolError::InvalidPayload)
    );

    let mut zero_generation = encoded.clone();
    zero_generation[12 + 32..12 + 40].copy_from_slice(&0_u64.to_be_bytes());
    assert_eq!(
        decode_message(&zero_generation),
        Err(ProtocolError::InvalidPayload)
    );

    let mut zero_epoch = encoded.clone();
    zero_epoch[12 + 56..12 + 72].fill(0);
    assert_eq!(
        decode_message(&zero_epoch),
        Err(ProtocolError::InvalidIdentity)
    );

    let mut zero_attachment = encoded.clone();
    zero_attachment[12 + 88..12 + 104].fill(0);
    assert_eq!(
        decode_message(&zero_attachment),
        Err(ProtocolError::InvalidIdentity)
    );
    Ok(())
}

#[test]
fn send_records_enforce_count_and_aggregate_bounds() -> Result<(), Box<dyn std::error::Error>> {
    // 64 is the wire-admitted ceiling; 65 never constructs.
    assert!(
        crate::SendRecords::new(vec![
            crate::SendRecord::Key {
                key: crate::SendKey::Enter,
            };
            crate::terminal_send::MAXIMUM_SEND_RECORDS
        ])
        .is_ok()
    );
    assert!(
        crate::SendRecords::new(vec![
            crate::SendRecord::Key {
                key: crate::SendKey::Enter,
            };
            crate::terminal_send::MAXIMUM_SEND_RECORDS + 1
        ])
        .is_err()
    );
    assert!(crate::SendRecords::new(Vec::new()).is_err());

    // The aggregate decoded bound is content bytes only — key records add
    // nothing, so 64 keys plus one 1MiB bytes record is the exact ceiling.
    let mut records = vec![
        crate::SendRecord::Key {
            key: crate::SendKey::Enter,
        };
        crate::terminal_send::MAXIMUM_SEND_RECORDS - 1
    ];
    records.push(crate::SendRecord::Bytes {
        bytes: crate::SendBytes::new(vec![0; crate::MAXIMUM_SEND_AGGREGATE_BYTES])?,
    });
    assert!(crate::SendRecords::new(records).is_ok());
    let mut over = vec![
        crate::SendRecord::Key {
            key: crate::SendKey::Enter,
        };
        crate::terminal_send::MAXIMUM_SEND_RECORDS - 1
    ];
    over.push(crate::SendRecord::Bytes {
        bytes: crate::SendBytes::new(vec![0; crate::MAXIMUM_SEND_BYTES_BYTES])?,
    });
    over.insert(
        0,
        crate::SendRecord::Bytes {
            bytes: crate::SendBytes::new(vec![0; 1])?,
        },
    );
    assert!(crate::SendRecords::new(over).is_err());
    Ok(())
}

#[test]
fn send_text_bound_counts_the_folded_return() -> Result<(), Box<dyn std::error::Error>> {
    let maximum = "x".repeat(crate::MAXIMUM_SEND_TEXT_BYTES);
    assert!(crate::SendText::new(maximum.clone()).is_ok());
    assert!(crate::SendTextInput::new(crate::SendText::new(maximum.clone())?, false).is_ok());
    assert!(crate::SendTextInput::new(crate::SendText::new(maximum)?, true).is_err());
    assert!(
        crate::SendTextInput::new(
            crate::SendText::new("x".repeat(crate::MAXIMUM_SEND_TEXT_BYTES - 1))?,
            true
        )
        .is_ok()
    );

    let request = CapabilityNodeMessage::TerminalSendRequest(send_request(one_record(
        text_record(&"x".repeat(crate::MAXIMUM_SEND_TEXT_BYTES - 1), true),
    )));
    let encoded = encode_message(&request)?;
    assert_eq!(
        encoded.len(),
        12 + 105 + 1 + 2 + (crate::MAXIMUM_SEND_TEXT_BYTES - 1) + 1
    );
    assert_eq!(decode_message(&encoded)?, request);

    // A wire text field declaring the full text bound plus the folded return
    // is one byte over the combined bound and must be rejected at decode.
    let mut overlong = encoded.clone();
    let record_at = 12 + 105;
    overlong[record_at + 1..record_at + 3]
        .copy_from_slice(&u16::try_from(crate::MAXIMUM_SEND_TEXT_BYTES)?.to_be_bytes());
    overlong.insert(record_at + 3 + crate::MAXIMUM_SEND_TEXT_BYTES - 1, b'x');
    let body_len = u32::try_from(overlong.len() - 12)?;
    overlong[8..12].copy_from_slice(&body_len.to_be_bytes());
    assert_eq!(
        decode_message(&overlong),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}

#[test]
fn send_combo_records_validate_the_closed_vocabulary() -> Result<(), Box<dyn std::error::Error>> {
    // A modifier-only combination is not a record.
    assert!(crate::SendModifiers::new(false, false, false).is_err());
    // `ctrl-c`/`ctrl-d` are already modified keys, never combo bases.
    assert!(crate::SendComboBase::new_named(crate::SendKey::CtrlC).is_err());
    assert!(crate::SendComboBase::new_named(crate::SendKey::CtrlD).is_err());
    assert!(crate::SendComboBase::new_named(crate::SendKey::Enter).is_ok());
    // Function bases are exactly F1..F12.
    assert!(crate::SendComboBase::new_function(0).is_err());
    assert!(crate::SendComboBase::new_function(12).is_ok());
    assert!(crate::SendComboBase::new_function(13).is_err());
    // Letter/digit bases are the physical ASCII identities.
    assert!(crate::SendComboBase::new_letter(b'a').is_ok());
    assert!(crate::SendComboBase::new_letter(b'A').is_err());
    assert!(crate::SendComboBase::new_digit(b'0').is_ok());
    assert!(crate::SendComboBase::new_digit(b'/').is_err());

    // Wire-level: reserved modifier bits, a modifier-only combo, a named
    // ctrl-c base, and nonzero reserved bytes on insert/space all refuse.
    let request = CapabilityNodeMessage::TerminalSendRequest(send_request(one_record(
        crate::SendRecord::Combo {
            modifiers: crate::SendModifiers::new(true, false, false)?,
            base: crate::SendComboBase::new_letter(b'x')?,
        },
    )));
    let encoded = encode_message(&request)?;
    let record_at = 12 + 105;
    assert_eq!(encoded[record_at], 3);
    assert_eq!(encoded[record_at + 1], 0b001);
    assert_eq!(encoded[record_at + 2], 5);
    assert_eq!(encoded[record_at + 3], b'x');

    let mut reserved_bits = encoded.clone();
    reserved_bits[record_at + 1] = 0b1000;
    assert_eq!(
        decode_message(&reserved_bits),
        Err(ProtocolError::InvalidPayload)
    );

    let mut modifier_only = encoded.clone();
    modifier_only[record_at + 1] = 0;
    assert_eq!(
        decode_message(&modifier_only),
        Err(ProtocolError::InvalidPayload)
    );

    let mut named_ctrl_c = encoded.clone();
    named_ctrl_c[record_at + 2] = 1;
    named_ctrl_c[record_at + 3] = 14;
    assert_eq!(
        decode_message(&named_ctrl_c),
        Err(ProtocolError::InvalidPayload)
    );

    let mut insert_reserved = encoded.clone();
    insert_reserved[record_at + 2] = 2;
    insert_reserved[record_at + 3] = 7;
    assert_eq!(
        decode_message(&insert_reserved),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}

#[test]
fn send_bytes_and_paste_bounds_hold_at_the_wire() -> Result<(), Box<dyn std::error::Error>> {
    assert!(crate::SendBytes::new(Vec::new()).is_err());
    assert!(crate::SendBytes::new(vec![0; crate::MAXIMUM_SEND_BYTES_BYTES]).is_ok());
    assert!(crate::SendBytes::new(vec![0; crate::MAXIMUM_SEND_BYTES_BYTES + 1]).is_err());
    assert!(crate::SendPasteContent::new(Vec::new()).is_err());
    assert!(crate::SendPasteContent::new(vec![0; crate::MAXIMUM_SEND_PASTE_BYTES]).is_ok());
    assert!(crate::SendPasteContent::new(vec![0; crate::MAXIMUM_SEND_PASTE_BYTES + 1]).is_err());

    // A maximum bytes record stays inside the declared record-body bound and
    // round-trips: this is the 1MiB frame-overhead proof.
    let request = CapabilityNodeMessage::TerminalSendRequest(send_request(one_record(
        crate::SendRecord::Bytes {
            bytes: crate::SendBytes::new(vec![0x5a; crate::MAXIMUM_SEND_BYTES_BYTES])?,
        },
    )));
    let encoded = encode_message(&request)?;
    assert_eq!(
        encoded.len(),
        12 + 105 + 5 + crate::MAXIMUM_SEND_BYTES_BYTES
    );
    assert_eq!(decode_message(&encoded)?, request);
    Ok(())
}

#[test]
fn send_result_golden_and_outcome_axes() -> Result<(), Box<dyn std::error::Error>> {
    let accepted = CapabilityNodeMessage::TerminalSendResult(crate::TerminalSendResult::new(
        OperationId::from_bytes([31; 16])?,
        crate::TerminalSendOutcome::Accepted {
            progress: crate::SendProgress::new(2, 2, 5)?,
            cleanup: crate::SendCleanupStatus::Confirmed,
        },
    )?);
    let mut expected = Vec::from(*b"OBCN");
    expected.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    expected.extend_from_slice(&29_u16.to_be_bytes());
    expected.push(14);
    expected.extend_from_slice(&[31; 16]);
    expected.push(3);
    expected.push(2);
    expected.push(2);
    expected.extend_from_slice(&5_u64.to_be_bytes());
    expected.push(2);
    assert_eq!(encode_message(&accepted)?, expected);
    assert_eq!(expected.len(), 8 + 29);
    assert_eq!(decode_message(&expected)?, accepted);

    // A key press admitted with zero encoded bytes is still confirmed.
    let zero_key_press = CapabilityNodeMessage::TerminalSendResult(crate::TerminalSendResult::new(
        OperationId::from_bytes([31; 16])?,
        crate::TerminalSendOutcome::Accepted {
            progress: crate::SendProgress::new(1, 1, 0)?,
            cleanup: crate::SendCleanupStatus::Unconfirmed,
        },
    )?);
    assert_eq!(
        decode_message(&encode_message(&zero_key_press)?)?,
        zero_key_press
    );

    for outcome in [
        crate::TerminalSendOutcome::Unavailable {
            reason: crate::SendUnavailable::HostUnavailable,
            progress: crate::SendProgress::none(),
            cleanup: crate::SendCleanupStatus::NotNeeded,
        },
        crate::TerminalSendOutcome::Unavailable {
            reason: crate::SendUnavailable::InputBudgetExhausted,
            progress: crate::SendProgress::stopped(3, 17),
            cleanup: crate::SendCleanupStatus::Confirmed,
        },
        crate::TerminalSendOutcome::Rejected {
            reason: crate::SendRejection::WrongWorker,
            progress: crate::SendProgress::none(),
            cleanup: crate::SendCleanupStatus::NotNeeded,
        },
        crate::TerminalSendOutcome::Rejected {
            reason: crate::SendRejection::HostInputBackpressure,
            progress: crate::SendProgress::stopped(1, 9),
            cleanup: crate::SendCleanupStatus::Unconfirmed,
        },
        crate::TerminalSendOutcome::OutcomeUnknown {
            progress: crate::SendProgress::stopped(2, 4),
            cleanup: crate::SendCleanupStatus::Unconfirmed,
        },
    ] {
        let message = CapabilityNodeMessage::TerminalSendResult(crate::TerminalSendResult::new(
            OperationId::from_bytes([31; 16])?,
            outcome,
        )?);
        assert_eq!(decode_message(&encode_message(&message)?)?, message);
    }
    Ok(())
}

#[test]
fn send_result_rejects_unknown_codes_and_impossible_pairs() -> Result<(), Box<dyn std::error::Error>>
{
    let result = CapabilityNodeMessage::TerminalSendResult(crate::TerminalSendResult::new(
        OperationId::from_bytes([31; 16])?,
        crate::TerminalSendOutcome::Rejected {
            reason: crate::SendRejection::HostInputInvalid,
            progress: crate::SendProgress::none(),
            cleanup: crate::SendCleanupStatus::Confirmed,
        },
    )?);
    let encoded = encode_message(&result)?;

    let mut unknown_input = encoded.clone();
    unknown_input[8 + 17] = 9;
    assert_eq!(
        decode_message(&unknown_input),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_reason = encoded.clone();
    unknown_reason[8 + 18] = 10;
    assert_eq!(
        decode_message(&unknown_reason),
        Err(ProtocolError::InvalidPayload)
    );

    let mut unknown_cleanup = encoded.clone();
    unknown_cleanup[8 + 29] = 4;
    assert_eq!(
        decode_message(&unknown_cleanup),
        Err(ProtocolError::InvalidPayload)
    );

    // `Accepted` with `not-needed` cleanup cannot arise from a real
    // Attach → records → Detach sequence and is rejected on the wire.
    let mut accepted_no_attachment = Vec::from(*b"OBCN");
    accepted_no_attachment.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    accepted_no_attachment.extend_from_slice(&28_u16.to_be_bytes());
    accepted_no_attachment.push(14);
    accepted_no_attachment.extend_from_slice(&[31; 16]);
    accepted_no_attachment.push(3);
    accepted_no_attachment.push(1);
    accepted_no_attachment.push(1);
    accepted_no_attachment.extend_from_slice(&0_u64.to_be_bytes());
    accepted_no_attachment.push(1);
    assert_eq!(
        decode_message(&accepted_no_attachment),
        Err(ProtocolError::InvalidPayload)
    );

    // `OutcomeUnknown` with confirmed cleanup is impossible.
    let mut unknown_confirmed = encoded.clone();
    unknown_confirmed[8 + 17] = 4;
    unknown_confirmed[8 + 18] = 0;
    unknown_confirmed[8 + 19] = 0;
    unknown_confirmed.truncate(8 + 20 + 8 + 1);
    unknown_confirmed[8 + 20..8 + 28].copy_from_slice(&0_u64.to_be_bytes());
    unknown_confirmed[8 + 28] = 2;
    let len = u16::try_from(unknown_confirmed.len() - 8)?;
    unknown_confirmed[6..8].copy_from_slice(&len.to_be_bytes());
    assert_eq!(
        decode_message(&unknown_confirmed),
        Err(ProtocolError::InvalidPayload)
    );

    // A progress whose confirmed prefix exceeds the stopped index is
    // contradictory and rejected on the wire.
    let mut inverted = encoded.clone();
    inverted[8 + 19] = 3;
    inverted[8 + 20] = 1;
    assert_eq!(
        decode_message(&inverted),
        Err(ProtocolError::InvalidPayload)
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    let trailing_length = u16::try_from(trailing.len() - 8)?;
    trailing[6..8].copy_from_slice(&trailing_length.to_be_bytes());
    assert_eq!(
        decode_message(&trailing),
        Err(ProtocolError::InvalidPayload)
    );

    // The summed encoded byte bound rejects an impossible total.
    let mut oversized_total = Vec::from(*b"OBCN");
    oversized_total.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    oversized_total.extend_from_slice(&28_u16.to_be_bytes());
    oversized_total.push(14);
    oversized_total.extend_from_slice(&[31; 16]);
    oversized_total.push(3);
    oversized_total.push(1);
    oversized_total.push(1);
    oversized_total
        .extend_from_slice(&(crate::MAXIMUM_SEND_ACCEPTED_ENCODED_BYTES + 1).to_be_bytes());
    oversized_total.push(2);
    assert_eq!(
        decode_message(&oversized_total),
        Err(ProtocolError::InvalidPayload)
    );

    assert!(
        crate::TerminalSendResult::new(
            OperationId::from_bytes([31; 16])?,
            crate::TerminalSendOutcome::Accepted {
                progress: crate::SendProgress::new(1, 1, 0)?,
                cleanup: crate::SendCleanupStatus::NotNeeded,
            },
        )
        .is_err()
    );
    assert!(
        crate::TerminalSendResult::new(
            OperationId::from_bytes([31; 16])?,
            crate::TerminalSendOutcome::OutcomeUnknown {
                progress: crate::SendProgress::none(),
                cleanup: crate::SendCleanupStatus::Confirmed,
            },
        )
        .is_err()
    );
    assert!(crate::SendProgress::new(2, 1, 0).is_err());
    Ok(())
}

#[test]
fn launch_environment_extension_roundtrips_and_rejects_hostile_tails()
-> Result<(), Box<dyn std::error::Error>> {
    let base = AuthorizedTerminalLaunch::new(
        authorization(TERMINAL_LAUNCH_CAPABILITY_VERSION)?,
        None,
        TerminalLaunchMode::Deferred,
        None,
    );
    let legacy = encode(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
        base.clone(),
    ))?;
    let environment = crate::LaunchEnvironment::new(vec![
        ("KEY".into(), "literal=值".into()),
        ("EMPTY".into(), String::new()),
    ])?;
    for selection in [
        None,
        Some(LaunchSelection::new(
            "C:\\agent.exe".into(),
            vec!["--arg".into()],
        )?),
    ] {
        let launch =
            AuthorizedTerminalLaunch::new(base.authorization(), None, base.mode(), selection);
        let old = encode(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
            launch.clone(),
        ))?;
        assert_eq!(
            encode(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
                launch
                    .clone()
                    .with_environment(crate::LaunchEnvironment::empty())
            ))?,
            old
        );
        let message = CapabilityNodeMessage::AuthorizedTerminalLaunch(
            launch.with_environment(environment.clone()),
        );
        let encoded = encode(&message)?;
        assert_eq!(decode_message(&encoded)?, message);
        for cut in (legacy.len() + 1)..encoded.len() {
            let mut truncated = encoded[..cut].to_vec();
            truncated[6..8].copy_from_slice(&u16::try_from(cut - 8)?.to_be_bytes());
            assert!(decode_message(&truncated).is_err(), "cut {cut}");
        }
        let mut trailing = encoded;
        append_payload_bytes(&mut trailing, &[0])?;
        assert!(decode_message(&trailing).is_err());
    }
    for tail in [
        vec![0, 0, 2, 0, 0, 1],                                     // unknown version
        vec![0, 0, 1, 2, 0, 1],                                     // unknown selection presence
        vec![0, 0, 1, 0, 0, 0],                                     // noncanonical empty extension
        vec![0, 0, 1, 0, 0, 33], // entry budget before allocation
        vec![0, 0, 1, 0, 0, 1, 0, 4, b'T', b'E', b'R', b'M', 0, 0], // reserved name
        vec![0, 0, 1, 0, 0, 1, 0, 1, b'A', 4, 0], // aggregate byte budget
        vec![0, 0, 1, 0, 0, 2, 0, 1, b'A', 0, 0, 0, 1, b'a', 0, 0], // duplicate case
    ] {
        let mut bad = legacy.clone();
        append_payload_bytes(&mut bad, &tail)?;
        assert!(decode_message(&bad).is_err(), "accepted {tail:?}");
    }
    Ok(())
}

#[test]
fn transient_clear_control_codec_keeps_old_tags_and_closes_new_result_boundaries() {
    use crate::{
        TerminalControlAction as Action, TerminalControlOutcome as Outcome, TerminalControlRequest,
        TerminalControlResult,
    };
    let request = TerminalControlRequest {
        request_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        viewer_id: OperationId::from_bytes([3; 16]).unwrap(),
        controller_id: OperationId::from_bytes([4; 16]).unwrap(),
        action: Action::Clear,
    };
    let message = CapabilityNodeMessage::TerminalControlRequest(request.clone());
    let bytes = encode_message(&message).unwrap();
    assert_eq!(*bytes.last().unwrap(), 6);
    assert_eq!(decode_message(&bytes).unwrap(), message);
    for outcome in [
        Outcome::Cleared {
            generation: NonZeroU64::MAX,
            revision: NonZeroU64::MIN,
            cleanup_confirmed: true,
        },
        Outcome::Cleared {
            generation: NonZeroU64::MIN,
            revision: NonZeroU64::MAX,
            cleanup_confirmed: false,
        },
        Outcome::ClearOutcomeUnknown,
        Outcome::Rejected(crate::ControlRejection::HostClearAlternateScreen),
    ] {
        let result = CapabilityNodeMessage::TerminalControlResult(TerminalControlResult {
            request_id: request.request_id,
            viewer_id: request.viewer_id,
            controller_id: request.controller_id,
            outcome,
        });
        let mut encoded = encode_message(&result).unwrap();
        assert_eq!(decode_message(&encoded).unwrap(), result);
        if matches!(outcome, Outcome::Cleared { .. }) {
            *encoded.last_mut().unwrap() = 2;
            assert!(decode_message(&encoded).is_err());
        }
    }
}
