//! A lost physical source must release the connection's capture owner before a sibling runs.
use super::super::Subscription;
use super::*;
use crate::{
    local_terminal_service::LocalTerminalService,
    terminal_coordinator::{BindingViewerTarget, WorkerPlacement},
    workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SemanticEventsAction, SemanticEventsRequest, SnapshotReadIntent,
    WorkbenchPanelId, WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome,
};
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex, atomic::Ordering, mpsc},
    time::Duration,
};

#[test]
fn a_source_lost_before_sample_releases_its_completed_job_for_a_sibling() {
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: false,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
            WorkerRegistry::new(),
        )))),
    );
    let id = WorkbenchRequestId::parse(&format!("{:032x}", 100)).unwrap();
    let WorkbenchExecution::Response(WorkbenchResponse::Snapshot(result)) =
        service.execute(WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: id.clone(),
        }))
    else {
        panic!()
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!()
    };
    let session = service.client_connection(NonZeroU64::MIN);
    let journal = service.terminals.event_journal().unwrap();
    let cursor = journal.head().unwrap();
    let mut capture = Capture::new(Arc::clone(&session.event_capture));
    let (finished, completion) = mpsc::sync_channel(1);
    capture
        .start(0, move |_| {
            finished.send(()).unwrap();
            Err(Failure::SourceChanged)
        })
        .unwrap();
    completion.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        session.event_capture.load(Ordering::Acquire),
        "completion is not consumption"
    );
    // This is the reachable post-removal cut: the old source no longer resolves, while its
    // earlier physical capture still belongs to A. The real observe path fails before sample().
    let mut subscription = Subscription {
        id: id.clone(),
        filters: vec![],
        journal_filters: vec![],
        definition_cursor: None,
        cursor,
        journal,
        next_probe: 0,
        probe_sequence: 0,
        capture,
        pending_failure: None,
        changed: Arc::default(),
        probes: vec![Probe::Output(Box::new(Output {
            source: Source {
                selector: SemanticEventSource {
                    panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 101)).unwrap(),
                    presentation_id: None,
                },
                presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 102)).unwrap(),
                expected: ProcessObservationTarget {
                    session_id: snapshot.session_id.clone(),
                    binding: orchbee_workbench_protocol::PresentationBinding {
                        worker_id: orchbee_workbench_protocol::WorkerId::parse(&format!(
                            "{:032x}",
                            1
                        ))
                        .unwrap(),
                        terminal_id: orchbee_workbench_protocol::TerminalId::parse(&format!(
                            "{:032x}",
                            2
                        ))
                        .unwrap(),
                    },
                    target: BindingViewerTarget {
                        terminal_id: orchbee_capability_node_protocol::TerminalId::from_bytes(
                            [2; 16],
                        )
                        .unwrap(),
                        worker: WorkerPlacement {
                            worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes(
                                [1; 16],
                            )
                            .unwrap(),
                            generation: WorkerGeneration::new(NonZeroU64::MIN),
                        },
                        identity: HostTerminalIdentity::new(
                            HostCreationId::from_bytes([3; 16]).unwrap(),
                            HostEpoch::from_bytes([4; 16]).unwrap(),
                            HostTerminalId::from_bytes([5; 16]).unwrap(),
                        ),
                    },
                },
            },
            kind: SemanticOutputSource::Recent,
            lines: 1,
            strip_ansi: true,
            matcher: Matcher::Substring("ready".into()),
            matching: false,
            observed: true,
            client: None,
            query: None,
        }))],
    };
    let request = SemanticEventsRequest {
        protocol_version: ProtocolVersion,
        request_id: id.clone(),
        session_id: snapshot.session_id,
        action: SemanticEventsAction::Read {
            subscription_id: id,
            limit: 1,
        },
    };
    assert!(matches!(
        subscription.read(&session, &request, 1, false),
        Err(Failure::NotFound)
    ));
    assert!(
        !session.event_capture.load(Ordering::Acquire),
        "failed A must cancel/join before returning"
    );
    let mut sibling = Capture::new(Arc::clone(&session.event_capture));
    let (started, reached) = mpsc::sync_channel(1);
    sibling
        .start(0, move |_| {
            started.send(()).unwrap();
            Err(Failure::Busy)
        })
        .unwrap();
    reached.recv_timeout(Duration::from_secs(2)).unwrap();
    sibling.cancel();
    assert!(!session.event_capture.load(Ordering::Acquire));
}
