//! Staged transport proofs: real `UnixStream` clients against the
//! endpoint thread and the SAME canonical `AgentService` cell.
//!
//! Settlement semantics are pinned exactly (join.rs :525-541): a
//! single-correlation envelope whose correlation is fully matched
//! applies INSIDE the admit fold and never schedules a timer (A1);
//! a partial match emits `ScheduleSettlement` for the source 25 ms
//! delay and only the endpoint's own tick can fire it (A2); an
//! unmatched envelope is ACK-able and produces no timer at all (B).
//! Case D is the liveness proof for `POLL_LIVENESS_CAP`: a timer
//! created through the native Instant funnel — outside the endpoint
//! thread — must still settle through the capped poll, with no manual
//! tick and no connection wake.
use orchbee_local_ipc::LocalStream as UnixStream;

use std::{
    collections::VecDeque,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentListOutcome, AgentReadRequest, AgentReadResult, AgentSafeInteger,
    AgentSessionId, ObapRequestId,
};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use serde_json::json;

use super::super::*;
use super::ACK;
use crate::{
    agent::native_observation::{Binding, CarrierIdentity, NativeClass, NativeEvent},
    agent_service::{AgentPolicy, AgentService},
    local_client_service::tests::agent_harness,
};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "orchbee-obs-endpoint-{tag}-{}-{}",
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn carrier_id(worker_seed: u8, generation: u64) -> CarrierIdentity {
    CarrierIdentity {
        worker_id: WorkerId::from_bytes([worker_seed; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}

fn host(seed: u8) -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([seed; 16]).unwrap(),
        HostEpoch::from_bytes([seed.wrapping_add(16); 16]).unwrap(),
        HostTerminalId::from_bytes([seed.wrapping_add(32); 16]).unwrap(),
    )
}

fn presentation(seed: u8) -> TerminalPresentationId {
    TerminalPresentationId::parse(&format!("{seed:032x}")).unwrap()
}

fn hint_json(correlation: u64, event: u64) -> String {
    format!(
        "{{\"correlationId\":\"correlation-{correlation:016}\",\
         \"eventId\":\"event-{event:016}\",\
         \"kind\":\"correlation\",\"version\":1}}"
    )
}

fn event(
    carrier: CarrierIdentity,
    sequence: u64,
    host: HostTerminalIdentity,
    raw: impl Into<Box<str>>,
) -> NativeEvent {
    NativeEvent::new(carrier, NonZeroU64::new(sequence).unwrap(), host, raw).unwrap()
}

fn envelope(correlations: &[u64], event: u64, visibility: &str) -> Vec<u8> {
    let correlations: Vec<_> = correlations
        .iter()
        .map(|correlation| {
            json!({
                "correlationId": format!("correlation-{correlation:016}"),
                "visibility": visibility,
            })
        })
        .collect();
    json!({
        "adapterId": "codex",
        "correlations": correlations,
        "eventId": format!("event-{event:016}"),
        "observation": {
            "activity": null,
            "agentKey": null,
            "event": "session-started",
            "operationKey": null,
            "sessionKey": "session-1",
            "turnKey": null,
            "version": 1,
        },
        "version": 1,
    })
    .to_string()
    .into_bytes()
}

fn request_id(seed: u8) -> ObapRequestId {
    ObapRequestId::parse(&format!("{seed:032x}")).unwrap()
}

fn list_request(seed: u8) -> AgentReadRequest {
    AgentReadRequest::List {
        request_id: request_id(seed),
        include_ended: false,
        limit: AgentSafeInteger::new(50),
        cursor: None,
    }
}

fn get_request(seed: u8, session: &str) -> AgentReadRequest {
    AgentReadRequest::Get {
        request_id: request_id(seed),
        session_id: AgentSessionId::parse(session).unwrap(),
    }
}

fn listed_sessions(service: &AgentService) -> Vec<String> {
    let AgentReadResult::List { result, .. } = service.execute(list_request(1)).unwrap() else {
        panic!("expected a list result");
    };
    let AgentListOutcome::Listed { items, .. } = result.outcome else {
        panic!("expected listed outcome, got {:?}", result.outcome);
    };
    items
        .iter()
        .map(|item| item.session_id.as_str().to_owned())
        .collect()
}

fn get_session_found(service: &AgentService, session: &str) -> bool {
    let AgentReadResult::Get { result, .. } = service.execute(get_request(2, session)).unwrap()
    else {
        panic!("expected a get result");
    };
    matches!(result.outcome, AgentGetOutcome::Found { .. })
}

/// The T1 correlation seed — carrier admission, held typed hint, then
/// committed-binding reconciliation — through the real staged native
/// seams. Afterwards `correlation-{seed:016}` has a live hint waiting
/// for a matching event.
fn seed_correlation(service: &AgentService, carrier_seed: u8, correlation: u64) {
    let t0 = Instant::now();
    let carrier = carrier_id(carrier_seed, 1);
    let host1 = host(carrier_seed);
    service.native_admit_carrier(carrier, t0).unwrap().unwrap();
    service
        .native_apply_event(
            event(carrier, 1, host1, hint_json(correlation, correlation)),
            t0,
        )
        .unwrap();
    service
        .native_reconcile_bindings(
            vec![Binding {
                presentation: presentation(carrier_seed),
                carrier,
                host: host1,
            }],
            t0,
        )
        .unwrap()
        .unwrap();
}

/// Reads exactly the ACK literal; returns what arrived (short reads
/// included) so callers can assert on truncation too.
fn read_ack(client: &mut UnixStream) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 64];
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match client.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() >= 30 {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "ack never arrived");
                thread::sleep(Duration::from_millis(2));
            }
            Err(_) => break,
        }
    }
    buf
}

fn connect_send(service_path: &std::path::Path, body: &[u8]) -> UnixStream {
    let mut client = UnixStream::connect(service_path).unwrap();
    client.write_all(body).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    client.set_nonblocking(true).unwrap();
    client
}

/// A1 — a single-correlation envelope whose correlation is fully
/// matched applies INSIDE the admit fold: the session exists by the
/// time the 30-byte ACK returns, and `next_deadline` stays `None`
/// because no settlement timer is ever produced. Seeded through the
/// staged native seams (the T1 recipe).
#[test]
fn matched_envelope_applies_immediately_without_a_timer() {
    let dir = tmp_dir("matched");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    seed_correlation(&service, 0xaa, 1);

    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut client = connect_send(endpoint.path(), &envelope(&[1], 1, "visible"));
    assert_eq!(read_ack(&mut client), ACK.to_vec());

    // Immediate apply: canonical state already holds the session, and
    // the honest no-timer discriminator stays `None` throughout.
    let sessions = listed_sessions(&service);
    assert_eq!(sessions.len(), 1);
    assert!(get_session_found(&service, &sessions[0]));
    assert_eq!(service.next_deadline().unwrap(), None);
    drop(endpoint);
    assert!(!dir.join(OBSERVATION_SOCKET_NAME).exists());
}

/// L2 — admit-side settle on the REAL endpoint: a two-correlation
/// envelope with exactly one hint-matched correlation is a PARTIAL
/// match — admitted (30-byte ACK) and held on the 25 ms settlement
/// timer inserted inside the endpoint's own admit fold. A partial
/// match can never immediate-apply, so the eventual Found is
/// attributable only to the endpoint's tick — an arrival assertion,
/// never a latency assertion (mid-flight deadline/emptiness reads on
/// a live endpoint race the 25 ms timer and prove nothing).
#[test]
fn partially_matched_envelope_settles_via_endpoint_tick() {
    let dir = tmp_dir("partial");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    seed_correlation(&service, 0xab, 1);

    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut client = connect_send(endpoint.path(), &envelope(&[1, 2], 1, "visible"));
    assert_eq!(read_ack(&mut client), ACK.to_vec());

    let budget = Instant::now() + Duration::from_secs(5);
    loop {
        let sessions = listed_sessions(&service);
        if sessions.len() == 1 && get_session_found(&service, &sessions[0]) {
            break;
        }
        assert!(Instant::now() < budget, "session never materialized");
        thread::sleep(Duration::from_millis(2));
    }
    // The fired timer is gone — the endpoint's tick consumed it.
    assert_eq!(service.next_deadline().unwrap(), None);
    drop(endpoint);
}

/// B1 — admitted is not applied: an envelope with no seeded
/// correlation is ACK'd (the literal proves the admit fold ran), the
/// canonical snapshot stays empty, and NO settlement timer exists —
/// `next_deadline` returning `None` throughout is the honest
/// no-producer evidence. A second envelope reusing the claimed
/// correlation with a different event id is rejected outright: the
/// correlation stays claimed until the join TTL, so admission refuses
/// and the connection closes bare.
#[test]
fn unmatched_envelope_acks_but_never_applies() {
    let dir = tmp_dir("unmatched");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut client = connect_send(endpoint.path(), &envelope(&[7], 7, "visible"));
    assert_eq!(read_ack(&mut client), ACK.to_vec());
    assert!(listed_sessions(&service).is_empty());
    assert_eq!(service.next_deadline().unwrap(), None);

    // Claimed-correlation reuse with a different event id: refused at
    // admission — bare close, no ACK bytes.
    let mut replay = connect_send(endpoint.path(), &envelope(&[7], 8, "visible"));
    assert!(read_ack(&mut replay).is_empty());
}

/// B2 — a zero-correlation envelope never reaches admission: the
/// oracle parse requires a non-empty correlations array
/// (hook_contract.rs :204), so the connection closes bare — no ACK,
/// no session, no timer.
#[test]
fn zero_correlation_envelope_is_refused_at_parse() {
    let dir = tmp_dir("zero-corr");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut client = connect_send(endpoint.path(), &envelope(&[], 3, "visible"));
    assert!(read_ack(&mut client).is_empty());
    assert!(listed_sessions(&service).is_empty());
    assert_eq!(service.next_deadline().unwrap(), None);
}

/// L1 — the liveness proof for `POLL_LIVENESS_CAP`: a settlement
/// timer created by a DIFFERENT caller — the native Instant funnel's
/// RouteHint replay (event-before-hint ordering) — produces no wake
/// of its own. Without the cap the poll blocks on `PollTimeout::NONE`
/// (zero connections, zero timers) and Found can never arrive: no
/// manual `tick` call and no second connection exist to wake the
/// loop, so the eventual Found discriminates the cap's presence
/// end-to-end. The deterministic deadline-VALUE discrimination lives
/// in the service-level units (agent_service::tests) — this is an
/// arrival assertion, never a latency assertion.
#[test]
fn externally_created_timer_settles_via_capped_poll() {
    let dir = tmp_dir("external-timer");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    // Binding live; NO held event — the hint routes straight through.
    let t0 = Instant::now();
    let carrier = carrier_id(0xcc, 1);
    let host1 = host(0xcc);
    service.native_admit_carrier(carrier, t0).unwrap().unwrap();
    service
        .native_reconcile_bindings(
            vec![Binding {
                presentation: presentation(0xcc),
                carrier,
                host: host1,
            }],
            t0,
        )
        .unwrap()
        .unwrap();

    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    // Two correlations, both unmatched -> pending event, no timer.
    let mut client = connect_send(endpoint.path(), &envelope(&[1, 2], 1, "visible"));
    assert_eq!(read_ack(&mut client), ACK.to_vec());
    // Deterministic pre-condition: no actor can have inserted a
    // settlement timer yet.
    assert_eq!(service.next_deadline().unwrap(), None);
    drop(client);

    // The hint arrives through the native funnel — outside the
    // endpoint thread. The partial match schedules its 25ms
    // settlement inside that fold; only the capped poll can ever
    // observe it.
    let t_hint = Instant::now() + Duration::from_millis(100);
    let routed = service
        .native_apply_event(event(carrier, 1, host1, hint_json(1, 1)), t_hint)
        .unwrap();
    assert_eq!(routed.class, NativeClass::Routed);

    // Bounded-poll arrival (budget ≫ 25ms + slack): the endpoint's
    // own capped-poll tick is the only actor that can fire the timer.
    let budget = Instant::now() + Duration::from_secs(1);
    loop {
        let sessions = listed_sessions(&service);
        if sessions.len() == 1 && get_session_found(&service, &sessions[0]) {
            break;
        }
        assert!(
            Instant::now() < budget,
            "externally created timer never settled — the capped poll missed it"
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(service.next_deadline().unwrap(), None);
    drop(endpoint);
}

/// C1 — oversized body destroys the connection without an ACK.
#[test]
fn oversize_body_is_destroyed_without_ack() {
    let dir = tmp_dir("oversize");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let body = vec![b'x'; MAX_OBSERVATION_BODY_BYTES + 1];
    let mut client = connect_send(endpoint.path(), &body);
    assert!(read_ack(&mut client).is_empty());
}

/// C2 — a stalled (never-EOF) reader is swept at the absolute
/// accept+500ms deadline even mid-Read.
#[test]
fn stalled_reader_is_swept_at_deadline() {
    let dir = tmp_dir("stalled");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut client = UnixStream::connect(endpoint.path()).unwrap();
    client.write_all(b"{\"partial\":").unwrap();
    thread::sleep(CONNECTION_DEADLINE + Duration::from_millis(400));
    client.write_all(&envelope(&[1], 1, "visible")).ok();
    client.shutdown(Shutdown::Write).ok();
    client.set_nonblocking(true).unwrap();
    assert!(read_ack(&mut client).is_empty());
}

/// C3 — a disposed runtime refuses admission honestly: the
/// connection closes without an ACK (disposal-before-admission).
#[test]
fn disposed_runtime_closes_without_ack() {
    let dir = tmp_dir("disposed");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    service.shutdown();
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut client = connect_send(endpoint.path(), &envelope(&[1], 1, "visible"));
    assert!(read_ack(&mut client).is_empty());
}

/// C4 — the one-rule fold-Err policy: a poisoned cell closes each
/// connection honestly (admit Err -> destroy, zero bytes) while the
/// ENDPOINT ITSELF keeps serving — a second fresh connection must
/// still be accepted-then-closed, never refused at connect. A
/// `?`-propagating implementation fails the second half by refusing
/// the connect outright.
#[test]
fn poisoned_cell_keeps_serving_honest_closes() {
    let dir = tmp_dir("poisoned");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    service
        .poison_state_after_mutation_for_test(&agent_harness::session_observed("s.fault", 7.0))
        .unwrap_err();
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut first = connect_send(endpoint.path(), &envelope(&[1], 1, "visible"));
    assert!(read_ack(&mut first).is_empty());
    let mut second = connect_send(endpoint.path(), &envelope(&[2], 2, "visible"));
    assert!(read_ack(&mut second).is_empty());
}

/// C7 — identity-gated teardown: a same-euid replacement at the bound
/// pathname is left untouched (mismatched recorded identity), the
/// capture-failure arm (`None`) unlinks nothing, and the endpoint's
/// own socket is reclaimed on a clean drop.
#[test]
fn identity_mismatch_leaves_replacement_path() {
    let dir = tmp_dir("identity");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let socket_path = dir.join(OBSERVATION_SOCKET_NAME);

    // Bound-never-started: own socket reclaimed on drop.
    {
        let bound = bind(&dir).unwrap();
        assert!(bound.path().exists());
        drop(bound);
    }
    assert!(!socket_path.exists());

    // Capture-failure arm: `None` recorded identity must never unlink.
    fs::write(&socket_path, b"unverifiable").unwrap();
    unlink_if_identity_matches(&socket_path, None);
    assert_eq!(fs::read(&socket_path).unwrap(), b"unverifiable");
    fs::remove_file(&socket_path).unwrap();

    // Replaced pathname: foreign content survives the drop.
    let endpoint = bind(&dir).unwrap().start(service);
    fs::remove_file(&socket_path).unwrap();
    fs::write(&socket_path, b"replacement").unwrap();
    drop(endpoint);
    assert_eq!(fs::read(&socket_path).unwrap(), b"replacement");
    fs::remove_file(&socket_path).unwrap();
}

/// C8 — unconditional join is real: dropping the endpoint with a
/// LIVE connection reaps the thread (drop() returning is itself the
/// join-completed evidence — no global thread oracle), the live
/// connection is destroyed, and the pathname is unlinked.
#[test]
fn drop_with_live_connection_reaps_thread() {
    let dir = tmp_dir("live-drop");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let socket_path = endpoint.path().to_owned();
    let mut live = UnixStream::connect(&socket_path).unwrap();
    live.write_all(b"{\"held\":").unwrap();
    live.set_nonblocking(true).unwrap();
    drop(endpoint);
    assert!(!socket_path.exists());
    let mut buf = [0u8; 8];
    let closed = matches!(live.read(&mut buf), Ok(0) | Err(_));
    assert!(closed, "live connection should be destroyed");
}

/// The 64-connection bound: every admitted connection inside the cap
/// is served; the cap+1 connection is dropped unACK'd.
#[test]
fn connection_cap_is_enforced() {
    let dir = tmp_dir("cap");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut held = Vec::new();
    for _ in 0..MAX_ACTIVE_CONNECTIONS {
        let client = UnixStream::connect(endpoint.path()).unwrap();
        held.push(client);
    }
    // The cap+1 connection is accepted-then-dropped under the bound —
    // its reads see a closed socket, never an ACK. Writes are
    // best-effort: the endpoint may already have dropped the fd.
    let mut over = UnixStream::connect(endpoint.path()).unwrap();
    over.write_all(&envelope(&[1], 1, "visible")).ok();
    over.shutdown(Shutdown::Write).ok();
    over.set_nonblocking(true).unwrap();
    assert!(read_ack(&mut over).is_empty());
    drop(held);
}

/// Interleaved connections: a matched and an unmatched client on the
/// same endpoint are each served inside their own deadline.
#[test]
fn interleaved_connections_each_served() {
    let dir = tmp_dir("interleaved");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    seed_correlation(&service, 0xbb, 9);
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut matched = connect_send(endpoint.path(), &envelope(&[9], 9, "visible"));
    let mut unmatched = connect_send(endpoint.path(), &envelope(&[77], 77, "hidden"));
    assert_eq!(read_ack(&mut matched), ACK.to_vec());
    assert_eq!(read_ack(&mut unmatched), ACK.to_vec());
    let sessions = listed_sessions(&service);
    assert_eq!(sessions.len(), 1);
    assert!(get_session_found(&service, &sessions[0]));
}

/// Per-connection deadlines are independent: two stalled readers
/// started ~200ms apart must close ~200ms apart — a shared or
/// connection-less deadline would close them together.
#[test]
fn stalled_readers_close_on_their_own_deadlines() {
    let dir = tmp_dir("staggered");
    let service = Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let endpoint = bind(&dir).unwrap().start(Arc::clone(&service));
    let mut first = UnixStream::connect(endpoint.path()).unwrap();
    first.write_all(b"{\"partial\":").unwrap();
    first.set_nonblocking(true).unwrap();
    thread::sleep(Duration::from_millis(200));
    let mut second = UnixStream::connect(endpoint.path()).unwrap();
    second.write_all(b"{\"partial\":").unwrap();
    second.set_nonblocking(true).unwrap();

    let eof = |client: &mut UnixStream| -> Option<Instant> {
        let mut buf = [0u8; 16];
        match client.read(&mut buf) {
            Ok(0) => Some(Instant::now()),
            Ok(_) => None,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
            Err(_) => Some(Instant::now()),
        }
    };
    let budget = Instant::now() + Duration::from_secs(3);
    let (mut first_eof, mut second_eof) = (None, None);
    while first_eof.is_none() || second_eof.is_none() {
        if first_eof.is_none() {
            first_eof = eof(&mut first);
        }
        if second_eof.is_none() {
            second_eof = eof(&mut second);
        }
        assert!(Instant::now() < budget, "stalled readers never closed");
        thread::sleep(Duration::from_millis(5));
    }
    let gap = second_eof.unwrap().duration_since(first_eof.unwrap());
    assert!(
        gap >= Duration::from_millis(100) && gap <= Duration::from_millis(450),
        "independent deadlines should close ~200ms apart, got {gap:?}"
    );
}

/// C5 — the private writer seam covers the arms a real socket cannot
/// force deterministically: partial write, WouldBlock, elapsed
/// deadline, and hard failure — labeled honestly as a seam, not a
/// backpressure claim (30 bytes fit the kernel buffer instantly on a
/// real socket; a non-reading client never reaches AckPending).
struct ScriptedWriter {
    steps: VecDeque<io::Result<usize>>,
}

impl ScriptedWriter {
    fn scripted(steps: impl IntoIterator<Item = io::Result<usize>>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }
}

impl Write for ScriptedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.steps.pop_front() {
            Some(result) => result.map(|n| n.min(buf.len())),
            None => Ok(buf.len()),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn ack_writer_seam_covers_partial_wouldblock_elapsed() {
    let now = Instant::now();
    let deadline = now + CONNECTION_DEADLINE;

    // Partial progress accumulates across calls; Done at exactly 30.
    let mut writer = ScriptedWriter::scripted([Ok(10), Ok(10)]);
    let mut written = 0;
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut written, deadline, now),
        AckProgress::Pending
    ));
    assert_eq!(written, 10);
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut written, deadline, now),
        AckProgress::Pending
    ));
    assert_eq!(written, 20);
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut written, deadline, now),
        AckProgress::Done
    ));
    assert_eq!(written, 30);

    // WouldBlock stays pending without consuming progress.
    let mut writer =
        ScriptedWriter::scripted([Err(io::Error::new(io::ErrorKind::WouldBlock, "scripted"))]);
    let mut written = 0;
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut written, deadline, now),
        AckProgress::Pending
    ));
    assert_eq!(written, 0);

    // An elapsed deadline wins before any write is attempted.
    let mut writer = ScriptedWriter::scripted([]);
    let mut written = 0;
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut written, now, now),
        AckProgress::DeadlineExpired
    ));
    assert_eq!(written, 0);

    // A hard error and a zero write both fail closed.
    let mut writer = ScriptedWriter::scripted([Err(io::Error::other("scripted"))]);
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut 0, deadline, now),
        AckProgress::Failed
    ));
    let mut writer = ScriptedWriter::scripted([Ok(0)]);
    assert!(matches!(
        advance_ack_write(&mut writer, ACK, &mut 0, deadline, now),
        AckProgress::Failed
    ));
}

/// C6 — the post-admit deadline recheck through the injectable seam:
/// an accepted fold whose re-check instant is already past the shared
/// deadline destroys the connection with zero ACK bytes; a fresh
/// instant still enters AckPending. Rejected admission always
/// destroys.
#[test]
fn post_admit_recheck_gates_the_ack_arm() {
    let now = Instant::now();
    let deadline = now + CONNECTION_DEADLINE;
    assert!(matches!(
        state_after_admit(Some(ACK), now, deadline),
        Some(ConnState::AckPending {
            acknowledgement,
            written: 0
        }) if acknowledgement == ACK
    ));
    assert!(state_after_admit(Some(ACK), deadline, deadline).is_none());
    assert!(state_after_admit(Some(ACK), deadline + Duration::from_millis(1), deadline).is_none());
    assert!(state_after_admit(None, now, deadline).is_none());
}

/// U4 — the timeout arithmetic is a pure `min`: the liveness cap
/// alone bounds an idle loop, a connection deadline inside the cap
/// wins, an elapsed deadline tightens to zero — and the result is
/// always finite: `PollTimeout::NONE` is unreachable even with zero
/// connections and zero settlement timers.
#[test]
fn poll_timeout_is_min_of_conn_deadline_and_cap() {
    let service = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let now = Instant::now();

    // Idle: the cap alone bounds the wake — always finite.
    assert_eq!(
        poll_timeout(&[], &service, now).as_millis(),
        Some(POLL_LIVENESS_CAP.as_millis() as u32)
    );

    // A connection inside the cap wins; elapsed tightens to zero,
    // never negative.
    let (stream, _peer) = UnixStream::pair().unwrap();
    let conns = [Conn {
        stream,
        deadline: now + Duration::from_millis(10),
        state: ConnState::Reading { body: Vec::new() },
    }];
    assert_eq!(poll_timeout(&conns, &service, now).as_millis(), Some(10));
    assert_eq!(
        poll_timeout(&conns, &service, now + Duration::from_millis(20)).as_millis(),
        Some(0)
    );
}

/// The accept drain is a bounded batch: a backlog deeper than the
/// per-iteration budget stops at the budget and leaves the remainder
/// queued for the next bounded iteration — a continuously fed queue
/// can never starve the stop pipe, the settlement tick, or live
/// connection work. Deterministic shape: three queued peers, budget
/// two — no flood.
#[test]
fn accept_batch_stops_at_the_iteration_budget() {
    let dir = tmp_dir("accept-batch");
    let path = dir.join("batch.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let _clients = [
        UnixStream::connect(&path).unwrap(),
        UnixStream::connect(&path).unwrap(),
        UnixStream::connect(&path).unwrap(),
    ];
    let mut conns = Vec::new();
    // Budget two: exactly two accepted, the third stays queued.
    accept_batch(&listener, &mut conns, 2).unwrap();
    assert_eq!(conns.len(), 2);
    // The remainder drains on the next bounded iteration; the empty
    // queue then reports drained rather than erroring.
    accept_batch(&listener, &mut conns, 2).unwrap();
    assert_eq!(conns.len(), 3);
    accept_batch(&listener, &mut conns, 2).unwrap();
    assert_eq!(conns.len(), 3);
}
