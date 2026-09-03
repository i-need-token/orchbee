use std::{io::Cursor, num::NonZeroU64};

use super::{
    ClientHello, FRAME_HEADER_BYTES, LocalClientMessage, MAXIMUM_COHORT_BYTES,
    MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, ProtocolError, ReadMessageError,
    RejectionReason, read_message, write_message,
};

#[test]
fn round_trips_every_admitted_message() -> Result<(), Box<dyn std::error::Error>> {
    let generation = NonZeroU64::new(3).ok_or("generation must be nonzero")?;
    let messages = [
        LocalClientMessage::ClientHello(ClientHello::new("0.1.0")?),
        LocalClientMessage::ClientAccepted { generation },
        LocalClientMessage::Rejected {
            reason: RejectionReason::IncompatibleProtocol,
        },
        LocalClientMessage::Rejected {
            reason: RejectionReason::IncompatibleCohort,
        },
        LocalClientMessage::Rejected {
            reason: RejectionReason::CapacityExceeded,
        },
        LocalClientMessage::Rejected {
            reason: RejectionReason::WrongMessage,
        },
        LocalClientMessage::Rejected {
            reason: RejectionReason::MalformedFrame,
        },
        LocalClientMessage::Rejected {
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
fn enforces_the_exact_cohort_boundary() {
    let maximum = "a".repeat(MAXIMUM_COHORT_BYTES);
    assert!(ClientHello::new(&maximum).is_ok());
    assert_eq!(
        ClientHello::new(&format!("{maximum}a")),
        Err(ProtocolError::InvalidCohort)
    );
    assert_eq!(ClientHello::new(""), Err(ProtocolError::InvalidCohort));
    assert_eq!(ClientHello::new("版本"), Err(ProtocolError::InvalidCohort));
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
        vec![1],
        vec![1, 1],
        vec![1, 1, 0xff],
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
    encoded.extend_from_slice(b"OBLC");
    encoded.extend_from_slice(&version.to_be_bytes());
    encoded.extend_from_slice(&payload_length.to_be_bytes());
    encoded.extend_from_slice(payload);
    encoded
}
