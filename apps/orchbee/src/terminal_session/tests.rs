use super::*;
use crate::local_client::test_server::FakeServer;
use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, ServerMessage, TerminalControlAction, TerminalControlOutcome,
    TerminalControlResult, TerminalViewerOutcome, TerminalViewerResult, ViewerId,
    read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use std::os::unix::net::UnixStream;
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};
const ID: &str = "11111111111111111111111111111111";
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn geometry() -> Result<Geometry, Box<dyn std::error::Error>> {
    Ok(Geometry::new(80, 24, 8, 16)?)
}
fn admit(stream: &mut UnixStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}
fn attach(stream: &mut UnixStream) -> io::Result<()> {
    let ClientMessage::TerminalViewer(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected viewer"));
    };
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            TerminalViewerOutcome::Attached {
                attachment_id: ViewerId::parse(ID).map_err(io::Error::other)?,
                host_epoch: HostEpoch::parse(ID).map_err(io::Error::other)?,
            },
        )),
    )
    .map_err(io::Error::other)
}
fn acquire(stream: &mut UnixStream) -> io::Result<()> {
    let ClientMessage::TerminalControl(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected acquire"));
    };
    assert!(matches!(
        request.action(),
        TerminalControlAction::Acquire {}
    ));
    write_server_message(
        stream,
        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
            request.request_id().clone(),
            TerminalControlOutcome::Acquired {
                controller_id: orchbee_client_server_protocol::ControllerId::parse(ID)
                    .map_err(io::Error::other)?,
            },
        )),
    )
    .map_err(io::Error::other)
}
fn spin(session: &mut TerminalSession, predicate: impl Fn(&TerminalSession) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate(session) {
        assert!(Instant::now() < deadline, "session did not settle");
        session.tick();
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn drop_interrupts_an_inflight_exchange_and_joins_its_owner() -> TestResult {
    let (sent, received) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        attach(&mut stream)?;
        let _acquire = read_client_message(&mut stream).map_err(io::Error::other)?;
        sent.send(()).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        assert!(bytes.is_empty());
        Ok(())
    })?;
    let session = TerminalSession::start(
        LocalClient::connect(server.endpoint(), "test-cohort")?,
        TerminalIntent::Attach(RequestId::parse(ID)?),
        geometry()?,
    )?;
    received.recv_timeout(Duration::from_secs(1))?;
    let start = Instant::now();
    drop(session);
    assert!(start.elapsed() < Duration::from_secs(1));
    server.finish()?;
    Ok(())
}
#[test]
fn lost_input_result_retires_without_replaying_the_effect() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        admit(&mut stream)?;
        attach(&mut stream)?;
        acquire(&mut stream)?;
        loop {
            match read_client_message(&mut stream).map_err(io::Error::other)? {
                ClientMessage::TerminalViewer(request) => write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::Empty {},
                    )),
                )
                .map_err(io::Error::other)?,
                ClientMessage::TerminalControl(request) => {
                    if let TerminalControlAction::Resize { geometry, .. } = request.action() {
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                TerminalControlOutcome::Resized {
                                    geometry: *geometry,
                                },
                            )),
                        )
                        .map_err(io::Error::other)?;
                        continue;
                    }
                    assert!(
                        matches!(request.action(), TerminalControlAction::Input {bytes,..} if bytes.as_bytes()==b"do-once")
                    );
                    return Ok(());
                }
                _ => return Err(io::Error::other("unexpected request")),
            }
        }
    })?;
    let mut session = TerminalSession::start(
        LocalClient::connect(server.endpoint(), "test-cohort")?,
        TerminalIntent::Attach(RequestId::parse(ID)?),
        geometry()?,
    )?;
    spin(&mut session, |s| s.status == TerminalStatus::Controlling);
    assert_eq!(session.status(), TerminalStatus::Blocked);
    session.forward(b"do-once").map_err(|_| "input")?;
    spin(&mut session, |s| {
        s.status() == TerminalStatus::OutcomeUnknown
    });
    assert!(session.forward(b"do-once").is_err());
    drop(session);
    server.finish()?;
    Ok(())
}
#[test]
fn input_admission_is_bounded_and_failure_cannot_accept_more_work() -> TestResult {
    let (shutdown, _peer) = UnixStream::pair()?;
    let (commands, _receiver) = mpsc::sync_channel(8);
    let mut session = TerminalSession {
        commands: Some(commands),
        events: None,
        task: None,
        shutdown: crate::local_client::test_server::shutdown_handle(shutdown),
        status: TerminalStatus::Controlling,
        projection: None,
        feedback: None,
        poll_pending: true,
        geometry: geometry()?,
    };
    for _ in 0..8 {
        assert!(session.forward(&[255; 4096]).is_ok());
    }
    assert!(session.forward(b"not-submitted").is_err());
    session.retire(TerminalStatus::OutcomeUnknown);
    assert!(session.forward(b"not-submitted").is_err());
    Ok(())
}

#[test]
fn create_uses_default_intent_and_a_busy_controller_stays_read_only() -> TestResult {
    use orchbee_client_server_protocol::{
        ControlRejectedReason, CreateAcceptedState, CreateOutcome, StartMode, TerminalCreateResult,
    };
    let server = FakeServer::start(|mut stream| {
        admit(&mut stream)?;
        let ClientMessage::TerminalCreate(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected Create"));
        };
        assert!(request.worker_constraint().is_none());
        assert!(request.cwd().is_none());
        let StartMode::Immediate { geometry } = request.start_mode() else {
            return Err(io::Error::other("expected immediate"));
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Accepted {
                    state: CreateAcceptedState::Started {
                        geometry: *geometry,
                    },
                },
            )),
        )
        .map_err(io::Error::other)?;
        attach(&mut stream)?;
        let ClientMessage::TerminalControl(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected acquire"));
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                },
            )),
        )
        .map_err(io::Error::other)?;
        loop {
            match read_client_message(&mut stream) {
                Ok(ClientMessage::TerminalViewer(request)) => {
                    let _result = write_server_message(
                        &mut stream,
                        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                            request.request_id().clone(),
                            TerminalViewerOutcome::Empty {},
                        )),
                    );
                }
                Err(_) => return Ok(()),
                _ => return Err(io::Error::other("read-only Client submitted control")),
            }
        }
    })?;
    let mut session = TerminalSession::start(
        LocalClient::connect(server.endpoint(), "test-cohort")?,
        TerminalIntent::Create { cwd: None },
        geometry()?,
    )?;
    spin(&mut session, |s| s.status == TerminalStatus::ReadOnly);
    assert_eq!(session.status(), TerminalStatus::Blocked);
    assert!(session.forward(b"never submitted").is_err());
    session.resize(Geometry::new(100, 30, 8, 16)?);
    drop(session);
    server.finish()?;
    Ok(())
}
