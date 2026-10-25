use super::*;
mod tab_move;
use orchbee_client_server_protocol::{
    RequestId, SiteId, TerminalId, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerUnavailableReason, WorkerId,
};
use orchbee_workbench_protocol::parse_workbench_request;

fn binding_request() -> ClientMessage {
    ClientMessage::TerminalViewer(TerminalViewerRequest::new(
        RequestId::parse("33333333333333333333333333333333").unwrap(),
        TerminalViewerAction::AttachBinding {
            site_id: SiteId::parse("11111111111111111111111111111111").unwrap(),
            worker_id: WorkerId::parse("22222222222222222222222222222222").unwrap(),
            terminal_id: TerminalId::parse("44444444444444444444444444444444").unwrap(),
        },
    ))
}

fn binding_outcome(service: &LocalTerminalService) -> TerminalViewerOutcome {
    let ServerMessage::TerminalViewerResult(result) = service.session().execute(binding_request())
    else {
        unreachable!("binding request must produce a viewer result")
    };
    result.into_outcome()
}

#[test]
fn failed_site_initialization_preserves_unavailable_workbench_and_direct_terminal_service() {
    let service = LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::ErrorKind::PermissionDenied.into()),
    );
    let request = parse_workbench_request(br#"{"kind":"workbench.snapshot.read","protocolVersion":1,"requestId":"11111111111111111111111111111111"}"#).unwrap();
    for _attempt in 0..2 {
        assert!(
            service
                .execute_workbench(request.clone().try_into().unwrap(), true)
                .is_none()
        );
        let create = orchbee_client_server_protocol::TerminalCreateRequest::new(
            orchbee_client_server_protocol::RequestId::parse("22222222222222222222222222222222")
                .unwrap(),
            None,
            None,
            orchbee_client_server_protocol::StartMode::Deferred,
            None,
        );
        assert!(
            matches!(service.execute(ClientMessage::TerminalCreate(create)),
                ServerMessage::TerminalCreateResult(result)
                    if result.outcome() == CreateOutcome::Unavailable { reason: CreateUnavailableReason::NoCompatibleWorker }
            )
        );
    }
}

#[test]
fn binding_attach_distinguishes_missing_workbench_before_missing_worker() {
    let service = LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::ErrorKind::PermissionDenied.into()),
    );

    assert_eq!(
        binding_outcome(&service),
        TerminalViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::WorkbenchUnavailable,
        }
    );
}

#[test]
fn binding_attach_reports_registry_and_site_poison_before_workbench_availability() {
    let workers = Arc::new(Mutex::new(WorkerRegistry::new()));
    let poison = Arc::clone(&workers);
    assert!(
        std::thread::spawn(move || {
            let _guard = poison.lock().unwrap();
            panic!("poison registry for precedence witness");
        })
        .join()
        .is_err()
    );
    let service = LocalTerminalService::from_identity(
        workers,
        Err(std::io::ErrorKind::PermissionDenied.into()),
    );
    assert_eq!(
        binding_outcome(&service),
        TerminalViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::ServerBusy,
        }
    );

    let service = LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::ErrorKind::PermissionDenied.into()),
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = service.coordinator.lock().unwrap();
            panic!("poison Site coordinator for precedence witness");
        }))
        .is_err()
    );
    assert_eq!(
        binding_outcome(&service),
        TerminalViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::ServerBusy,
        }
    );
}

#[test]
fn close_site_lookup_returns_unavailable_for_missing_or_poisoned_site_owner() {
    use crate::workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService};
    use orchbee_workbench_protocol::{
        ProtocolVersion, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
        WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
        WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
        WorkbenchTabCloseUnavailableReason, WorkbenchTabId, WorkbenchTerminalContextId,
    };
    let site =
        orchbee_workbench_protocol::SiteId::parse("11111111111111111111111111111111").unwrap();
    let missing = Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::ErrorKind::PermissionDenied.into()),
    ));
    assert_eq!(missing.workbench_site_matches(&site), None);
    let service = Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(site.clone()),
    ));
    assert_eq!(service.workbench_site_matches(&site), Some(true));
    let poison = Arc::clone(&service);
    assert!(
        std::thread::spawn(move || {
            let _gate = poison.coordinator.lock().unwrap();
            panic!("poison Site for Close refusal witness");
        })
        .join()
        .is_err()
    );
    assert_eq!(service.workbench_site_matches(&site), None);
    for unavailable in [missing, service] {
        for allowed in [false, true] {
            let workbench = WorkbenchService::new(
                WorkbenchPolicy {
                    read: allowed,
                    manage_navigation: true,
                    associate_directory: false,
                    create_terminal: true,
                    manage_terminal_lifecycle: allowed,
                },
                Arc::clone(&unavailable),
            );
            let read = WorkbenchTabCloseRead {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse("22222222222222222222222222222222").unwrap(),
                site_id: site.clone(),
                terminal_context_id: WorkbenchTerminalContextId::parse(
                    "33333333333333333333333333333333",
                )
                .unwrap(),
                tab_id: WorkbenchTabId::parse("44444444444444444444444444444444").unwrap(),
            };
            let WorkbenchExecution::Response(WorkbenchResponse::TabCloseRead(result)) =
                workbench.execute(WorkbenchRequest::TabCloseRead(read.clone()))
            else {
                panic!("Close read result expected")
            };
            assert_eq!(
                result.outcome,
                if allowed {
                    WorkbenchTabCloseReadOutcome::Unavailable {
                        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
                    }
                } else {
                    WorkbenchTabCloseReadOutcome::Denied {}
                }
            );
            let resume = WorkbenchTabCloseResume {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                site_id: read.site_id,
                terminal_context_id: read.terminal_context_id,
                tab_id: read.tab_id,
                close_request_id: WorkbenchRequestId::parse("55555555555555555555555555555555")
                    .unwrap(),
                expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            };
            let WorkbenchExecution::Response(WorkbenchResponse::TabCloseResume(result)) =
                workbench.execute(WorkbenchRequest::TabCloseResume(resume.clone()))
            else {
                panic!("Close Resume result expected")
            };
            assert_eq!(
                result.outcome,
                if allowed {
                    WorkbenchTabCloseResumeOutcome::Unavailable {
                        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
                    }
                } else {
                    WorkbenchTabCloseResumeOutcome::Denied {}
                }
            );
            assert_eq!(result.request_id, resume.request_id);
            assert_eq!(result.site_id, resume.site_id);
            assert_eq!(result.terminal_context_id, resume.terminal_context_id);
            assert_eq!(result.tab_id, resume.tab_id);
            assert_eq!(result.close_request_id, resume.close_request_id);
        }
    }
}

impl LocalTerminalService {
    pub(crate) fn workspace_fixture(workers: Arc<Mutex<WorkerRegistry>>) -> Self {
        use crate::test_support::WorkbenchFixture;
        Self::from_workbench(
            workers,
            Some(
                orchbee_server::WorkbenchAuthority::workspace_fixture(random_site_id().unwrap())
                    .unwrap(),
            ),
        )
    }
}
