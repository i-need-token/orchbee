use std::{io::Write, thread, time::Duration};

use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateUnavailableReason, Geometry, RequestId, ServerMessage,
    StartMode, StartOutcome, StartRejectedReason, TerminalCreateRequest, TerminalCreateResult,
    TerminalStartRequest, TerminalStartResult, encode_client_frame, read_server_message,
    write_client_message as write_obcs_message,
};
use orchbee_local_client_protocol::{
    ClientHello, LocalClientMessage, read_message as read_oblc_message,
    write_message as write_oblc_message,
};
use orchbee_local_site_protocol::{LocalSiteMessage, RejectionReason as WorkerRejectionReason};

use super::{
    FIRST_WORKER, RunningServer, SECOND_WORKER, TestResult, connect_bounded_client,
    connect_candidate, connect_worker, exchange_worker, expect_connection_closed,
};

const FIRST_REQUEST_ID: &str = "11111111111111111111111111111111";
const SECOND_REQUEST_ID: &str = "22222222222222222222222222222222";
const POST_ADMISSION_STALL: Duration = Duration::from_millis(2_500);

#[test]
fn coalesced_oblc_hello_and_start_observe_acceptance_before_missing_reference() -> TestResult {
    let server = RunningServer::start()?;
    let start = start_request()?;
    let mut bytes = Vec::new();
    write_oblc_message(
        &mut bytes,
        &LocalClientMessage::ClientHello(ClientHello::new(env!("CARGO_PKG_VERSION"))?),
    )?;
    bytes.extend_from_slice(&encode_client_frame(&start)?);

    let mut stream = connect_candidate(server.client_endpoint())?;
    let written = stream.write(&bytes)?;
    assert_eq!(written, bytes.len());
    assert!(matches!(
        read_oblc_message(&mut stream)?,
        LocalClientMessage::ClientAccepted { .. }
    ));
    assert_eq!(read_server_message(&mut stream)?, denied_result(&start));
    Ok(())
}

#[test]
fn sequential_create_and_start_report_missing_worker_and_reference() -> TestResult {
    let server = RunningServer::start()?;
    let (mut client, _generation) = connect_bounded_client(server.client_endpoint())?;

    let create = deferred_create()?;
    write_obcs_message(&mut client, &create)?;
    assert_eq!(read_server_message(&mut client)?, denied_result(&create));

    let worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;

    let start = start_request()?;
    write_obcs_message(&mut client, &start)?;
    assert_eq!(read_server_message(&mut client)?, denied_result(&start));

    client.write_all(b"OBCS\x00\x01\x00\x01{")?;
    expect_connection_closed(&mut client)?;
    assert_eq!(
        exchange_worker(server.worker_endpoint(), SECOND_WORKER)?,
        LocalSiteMessage::Rejected {
            reason: WorkerRejectionReason::DuplicateWorker,
        }
    );
    drop(worker);
    Ok(())
}

#[test]
fn post_admission_stall_has_no_timeout_and_does_not_block_other_peers() -> TestResult {
    let server = RunningServer::start()?;
    let (mut stalled, _stalled_generation) = connect_bounded_client(server.client_endpoint())?;
    thread::sleep(POST_ADMISSION_STALL);

    let (mut other, _other_generation) = connect_bounded_client(server.client_endpoint())?;
    let other_create = immediate_create()?;
    write_obcs_message(&mut other, &other_create)?;
    assert_eq!(
        read_server_message(&mut other)?,
        denied_result(&other_create)
    );

    let recovered_start = start_request()?;
    write_obcs_message(&mut stalled, &recovered_start)?;
    assert_eq!(
        read_server_message(&mut stalled)?,
        denied_result(&recovered_start)
    );
    Ok(())
}

fn immediate_create() -> Result<ClientMessage, Box<dyn std::error::Error>> {
    Ok(ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        RequestId::parse(SECOND_REQUEST_ID)?,
        None,
        None,
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

fn start_request() -> Result<ClientMessage, Box<dyn std::error::Error>> {
    Ok(ClientMessage::TerminalStart(TerminalStartRequest::new(
        RequestId::parse(SECOND_REQUEST_ID)?,
        RequestId::parse(FIRST_REQUEST_ID)?,
        Geometry::new(120, 40, 7, 14)?,
    )?))
}

fn denied_result(request: &ClientMessage) -> ServerMessage {
    match request {
        ClientMessage::TerminalOpen(_)
        | ClientMessage::TerminalControl(_)
        | ClientMessage::TerminalViewer(_) => {
            unreachable!("launch-only test fixture")
        }
        ClientMessage::TerminalCreate(request) => {
            ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::NoCompatibleWorker,
                },
            ))
        }
        ClientMessage::TerminalStart(request) => {
            ServerMessage::TerminalStartResult(TerminalStartResult::new(
                request.request_id().clone(),
                StartOutcome::Rejected {
                    reason: StartRejectedReason::TerminalCreateUnavailable,
                },
            ))
        }
        ClientMessage::TerminalSend(request) => ServerMessage::TerminalSendResult(
            match orchbee_client_server_protocol::TerminalSendResult::new(
                request.request_id().clone(),
                None,
                orchbee_client_server_protocol::TerminalSendOutcome::Unavailable {
                    reason:
                        orchbee_client_server_protocol::SendUnavailableReason::WorkerUnavailable,
                    progress: orchbee_client_server_protocol::SendProgress::none(),
                    cleanup: orchbee_client_server_protocol::SendCleanupStatus::NotNeeded,
                },
            ) {
                Ok(result) => result,
                Err(_) => unreachable!("validated send outcomes are encodable"),
            },
        ),
        ClientMessage::TerminalLaunchRead(request) => {
            ServerMessage::TerminalLaunchReadResult(
                orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                    request.request_id().clone(),
                    orchbee_client_server_protocol::TerminalLaunchReadOutcome::Unavailable {
                        reason:
                            orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
                    },
                ),
            )
        }
    }
}

#[test]
fn partial_post_admission_record_expires_without_blocking_another_session() -> TestResult {
    let server = RunningServer::start()?;
    let (mut partial, _) = connect_bounded_client(server.client_endpoint())?;
    partial.write_all(b"O")?;
    let (mut active, _) = connect_bounded_client(server.client_endpoint())?;
    let create = deferred_create()?;
    write_obcs_message(&mut active, &create)?;
    assert_eq!(read_server_message(&mut active)?, denied_result(&create));
    expect_connection_closed(&mut partial)?;
    let start = start_request()?;
    write_obcs_message(&mut active, &start)?;
    assert_eq!(read_server_message(&mut active)?, denied_result(&start));
    Ok(())
}
