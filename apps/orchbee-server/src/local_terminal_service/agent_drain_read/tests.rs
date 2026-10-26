//! Focused coverage for the staged Agent drain read pump.
//!
//! Every fixture composes REAL staged surfaces — `WorkerRegistry` +
//! `register_carrier`, the registered wire owner on a real
//! `UnixStream::pair` driven through the PRODUCTION `run()` entry
//! (`run_configured_for_test` skips `configure_idle_stream` and is for
//! fake streams only — Agent43), a real `SiteCoordinator` fed through
//! `prepare_terminal_tab_create`/`complete_terminal_tab_create`, and a real `AgentService`. The only fake
//! element is the remote CNP peer, and it HOLDS the socket open on an
//! owned channel until the pump's result is in hand — closing early
//! could retire the carrier before the pump's final fold.
//!
//! The expected Agent state is DERIVED, never seeded: the canonical
//! correlation hint flows through a real contiguous drain page, the
//! rich observation joins through the real `admit_hook_envelope` path,
//! and the presentation binding comes from the real
//! `agent_registration_snapshot` projection — the same hint/envelope
//! literals as the accepted native-leaf oracle.
use orchbee_local_ipc::LocalStream as UnixStream;

use super::*;

use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};

use serde_json::json;

use crate::{
    agent_service::{AgentPolicy, AgentReadFault},
    local_terminal_service::{LocalTerminalService, lease_expiry},
    site_coordinator::PreparedTerminalTabCreate,
    terminal_coordinator::{Completion, WorkerPlacement},
    worker_obcn_carrier::{WorkerWireExit, create_worker_carrier},
    worker_registry::{RegisteredWorkerWireOwner, WorkerAdmission},
};
use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentLifecycle, AgentListOutcome, AgentPresentationVisibility,
    AgentReadRequest, AgentReadResult, AgentSafeInteger, AgentSessionId, AgentUnavailableReason,
    ObapRequestId,
};
use orchbee_capability_node_protocol::{
    AgentDrainEntry, AgentDrainReply, CapabilityNodeMessage, HostCreationId, HostEpoch,
    HostLaunchAccepted, HostLaunchState, HostTerminalId, HostTerminalIdentity, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchResult, WorkerGeneration, WorkerId, read_message,
    write_message,
};
use orchbee_client_server_protocol::RequestId;
use orchbee_local_site_protocol::WorkerId as LspWorkerId;

const WORKER_BYTES: [u8; 16] = [9; 16];
const RETRY: Duration = Duration::from_millis(1);
const WINDOW: Duration = Duration::from_secs(30);

fn cnp_worker() -> WorkerId {
    WorkerId::from_bytes(WORKER_BYTES).unwrap()
}

fn lsp_worker() -> LspWorkerId {
    LspWorkerId::from_bytes(WORKER_BYTES)
}

fn carrier(generation: NonZeroU64) -> CarrierIdentity {
    CarrierIdentity {
        worker_id: cnp_worker(),
        generation: WorkerGeneration::new(generation),
    }
}

/// The one host identity shared by the launch completion and the drained
/// event — the binding reconciliation keys on it. Oracle literals: the
/// creation id "01"*16 surfaces as the attachment generation.
fn host_identity() -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([0x01; 16]).unwrap(),
        HostEpoch::from_bytes([0x11; 16]).unwrap(),
        HostTerminalId::from_bytes([0x21; 16]).unwrap(),
    )
}

fn hint_json(correlation: u64, event: u64) -> String {
    format!(
        "{{\"correlationId\":\"correlation-{correlation:016}\",\
         \"eventId\":\"event-{event:016}\",\
         \"kind\":\"correlation\",\"version\":1}}"
    )
}

fn envelope_json(correlation: u64, event: u64) -> Vec<u8> {
    json!({
        "adapterId": "codex",
        "correlations": [{
            "correlationId": format!("correlation-{correlation:016}"),
            "visibility": "visible",
        }],
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

fn listed(result: AgentReadResult) -> (Vec<String>, u64) {
    let AgentReadResult::List { result, .. } = result else {
        panic!("expected a list result")
    };
    let AgentListOutcome::Listed {
        items, revision, ..
    } = result.outcome
    else {
        panic!("expected a listed outcome")
    };
    (
        items
            .iter()
            .map(|item| item.session_id.as_str().to_owned())
            .collect(),
        revision.value(),
    )
}

fn unavailable(result: AgentReadResult) -> AgentUnavailableReason {
    let AgentReadResult::List { result, .. } = result else {
        panic!("expected a list result")
    };
    let AgentListOutcome::Unavailable { reason } = result.outcome else {
        panic!("expected an unavailable outcome")
    };
    reason
}

fn registry() -> Arc<Mutex<WorkerRegistry>> {
    Arc::new(Mutex::new(WorkerRegistry::new()))
}

fn admit_generation(workers: &Arc<Mutex<WorkerRegistry>>) -> NonZeroU64 {
    match workers.lock().unwrap().admit(lsp_worker()) {
        WorkerAdmission::Accepted { generation } => generation,
        other => panic!("worker was not admitted: {other:?}"),
    }
}

fn register(
    workers: &Arc<Mutex<WorkerRegistry>>,
    generation: NonZeroU64,
) -> RegisteredWorkerWireOwner {
    WorkerRegistry::register_carrier(
        workers,
        generation,
        create_worker_carrier(lsp_worker(), generation),
    )
    .unwrap_or_else(|| panic!("carrier was not registered"))
}

/// One completed member terminal through the real coordinator APIs:
/// `prepare_terminal_tab_create` dispatch -> `AuthorizedTerminalLaunch` command ->
/// `Completion::Launch` with the shared host identity. Returns the
/// presentation id the REAL registration projection recorded.
fn open_member(service: &LocalTerminalService, generation: NonZeroU64) -> String {
    let worker = WorkerPlacement {
        worker_id: cnp_worker(),
        generation: WorkerGeneration::new(generation),
    };
    let mut coordinator = service.coordinator.lock().unwrap();
    let snapshot = coordinator.close_authority().unwrap().snapshot().clone();
    let command = orchbee_workbench_protocol::WorkbenchCommand {
        protocol_version: orchbee_workbench_protocol::ProtocolVersion,
        request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(
            "11111111111111111111111111111111",
        )
        .unwrap(),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: orchbee_workbench_protocol::WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: orchbee_workbench_protocol::WorkbenchLabel::parse("Agent").unwrap(),
            geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 8, 16)
                .unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let PreparedTerminalTabCreate::Dispatch(pending) = coordinator.prepare_terminal_tab_create(
        command,
        Some(RequestId::parse("22222222222222222222222222222222").unwrap()),
        lease_expiry(),
        Some(worker),
    ) else {
        panic!("Workspace tab dispatch expected")
    };
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch command expected")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("immediate launch expected")
    };
    let result = TerminalLaunchResult::new(
        launch.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            host_identity(),
            HostLaunchState::Started(geometry),
        )),
    )
    .unwrap();
    coordinator
        .complete_terminal_tab_create(pending, Completion::Launch(result), Some(worker))
        .unwrap();
    let facts = coordinator.agent_registration_snapshot().unwrap();
    assert_eq!(facts.len(), 1, "one completed member expected");
    assert_eq!(facts[0].worker_id(), cnp_worker());
    assert_eq!(
        facts[0].worker_generation(),
        WorkerGeneration::new(generation)
    );
    assert_eq!(facts[0].identity(), host_identity());
    facts[0].presentation_id().as_str().to_owned()
}

/// Runs the real registered wire owner on one socketpair end through the
/// production `run()` entry and returns the peer end plus the owner
/// thread.
fn spawn_owner(owner: RegisteredWorkerWireOwner) -> (UnixStream, JoinHandle<WorkerWireExit>) {
    let (server, peer) = UnixStream::pair().unwrap();
    (peer, thread::spawn(move || owner.run(server)))
}

/// A scripted CNP peer: answers each incoming drain request through the
/// supplied responder, then HOLDS THE WIRE OPEN on `close` until the
/// pump's result is in hand — closing early could retire the carrier
/// before the pump's final fold.
fn scripted_peer(
    mut peer: UnixStream,
    exchanges: usize,
    respond: impl Fn(&AgentDrainRequest) -> AgentDrainReply + Send + 'static,
    close: mpsc::Receiver<()>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        for _ in 0..exchanges {
            let observed = read_message(&mut peer).unwrap();
            let CapabilityNodeMessage::AgentDrainRequest(request) = observed else {
                panic!("unexpected peer frame: {observed:?}")
            };
            write_message(
                &mut peer,
                &CapabilityNodeMessage::AgentDrainReply(respond(&request)),
            )
            .unwrap();
        }
        close.recv().unwrap();
    })
}

fn list_via_pump(
    service: &LocalTerminalService,
    agents: &AgentService,
    deadline: Instant,
    seed: u8,
) -> Result<AgentReadResult, AgentReadFault> {
    agents.execute_prepared_for_test(list_request(seed), || {
        agent_drain_prepare(
            service.workers.as_ref(),
            &service.coordinator,
            agents,
            deadline,
            RETRY,
        )
    })
}

/// The primary composition proof: real registry + real Worker wire + real
/// coordinator + the canonical Agent leaf. A contiguous sequence-1 page
/// carries the real correlation hint; the real projection supplies the
/// binding; the rich envelope joins; a second fresh exchange serves the
/// derived member — asserted exactly through List and Get.
#[test]
fn drain_read_serves_the_canonical_member() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let owner = register(&workers, generation);
    let service = LocalTerminalService::new(workers);
    let presentation = open_member(&service, generation);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents
        .native_admit_carrier(carrier(generation), Instant::now())
        .unwrap()
        .unwrap();

    let (peer, owner_thread) = spawn_owner(owner);
    let (close_tx, close_rx) = mpsc::channel();
    let calls = AtomicU64::new(0);
    let peer_thread = scripted_peer(
        peer,
        2,
        move |request| {
            let reply = match calls.fetch_add(1, Ordering::SeqCst) {
                0 => AgentDrainReply::new(
                    request.token,
                    request.acknowledged,
                    1,
                    vec![AgentDrainEntry::Event {
                        sequence: NonZeroU64::MIN,
                        identity: host_identity(),
                        payload: hint_json(1, 1),
                    }],
                ),
                _ => {
                    // This is a SECOND pump's first exchange: `acknowledged`
                    // is the pump-local w_obs, which starts at 0 on a fresh
                    // read — a stale-low ack is a legal re-offer (the leaf
                    // dedups covered sequences).
                    assert_eq!(request.acknowledged, 0);
                    AgentDrainReply::new(request.token, request.acknowledged, 1, Vec::new())
                }
            };
            reply.unwrap()
        },
        close_rx,
    );

    // READ 1 — drains the hint event, reconciles the real binding inside
    // the fenced final fold, and serves the pre-join snapshot: the hint
    // replayed under the real binding, no session listed yet.
    let first = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    let (items, _revision) = listed(first);
    assert!(items.is_empty(), "a held hint alone cannot mint a member");
    assert_eq!(agents.native_held_count().unwrap(), 0);

    // The matching rich observation joins through the real envelope path.
    assert!(
        agents
            .admit_hook_envelope(&envelope_json(1, 1), 1000.0)
            .unwrap()
    );
    agents.advance_hook_settlement(50.0, 1050.0).unwrap();

    // READ 2 — a fresh exchange (empty dedup page); the fenced final
    // snapshot now projects the joined canonical member.
    let second = list_via_pump(&service, &agents, Instant::now() + WINDOW, 2).unwrap();
    let (items, revision) = listed(second);
    assert_eq!(items, vec!["desktop:agent:session:2".to_string()]);
    assert_eq!(revision, 2);

    // The peer stays open until both results are in hand; the owner then
    // exits on the controlled close.
    close_tx.send(()).unwrap();
    peer_thread.join().unwrap();
    assert_eq!(owner_thread.join().unwrap(), WorkerWireExit::PeerClosed);

    // Exact session/presentation/state through the production read path.
    let get = agents
        .execute(get_request(3, "desktop:agent:session:2"))
        .unwrap();
    let AgentReadResult::Get { result, .. } = get else {
        panic!("expected a get result")
    };
    let AgentGetOutcome::Found { session, .. } = result.outcome else {
        panic!("expected the session to be found")
    };
    let snapshot = session.snapshot;
    assert_eq!(snapshot.session_id.as_str(), "desktop:agent:session:2");
    assert_eq!(snapshot.provider_id.as_str(), "codex");
    assert_eq!(snapshot.lifecycle, AgentLifecycle::Live);
    assert_eq!(snapshot.presentation_attachments.len(), 1);
    let attachment = &snapshot.presentation_attachments[0];
    assert_eq!(attachment.presentation_id.as_str(), presentation);
    assert_eq!(attachment.generation.as_str(), "01".repeat(16));
    assert!(attachment.navigable);
    assert_eq!(attachment.visibility, AgentPresentationVisibility::Visible);
}

/// `NoCarrier` is the only true-none read: with a completed member on the
/// coordinator but no registry carrier, the pump serves the (empty)
/// snapshot instead of reporting Unavailable.
#[test]
fn no_carrier_serves_an_empty_list() {
    let service = LocalTerminalService::new(registry());
    let _presentation = open_member(&service, NonZeroU64::MIN);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    let (items, _revision) = listed(result);
    assert!(items.is_empty());
}

/// An admitted worker with no registered carrier is `CarrierUnavailable`,
/// never conflated with `NoCarrier`.
#[test]
fn pending_worker_admission_is_unavailable_not_no_carrier() {
    let workers = registry();
    let _generation = admit_generation(&workers);
    let service = LocalTerminalService::new(workers);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );
}

/// Carrier retirement + replacement between leaf admission and the drain
/// read: the leaf still fences on the retired generation and the
/// carrier-matched page is refused — typed `CarrierMismatch` folds to
/// Unavailable.
#[test]
fn carrier_rotation_between_admission_and_drain_is_fenced() {
    let workers = registry();
    let gen1 = admit_generation(&workers);
    let owner1 = register(&workers, gen1);
    let service = LocalTerminalService::new(workers.clone());
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents
        .native_admit_carrier(carrier(gen1), Instant::now())
        .unwrap()
        .unwrap();

    // Retire the first carrier (dropping the registered owner drops the
    // generation registration), then admit + register the replacement.
    drop(owner1);
    let gen2 = admit_generation(&workers);
    assert_ne!(gen1, gen2);
    let owner2 = register(&workers, gen2);

    let (peer, owner_thread) = spawn_owner(owner2);
    let (close_tx, close_rx) = mpsc::channel();
    let peer_thread = scripted_peer(
        peer,
        1,
        |request| AgentDrainReply::new(request.token, request.acknowledged, 0, Vec::new()).unwrap(),
        close_rx,
    );

    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );

    close_tx.send(()).unwrap();
    peer_thread.join().unwrap();
    owner_thread.join().unwrap();
}

/// An unanswered exchange waits only until the deadline and reports
/// Unavailable; the abandoned exchange is discardable and the carrier
/// survives.
#[test]
fn unanswered_exchange_reports_unavailable_at_the_deadline() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let _owner = register(&workers, generation);
    let service = LocalTerminalService::new(workers);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents
        .native_admit_carrier(carrier(generation), Instant::now())
        .unwrap()
        .unwrap();

    let result = list_via_pump(
        &service,
        &agents,
        Instant::now() + Duration::from_millis(50),
        1,
    )
    .unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );

    let registry = service.workers.lock().unwrap();
    assert!(matches!(
        registry.agent_drain_carrier(),
        AgentDrainCarrier::Live(_)
    ));
}

/// An already-expired deadline refuses before any exchange: no token is
/// ever minted, so the shared mint floor still sits at one.
#[test]
fn expired_deadline_refuses_before_any_exchange() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let _owner = register(&workers, generation);
    let service = LocalTerminalService::new(workers);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents
        .native_admit_carrier(carrier(generation), Instant::now())
        .unwrap()
        .unwrap();

    let result = list_via_pump(
        &service,
        &agents,
        Instant::now() - Duration::from_millis(1),
        1,
    )
    .unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );

    let registry = service.workers.lock().unwrap();
    let AgentDrainCarrier::Live(target) = registry.agent_drain_carrier() else {
        panic!("carrier must still be live")
    };
    assert_eq!(target.mint.floor(), Some(1));
}

/// A structurally valid page whose carrier does not match the leaf is
/// refused at the fold — the page never mints a barrier.
#[test]
fn unadmitted_leaf_refuses_a_valid_page() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let owner = register(&workers, generation);
    let service = LocalTerminalService::new(workers);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    // No `native_admit_carrier` — the leaf has no carrier tuple, so the
    // fold fences the valid reply as `CarrierMismatch`.
    let (peer, owner_thread) = spawn_owner(owner);
    let (close_tx, close_rx) = mpsc::channel();
    let peer_thread = scripted_peer(
        peer,
        1,
        |request| {
            AgentDrainReply::new(
                request.token,
                request.acknowledged,
                1,
                vec![AgentDrainEntry::Event {
                    sequence: NonZeroU64::MIN,
                    identity: host_identity(),
                    payload: hint_json(1, 1),
                }],
            )
            .unwrap()
        },
        close_rx,
    );

    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );

    close_tx.send(()).unwrap();
    peer_thread.join().unwrap();
    owner_thread.join().unwrap();
}

/// An incomplete page (coverage below the reported queue tail) forces a
/// second exchange; the pump serves only once the barrier is met, and
/// the next request echoes the last applied watermark.
#[test]
fn an_incomplete_page_forces_a_second_exchange_before_serving() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let owner = register(&workers, generation);
    let service = LocalTerminalService::new(workers);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents
        .native_admit_carrier(carrier(generation), Instant::now())
        .unwrap()
        .unwrap();

    let (peer, owner_thread) = spawn_owner(owner);
    let (close_tx, close_rx) = mpsc::channel();
    let calls = AtomicU64::new(0);
    let peer_thread = scripted_peer(
        peer,
        2,
        move |request| {
            let reply = match calls.fetch_add(1, Ordering::SeqCst) {
                // tail 5 but coverage only reaches 1 — the pump must not
                // serve yet, and the barrier pins at 5 from this reply.
                0 => AgentDrainReply::new(
                    request.token,
                    request.acknowledged,
                    5,
                    vec![AgentDrainEntry::Event {
                        sequence: NonZeroU64::MIN,
                        identity: host_identity(),
                        payload: hint_json(1, 1),
                    }],
                ),
                _ => {
                    assert_eq!(request.acknowledged, 1);
                    AgentDrainReply::new(
                        request.token,
                        request.acknowledged,
                        5,
                        vec![
                            AgentDrainEntry::Event {
                                sequence: NonZeroU64::new(2).unwrap(),
                                identity: host_identity(),
                                payload: hint_json(2, 2),
                            },
                            AgentDrainEntry::Gap {
                                low: NonZeroU64::new(3).unwrap(),
                                high: 5,
                            },
                        ],
                    )
                }
            };
            reply.unwrap()
        },
        close_rx,
    );

    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    let (items, _revision) = listed(result);
    assert!(items.is_empty());

    close_tx.send(()).unwrap();
    peer_thread.join().unwrap();
    owner_thread.join().unwrap();
}

/// A poisoned AgentState cell maps the final fold fault to the existing
/// typed `RuntimeUnavailable` wire arm.
#[test]
fn poisoned_agent_state_reports_unavailable() {
    let service = LocalTerminalService::new(registry());
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    assert!(
        agents
            .native_poison_after_mutation_for_test(carrier(NonZeroU64::MIN), Instant::now())
            .is_err()
    );
    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );
}

/// A disposed runtime reports `Ok(Ok(None))` at the final fold — the pump
/// maps it to Unavailable like the existing snapshot path.
#[test]
fn disposed_runtime_reports_unavailable() {
    let service = LocalTerminalService::new(registry());
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents.shutdown();
    let result = list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    assert_eq!(
        unavailable(result),
        AgentUnavailableReason::RuntimeUnavailable
    );
}

/// After the worker carrier retires — and the leaf's carrier tuple is
/// retired through the same staged seam the production close path will
/// drive — a NoCarrier read still serves the retained canonical store.
#[test]
fn retired_carrier_serves_the_retained_snapshot() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let owner = register(&workers, generation);
    let service = LocalTerminalService::new(workers);
    let presentation = open_member(&service, generation);
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    agents
        .native_admit_carrier(carrier(generation), Instant::now())
        .unwrap()
        .unwrap();

    let (peer, owner_thread) = spawn_owner(owner);
    let (close_tx, close_rx) = mpsc::channel();
    let calls = AtomicU64::new(0);
    let peer_thread = scripted_peer(
        peer,
        2,
        move |request| {
            let reply = match calls.fetch_add(1, Ordering::SeqCst) {
                0 => AgentDrainReply::new(
                    request.token,
                    request.acknowledged,
                    1,
                    vec![AgentDrainEntry::Event {
                        sequence: NonZeroU64::MIN,
                        identity: host_identity(),
                        payload: hint_json(1, 1),
                    }],
                ),
                _ => AgentDrainReply::new(request.token, request.acknowledged, 1, Vec::new()),
            };
            reply.unwrap()
        },
        close_rx,
    );

    list_via_pump(&service, &agents, Instant::now() + WINDOW, 1).unwrap();
    assert!(
        agents
            .admit_hook_envelope(&envelope_json(1, 1), 1000.0)
            .unwrap()
    );
    agents.advance_hook_settlement(50.0, 1050.0).unwrap();
    let (items, _revision) =
        listed(list_via_pump(&service, &agents, Instant::now() + WINDOW, 2).unwrap());
    assert_eq!(items.len(), 1);

    // Release the peer, close the wire — joining the owner drops the
    // registered owner inside `run`, which retires the carrier — and
    // retire the leaf's carrier tuple through the same staged seam the
    // production close path will drive.
    close_tx.send(()).unwrap();
    peer_thread.join().unwrap();
    owner_thread.join().unwrap();
    {
        let registry = service.workers.lock().unwrap();
        assert!(matches!(
            registry.agent_drain_carrier(),
            AgentDrainCarrier::NoCarrier
        ));
    }
    agents
        .native_retire_carrier(carrier(generation), Instant::now())
        .unwrap();

    // The registry now reads NoCarrier and the leaf no-carrier fence
    // matches — the retained store still serves the member.
    let (items, _revision) =
        listed(list_via_pump(&service, &agents, Instant::now() + WINDOW, 3).unwrap());
    assert_eq!(items.len(), 1);
    let get = agents.execute(get_request(4, &items[0])).unwrap();
    let AgentReadResult::Get { result, .. } = get else {
        panic!("expected a get result")
    };
    let AgentGetOutcome::Found { session, .. } = result.outcome else {
        panic!("expected the session to be found")
    };
    assert_eq!(
        session.snapshot.presentation_attachments[0]
            .presentation_id
            .as_str(),
        presentation
    );
}

#[test]
fn names_finalization_keeps_registry_and_coordinator_guards_until_leaf_commit() {
    let service = LocalTerminalService::new(registry());
    let agents = AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })
    .unwrap();
    let value = agent_drain_finish(
        &service.workers,
        &service.coordinator,
        &agents,
        Instant::now() + WINDOW,
        RETRY,
        |_, snapshot| {
            assert!(matches!(
                service.workers.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
            assert!(matches!(
                service.coordinator.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
            assert!(snapshot.sessions.is_empty());
            Ok(42)
        },
    )
    .unwrap();
    assert_eq!(value, 42);
    assert!(service.workers.try_lock().is_ok());
    assert!(service.coordinator.try_lock().is_ok());
}
