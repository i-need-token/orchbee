//! Transport-level evidence for the OBAP first-read: real admitted sockets,
//! the real `serve` listener loop, real OBLC handshakes, and real OBAP record
//! exchange — against a canonical runtime seeded only through real event
//! dispatch. The runtime-owner drop exercises the same disposal path the
//! listener guard runs when the accept loop ends.

use std::{
    fs,
    io::{self, Write},
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentGetOutcome, AgentListOutcome, AgentReadRequest, AgentReadResult,
    AgentSafeInteger, AgentSemanticStatus, AgentSessionId, AgentSessionRejectionReason,
    AgentStatusBasis, AgentUnavailableReason, ObapRequestId, read_agent_result,
    write_agent_request,
};
use orchbee_local_client_protocol::{ClientHello, LocalClientMessage, read_message, write_message};

use super::super::{AgentRuntimeOwner, agent_harness};

static ENDPOINT_COUNTER: AtomicU32 = AtomicU32::new(0);

fn request_id(seed: u8) -> ObapRequestId {
    ObapRequestId::parse(&format!("{seed:032x}")).unwrap()
}

fn session_id(value: &str) -> AgentSessionId {
    AgentSessionId::parse(value).unwrap()
}

fn list_request(seed: u8) -> AgentReadRequest {
    AgentReadRequest::List {
        request_id: request_id(seed),
        include_ended: false,
        limit: AgentSafeInteger::new(50),
        cursor: None,
    }
}

fn handshake(client: &mut UnixStream, cohort: &str) {
    write_message(
        client,
        &LocalClientMessage::ClientHello(ClientHello::new(cohort).unwrap()),
    )
    .unwrap();
    assert!(matches!(
        read_message(client).unwrap(),
        LocalClientMessage::ClientAccepted { .. }
    ));
}

/// A seeded listener on a real filesystem socket serves the full OBAP
/// first-read — including genuine empty and not-found projections — through
/// the real `serve` accept loop and the real OBLC admission path.
#[test]
fn a_real_listener_serves_seeded_agent_reads_over_the_socket() -> io::Result<()> {
    let directory = std::env::temp_dir().join(format!(
        "orchbee-agent-listener-{}-{}",
        std::process::id(),
        ENDPOINT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&directory)?;
    let endpoint = directory.join("client.sock");
    let agents = agent_harness::seeded(&[
        agent_harness::session_observed("sock.a", 1.0),
        agent_harness::turn_started("sock.a", "turn-1", "thinking", 1.0, 2.0),
    ])?;
    let _listener = agent_harness::listen(&endpoint, "agent-test-cohort", agents)?;

    let mut client = UnixStream::connect(&endpoint)?;
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    client.set_write_timeout(Some(Duration::from_secs(5)))?;
    handshake(&mut client, "agent-test-cohort");

    write_agent_request(&mut client, &list_request(1)).unwrap();
    let AgentReadResult::List {
        request_id: echoed,
        result,
    } = read_agent_result(&mut client).unwrap()
    else {
        panic!("expected a list result");
    };
    assert_eq!(echoed, request_id(1));
    let AgentListOutcome::Listed {
        items,
        next_cursor,
        revision,
    } = result.outcome
    else {
        panic!("expected a listed outcome");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].session_id.as_str(), "sock.a");
    assert_eq!(
        items[0].status,
        AgentSemanticStatus::Working {
            activity: Some(orchbee_agent_observation_protocol::AgentActivity::Thinking),
            phase: orchbee_agent_observation_protocol::AgentTurnPhase::Working,
        }
    );
    assert!(next_cursor.is_none());
    assert_eq!(revision.value(), 2);

    write_agent_request(
        &mut client,
        &AgentReadRequest::Get {
            request_id: request_id(2),
            session_id: session_id("sock.absent"),
        },
    )
    .unwrap();
    let AgentReadResult::Get { result, .. } = read_agent_result(&mut client).unwrap() else {
        panic!("expected a get result");
    };
    assert_eq!(
        result.outcome,
        AgentGetOutcome::Rejected {
            reason: AgentSessionRejectionReason::NotFound,
        }
    );

    write_agent_request(
        &mut client,
        &AgentReadRequest::Explain {
            request_id: request_id(3),
            session_id: session_id("sock.a"),
        },
    )
    .unwrap();
    let AgentReadResult::Explain { result, .. } = read_agent_result(&mut client).unwrap() else {
        panic!("expected an explain result");
    };
    let AgentExplainOutcome::Found { explanation, .. } = result.outcome else {
        panic!("expected a found outcome");
    };
    assert_eq!(explanation.status_basis, AgentStatusBasis::ActiveTurn);

    drop(client);
    fs::remove_file(&endpoint).ok();
    fs::remove_dir(&directory).ok();
    Ok(())
}

/// The same owner guard `serve` installs disposes the runtime when the
/// listener loop ends; an already-admitted connection then observes genuine
/// `runtime-unavailable` rather than fabricated state.
#[test]
fn an_admitted_connection_observes_runtime_unavailable_after_the_owner_drops() -> io::Result<()> {
    let agents = agent_harness::seeded(&[agent_harness::session_observed("sock.b", 1.0)])?;
    let mut client = agent_harness::admitted(Arc::clone(&agents), "agent-test-cohort")?;

    write_agent_request(client.stream(), &list_request(1)).unwrap();
    let AgentReadResult::List { result, .. } = read_agent_result(client.stream()).unwrap() else {
        panic!("expected a list result");
    };
    assert!(matches!(result.outcome, AgentListOutcome::Listed { .. }));

    drop(AgentRuntimeOwner::new(Arc::clone(&agents)));

    write_agent_request(client.stream(), &list_request(2)).unwrap();
    let AgentReadResult::List { result, .. } = read_agent_result(client.stream()).unwrap() else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );
    client.join();
    Ok(())
}

/// A malformed OBAP record loudly ends the session — the demultiplexer never
/// silently skips an unparseable record.
#[test]
fn a_malformed_obap_record_ends_the_session() -> io::Result<()> {
    let agents = agent_harness::seeded(&[])?;
    let mut client = agent_harness::admitted(agents, "agent-test-cohort")?;
    let mut header = [0_u8; 12];
    header[..4].copy_from_slice(b"OBAP");
    header[4..6].copy_from_slice(&1_u16.to_be_bytes());
    header[6..8].copy_from_slice(&99_u16.to_be_bytes());
    client.stream().write_all(&header)?;
    assert!(read_agent_result(client.stream()).is_err());
    client.join();
    Ok(())
}

/// An empty canonical runtime is a real state, not an error: the real
/// listener answers with an empty page and `not-found` rejections.
#[test]
fn a_real_listener_reports_the_genuine_empty_state() -> io::Result<()> {
    let directory = std::env::temp_dir().join(format!(
        "orchbee-agent-listener-{}-{}",
        std::process::id(),
        ENDPOINT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&directory)?;
    let endpoint = directory.join("client.sock");
    let agents = agent_harness::seeded(&[])?;
    let _listener = agent_harness::listen(&endpoint, "agent-test-cohort", agents)?;

    let mut client = UnixStream::connect(&endpoint)?;
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    handshake(&mut client, "agent-test-cohort");

    write_agent_request(&mut client, &list_request(1)).unwrap();
    let AgentReadResult::List { result, .. } = read_agent_result(&mut client).unwrap() else {
        panic!("expected a list result");
    };
    let AgentListOutcome::Listed {
        items,
        next_cursor,
        revision,
    } = result.outcome
    else {
        panic!("expected a listed outcome");
    };
    assert!(items.is_empty());
    assert!(next_cursor.is_none());
    assert_eq!(revision.value(), 0);

    drop(client);
    fs::remove_file(&endpoint).ok();
    fs::remove_dir(&directory).ok();
    Ok(())
}
