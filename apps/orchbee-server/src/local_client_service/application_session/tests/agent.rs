//! OBAP demultiplexing coverage inside the shared sequential record loop:
//! the third `OBAP` arm routes to its own executor, preserves exact
//! response-before-next-request ordering against the OBCS family, ends
//! loudly on malformed records, and surfaces executor faults as session
//! errors — never silently skipping or fabricating.

use std::io::Cursor;

use orchbee_agent_observation_protocol::{
    AgentListOutcome, AgentListOutcomeEnvelope, AgentListRejectionReason, AgentNamesRequest,
    AgentReadRequest, AgentReadResult, AgentRequest, AgentResult, AgentSafeInteger, AgentWireError,
    ObapRequestId, read_agent_result, write_agent_request, write_agent_result,
};
use orchbee_client_server_protocol::{ClientMessage, ServerMessage, read_server_message};

use super::super::ApplicationSessionError;
use crate::agent_service::AgentReadFault;

fn list_request() -> AgentReadRequest {
    AgentReadRequest::List {
        request_id: ObapRequestId::parse(super::FIRST_REQUEST_ID).unwrap(),
        include_ended: false,
        limit: AgentSafeInteger::new(50),
        cursor: None,
    }
}

fn denied_list_result(request: &AgentReadRequest) -> AgentReadResult {
    AgentReadResult::List {
        request_id: request.request_id().clone(),
        result: AgentListOutcomeEnvelope {
            outcome: AgentListOutcome::Rejected {
                reason: AgentListRejectionReason::ReadDenied,
            },
        },
    }
}

fn run_agent<Stream: std::io::Read + std::io::Write>(
    stream: &mut Stream,
    result: AgentReadResult,
) -> Result<(), ApplicationSessionError> {
    super::super::run(
        stream,
        |_request| panic!("unexpected OBCS request"),
        |_request| panic!("unexpected Workbench request"),
        |request| {
            assert_eq!(request, AgentRequest::Read(list_request()));
            Ok(AgentResult::Read(result.clone()))
        },
    )
}

#[test]
fn routes_agent_records_through_the_agent_executor_and_writes_the_correlated_result() {
    let request = list_request();
    let result = denied_list_result(&request);
    let mut input = Vec::new();
    write_agent_request(&mut input, &AgentRequest::Read(request)).unwrap();
    let mut expected = Vec::new();
    write_agent_result(&mut expected, &AgentResult::Read(result.clone())).unwrap();

    let mut stream = super::MemoryStream::new(input).with_chunks(1, 2);
    let error = run_agent(&mut stream, result).expect_err("EOF must end the session");
    assert!(matches!(
        error,
        ApplicationSessionError::Prefix(error) if error.kind() == std::io::ErrorKind::UnexpectedEof
    ));
    assert_eq!(stream.output, expected);
    assert_eq!(stream.flush_count, 0);
}

#[test]
fn interleaves_agent_and_obcs_records_preserving_response_before_next_request() {
    let terminal = super::deferred_create().unwrap();
    let start = super::start().unwrap();
    let request = list_request();
    let result = denied_list_result(&request);

    let terminal_frame = orchbee_client_server_protocol::encode_client_frame(&terminal).unwrap();
    let mut agent_record = Vec::new();
    write_agent_request(&mut agent_record, &AgentRequest::Read(request.clone())).unwrap();
    let start_frame = orchbee_client_server_protocol::encode_client_frame(&start).unwrap();

    let terminal_result =
        orchbee_client_server_protocol::encode_server_frame(&super::denied_result(&terminal))
            .unwrap();
    let mut agent_result_bytes = Vec::new();
    write_agent_result(&mut agent_result_bytes, &AgentResult::Read(result.clone())).unwrap();

    let mut input = terminal_frame.clone();
    input.extend_from_slice(&agent_record);
    input.extend_from_slice(&start_frame);

    // The third record must not be read until both earlier responses were
    // written — the same sequencing guarantee the OBCS arms enjoy.
    let mut stream = super::MemoryStream::new(input)
        .with_chunks(1, 2)
        .with_read_barrier(
            terminal_frame.len() + agent_record.len(),
            terminal_result.len() + agent_result_bytes.len(),
        );

    let error = super::super::run(
        &mut stream,
        |request: ClientMessage| -> ServerMessage { super::denied_result(&request) },
        |_request| panic!("unexpected Workbench request"),
        |received| {
            assert_eq!(received, AgentRequest::Read(request.clone()));
            Ok(AgentResult::Read(result.clone()))
        },
    )
    .expect_err("EOF must end the session");
    assert!(matches!(
        error,
        ApplicationSessionError::Prefix(error) if error.kind() == std::io::ErrorKind::UnexpectedEof
    ));

    let mut output = Cursor::new(stream.output);
    assert_eq!(
        read_server_message(&mut output).unwrap(),
        super::denied_result(&terminal)
    );
    assert_eq!(
        read_agent_result(&mut output).unwrap(),
        AgentResult::Read(result)
    );
    assert_eq!(
        read_server_message(&mut output).unwrap(),
        super::denied_result(&start)
    );
    assert_eq!(
        output.position(),
        u64::try_from(output.get_ref().len()).unwrap()
    );
}

#[test]
fn ends_on_malformed_agent_records_without_reaching_the_executor() {
    let executor = |_request: AgentRequest| -> Result<AgentResult, AgentReadFault> {
        unreachable!("malformed records must not execute")
    };

    // A kind outside the request direction is an invalid header.
    let mut invalid_kind = Vec::from(&b"OBAP\0\x01\0\x63"[..]);
    invalid_kind.extend_from_slice(&0_u32.to_be_bytes());
    let mut stream = super::MemoryStream::new(invalid_kind);
    assert!(matches!(
        super::super::run(
            &mut stream,
            |_request| panic!("unexpected OBCS request"),
            |_request| panic!("unexpected Workbench request"),
            executor,
        ),
        Err(ApplicationSessionError::Agent(
            AgentWireError::InvalidHeader
        ))
    ));
    assert!(stream.output.is_empty());

    // A well-formed request kind with a broken JSON body is a contract fault.
    let mut invalid_body = Vec::from(&b"OBAP\0\x01\0\x01"[..]);
    invalid_body.extend_from_slice(&1_u32.to_be_bytes());
    invalid_body.extend_from_slice(b"{");
    let mut stream = super::MemoryStream::new(invalid_body);
    assert!(matches!(
        super::super::run(
            &mut stream,
            |_request| panic!("unexpected OBCS request"),
            |_request| panic!("unexpected Workbench request"),
            executor,
        ),
        Err(ApplicationSessionError::Agent(AgentWireError::Contract(_)))
    ));
    assert!(stream.output.is_empty());

    // A hostile declared length is bounded before allocation.
    let mut oversized = Vec::from(&b"OBAP\0\x01\0\x01"[..]);
    oversized.extend_from_slice(
        &(u32::try_from(orchbee_agent_observation_protocol::MAXIMUM_AGENT_REQUEST_BYTES + 1)
            .unwrap())
        .to_be_bytes(),
    );
    let mut stream = super::MemoryStream::new(oversized);
    assert!(matches!(
        super::super::run(
            &mut stream,
            |_request| panic!("unexpected OBCS request"),
            |_request| panic!("unexpected Workbench request"),
            executor,
        ),
        Err(ApplicationSessionError::Agent(AgentWireError::Contract(_)))
    ));
    assert!(stream.output.is_empty());
}

/// Names share the same sequential record loop and correlation rules as reads.
#[test]
fn routes_names_without_reinterpreting_them_as_reads() {
    use orchbee_agent_observation_protocol::{
        AgentNamesListOutcome, AgentNamesListOutcomeEnvelope, AgentNamesResult,
    };
    let request_id = ObapRequestId::parse(super::FIRST_REQUEST_ID).unwrap();
    let request = AgentRequest::Names(AgentNamesRequest::NamesList {
        request_id: request_id.clone(),
    });
    let response = AgentResult::Names(AgentNamesResult::NamesList {
        request_id,
        result: AgentNamesListOutcomeEnvelope {
            outcome: AgentNamesListOutcome::Listed {
                bindings: vec![],
                pending: vec![],
            },
        },
    });
    let mut input = Vec::new();
    write_agent_request(&mut input, &request).unwrap();
    let mut stream = super::MemoryStream::new(input).with_chunks(1, 2);
    assert!(matches!(
        super::super::run(
            &mut stream,
            |_| panic!("unexpected OBCS request"),
            |_| panic!("unexpected Workbench request"),
            |received| {
                assert_eq!(received, request);
                Ok(response.clone())
            },
        ),
        Err(ApplicationSessionError::Prefix(_))
    ));
    assert_eq!(
        read_agent_result(&mut Cursor::new(stream.output)).unwrap(),
        response
    );
}

#[test]
fn maps_executor_faults_to_agent_fault_session_errors() {
    let mut input = Vec::new();
    write_agent_request(&mut input, &AgentRequest::Read(list_request())).unwrap();
    let mut stream = super::MemoryStream::new(input);
    assert!(matches!(
        super::super::run(
            &mut stream,
            |_request| panic!("unexpected OBCS request"),
            |_request| panic!("unexpected Workbench request"),
            |_request| Err(AgentReadFault(
                "canonical state is outside the native wire profile"
            )),
        ),
        Err(ApplicationSessionError::AgentFault(_))
    ));
    assert!(stream.output.is_empty());
}

#[test]
fn ends_on_agent_result_write_failure() {
    let request = list_request();
    let result = denied_list_result(&request);
    let mut input = Vec::new();
    write_agent_request(&mut input, &AgentRequest::Read(request)).unwrap();
    let mut expected = Vec::new();
    write_agent_result(&mut expected, &AgentResult::Read(result.clone())).unwrap();
    let failure_offset = expected.len() / 2;

    let mut stream = super::MemoryStream::new(input)
        .with_chunks(usize::MAX, 3)
        .with_write_failure(failure_offset, std::io::ErrorKind::BrokenPipe);
    assert!(matches!(
        run_agent(&mut stream, result),
        Err(ApplicationSessionError::Agent(AgentWireError::Io(error)))
            if error.kind() == std::io::ErrorKind::BrokenPipe
    ));
    assert_eq!(stream.output, expected[..failure_offset]);
}
