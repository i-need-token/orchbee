use super::*;
use orchbee_workbench_protocol::{
    Notification, NotificationAction, NotificationFailure, NotificationOutcome,
    NotificationPosition, NotificationReason, NotificationRequest, NotificationSound, SiteId,
};
use std::num::NonZeroU64;

#[test]
fn notification_admission_checks_grants_site_and_session_drop_reclaims_inbox() {
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
        let subscriber = service.client_session(NonZeroU64::new(1).unwrap());
        let caller = service.client_session(NonZeroU64::new(2).unwrap());
        let run = |session: &WorkbenchClientSession<'_>, site: SiteId, id: u64, action| {
            let WorkbenchResponse::Notification(result) = response(session.execute(
                WorkbenchRequest::Notification(NotificationRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
                    site_id: site,
                    action,
                }),
            )) else {
                panic!()
            };
            result.validate().unwrap();
            result.outcome
        };
        let listen = NotificationAction::Listen { accepting: true };
        assert_eq!(
            run(&subscriber, snapshot.site_id.clone(), 2, listen.clone()),
            if read {
                NotificationOutcome::Listening { notification: None }
            } else {
                NotificationOutcome::Failed {
                    reason: NotificationFailure::Denied,
                }
            }
        );
        assert_eq!(
            run(
                &subscriber,
                SiteId::parse(&"f".repeat(32)).unwrap(),
                3,
                listen.clone()
            ),
            NotificationOutcome::Failed {
                reason: if read {
                    NotificationFailure::SiteMismatch
                } else {
                    NotificationFailure::Denied
                }
            }
        );
        let show = NotificationAction::Show {
            notification: Notification::normalize(
                "hello",
                None,
                NotificationPosition::TopRight,
                NotificationSound::None,
            )
            .unwrap(),
        };
        let outcome = run(&caller, snapshot.site_id.clone(), 4, show.clone());
        assert_eq!(
            outcome,
            if read && navigate {
                NotificationOutcome::Show {
                    shown: true,
                    reason: NotificationReason::Shown,
                }
            } else {
                NotificationOutcome::Failed {
                    reason: NotificationFailure::Denied,
                }
            }
        );
        if read && navigate {
            assert!(matches!(
                run(&subscriber, snapshot.site_id.clone(), 5, listen.clone()),
                NotificationOutcome::Listening {
                    notification: Some(_)
                }
            ));
            assert_eq!(
                run(&subscriber, snapshot.site_id.clone(), 6, listen),
                NotificationOutcome::Listening { notification: None }
            );
            drop(subscriber);
            assert_eq!(
                run(&caller, snapshot.site_id, 7, show),
                NotificationOutcome::Show {
                    shown: false,
                    reason: NotificationReason::NoForegroundClient
                }
            );
        }
    }
}
