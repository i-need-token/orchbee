use std::io::{self, Cursor, Read, Write};

use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateRejectedReason, FRAME_HEADER_BYTES, FRAME_MAGIC,
    FRAME_VERSION, Geometry, MAXIMUM_FRAME_PAYLOAD_BYTES, ProtocolError, ReadMessageError,
    RequestId, ServerMessage, StartMode, StartOutcome, StartRejectedReason, TerminalCreateRequest,
    TerminalCreateResult, TerminalStartRequest, TerminalStartResult, WorkerConstraint, WorkerId,
    WorkingDirectory, WriteMessageError, decode_server_frame, encode_client_frame,
    encode_server_frame,
};

use super::ApplicationSessionError;

mod agent;
mod workbench;

fn run<Stream: Read + Write>(stream: &mut Stream) -> Result<(), ApplicationSessionError> {
    super::run(
        stream,
        |request| denied_result(&request),
        |_request| panic!("unexpected Workbench request"),
        |_request| panic!("unexpected Agent request"),
    )
}

const FIRST_REQUEST_ID: &str = "11111111111111111111111111111111";
const SECOND_REQUEST_ID: &str = "22222222222222222222222222222222";
const WORKER_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn stop_admission_closes_before_effect_and_preserves_the_previous_complete_result() {
    let request = immediate_create().unwrap();
    let frame = encode_client_frame(&request).unwrap();
    let mut input = frame.clone();
    input.extend_from_slice(&frame);
    let mut stream = MemoryStream::new(input);
    let mut admissions = 0;
    let mut effects = 0;
    let result = super::run_guarded(
        &mut stream,
        |request| {
            effects += 1;
            denied_result(&request)
        },
        |_| panic!("Workbench"),
        |_| panic!("Agent"),
        || {
            admissions += 1;
            (admissions == 1).then_some(())
        },
    );
    assert!(matches!(result, Err(ApplicationSessionError::Stopping)));
    assert_eq!(effects, 1);
    assert_eq!(
        decode_server_frame(&stream.output).unwrap(),
        denied_result(&request)
    );
}

#[test]
fn writes_injected_results_with_only_their_correlation_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let requests = [immediate_create()?, deferred_create()?, start()?];

    for request in requests {
        let expected = denied_result(&request);
        let mut stream = MemoryStream::new(encode_client_frame(&request)?);
        let error = run(&mut stream).expect_err("EOF must end the OBCS Session");
        assert!(matches!(
            error,
            ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error)
                if error.kind() == io::ErrorKind::UnexpectedEof
        ));
        assert_eq!(decode_server_frame(&stream.output)?, expected);
        assert_eq!(stream.flush_count, 0);
    }
    Ok(())
}

#[test]
fn writes_each_injected_result_before_reading_the_next_distinct_request()
-> Result<(), Box<dyn std::error::Error>> {
    let first = deferred_create()?;
    let second = immediate_create()?;
    let first_frame = encode_client_frame(&first)?;
    let second_frame = encode_client_frame(&second)?;
    let first_result = encode_server_frame(&denied_result(&first))?;
    let second_result = encode_server_frame(&denied_result(&second))?;
    let mut input = first_frame.clone();
    input.extend_from_slice(&second_frame);
    let mut stream = MemoryStream::new(input)
        .with_chunks(1, 2)
        .with_read_barrier(first_frame.len(), first_result.len());

    let error = run(&mut stream).expect_err("EOF must end the OBCS Session");
    assert!(matches!(
        error,
        ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error)
            if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    let mut expected = first_result;
    expected.extend_from_slice(&second_result);
    assert_eq!(stream.output, expected);
    assert_eq!(stream.flush_count, 0);
    Ok(())
}

#[test]
fn processes_duplicate_request_ids_again_without_a_ledger() -> Result<(), Box<dyn std::error::Error>>
{
    let first = deferred_create()?;
    let second = ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        None,
        None,
        StartMode::Immediate {
            geometry: Geometry::new(100, 30, 8, 16)?,
        },
        None,
    ));
    let mut input = encode_client_frame(&first)?;
    input.extend_from_slice(&encode_client_frame(&second)?);
    let mut stream = MemoryStream::new(input);

    let error = run(&mut stream).expect_err("EOF must end the OBCS Session");
    assert!(matches!(
        error,
        ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error)
            if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    let mut expected = encode_server_frame(&denied_result(&first))?;
    expected.extend_from_slice(&encode_server_frame(&denied_result(&second))?);
    assert_eq!(stream.output, expected);
    assert_eq!(stream.flush_count, 0);
    Ok(())
}

#[test]
fn ends_on_read_eof_truncation_malformed_direction_and_oversized_header()
-> Result<(), Box<dyn std::error::Error>> {
    assert_unexpected_eof(Vec::new());
    assert_unexpected_eof(FRAME_MAGIC[..3].to_vec());

    let mut partial_payload = encode_client_frame(&deferred_create()?)?;
    let removed = partial_payload.pop();
    assert!(removed.is_some());
    assert_unexpected_eof(partial_payload);

    let malformed = frame(b"{")?;
    assert!(matches!(
        run(&mut MemoryStream::new(malformed)),
        Err(ApplicationSessionError::Read(ReadMessageError::Protocol(
            ProtocolError::InvalidPayload
        )))
    ));

    let wrong_direction = encode_server_frame(&denied_result(&deferred_create()?))?;
    assert!(matches!(
        run(&mut MemoryStream::new(wrong_direction)),
        Err(ApplicationSessionError::Read(ReadMessageError::Protocol(
            ProtocolError::InvalidPayload
        )))
    ));

    let oversized_length = u16::try_from(MAXIMUM_FRAME_PAYLOAD_BYTES + 1)?;
    let mut oversized = Vec::from(FRAME_MAGIC);
    oversized.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    oversized.extend_from_slice(&oversized_length.to_be_bytes());
    assert!(matches!(
        run(&mut MemoryStream::new(oversized)),
        Err(ApplicationSessionError::Read(ReadMessageError::Protocol(
            ProtocolError::PayloadTooLarge { actual, maximum }
        ))) if actual == MAXIMUM_FRAME_PAYLOAD_BYTES + 1
            && maximum == MAXIMUM_FRAME_PAYLOAD_BYTES
    ));

    let mut read_failure = MemoryStream::new(encode_client_frame(&deferred_create()?)?)
        .with_read_failure(0, io::ErrorKind::ConnectionReset);
    assert!(matches!(
        run(&mut read_failure),
        Err(ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error))
            if error.kind() == io::ErrorKind::ConnectionReset
    ));
    assert!(read_failure.output.is_empty());
    Ok(())
}

#[test]
fn ends_on_a_deterministic_partial_result_write_failure() -> Result<(), Box<dyn std::error::Error>>
{
    let request = deferred_create()?;
    let result = encode_server_frame(&denied_result(&request))?;
    let failure_offset = result.len() / 2;
    let mut stream = MemoryStream::new(encode_client_frame(&request)?)
        .with_chunks(usize::MAX, 3)
        .with_write_failure(failure_offset, io::ErrorKind::BrokenPipe);

    assert!(matches!(
        run(&mut stream),
        Err(ApplicationSessionError::Write(WriteMessageError::Io(error)))
            if error.kind() == io::ErrorKind::BrokenPipe
    ));
    assert_eq!(stream.output, result[..failure_offset]);
    assert_eq!(stream.flush_count, 0);
    Ok(())
}

fn immediate_create() -> Result<ClientMessage, Box<dyn std::error::Error>> {
    Ok(ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        RequestId::parse(SECOND_REQUEST_ID)?,
        Some(WorkerConstraint::Exact {
            worker_id: WorkerId::parse(WORKER_ID)?,
        }),
        Some(WorkingDirectory::parse("/tmp/orchbee")?),
        StartMode::Immediate {
            geometry: Geometry::new(80, 24, 9, 18)?,
        },
        None,
    )))
}

fn deferred_create() -> Result<ClientMessage, Box<dyn std::error::Error>> {
    Ok(ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        None,
        None,
        StartMode::Deferred,
        None,
    )))
}

fn start() -> Result<ClientMessage, Box<dyn std::error::Error>> {
    Ok(ClientMessage::TerminalStart(TerminalStartRequest::new(
        RequestId::parse(SECOND_REQUEST_ID)?,
        RequestId::parse(FIRST_REQUEST_ID)?,
        Geometry::new(120, 40, 7, 14)?,
    )?))
}

#[test]
fn routes_paste_records_through_the_control_executor() -> Result<(), Box<dyn std::error::Error>> {
    use orchbee_client_server_protocol::{
        ControllerId, PasteAcceptedBytes, PasteContent, TerminalControlAction,
        TerminalControlOutcome, TerminalControlRequest, TerminalControlResult, ViewerId,
    };
    use std::num::NonZeroU64;

    let request = ClientMessage::TerminalControl(TerminalControlRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        TerminalControlAction::Paste {
            attachment_id: ViewerId::parse("33333333333333333333333333333333")?,
            controller_id: ControllerId::parse("44444444444444444444444444444444")?,
            sequence: NonZeroU64::MIN,
            content: PasteContent::new(vec![0, 0x1b, 0xff])?,
        },
    ));
    let result = ServerMessage::TerminalControlResult(TerminalControlResult::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        TerminalControlOutcome::Pasted {
            accepted_bytes: PasteAcceptedBytes::new(15)?,
        },
    ));
    let mut stream = MemoryStream::new(encode_client_frame(&request)?);
    let error = super::run(
        &mut stream,
        |message| {
            assert!(matches!(
                message,
                ClientMessage::TerminalControl(request)
                    if matches!(
                        request.action(),
                        TerminalControlAction::Paste { content, .. }
                            if content.as_bytes() == [0, 0x1b, 0xff]
                    )
            ));
            result.clone()
        },
        |_request| panic!("unexpected Workbench request"),
        |_request| panic!("unexpected Agent request"),
    )
    .expect_err("EOF must end the OBCS Session");
    assert!(matches!(
        error,
        ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error)
            if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    assert_eq!(stream.output, encode_server_frame(&result)?);
    Ok(())
}

#[test]
fn rejects_malformed_paste_records_without_reaching_the_executor()
-> Result<(), Box<dyn std::error::Error>> {
    use orchbee_client_server_protocol::{
        ControllerId, MAXIMUM_PASTE_CONTENT_BYTES, PasteContent, TerminalControlAction,
        TerminalControlRequest, ViewerId,
    };
    use std::num::NonZeroU64;

    let request = ClientMessage::TerminalControl(TerminalControlRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        TerminalControlAction::Paste {
            attachment_id: ViewerId::parse("33333333333333333333333333333333")?,
            controller_id: ControllerId::parse("44444444444444444444444444444444")?,
            sequence: NonZeroU64::MIN,
            content: PasteContent::new(b"content".to_vec())?,
        },
    ));
    let executor = |_message: ClientMessage| -> ServerMessage {
        unreachable!("malformed records must not execute")
    };
    let record = encode_client_frame(&request)?;
    assert!(record.starts_with(b"OBCQ"));

    for mutated in [
        {
            let mut record = record.clone();
            record[5] = 2;
            record
        },
        {
            let mut record = record.clone();
            record[7] = 1;
            record
        },
        record[..record.len() - 1].to_vec(),
    ] {
        let mut stream = MemoryStream::new(mutated);
        assert!(
            super::run(
                &mut stream,
                executor,
                |_request| { panic!("unexpected Workbench request") },
                |_request| { panic!("unexpected Agent request") }
            )
            .is_err()
        );
        assert!(stream.output.is_empty());
    }

    let mut oversized = Vec::from(&b"OBCQ\0\x01\0\0"[..]);
    oversized.extend_from_slice(&(MAXIMUM_PASTE_CONTENT_BYTES as u32 + 57).to_be_bytes());
    let mut stream = MemoryStream::new(oversized);
    assert!(matches!(
        super::run(
            &mut stream,
            executor,
            |_request| { panic!("unexpected Workbench request") },
            |_request| { panic!("unexpected Agent request") }
        ),
        Err(ApplicationSessionError::Read(ReadMessageError::Protocol(
            ProtocolError::PayloadTooLarge { .. }
        )))
    ));
    assert!(stream.output.is_empty());
    Ok(())
}

#[test]
fn routes_send_records_through_the_control_executor() -> Result<(), Box<dyn std::error::Error>> {
    use orchbee_client_server_protocol::{
        SendRecord, SendRecords, SendText, SendTextInput, SiteId, TerminalId, TerminalSendRequest,
    };

    let request = ClientMessage::TerminalSend(TerminalSendRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        SiteId::parse("33333333333333333333333333333333")?,
        WorkerId::parse(WORKER_ID)?,
        TerminalId::parse("55555555555555555555555555555555")?,
        SendRecords::new(vec![SendRecord::Text(SendTextInput::new(
            SendText::new("echo ok".to_owned())?,
            true,
        )?)])?,
    ));
    let record = encode_client_frame(&request)?;
    assert!(record.starts_with(b"OBSN"));
    let expected_request = request.clone();
    let result = denied_result(&request);

    let mut stream = MemoryStream::new(record).with_chunks(1, 2);
    let error = super::run(
        &mut stream,
        |message| {
            assert_eq!(message, expected_request);
            result.clone()
        },
        |_request| panic!("unexpected Workbench request"),
        |_request| panic!("unexpected Agent request"),
    )
    .expect_err("EOF must end the OBCS Session");
    assert!(matches!(
        error,
        ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error)
            if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    assert_eq!(stream.output, encode_server_frame(&result)?);
    assert_eq!(stream.flush_count, 0);
    Ok(())
}

#[test]
fn rejects_malformed_send_records_without_reaching_the_executor()
-> Result<(), Box<dyn std::error::Error>> {
    use orchbee_client_server_protocol::{
        SendRecord, SendRecords, SendText, SendTextInput, SiteId, TerminalId, TerminalSendRequest,
    };

    let request = ClientMessage::TerminalSend(TerminalSendRequest::new(
        RequestId::parse(FIRST_REQUEST_ID)?,
        SiteId::parse("33333333333333333333333333333333")?,
        WorkerId::parse(WORKER_ID)?,
        TerminalId::parse("55555555555555555555555555555555")?,
        SendRecords::new(vec![SendRecord::Text(SendTextInput::new(
            SendText::new("echo ok".to_owned())?,
            true,
        )?)])?,
    ));
    let executor = |_message: ClientMessage| -> ServerMessage {
        unreachable!("malformed records must not execute")
    };
    let record = encode_client_frame(&request)?;
    assert!(record.starts_with(b"OBSN"));

    for mutated in [
        {
            let mut record = record.clone();
            record[5] = 2;
            record
        },
        {
            let mut record = record.clone();
            record[7] = 1;
            record
        },
        record[..record.len() - 1].to_vec(),
    ] {
        let mut stream = MemoryStream::new(mutated);
        assert!(
            super::run(
                &mut stream,
                executor,
                |_request| { panic!("unexpected Workbench request") },
                |_request| { panic!("unexpected Agent request") }
            )
            .is_err()
        );
        assert!(stream.output.is_empty());
    }

    // A declared body length past the send-record bound fails before any
    // allocation — the same boundary the oversized-paste case exercises.
    let mut oversized = Vec::from(&b"OBSN\0\x01\0\0"[..]);
    oversized.extend_from_slice(&u32::MAX.to_be_bytes());
    let mut stream = MemoryStream::new(oversized);
    assert!(matches!(
        super::run(
            &mut stream,
            executor,
            |_request| { panic!("unexpected Workbench request") },
            |_request| { panic!("unexpected Agent request") }
        ),
        Err(ApplicationSessionError::Read(ReadMessageError::Protocol(
            ProtocolError::PayloadTooLarge { .. }
        )))
    ));
    assert!(stream.output.is_empty());
    Ok(())
}

fn denied_result(request: &ClientMessage) -> ServerMessage {
    match request {
        ClientMessage::TerminalControl(_) | ClientMessage::TerminalViewer(_) => {
            unreachable!("launch-only test fixture")
        }
        ClientMessage::TerminalCreate(request) => {
            ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Rejected {
                    reason: CreateRejectedReason::NotAuthorized,
                },
            ))
        }
        ClientMessage::TerminalStart(request) => {
            ServerMessage::TerminalStartResult(TerminalStartResult::new(
                request.request_id().clone(),
                StartOutcome::Rejected {
                    reason: StartRejectedReason::NotAuthorized,
                },
            ))
        }
        ClientMessage::TerminalSend(request) => ServerMessage::TerminalSendResult(
            match orchbee_client_server_protocol::TerminalSendResult::new(
                request.request_id().clone(),
                None,
                orchbee_client_server_protocol::TerminalSendOutcome::Rejected {
                    reason: orchbee_client_server_protocol::SendRejectedReason::TerminalUnavailable,
                    progress: orchbee_client_server_protocol::SendProgress::none(),
                    cleanup: orchbee_client_server_protocol::SendCleanupStatus::NotNeeded,
                },
            ) {
                Ok(result) => result,
                Err(_) => unreachable!("validated send outcomes are encodable"),
            },
        ),
        ClientMessage::TerminalLaunchRead(request) => ServerMessage::TerminalLaunchReadResult(
            orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalLaunchReadOutcome::NotFound,
            ),
        ),
    }
}

fn assert_unexpected_eof(input: Vec<u8>) {
    let mut stream = MemoryStream::new(input);
    assert!(matches!(
        run(&mut stream),
        Err(ApplicationSessionError::Read(ReadMessageError::Io(error)) | ApplicationSessionError::Prefix(error))
            if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    assert!(stream.output.is_empty());
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

struct MemoryStream {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    maximum_read_chunk: usize,
    maximum_write_chunk: usize,
    read_barrier: Option<(usize, usize)>,
    read_failure: Option<(usize, io::ErrorKind)>,
    write_failure: Option<(usize, io::ErrorKind)>,
    flush_count: usize,
}

impl MemoryStream {
    fn new(input: Vec<u8>) -> Self {
        Self {
            input: Cursor::new(input),
            output: Vec::new(),
            maximum_read_chunk: usize::MAX,
            maximum_write_chunk: usize::MAX,
            read_barrier: None,
            read_failure: None,
            write_failure: None,
            flush_count: 0,
        }
    }

    const fn with_chunks(mut self, maximum_read_chunk: usize, maximum_write_chunk: usize) -> Self {
        self.maximum_read_chunk = maximum_read_chunk;
        self.maximum_write_chunk = maximum_write_chunk;
        self
    }

    const fn with_read_barrier(
        mut self,
        input_offset: usize,
        required_output_bytes: usize,
    ) -> Self {
        self.read_barrier = Some((input_offset, required_output_bytes));
        self
    }

    const fn with_read_failure(mut self, offset: usize, kind: io::ErrorKind) -> Self {
        self.read_failure = Some((offset, kind));
        self
    }

    const fn with_write_failure(mut self, offset: usize, kind: io::ErrorKind) -> Self {
        self.write_failure = Some((offset, kind));
        self
    }
}

impl Read for MemoryStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let input_offset = usize::try_from(self.input.position())
            .map_err(|_error| io::Error::other("test input offset exceeded usize"))?;
        if let Some((failure_offset, kind)) = self.read_failure
            && input_offset >= failure_offset
        {
            return Err(io::Error::new(kind, "deterministic test read failure"));
        }

        let barrier_bytes = match self.read_barrier {
            Some((barrier_offset, required_output_bytes))
                if self.output.len() < required_output_bytes =>
            {
                if input_offset >= barrier_offset {
                    return Err(io::Error::other(
                        "second request was read before the first result was written",
                    ));
                }
                barrier_offset - input_offset
            }
            Some(_) | None => usize::MAX,
        };
        let chunk_length = buffer.len().min(self.maximum_read_chunk).min(barrier_bytes);
        self.input.read(&mut buffer[..chunk_length])
    }
}

impl Write for MemoryStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let failure_bytes = match self.write_failure {
            Some((failure_offset, kind)) => {
                if self.output.len() >= failure_offset {
                    return Err(io::Error::new(kind, "deterministic test write failure"));
                }
                failure_offset - self.output.len()
            }
            None => usize::MAX,
        };
        let chunk_length = buffer
            .len()
            .min(self.maximum_write_chunk)
            .min(failure_bytes);
        self.output.extend_from_slice(&buffer[..chunk_length]);
        Ok(chunk_length)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_count += 1;
        Ok(())
    }
}
