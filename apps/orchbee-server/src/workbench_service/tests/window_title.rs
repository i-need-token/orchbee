use super::*;
use orchbee_workbench_protocol::{
    SessionId, WindowTitle, WindowTitleAction, WindowTitleFailure, WindowTitleOutcome,
    WindowTitleReason, WindowTitleRequest, WindowTitleSnapshot, WorkbenchRevision,
};
use std::num::NonZeroU64;

#[test]
fn window_title_admission_checks_grants_session_and_disconnect() {
    for (read, navigate) in [(false, false), (false, true), (true, false), (true, true)] {
        let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
            WorkerRegistry::new(),
        ))));
        let bootstrap = WorkbenchService::new(
            WorkbenchPolicy {
                read: true,
                manage_navigation: false,
                associate_directory: false,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            terminals.clone(),
        );
        let WorkbenchResponse::Snapshot(snapshot) = response(bootstrap.execute(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
            }),
        )) else {
            panic!()
        };
        let WorkbenchSnapshotOutcome::Available { snapshot } = snapshot.outcome else {
            panic!()
        };
        let service = WorkbenchService::new(
            WorkbenchPolicy {
                read,
                manage_navigation: navigate,
                associate_directory: false,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            terminals,
        );
        let subscriber = service.client_connection(NonZeroU64::new(1).unwrap());
        let caller = service.client_connection(NonZeroU64::new(2).unwrap());
        let run = |session: &WorkbenchClientConnection<'_>, session: SessionId, id: u64, action| {
            let WorkbenchResponse::WindowTitle(result) = response(session.execute(
                WorkbenchRequest::WindowTitle(WindowTitleRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
                    session_id: session,
                    action,
                }),
            )) else {
                panic!()
            };
            result.validate().unwrap();
            result.outcome
        };
        let listen = WindowTitleAction::Listen {
            focused: true,
            context: Default::default(),
        };
        assert_eq!(
            run(&subscriber, snapshot.session_id.clone(), 2, listen.clone()),
            if read {
                WindowTitleOutcome::Listening { snapshot: None }
            } else {
                WindowTitleOutcome::Failed {
                    reason: WindowTitleFailure::Denied,
                }
            }
        );
        assert_eq!(
            run(
                &subscriber,
                SessionId::parse(&"f".repeat(32)).unwrap(),
                3,
                listen.clone()
            ),
            WindowTitleOutcome::Failed {
                reason: if read {
                    WindowTitleFailure::SessionMismatch
                } else {
                    WindowTitleFailure::Denied
                }
            }
        );
        let action = WindowTitleAction::Set {
            title: WindowTitle::normalize("hello").unwrap(),
        };
        let published = |revision, changed| WindowTitleOutcome::Published {
            snapshot: WindowTitleSnapshot {
                revision: WorkbenchRevision::new(revision),
                title: Some(WindowTitle::normalize("hello").unwrap()),
            },
            changed,
            reason: if changed {
                WindowTitleReason::Set
            } else {
                WindowTitleReason::NoForegroundClient
            },
        };
        assert_eq!(
            run(&caller, snapshot.session_id.clone(), 4, action.clone()),
            if read && navigate {
                published(1, true)
            } else {
                WindowTitleOutcome::Failed {
                    reason: WindowTitleFailure::Denied,
                }
            }
        );
        if read && navigate {
            assert!(matches!(
                run(&subscriber, snapshot.session_id.clone(), 5, listen),
                WindowTitleOutcome::Listening { snapshot: Some(_) }
            ));
            drop(subscriber);
            assert_eq!(
                run(&caller, snapshot.session_id, 6, action),
                published(2, false)
            );
        }
    }
}
