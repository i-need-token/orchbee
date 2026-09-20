use std::io::{self, Cursor, Read, Write};

use crate::{
    CreateAcceptedState, CreateOutcome, Geometry, PanelId, PresentationId, RequestId,
    ServerMessage, SiteId, StartMode, TabId, TerminalContextId, TerminalCreateRequest,
    TerminalCreateResult, TerminalId, TerminalLaunchBoundTo, TerminalLaunchFact,
    TerminalLaunchReadOutcome, TerminalLaunchReadResult, TerminalLaunchRoute, TerminalLaunchState,
    WorkerId, WorkingDirectory,
};
use std::num::NonZeroU64;

use super::{
    FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION, MAXIMUM_FRAME_PAYLOAD_BYTES, ProtocolError,
    ReadMessageError, decode_client_frame, decode_server_frame, encode_client_frame,
    encode_server_frame, read_client_message, read_server_message, write_client_message,
    write_server_message,
};
use crate::ClientMessage;

#[test]
fn frame_header_uses_exact_magic_version_and_big_endian_length()
-> Result<(), Box<dyn std::error::Error>> {
    let message = deferred_create()?;
    let frame = encode_client_frame(&message)?;
    assert_eq!(frame.get(..4), Some(FRAME_MAGIC.as_slice()));
    assert_eq!(
        frame.get(4..6),
        Some(FRAME_VERSION.to_be_bytes().as_slice())
    );
    let declared = u16::from_be_bytes([frame[6], frame[7]]);
    assert_eq!(usize::from(declared), frame.len() - FRAME_HEADER_BYTES);
    assert_eq!(decode_client_frame(&frame)?, message);
    Ok(())
}

#[test]
fn rejects_invalid_headers_and_exact_frame_length_mismatches_before_payload_decode()
-> Result<(), Box<dyn std::error::Error>> {
    let valid = encode_client_frame(&deferred_create()?)?;

    let mut invalid_magic = valid.clone();
    invalid_magic[0] = b'X';
    assert_eq!(
        decode_client_frame(&invalid_magic),
        Err(ProtocolError::InvalidMagic)
    );

    let mut invalid_version = valid.clone();
    invalid_version[4..6].copy_from_slice(&(FRAME_VERSION + 1).to_be_bytes());
    assert_eq!(
        decode_client_frame(&invalid_version),
        Err(ProtocolError::IncompatibleFrameVersion {
            received: FRAME_VERSION + 1,
        })
    );

    let oversized_length = u16::try_from(MAXIMUM_FRAME_PAYLOAD_BYTES + 1)?;
    let mut oversized = Vec::from(FRAME_MAGIC);
    oversized.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    oversized.extend_from_slice(&oversized_length.to_be_bytes());
    assert_eq!(
        decode_client_frame(&oversized),
        Err(ProtocolError::PayloadTooLarge {
            actual: MAXIMUM_FRAME_PAYLOAD_BYTES + 1,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        })
    );

    assert_eq!(
        decode_client_frame(&valid[..FRAME_HEADER_BYTES - 1]),
        Err(ProtocolError::FrameLengthMismatch {
            expected: FRAME_HEADER_BYTES,
            actual: FRAME_HEADER_BYTES - 1,
        })
    );

    let mut truncated = valid.clone();
    truncated.pop();
    assert!(matches!(
        decode_client_frame(&truncated),
        Err(ProtocolError::FrameLengthMismatch { expected, actual })
            if expected == valid.len() && actual == valid.len() - 1
    ));

    let mut extra = valid.clone();
    extra.push(0);
    assert!(matches!(
        decode_client_frame(&extra),
        Err(ProtocolError::FrameLengthMismatch { expected, actual })
            if expected == valid.len() && actual == valid.len() + 1
    ));
    Ok(())
}

#[test]
fn stream_reader_reports_truncation_and_rejects_oversized_header_without_reading_payload()
-> Result<(), Box<dyn std::error::Error>> {
    let valid = encode_client_frame(&deferred_create()?)?;
    for truncated in [
        valid[..FRAME_HEADER_BYTES - 1].to_vec(),
        valid[..valid.len() - 1].to_vec(),
    ] {
        assert!(matches!(
            read_client_message(&mut Cursor::new(truncated)),
            Err(ReadMessageError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof
        ));
    }

    let oversized_length = u16::try_from(MAXIMUM_FRAME_PAYLOAD_BYTES + 1)?;
    let mut header = Vec::from(FRAME_MAGIC);
    header.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    header.extend_from_slice(&oversized_length.to_be_bytes());
    assert!(matches!(
        read_client_message(&mut Cursor::new(header)),
        Err(ReadMessageError::Protocol(ProtocolError::PayloadTooLarge {
            actual,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        })) if actual == MAXIMUM_FRAME_PAYLOAD_BYTES + 1
    ));
    Ok(())
}

#[test]
fn rejects_utf8_bom_invalid_utf8_duplicate_keys_trailing_values_and_non_objects()
-> Result<(), Box<dyn std::error::Error>> {
    let canonical = payload(&encode_client_frame(&deferred_create()?)?)?.to_vec();
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend_from_slice(&canonical);
    assert_eq!(
        decode_client_frame(&frame(&bom)?),
        Err(ProtocolError::Utf8Bom)
    );
    assert_eq!(
        decode_client_frame(&frame(&[0xff])?),
        Err(ProtocolError::InvalidUtf8)
    );

    for invalid in [
        b"[]".as_slice(),
        b"null".as_slice(),
        b"{\"kind\":\"terminal.create\",\"kind\":\"terminal.create\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"workerConstraint\":null,\"cwd\":null,\"startMode\":{\"kind\":\"deferred\"}}".as_slice(),
        b"{\"kind\":\"terminal.create\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"workerConstraint\":null,\"cwd\":null,\"startMode\":{\"kind\":\"deferred\",\"kind\":\"deferred\"}}".as_slice(),
        b"{\"kind\":\"terminal.create\",\"protocolVersion\":1,\"requestId\":\"11111111111111111111111111111111\",\"workerConstraint\":null,\"cwd\":null,\"startMode\":{\"kind\":\"deferred\"}} {}".as_slice(),
    ] {
        assert_eq!(
            decode_client_frame(&frame(invalid)?),
            Err(ProtocolError::InvalidPayload)
        );
    }
    Ok(())
}

#[test]
fn direction_specific_decoders_reject_messages_from_the_other_direction()
-> Result<(), Box<dyn std::error::Error>> {
    let client_frame = encode_client_frame(&deferred_create()?)?;
    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    let server_message = ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
        request_id,
        CreateOutcome::OutcomeUnknown,
    ));
    let server_frame = encode_server_frame(&server_message)?;

    assert_eq!(
        decode_server_frame(&client_frame),
        Err(ProtocolError::InvalidPayload)
    );
    assert_eq!(
        decode_client_frame(&server_frame),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}

#[test]
fn supports_partial_io_multiple_frames_and_never_flushes_the_caller()
-> Result<(), Box<dyn std::error::Error>> {
    let first = deferred_create()?;
    let second = immediate_create_with_worst_case_json_cwd()?;
    let mut encoded = encode_client_frame(&first)?;
    encoded.extend_from_slice(&encode_client_frame(&second)?);
    let mut reader = ChunkedReader::new(encoded, 1);
    assert_eq!(read_client_message(&mut reader)?, first);
    assert_eq!(read_client_message(&mut reader)?, second);

    let mut client_writer = ChunkedWriter::new(1);
    write_client_message(&mut client_writer, &first)?;
    assert_eq!(client_writer.bytes, encode_client_frame(&first)?);
    assert_eq!(client_writer.flush_count, 0);

    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    let server_message = ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
        request_id,
        CreateOutcome::OutcomeUnknown,
    ));
    let mut server_writer = ChunkedWriter::new(2);
    write_server_message(&mut server_writer, &server_message)?;
    assert_eq!(server_writer.bytes, encode_server_frame(&server_message)?);
    assert_eq!(server_writer.flush_count, 0);
    assert_eq!(
        read_server_message(&mut ChunkedReader::new(server_writer.bytes, 1))?,
        server_message
    );
    Ok(())
}

#[test]
fn worst_case_json_escaping_stays_inside_the_frame_bound() -> Result<(), Box<dyn std::error::Error>>
{
    let message = immediate_create_with_worst_case_json_cwd()?;
    let encoded = encode_client_frame(&message)?;
    assert!(encoded.len() - FRAME_HEADER_BYTES <= MAXIMUM_FRAME_PAYLOAD_BYTES);
    assert!(encoded.len() - FRAME_HEADER_BYTES > 24 * 1024);
    assert_eq!(decode_client_frame(&encoded)?, message);
    Ok(())
}

#[test]
fn maximum_launch_read_result_stays_inside_the_frame_bound()
-> Result<(), Box<dyn std::error::Error>> {
    // The largest legal shape: every 32-hex identity present, u64::MAX for the
    // decimal-string revision and generation, and the `started` fact that
    // carries both a create outcome and a geometry.
    let message = ServerMessage::TerminalLaunchReadResult(TerminalLaunchReadResult::new(
        RequestId::parse("ffffffffffffffffffffffffffffffff")?,
        TerminalLaunchReadOutcome::Found {
            site_id: SiteId::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")?,
            workbench_revision: u64::MAX,
            terminal: Box::new(TerminalLaunchRoute::new(
                TerminalId::parse("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")?,
                WorkerId::parse("cccccccccccccccccccccccccccccccc")?,
                TerminalLaunchBoundTo::new(
                    PresentationId::parse("dddddddddddddddddddddddddddddddd")?,
                    PanelId::parse("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")?,
                    TabId::parse("11111111111111111111111111111111")?,
                    TerminalContextId::parse("22222222222222222222222222222222")?,
                ),
            )),
            launch: Box::new(TerminalLaunchFact::new(
                TerminalLaunchState::Started,
                NonZeroU64::new(u64::MAX),
                Some(CreateOutcome::Accepted {
                    state: CreateAcceptedState::Started {
                        geometry: Geometry::new(255, 255, 257, 257)?,
                    },
                }),
                Some(Geometry::new(255, 255, 257, 257)?),
            )?),
        },
    ));
    let encoded = encode_server_frame(&message)?;
    assert!(encoded.len() - FRAME_HEADER_BYTES <= MAXIMUM_FRAME_PAYLOAD_BYTES);
    assert_eq!(decode_server_frame(&encoded)?, message);
    Ok(())
}

#[test]
fn accepts_the_maximum_declared_length_before_rejecting_invalid_json()
-> Result<(), Box<dyn std::error::Error>> {
    let payload = vec![b' '; MAXIMUM_FRAME_PAYLOAD_BYTES];
    assert_eq!(
        decode_client_frame(&frame(&payload)?),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}

fn deferred_create() -> Result<ClientMessage, Box<dyn std::error::Error>> {
    let request_id = RequestId::parse("11111111111111111111111111111111")?;
    Ok(ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        request_id,
        None,
        None,
        StartMode::Deferred,
    )))
}

fn immediate_create_with_worst_case_json_cwd() -> Result<ClientMessage, Box<dyn std::error::Error>>
{
    let request_id = RequestId::parse("22222222222222222222222222222222")?;
    let cwd = WorkingDirectory::parse(&format!("/{}", "\u{1}".repeat(4095)))?;
    let geometry = Geometry::new(80, 24, 9, 18)?;
    Ok(ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        request_id,
        None,
        Some(cwd),
        StartMode::Immediate { geometry },
    )))
}

fn frame(payload: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let payload_length = u16::try_from(payload.len())?;
    let mut encoded = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    encoded.extend_from_slice(&FRAME_MAGIC);
    encoded.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    encoded.extend_from_slice(&payload_length.to_be_bytes());
    encoded.extend_from_slice(payload);
    Ok(encoded)
}

fn payload(frame: &[u8]) -> Result<&[u8], Box<dyn std::error::Error>> {
    frame
        .get(FRAME_HEADER_BYTES..)
        .ok_or_else(|| "frame has no payload boundary".into())
}

struct ChunkedReader {
    cursor: Cursor<Vec<u8>>,
    maximum_chunk: usize,
}

impl ChunkedReader {
    fn new(bytes: Vec<u8>, maximum_chunk: usize) -> Self {
        Self {
            cursor: Cursor::new(bytes),
            maximum_chunk,
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let chunk_length = buffer.len().min(self.maximum_chunk);
        self.cursor.read(&mut buffer[..chunk_length])
    }
}

struct ChunkedWriter {
    bytes: Vec<u8>,
    flush_count: usize,
    maximum_chunk: usize,
}

impl ChunkedWriter {
    const fn new(maximum_chunk: usize) -> Self {
        Self {
            bytes: Vec::new(),
            flush_count: 0,
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
        self.flush_count += 1;
        Ok(())
    }
}

#[test]
fn paste_request_encodes_as_obcq_record_and_round_trips_opaque_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{
        ControllerId, PasteContent, TerminalControlAction, TerminalControlRequest, ViewerId,
    };
    use std::num::NonZeroU64;

    let mut content: Vec<u8> = (0..8200_u32).map(|index| (index % 253) as u8).collect();
    content.extend_from_slice(&[0x00, 0x02, 0x1b, 0x80, 0xff]);
    let message = ClientMessage::TerminalControl(TerminalControlRequest::new(
        RequestId::parse("11111111111111111111111111111111")?,
        TerminalControlAction::Paste {
            attachment_id: ViewerId::parse("33333333333333333333333333333333")?,
            controller_id: ControllerId::parse("22222222222222222222222222222222")?,
            sequence: NonZeroU64::new(9).ok_or(ProtocolError::InvalidPayload)?,
            content: PasteContent::new(content)?,
        },
    ));

    let record = encode_client_frame(&message)?;
    assert_eq!(record.get(..8), Some(b"OBCQ\0\x01\0\0".as_slice()));
    assert_eq!(record.len(), 12 + 56 + 8200 + 5);
    assert_eq!(decode_client_frame(&record)?, message);

    let mut reader = ChunkedReader::new(record.clone(), 5);
    assert_eq!(read_client_message(&mut reader)?, message);

    let mut adjacent = record;
    adjacent.extend_from_slice(&encode_client_frame(&deferred_create()?)?);
    let mut stream = ChunkedReader::new(adjacent, 3);
    assert_eq!(read_client_message(&mut stream)?, message);
    assert_eq!(read_client_message(&mut stream)?, deferred_create()?);
    Ok(())
}

#[test]
fn obcq_record_rejects_malformed_headers_bodies_and_truncation()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{
        ControllerId, PasteContent, TerminalControlAction, TerminalControlRequest, ViewerId,
    };
    use std::num::NonZeroU64;

    let message = ClientMessage::TerminalControl(TerminalControlRequest::new(
        RequestId::parse("11111111111111111111111111111111")?,
        TerminalControlAction::Paste {
            attachment_id: ViewerId::parse("33333333333333333333333333333333")?,
            controller_id: ControllerId::parse("22222222222222222222222222222222")?,
            sequence: NonZeroU64::new(9).ok_or(ProtocolError::InvalidPayload)?,
            content: PasteContent::new(b"raw".to_vec())?,
        },
    ));
    let record = encode_client_frame(&message)?;

    let mut wrong_version = record.clone();
    wrong_version[4..6].copy_from_slice(&2_u16.to_be_bytes());
    assert!(matches!(
        decode_client_frame(&wrong_version),
        Err(ProtocolError::IncompatibleFrameVersion { received: 2 })
    ));

    let mut reserved = record.clone();
    reserved[7] = 1;
    assert_eq!(
        decode_client_frame(&reserved),
        Err(ProtocolError::InvalidPayload)
    );

    let mut wrong_direction = record.clone();
    wrong_direction[3] = b'R';
    assert_eq!(
        decode_client_frame(&wrong_direction),
        Err(ProtocolError::InvalidMagic)
    );

    for body_length in [0_u32, 55, 56] {
        let mut undersized = record.clone();
        undersized[8..12].copy_from_slice(&body_length.to_be_bytes());
        assert_eq!(
            decode_client_frame(&undersized),
            Err(ProtocolError::InvalidPayload)
        );
    }

    let mut oversized = record.clone();
    oversized[8..12].copy_from_slice(&262_201_u32.to_be_bytes());
    assert!(matches!(
        decode_client_frame(&oversized),
        Err(ProtocolError::PayloadTooLarge {
            maximum: 262_200,
            ..
        })
    ));
    assert!(matches!(
        read_client_message(&mut Cursor::new(oversized)),
        Err(ReadMessageError::Protocol(ProtocolError::PayloadTooLarge {
            maximum: 262_200,
            ..
        }))
    ));

    let mut trailing = record.clone();
    trailing.push(0);
    assert_eq!(
        decode_client_frame(&trailing),
        Err(ProtocolError::InvalidPayload)
    );

    assert!(matches!(
        read_client_message(&mut Cursor::new(&record[..record.len() - 1])),
        Err(ReadMessageError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));

    let mut zero_sequence = record.clone();
    zero_sequence[12 + 48..12 + 56].copy_from_slice(&0_u64.to_be_bytes());
    assert_eq!(
        decode_client_frame(&zero_sequence),
        Err(ProtocolError::InvalidPayload)
    );

    let mut bad_viewer = record.clone();
    bad_viewer[12 + 16..12 + 32].fill(0);
    assert_eq!(
        decode_client_frame(&bad_viewer),
        Err(ProtocolError::InvalidPayload)
    );
    Ok(())
}
