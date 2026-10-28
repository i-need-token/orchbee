use super::*;
use crate::{
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SemanticEventData, SemanticEventKind, SemanticEventMatch,
    SemanticEventPosition, SemanticEventScope, Snapshot, SnapshotReadIntent, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTabId,
};
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
};
fn id(n: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn service(read: bool, navigate: bool) -> WorkbenchService {
    WorkbenchService::new(
        WorkbenchPolicy {
            read,
            manage_navigation: navigate,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
            Mutex::new(WorkerRegistry::new()),
        ))),
    )
}
fn snapshot(service: &WorkbenchService) -> Snapshot {
    let WorkbenchExecution::Response(WorkbenchResponse::Snapshot(result)) =
        service.execute(WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: id(90),
        }))
    else {
        panic!("snapshot result");
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available snapshot");
    };
    snapshot
}
fn execute(session: &WorkbenchClientConnection<'_>, action: Action) -> Outcome {
    let request = SemanticEventsRequest {
        protocol_version: ProtocolVersion,
        request_id: id(91),
        session_id: snapshot(session.service).session_id,
        action,
    };
    let WorkbenchExecution::Response(WorkbenchResponse::Events(result)) =
        session.execute(WorkbenchRequest::Events(request.clone()))
    else {
        panic!("events result");
    };
    assert!(result.answers(&request));
    result.outcome
}
fn filter(kind: SemanticEventKind) -> SemanticEventFilter {
    SemanticEventFilter::Event {
        event: kind,
        scope: SemanticEventScope::All {},
        condition: SemanticEventMatch::Any {},
    }
}
fn subscribe(
    session: &WorkbenchClientConnection<'_>,
    after: Option<SemanticEventCursor>,
) -> Outcome {
    execute(
        session,
        Action::Subscribe {
            subscription_id: id(100),
            filters: vec![filter(SemanticEventKind::TabRenamed)],
            after,
        },
    )
}
fn tab(service: &WorkbenchService, label: &str) {
    let current = snapshot(service);
    assert!(service.terminals.event_journal().unwrap().publish(
        current.revision,
        None,
        SemanticEventKind::TabRenamed,
        SemanticEventData::Tab {
            terminal_context_id: current.navigation.workspaces[0].terminal_context_id.clone(),
            tab_id: WorkbenchTabId::parse(&format!("{:032x}", 101)).unwrap(),
            label: label.into(),
            index: 0
        }
    ));
}
#[test]
fn semantic_events_connection_ownership_definition_and_baseline_do_not_replay() {
    let service = service(true, false);
    tab(&service, "before");
    let first = service.client_connection(NonZeroU64::new(1).unwrap());
    let second = service.client_connection(NonZeroU64::new(2).unwrap());
    assert!(matches!(
        subscribe(&first, None),
        Outcome::Opened {
            existing: false,
            ..
        }
    ));
    assert!(matches!(
        subscribe(&first, None),
        Outcome::Opened { existing: true, .. }
    ));
    assert_eq!(
        execute(
            &second,
            Action::Read {
                subscription_id: id(100),
                limit: 32
            }
        ),
        Outcome::Failed {
            reason: Failure::NotFound
        }
    );
    assert!(
        matches!(execute(&first, Action::Read { subscription_id: id(100), limit: 32 }), Outcome::Events { events, gap: None, .. } if events.is_empty())
    );
    assert_eq!(
        execute(
            &first,
            Action::Subscribe {
                subscription_id: id(100),
                filters: vec![filter(SemanticEventKind::PaneClosed)],
                after: None
            }
        ),
        Outcome::Failed {
            reason: Failure::Conflict
        }
    );
    tab(&service, "first");
    tab(&service, "second");
    let Outcome::Events {
        cursor,
        events,
        more,
        ..
    } = execute(
        &first,
        Action::Read {
            subscription_id: id(100),
            limit: 1,
        },
    )
    else {
        panic!("page");
    };
    assert!(more);
    assert_eq!(events.len(), 1);
    assert_eq!(cursor.sequence.value(), 2);
    assert!(matches!(&events[0].data, SemanticEventData::Tab { label, .. } if label == "first"));
    assert!(
        matches!(execute(&first, Action::Read { subscription_id: id(100), limit: 32 }), Outcome::Events { events, .. }
        if events.len() == 1 && matches!(&events[0].data, SemanticEventData::Tab { label, .. } if label == "second"))
    );
    assert!(matches!(
        execute(
            &first,
            Action::Close {
                subscription_id: id(100)
            }
        ),
        Outcome::Closed { .. }
    ));
    assert_eq!(
        execute(
            &first,
            Action::Read {
                subscription_id: id(100),
                limit: 1
            }
        ),
        Outcome::Failed {
            reason: Failure::NotFound
        }
    );
    assert!(matches!(
        subscribe(&second, None),
        Outcome::Opened {
            existing: false,
            ..
        }
    ));
}
#[test]
fn semantic_events_wait_checks_match_before_deadline_and_gap_preserves_next_match() {
    let service = service(true, false);
    let session = service.client_connection(NonZeroU64::new(1).unwrap());
    subscribe(&session, None);
    tab(&service, "match");
    assert!(
        matches!(execute(&session, Action::Wait { subscription_id: id(100), timeout_milliseconds: 0 }),
        Outcome::Matched { event, .. } if matches!(&event.data, SemanticEventData::Tab { label, .. } if label == "match"))
    );
    let started = Instant::now();
    assert!(matches!(
        execute(
            &session,
            Action::Wait {
                subscription_id: id(100),
                timeout_milliseconds: 25
            }
        ),
        Outcome::TimedOut { .. }
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    for _ in 0..514 {
        tab(&service, "retained");
    }
    let Outcome::Gap { cursor, gap, .. } = execute(
        &session,
        Action::Wait {
            subscription_id: id(100),
            timeout_milliseconds: 0,
        },
    ) else {
        panic!("gap");
    };
    assert_eq!(gap.after.value(), 1);
    assert_eq!(cursor.sequence, gap.through);
    let Outcome::Matched { event, .. } = execute(
        &session,
        Action::Wait {
            subscription_id: id(100),
            timeout_milliseconds: 0,
        },
    ) else {
        panic!("retained match");
    };
    assert!(
        matches!(&event.position, SemanticEventPosition::Journal { cursor } if cursor.sequence.value() == gap.through.value() + 1)
    );
}
fn command(
    service: &WorkbenchService,
    n: u128,
    action: WorkbenchAction,
) -> WorkbenchCommandOutcome {
    let before = snapshot(service);
    let WorkbenchExecution::Response(WorkbenchResponse::Command(result)) =
        service.execute(WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: id(n),
            session_id: before.session_id,
            expected_revision: before.revision,
            action,
        }))
    else {
        panic!("command");
    };
    result.outcome
}
#[test]
fn semantic_events_real_workspace_commits_arrive_without_a_snapshot_poll() {
    let service = service(true, true);
    let session = service.client_connection(NonZeroU64::new(1).unwrap());
    execute(
        &session,
        Action::Subscribe {
            subscription_id: id(100),
            filters: vec![
                filter(SemanticEventKind::WorkspaceCreated),
                filter(SemanticEventKind::WorkspaceRenamed),
            ],
            after: None,
        },
    );
    assert!(matches!(
        command(
            &service,
            110,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("project").unwrap()
            }
        ),
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
    let project = snapshot(&service).navigation.projects[0].project_id.clone();
    assert!(matches!(
        command(
            &service,
            111,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project.clone()),
                label: WorkbenchLabel::parse("one").unwrap()
            }
        ),
        WorkbenchCommandOutcome::WorkspaceCreated { .. }
    ));
    let workspace = snapshot(&service).navigation.workspaces[0]
        .workspace_id
        .clone();
    for (n, label) in [(112, "two"), (113, "three")] {
        command(
            &service,
            n,
            WorkbenchAction::WorkspaceRename {
                workspace_id: workspace.clone(),
                label: WorkbenchLabel::parse(label).unwrap(),
            },
        );
    }
    let Outcome::Events { events, gap, .. } = execute(
        &session,
        Action::Read {
            subscription_id: id(100),
            limit: 32,
        },
    ) else {
        panic!("events");
    };
    assert!(gap.is_none());
    assert_eq!(
        events
            .iter()
            .map(|event| match &event.data {
                SemanticEventData::Workspace { workspace, .. } => workspace.label.as_str(),
                _ => panic!("workspace event"),
            })
            .collect::<Vec<_>>(),
        ["one", "two", "three"]
    );
}
#[test]
fn semantic_events_denied_invalid_regex_and_foreign_cursor_leave_owner_vacant() {
    let denied = service(false, false);
    let denied_session = denied.client_connection(NonZeroU64::new(1).unwrap());
    // Policy refusal precedes Session or source inspection.
    let request = SemanticEventsRequest {
        protocol_version: ProtocolVersion,
        request_id: id(91),
        session_id: orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 9)).unwrap(),
        action: Action::Subscribe {
            subscription_id: id(100),
            filters: vec![filter(SemanticEventKind::TabRenamed)],
            after: None,
        },
    };
    assert_eq!(
        denied_session.semantic_events(request).outcome,
        Outcome::Failed {
            reason: Failure::Denied
        }
    );
    assert!(denied_session.events.lock().unwrap().is_empty());
    let service = service(true, false);
    let session = service.client_connection(NonZeroU64::new(1).unwrap());
    let mut cursor = service.terminals.event_journal().unwrap().head().unwrap();
    cursor.sequence = WorkbenchRevision::new(1);
    assert_eq!(
        subscribe(&session, Some(cursor)),
        Outcome::Failed {
            reason: Failure::CursorAhead
        }
    );
    assert!(session.events.lock().unwrap().is_empty());
    assert_eq!(
        execute(
            &session,
            Action::Subscribe {
                subscription_id: id(100),
                after: None,
                filters: vec![SemanticEventFilter::OutputMatched {
                    target: orchbee_workbench_protocol::SemanticEventSource {
                        panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&format!(
                            "{:032x}",
                            4
                        ))
                        .unwrap(),
                        presentation_id: None
                    },
                    source: orchbee_workbench_protocol::SemanticOutputSource::Recent,
                    lines: 80,
                    matcher: orchbee_workbench_protocol::SemanticOutputMatch::Regex {
                        value: "[".into()
                    },
                    strip_ansi: true,
                    client_id: None,
                }]
            }
        ),
        Outcome::Failed {
            reason: Failure::InvalidRegex
        }
    );
    assert!(session.events.lock().unwrap().is_empty());
}
