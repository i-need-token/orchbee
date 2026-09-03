use std::{io::Cursor, num::NonZeroU64};

use super::{
    FRAME_HEADER_BYTES, LocalSiteMessage, MAXIMUM_COHORT_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES,
    PROTOCOL_VERSION, ProtocolError, ReadMessageError, RejectionReason, WorkerHello, WorkerId,
    read_message, write_message,
};

const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);

#[test]
fn round_trips_every_admitted_message() -> Result<(), Box<dyn std::error::Error>> {
    let generation = NonZeroU64::new(3).ok_or("generation must be nonzero")?;
    let messages = [
        LocalSiteMessage::WorkerHello(WorkerHello::new("0.1.0", WORKER_ID)?),
        LocalSiteMessage::WorkerAccepted { generation },
        LocalSiteMessage::Rejected {
            reason: RejectionReason::IncompatibleProtocol,
        },
        LocalSiteMessage::Rejected {
            reason: RejectionReason::IncompatibleCohort,
        },
        LocalSiteMessage::Rejected {
            reason: RejectionReason::DuplicateWorker,
        },
        LocalSiteMessage::Rejected {
            reason: RejectionReason::WrongMessage,
        },
        LocalSiteMessage::Rejected {
            reason: RejectionReason::MalformedFrame,
        },
        LocalSiteMessage::Rejected {
            reason: RejectionReason::HandshakeTimeout,
        },
    ];

    for expected in messages {
        let mut encoded = Vec::new();
        write_message(&mut encoded, &expected)?;
        assert_eq!(read_message(&mut Cursor::new(encoded))?, expected);
    }
    Ok(())
}

#[test]
fn enforces_the_exact_cohort_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let maximum = "a".repeat(MAXIMUM_COHORT_BYTES);
    assert!(WorkerHello::new(&maximum, WORKER_ID).is_ok());
    assert_eq!(
        WorkerHello::new(&format!("{maximum}a"), WORKER_ID),
        Err(ProtocolError::InvalidCohort)
    );
    assert_eq!(
        WorkerHello::new("", WORKER_ID),
        Err(ProtocolError::InvalidCohort)
    );
    assert_eq!(
        WorkerHello::new("版本", WORKER_ID),
        Err(ProtocolError::InvalidCohort)
    );
    Ok(())
}

#[test]
fn rejects_invalid_headers_before_allocating_payload() {
    let mut invalid_magic = frame(PROTOCOL_VERSION, &[1]);
    invalid_magic[0] = b'X';
    assert!(matches!(
        read_message(&mut Cursor::new(invalid_magic)),
        Err(ReadMessageError::Protocol(ProtocolError::InvalidMagic))
    ));

    let incompatible = frame(PROTOCOL_VERSION - 1, &[1]);
    assert!(matches!(
        read_message(&mut Cursor::new(incompatible)),
        Err(ReadMessageError::Protocol(
            ProtocolError::IncompatibleVersion { received }
        )) if received == PROTOCOL_VERSION - 1
    ));

    let oversized_length = u16::try_from(MAXIMUM_FRAME_PAYLOAD_BYTES + 1)
        .unwrap_or(u16::MAX)
        .to_be_bytes();
    let mut oversized = frame(PROTOCOL_VERSION, &[]);
    oversized[6] = oversized_length[0];
    oversized[7] = oversized_length[1];
    assert!(matches!(
        read_message(&mut Cursor::new(oversized)),
        Err(ReadMessageError::Protocol(ProtocolError::PayloadTooLarge {
            actual,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES
        })) if actual == MAXIMUM_FRAME_PAYLOAD_BYTES + 1
    ));
}

#[test]
fn rejects_unknown_truncated_and_noncanonical_payloads() {
    for payload in [
        vec![],
        vec![99],
        vec![2],
        vec![2, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![3, 99],
        vec![3, 1, 0],
        vec![1, 1],
        vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff],
    ] {
        assert!(matches!(
            read_message(&mut Cursor::new(frame(PROTOCOL_VERSION, &payload))),
            Err(ReadMessageError::Protocol(
                ProtocolError::InvalidPayload | ProtocolError::InvalidCohort
            ))
        ));
    }

    let mut truncated = frame(PROTOCOL_VERSION, &[3, 1]);
    truncated.truncate(FRAME_HEADER_BYTES + 1);
    assert!(matches!(
        read_message(&mut Cursor::new(truncated)),
        Err(ReadMessageError::Io(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof
    ));
}

fn frame(version: u16, payload: &[u8]) -> Vec<u8> {
    let payload_length = u16::try_from(payload.len()).unwrap_or(u16::MAX);
    let mut encoded = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    encoded.extend_from_slice(b"OBLS");
    encoded.extend_from_slice(&version.to_be_bytes());
    encoded.extend_from_slice(&payload_length.to_be_bytes());
    encoded.extend_from_slice(payload);
    encoded
}
