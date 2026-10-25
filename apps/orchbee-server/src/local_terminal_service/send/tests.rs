use super::*;
use crate::local_terminal_service::lease_expiry;
use crate::site_coordinator::PreparedTerminalTabCreate;
use crate::terminal_coordinator::{Completion, WorkerPlacement};
use crate::worker_obcn_carrier::create_worker_carrier;
use crate::worker_registry::{WorkerAdmission, WorkerRegistry};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, SendCleanupStatus as WorkerCleanup,
    SendProgress as WorkerProgress, SendRecord as WorkerRecord, SendRecords as WorkerRecords,
    SendRejection as WorkerRejection, SendText as WorkerText, SendTextInput as WorkerTextInput,
    SendUnavailable as WorkerUnavailable, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchResult, TerminalSendOutcome as WorkerOutcome,
    TerminalSendRequest as WorkerRequest, TerminalSendResult as WorkerResult, WorkerGeneration,
    read_message, write_message,
};
use orchbee_client_server_protocol::{
    ClientMessage, RequestId, SendKey, SendProgress, SendRecord, SendRecords, SendText,
    SendTextInput, ServerMessage, SiteId as ClientSiteId, TerminalId as ClientTerminalId,
    TerminalSendRequest, WorkerId as ClientWorkerId,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_workbench_protocol::{
    ProtocolVersion, SiteId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchLabel, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchTerminalGeometry,
};
use std::error::Error;
use std::num::NonZeroU64;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
const WORKER_BYTES: [u8; 16] = [9; 16];

struct Fixture {
    service: LocalTerminalService,
    requests: Mutex<mpsc::Receiver<WorkerRequest>>,
    replies: Mutex<Option<mpsc::Sender<CapabilityNodeMessage>>>,
    stop: Mutex<UnixStream>,
    owner: Mutex<Option<JoinHandle<crate::worker_obcn_carrier::WorkerWireExit>>>,
    peer: Mutex<Option<JoinHandle<()>>>,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
        let worker_id = orchbee_local_site_protocol::WorkerId::from_bytes(WORKER_BYTES);
        let WorkerAdmission::Accepted { generation } = registry
            .lock()
            .map_err(|_| "registry poisoned")?
            .admit(worker_id)
        else {
            return Err("worker admission".into());
        };
        let owner = WorkerRegistry::register_carrier(
            &registry,
            generation,
            create_worker_carrier(worker_id, generation),
        )
        .ok_or("carrier registration")?;
        let (server, peer) = UnixStream::pair()?;
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        let stop = server.try_clone()?;
        let (requests, worker_requests) = mpsc::channel();
        let (worker_replies, replies) = mpsc::channel();
        let owner = thread::spawn(move || owner.run(server));
        let peer = thread::spawn(move || run_worker(peer, requests, replies));
        let site_id = SiteId::parse(&format!("{:032x}", 1))?;
        let service = LocalTerminalService::from_identity(registry, Ok(site_id));
        Ok(Self {
            service,
            requests: Mutex::new(worker_requests),
            replies: Mutex::new(Some(worker_replies)),
            stop: Mutex::new(stop),
            owner: Mutex::new(Some(owner)),
            peer: Mutex::new(Some(peer)),
        })
    }

    fn reply(&self, outcome: WorkerOutcome) -> TestResult<WorkerRequest> {
        let request = self.requests.lock().map_err(|_| "poisoned")?.recv()?;
        let reply = CapabilityNodeMessage::TerminalSendResult(WorkerResult::new(
            request.request_id,
            outcome,
        )?);
        self.replies
            .lock()
            .map_err(|_| "poisoned")?
            .as_ref()
            .ok_or("reply channel closed")?
            .send(reply)?;
        Ok(request)
    }

    fn add_terminal(&self, index: u8, started: bool) -> TestResult<WorkbenchCommand> {
        let snapshot = read_snapshot(&self.service)?;
        let create = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: workbench_id(10 + u128::from(index) * 100),
            site_id: snapshot.site_id.clone(),
            expected_revision: snapshot.revision,
            action: WorkbenchAction::TabCreate {
                placement: None,
                automatic_label: false,
                terminal_context_id: snapshot.navigation.standalone_terminal_context_id,
                label: WorkbenchLabel::parse("send routing")?,
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        };
        let mut site = self.service.coordinator.lock().map_err(|_| "poisoned")?;
        let PreparedTerminalTabCreate::Dispatch(pending) = site.prepare_terminal_tab_create(
            create,
            Some(client_id(11 + u128::from(index) * 100)),
            lease_expiry(),
            Some(placement_value()),
        ) else {
            return Err("create pending".into());
        };
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
            return Err("launch".into());
        };
        let identity = HostTerminalIdentity::new(
            HostCreationId::from_bytes([3 + index; 16])?,
            HostEpoch::from_bytes([4; 16])?,
            HostTerminalId::from_bytes([5 + index; 16])?,
        );
        let state = if started {
            let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
                return Err("geometry".into());
            };
            HostLaunchState::Started(geometry)
        } else {
            HostLaunchState::AwaitingStart
        };
        let result = TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(identity, state)),
        )?;
        site.complete_terminal_tab_create(
            pending,
            Completion::Launch(result),
            Some(placement_value()),
        )
        .map_err(|_| "create completion")?;
        drop(site);
        let snapshot = read_snapshot(&self.service)?;
        let tab_id = snapshot.terminal_contexts[0]
            .tabs
            .last()
            .ok_or("created tab")?
            .tab_id
            .clone();
        Ok(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: workbench_id(20 + u128::from(index) * 100),
            site_id: snapshot.site_id,
            expected_revision: snapshot.revision,
            action: WorkbenchAction::TabClose {
                terminal_context_id: snapshot.navigation.standalone_terminal_context_id,
                tab_id,
            },
        })
    }

    fn selector(&self, index: u8) -> TestResult<(ClientSiteId, ClientWorkerId, ClientTerminalId)> {
        let snapshot = read_snapshot(&self.service)?;
        let binding = snapshot
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .filter_map(|tab| match &tab.root {
                orchbee_workbench_protocol::PanelNode::Panel {
                    primary_presentation,
                    ..
                } => primary_presentation.binding.as_ref(),
                _ => None,
            })
            .nth(usize::from(index))
            .ok_or("exposed binding")?;
        Ok((
            ClientSiteId::parse(snapshot.site_id.as_str())?,
            ClientWorkerId::parse(&format!("{:032x}", u128::from_be_bytes(WORKER_BYTES)))?,
            ClientTerminalId::parse(binding.terminal_id.as_str())?,
        ))
    }

    fn fence(&self, close: &mut WorkbenchCommand) -> TestResult {
        close.expected_revision = read_snapshot(&self.service)?.revision;
        let result = self
            .service
            .coordinator
            .lock()
            .map_err(|_| "poisoned")?
            .execute_terminal_tab_close(
                close.clone(),
                Some(placement_value()),
                true,
                std::time::Instant::now(),
            )
            .ok_or("close execution")?;
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::TabCloseAccepted { .. }
        ));
        Ok(())
    }

    fn kill_carrier(&self) -> TestResult {
        self.stop
            .lock()
            .map_err(|_| "poisoned")?
            .shutdown(std::net::Shutdown::Both)?;
        if let Some(owner) = self.owner.lock().map_err(|_| "poisoned")?.take() {
            owner.join().map_err(|_| "owner panicked")?;
        }
        Ok(())
    }

    fn replace_worker(&self) -> TestResult {
        self.kill_carrier()?;
        let registry = &self.service.workers;
        let worker_id = orchbee_local_site_protocol::WorkerId::from_bytes(WORKER_BYTES);
        let WorkerAdmission::Accepted { generation } =
            registry.lock().map_err(|_| "poisoned")?.admit(worker_id)
        else {
            return Err("replacement admission".into());
        };
        assert_eq!(generation.get(), 2);
        let owner = WorkerRegistry::register_carrier(
            registry,
            generation,
            create_worker_carrier(worker_id, generation),
        )
        .ok_or("replacement carrier")?;
        let (server, peer) = UnixStream::pair()?;
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        *self.stop.lock().map_err(|_| "poisoned")? = server.try_clone()?;
        *self.owner.lock().map_err(|_| "poisoned")? =
            Some(thread::spawn(move || owner.run(server)));
        let (requests, worker_requests) = mpsc::channel();
        let (worker_replies, replies) = mpsc::channel();
        *self.peer.lock().map_err(|_| "poisoned")? =
            Some(thread::spawn(move || run_worker(peer, requests, replies)));
        *self.requests.lock().map_err(|_| "poisoned")? = worker_requests;
        *self.replies.lock().map_err(|_| "poisoned")? = Some(worker_replies);
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(stop) = self.stop.get_mut() {
            let _ = stop.shutdown(std::net::Shutdown::Both);
        }
        if let Ok(owner) = self.owner.get_mut()
            && let Some(owner) = owner.take()
        {
            let _ = owner.join();
        }
        if let Ok(peer) = self.peer.get_mut()
            && let Some(peer) = peer.take()
        {
            let _ = peer.join();
        }
    }
}

fn run_worker(
    mut peer: UnixStream,
    requests: mpsc::Sender<WorkerRequest>,
    replies: mpsc::Receiver<CapabilityNodeMessage>,
) {
    loop {
        let Ok(message) = read_message(&mut peer) else {
            return;
        };
        let CapabilityNodeMessage::TerminalSendRequest(request) = message else {
            return;
        };
        if requests.send(request).is_err() {
            return;
        }
        let Ok(reply) = replies.recv() else {
            return;
        };
        if write_message(&mut peer, &reply).is_err() {
            return;
        }
    }
}

fn placement_value() -> WorkerPlacement {
    WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes(WORKER_BYTES)
            .expect("worker id"),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    }
}

fn client_id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).expect("request id")
}
fn workbench_id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).expect("workbench id")
}

fn read_snapshot(
    service: &LocalTerminalService,
) -> TestResult<orchbee_workbench_protocol::Snapshot> {
    let request = WorkbenchRequest::SnapshotRead(orchbee_workbench_protocol::SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(9000),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(request.try_into().map_err(|_| "workbench request")?, false)
        .ok_or("snapshot result")?
    else {
        return Err("snapshot result kind".into());
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        return Err("snapshot availability".into());
    };
    Ok(snapshot)
}

fn send_request(
    fixture: &Fixture,
    index: u8,
    records: SendRecords,
) -> TestResult<TerminalSendRequest> {
    let (site_id, worker_id, terminal_id) = fixture.selector(index)?;
    Ok(TerminalSendRequest::new(
        client_id(30 + u128::from(index) * 100),
        site_id,
        worker_id,
        terminal_id,
        records,
    ))
}

fn send(service: &LocalTerminalService, request: TerminalSendRequest) -> TerminalSendResult {
    let ServerMessage::TerminalSendResult(result) =
        service.execute(ClientMessage::TerminalSend(request))
    else {
        panic!("send must produce a send result")
    };
    result
}

fn one_text(value: &str, enter: bool) -> TestResult<SendRecords> {
    Ok(SendRecords::new(vec![SendRecord::Text(
        SendTextInput::new(SendText::new(value.to_owned())?, enter)?,
    )])?)
}

fn one_key(key: SendKey) -> TestResult<SendRecords> {
    Ok(SendRecords::new(vec![SendRecord::Stroke(
        orchbee_client_server_protocol::SendKeyStroke::new(
            orchbee_client_server_protocol::SendStrokeKey::Named(key),
            0,
        )?,
    )])?)
}

fn hex_id(value: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(value))
}

#[test]
fn send_to_started_terminal_delivers_accepted_with_the_pinned_target() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    let request = send_request(&fixture, 0, one_text("echo hi", true)?)?;
    thread::scope(|scope| -> TestResult {
        let execution = scope.spawn(|| send(&fixture.service, request));
        let delivered = fixture.reply(WorkerOutcome::Accepted {
            progress: WorkerProgress::complete(1, 8),
            cleanup: WorkerCleanup::Confirmed,
        })?;
        let result = execution.join().map_err(|_| "execute panicked")?;
        assert_eq!(
            delivered.worker_id.as_bytes(),
            WORKER_BYTES,
            "the pinned Worker identity reaches the wire"
        );
        assert_eq!(delivered.worker_generation.get(), 1);
        assert_eq!(
            delivered.target,
            HostTerminalIdentity::new(
                HostCreationId::from_bytes([3; 16])?,
                HostEpoch::from_bytes([4; 16])?,
                HostTerminalId::from_bytes([5; 16])?,
            ),
            "the exact Host incarnation is pinned on the request"
        );
        assert_ne!(delivered.request_id, delivered.attachment_id);
        assert_eq!(
            delivered.records,
            WorkerRecords::new(vec![WorkerRecord::Text(WorkerTextInput::new(
                WorkerText::new("echo hi".to_owned())?,
                true,
            )?)])?
        );
        let Outcome::Accepted { progress, cleanup } = result.outcome() else {
            panic!("accepted outcome")
        };
        assert_eq!(progress.records_accepted(), 1);
        assert_eq!(progress.accepted_encoded_bytes(), 8);
        assert_eq!(*cleanup, Cleanup::Confirmed);
        let target = result.target().expect("pinned send target");
        assert_eq!(target.worker_generation(), NonZeroU64::MIN);
        assert_eq!(target.host_epoch().as_str(), hex_id([4; 16]));
        assert_eq!(target.host_terminal_id().as_str(), hex_id([5; 16]));
        Ok(())
    })
}

#[test]
fn send_key_payload_maps_to_the_closed_key_set() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    let request = send_request(
        &fixture,
        0,
        SendRecords::new(vec![SendRecord::Stroke(
            orchbee_client_server_protocol::SendKeyStroke::new(
                orchbee_client_server_protocol::SendStrokeKey::Character('c'),
                1,
            )?,
        )])?,
    )?;
    thread::scope(|scope| -> TestResult {
        let execution = scope.spawn(|| send(&fixture.service, request));
        let delivered = fixture.reply(WorkerOutcome::Rejected {
            progress: WorkerProgress::none(),
            reason: WorkerRejection::HostRejected,
            cleanup: WorkerCleanup::Confirmed,
        })?;
        let result = execution.join().map_err(|_| "execute panicked")?;
        assert_eq!(
            delivered.records,
            WorkerRecords::new(vec![WorkerRecord::Stroke(
                orchbee_capability_node_protocol::SendKeyStroke::new(
                    orchbee_capability_node_protocol::SendStrokeKey::Character('c'),
                    1
                )?
            )])?
        );
        let Outcome::Rejected {
            reason, cleanup, ..
        } = result.outcome()
        else {
            panic!("rejected outcome")
        };
        assert_eq!(*reason, Rejected::HostRejected);
        assert_eq!(*cleanup, Cleanup::Confirmed);
        assert!(result.target().is_some());
        Ok(())
    })
}

#[test]
fn send_accepted_with_unconfirmed_cleanup_preserves_both_axes() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    let request = send_request(&fixture, 0, one_text("x", false)?)?;
    thread::scope(|scope| -> TestResult {
        let execution = scope.spawn(|| send(&fixture.service, request));
        fixture.reply(WorkerOutcome::Accepted {
            progress: WorkerProgress::complete(1, 0),
            cleanup: WorkerCleanup::Unconfirmed,
        })?;
        let result = execution.join().map_err(|_| "execute panicked")?;
        let Outcome::Accepted { progress, cleanup } = result.outcome() else {
            panic!("accepted outcome")
        };
        // A legal zero-byte admission is echoed verbatim; the attachment is
        // reported unconfirmed rather than released or unsent.
        assert_eq!(progress.records_accepted(), 1);
        assert_eq!(progress.accepted_encoded_bytes(), 0);
        assert_eq!(*cleanup, Cleanup::Unconfirmed);
        assert!(result.target().is_some());
        Ok(())
    })
}

#[test]
fn send_worker_unavailable_and_unknown_outcomes_keep_the_pinned_target() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    for (worker_outcome, expected) in [
        (
            WorkerOutcome::Unavailable {
                progress: WorkerProgress::none(),
                reason: WorkerUnavailable::HostUnavailable,
                cleanup: WorkerCleanup::NotNeeded,
            },
            Outcome::Unavailable {
                progress: SendProgress::none(),
                reason: Unavailable::HostUnavailable,
                cleanup: Cleanup::NotNeeded,
            },
        ),
        (
            WorkerOutcome::Unavailable {
                progress: WorkerProgress::none(),
                reason: WorkerUnavailable::HostProtocolFault,
                cleanup: WorkerCleanup::NotNeeded,
            },
            Outcome::Unavailable {
                progress: SendProgress::none(),
                reason: Unavailable::HostProtocolFault,
                cleanup: Cleanup::NotNeeded,
            },
        ),
        (
            WorkerOutcome::OutcomeUnknown {
                progress: WorkerProgress::none(),
                cleanup: WorkerCleanup::Unconfirmed,
            },
            Outcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: Cleanup::Unconfirmed,
            },
        ),
        (
            WorkerOutcome::Rejected {
                progress: WorkerProgress::none(),
                reason: WorkerRejection::WrongWorker,
                cleanup: WorkerCleanup::NotNeeded,
            },
            Outcome::Unavailable {
                progress: SendProgress::none(),
                reason: Unavailable::WorkerUnavailable,
                cleanup: Cleanup::NotNeeded,
            },
        ),
        (
            WorkerOutcome::Rejected {
                progress: WorkerProgress::none(),
                reason: WorkerRejection::StaleWorkerGeneration,
                cleanup: WorkerCleanup::Confirmed,
            },
            Outcome::Rejected {
                progress: SendProgress::none(),
                reason: Rejected::StaleWorkerGeneration,
                cleanup: Cleanup::Confirmed,
            },
        ),
    ] {
        let request = send_request(&fixture, 0, one_key(SendKey::Enter)?)?;
        thread::scope(|scope| -> TestResult {
            let execution = scope.spawn(|| send(&fixture.service, request));
            fixture.reply(worker_outcome)?;
            let result = execution.join().map_err(|_| "execute panicked")?;
            assert_eq!(result.outcome(), &expected);
            assert!(result.target().is_some());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn send_without_a_matching_binding_is_rejected_before_admission() -> TestResult {
    let fixture = Fixture::new()?;
    let request = TerminalSendRequest::new(
        client_id(30),
        ClientSiteId::parse(&format!("{:032x}", 1))?,
        ClientWorkerId::parse(&format!("{:032x}", u128::from_be_bytes(WORKER_BYTES)))?,
        ClientTerminalId::parse(&format!("{:032x}", 99))?,
        one_key(SendKey::Enter)?,
    );
    let result = send(&fixture.service, request);
    assert_eq!(
        result.outcome(),
        &Outcome::Rejected {
            progress: SendProgress::none(),
            reason: Rejected::TerminalUnavailable,
            cleanup: Cleanup::NotNeeded,
        }
    );
    assert!(result.target().is_none());
    Ok(())
}

#[test]
fn send_to_awaiting_start_is_rejected_before_admission() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, false)?;
    let request = send_request(&fixture, 0, one_key(SendKey::Enter)?)?;
    let result = send(&fixture.service, request);
    assert_eq!(
        result.outcome(),
        &Outcome::Rejected {
            progress: SendProgress::none(),
            reason: Rejected::TerminalUnavailable,
            cleanup: Cleanup::NotNeeded,
        }
    );
    assert!(result.target().is_none());
    Ok(())
}

#[test]
fn send_to_a_fenced_binding_is_rejected_before_admission() -> TestResult {
    let fixture = Fixture::new()?;
    let mut close = fixture.add_terminal(0, true)?;
    fixture.fence(&mut close)?;
    let request = send_request(&fixture, 0, one_key(SendKey::Enter)?)?;
    let result = send(&fixture.service, request);
    assert_eq!(
        result.outcome(),
        &Outcome::Rejected {
            progress: SendProgress::none(),
            reason: Rejected::TerminalUnavailable,
            cleanup: Cleanup::NotNeeded,
        }
    );
    assert!(result.target().is_none());
    Ok(())
}

#[test]
fn send_after_worker_replacement_reports_stale_runtime() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    let request = send_request(&fixture, 0, one_key(SendKey::Enter)?)?;
    fixture.replace_worker()?;
    let result = send(&fixture.service, request);
    assert_eq!(
        result.outcome(),
        &Outcome::Unavailable {
            progress: SendProgress::none(),
            reason: Unavailable::TerminalRuntimeUnavailable,
            cleanup: Cleanup::NotNeeded,
        }
    );
    assert!(result.target().is_none());
    Ok(())
}

#[test]
fn send_response_loss_after_delivery_reports_outcome_unknown() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    let request = send_request(&fixture, 0, one_key(SendKey::Enter)?)?;
    thread::scope(|scope| -> TestResult {
        let execution = scope.spawn(|| send(&fixture.service, request));
        let delivered = fixture.requests.lock().map_err(|_| "poisoned")?.recv()?;
        assert_eq!(delivered.target.terminal_id().as_bytes(), [5; 16]);
        // The Worker-side connection dies after the request was written, so the
        // outcome is honestly unknown rather than reported as unsent.
        drop(fixture.replies.lock().map_err(|_| "poisoned")?.take());
        let result = execution.join().map_err(|_| "execute panicked")?;
        assert_eq!(
            result.outcome(),
            &Outcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: Cleanup::Unconfirmed,
            }
        );
        assert!(result.target().is_some());
        Ok(())
    })
}

#[test]
fn send_on_a_dead_carrier_is_unavailable_and_not_reused() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.add_terminal(0, true)?;
    let request = send_request(&fixture, 0, one_key(SendKey::Enter)?)?;
    fixture.kill_carrier()?;
    // Once the carrier is retired the registry stops serving a handle, so each
    // send resolves to the same unavailable outcome instead of reusing a dead
    // stream.
    for _attempt in 0..2 {
        let result = send(&fixture.service, request.clone());
        assert_eq!(
            result.outcome(),
            &Outcome::Unavailable {
                progress: SendProgress::none(),
                reason: Unavailable::TerminalRuntimeUnavailable,
                cleanup: Cleanup::NotNeeded,
            }
        );
        assert!(result.target().is_none());
    }
    Ok(())
}
