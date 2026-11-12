use super::*;
use orchbee_workbench_protocol::{
    ClientViewPhase, ProtocolVersion, SessionId, WorkbenchClientViewResult, WorkbenchRequest,
    WorkbenchRevision, parse_workbench_request,
};

fn owner(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}
fn client(value: u64) -> WorkbenchClientId {
    WorkbenchClientId::parse(&format!("{value:032x}")).unwrap()
}
fn operation_id(value: u64) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}
fn sample() -> ClientViewAction {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let request =
        parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
            .unwrap();
    let WorkbenchRequest::ClientView(request) = request else {
        panic!("view request");
    };
    request.action
}

#[cfg(windows)]
#[test]
fn graphics_freshness_partial_scope_and_heartbeat_change_do_not_fabricate_visibility() {
    use orchbee_workbench_protocol::{
        PaneGraphicsCapabilities, PaneGraphicsClientObservation, PaneGraphicsTransport,
        SemanticPane, TerminalPresentationId,
    };
    let mut registry = ClientViewRegistry::default();
    let now = Instant::now();
    let ClientViewOutcome::Synchronized { client: record, .. } =
        execute(&mut registry, 1, sample(), now)
    else {
        panic!("Client record");
    };
    let selection = record.view.selection.as_ref().unwrap();
    let source = SemanticPane {
        terminal_context_id: selection.terminal_context_id.clone(),
        tab_id: selection.tab_id.clone().unwrap(),
        panel_id: selection.panel_id.clone().unwrap(),
        presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 9)).unwrap(),
        binding: None,
        label: "Panel".into(),
    };
    let mut observation = PaneGraphicsClientObservation {
        workbench_revision: record.view.workbench_revision,
        capabilities: PaneGraphicsCapabilities {
            transport: PaneGraphicsTransport::CellRaster,
            cell_pixels: None,
            pixel_mouse: false,
        },
        complete: false,
        panes: vec![],
    };
    registry
        .observe_graphics(
            owner(1),
            &record.client_id,
            record.heartbeat,
            observation.clone(),
            now,
        )
        .unwrap();
    let info = registry
        .graphics_info(
            Some(&record.client_id),
            &source,
            WorkbenchRevision::new(1),
            now,
        )
        .unwrap()
        .unwrap();
    assert_eq!(info.visible, None);
    assert_eq!(info.drawn, None);
    assert!(info.capabilities.is_some());
    observation.complete = true;
    registry
        .observe_graphics(
            owner(1),
            &record.client_id,
            record.heartbeat,
            observation.clone(),
            now,
        )
        .unwrap();
    assert_eq!(
        registry
            .graphics_info(None, &source, WorkbenchRevision::new(1), now)
            .unwrap()
            .unwrap()
            .visible,
        Some(false)
    );
    let later = now + Duration::from_secs(6);
    execute(&mut registry, 1, sample(), later); // Idle time does not expire a live connection.
    let current = registry
        .graphics_info(
            Some(&record.client_id),
            &source,
            WorkbenchRevision::new(1),
            later,
        )
        .unwrap()
        .unwrap();
    assert!(current.capabilities.is_some());
    assert_eq!(current.visible, Some(false));
    assert!(
        registry
            .observe_graphics(
                owner(1),
                &record.client_id,
                record.heartbeat,
                observation,
                later
            )
            .is_ok()
    );
    let mut next = sample();
    let ClientViewAction::Synchronize { heartbeat, .. } = &mut next else {
        unreachable!();
    };
    *heartbeat = ClientViewSequence::new(record.heartbeat.value() + 1).unwrap();
    execute(&mut registry, 1, next, later);
    let unobserved = registry
        .graphics_info(
            Some(&record.client_id),
            &source,
            WorkbenchRevision::new(1),
            later,
        )
        .unwrap()
        .unwrap();
    assert!(unobserved.capabilities.is_none());
    assert!(unobserved.visible.is_none());
}
fn command(id: u64, target: u64) -> ClientViewControl {
    let ClientViewAction::Synchronize {
        view,
        view_revision,
        ..
    } = sample()
    else {
        panic!("sample");
    };
    let selection = view.selection.unwrap();
    ClientViewControl {
        operation_id: operation_id(id),
        client_id: client(target),
        expected_view_revision: view_revision,
        minimum_workbench_revision: WorkbenchRevision::new(12),
        intent: orchbee_workbench_protocol::ClientViewIntent::Panel {
            terminal_context_id: selection.terminal_context_id,
            tab_id: selection.tab_id.unwrap(),
            panel_id: selection.panel_id.unwrap(),
        },
    }
}
fn execute(
    registry: &mut ClientViewRegistry,
    session: u64,
    action: ClientViewAction,
    now: Instant,
) -> ClientViewOutcome {
    let outcome = registry.execute(owner(session), action, now);
    WorkbenchClientViewResult {
        protocol_version: ProtocolVersion,
        request_id: operation_id(1000),
        session_id: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
        outcome: outcome.clone(),
    }
    .validate()
    .unwrap();
    outcome
}
fn submit(
    registry: &mut ClientViewRegistry,
    id: u64,
    target: u64,
    now: Instant,
) -> ClientViewOutcome {
    execute(
        registry,
        20,
        ClientViewAction::Submit {
            command: Box::new(command(id, target)),
        },
        now,
    )
}
fn status(outcome: ClientViewOutcome) -> ClientViewControlStatus {
    let ClientViewOutcome::Operation { operation } = outcome else {
        panic!("expected operation: {outcome:?}");
    };
    operation.status
}
fn read(registry: &mut ClientViewRegistry, id: u64, now: Instant) -> ClientViewOutcome {
    execute(
        registry,
        20,
        ClientViewAction::Read {
            operation_id: operation_id(id),
        },
        now,
    )
}
fn acknowledge(id: u64) -> ClientViewAction {
    let mut action = sample();
    let ClientViewAction::Synchronize {
        completion,
        heartbeat,
        ..
    } = &mut action
    else {
        panic!("sample");
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    *completion = Some(ClientViewCompletion {
        operation_id: operation_id(id),
        outcome: ClientViewCompletionOutcome::Applied {},
    });
    action
}

#[test]
fn semantic_events_publish_admitted_focus_and_client_ratio_changes_without_heartbeat_noise() {
    use orchbee_workbench_protocol::{
        SemanticEventFilter, SemanticEventKind as K, SemanticEventMatch, SemanticEventScope,
    };
    let authority =
        orchbee_server::WorkbenchAuthority::new(SessionId::parse(&format!("{:032x}", 1)).unwrap())
            .unwrap();
    let journal = authority.event_journal();
    let cursor = journal.head().unwrap();
    let mut registry = ClientViewRegistry::with_events(Some(journal.clone()));
    let mut action = sample();
    let ClientViewAction::Synchronize { view, .. } = &mut action else {
        panic!();
    };
    view.split_revision = Some(WorkbenchRevision::new(0));
    let now = Instant::now();
    execute(&mut registry, 1, action.clone(), now);
    let filters = [
        K::WorkspaceFocused,
        K::TabFocused,
        K::PaneFocused,
        K::LayoutUpdated,
    ]
    .map(|event| SemanticEventFilter::Event {
        event,
        scope: SemanticEventScope::All {},
        condition: SemanticEventMatch::Any {},
    });
    let focused = journal.read(&cursor, &filters, 32).unwrap();
    assert_eq!(
        focused
            .events
            .iter()
            .map(|event| event.event)
            .collect::<Vec<_>>(),
        [K::WorkspaceFocused, K::TabFocused, K::PaneFocused]
    );
    let ClientViewAction::Synchronize {
        heartbeat,
        view,
        view_revision,
        ..
    } = &mut action
    else {
        panic!()
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    view.split_revision = Some(WorkbenchRevision::new(u64::MAX));
    *view_revision = ClientViewSequence::new(view_revision.value() + 1).unwrap();
    execute(&mut registry, 1, action.clone(), now);
    let ratio = journal.read(&focused.cursor, &filters, 32).unwrap();
    assert_eq!(ratio.events.len(), 1);
    assert_eq!(ratio.events[0].event, K::LayoutUpdated);
    let ClientViewAction::Synchronize { heartbeat, .. } = &mut action else {
        panic!()
    };
    *heartbeat = ClientViewSequence::new(3).unwrap();
    execute(&mut registry, 1, action, now);
    assert!(
        journal
            .read(&ratio.cursor, &filters, 32)
            .unwrap()
            .events
            .is_empty()
    );
}

#[test]
fn agent_focus_can_be_delivered_while_busy_but_still_owns_the_single_effect_slot() {
    for phase in [
        ClientViewPhase::Ready,
        ClientViewPhase::Busy,
        ClientViewPhase::Starting,
    ] {
        let now = Instant::now();
        let mut registry = ClientViewRegistry::default();
        let mut sample = sample();
        let ClientViewAction::Synchronize { view, .. } = &mut sample else {
            panic!()
        };
        view.phase = phase;
        execute(&mut registry, 1, sample.clone(), now);
        let mut focus = command(990, 1);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = focus.intent
        else {
            panic!()
        };
        focus.intent = serde_json::from_value(serde_json::json!({
            "kind":"agent-focus", "terminalContextId":terminal_context_id,
            "tabId":tab_id, "panelId":panel_id, "presentationId":"3".repeat(32),
            "target":{"kind":"session", "sessionId":"agent-1"}
        }))
        .unwrap();
        assert!(!focus.intent.is_passive_read());
        let outcome = execute(
            &mut registry,
            20,
            ClientViewAction::Submit {
                command: Box::new(focus.clone()),
            },
            now,
        );
        if phase == ClientViewPhase::Starting {
            assert_eq!(
                status(outcome),
                ClientViewControlStatus::Rejected {
                    reason: ClientViewControlReason::Busy
                }
            );
            continue;
        }
        assert_eq!(status(outcome), ClientViewControlStatus::Queued {});
        let mut second = focus.clone();
        second.operation_id = operation_id(991);
        assert_eq!(
            status(execute(
                &mut registry,
                20,
                ClientViewAction::Submit {
                    command: Box::new(second)
                },
                now
            )),
            ClientViewControlStatus::Rejected {
                reason: ClientViewControlReason::Busy
            }
        );
        let ClientViewOutcome::Synchronized { control, .. } =
            execute(&mut registry, 1, sample, now)
        else {
            panic!()
        };
        assert_eq!(control, Some(Box::new(focus)));
    }
}

#[test]
fn navigation_receipts_validate_owner_and_correlated_memory_without_reexecution() {
    use orchbee_workbench_protocol::{ClientContextSelection, ClientNavigationSnapshot};
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    execute(&mut registry, 1, sample(), now);
    let mut command = command(903, 1);
    let ClientViewIntent::Panel {
        terminal_context_id,
        tab_id,
        ..
    } = command.intent
    else {
        panic!()
    };
    command.intent = ClientViewIntent::Selections {
        terminal_context_ids: vec![terminal_context_id.clone()],
    };
    let submit = ClientViewAction::Submit {
        command: Box::new(command),
    };
    assert_eq!(
        status(execute(&mut registry, 20, submit.clone(), now)),
        ClientViewControlStatus::Queued {}
    );
    assert_eq!(
        registry.receipts[&operation_id(903)].payload_reservation,
        orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES
    );
    assert!(matches!(
        execute(&mut registry, 1, sample(), now),
        ClientViewOutcome::Synchronized {
            control: Some(_),
            ..
        }
    ));
    let mut ack = acknowledge(903);
    let ClientViewAction::Synchronize { completion, .. } = &mut ack else {
        panic!()
    };
    completion.as_mut().unwrap().outcome = ClientViewCompletionOutcome::NavigationRead {
        navigation: Box::new(ClientNavigationSnapshot {
            workbench_revision: WorkbenchRevision::new(12),
            contexts: vec![ClientContextSelection {
                terminal_context_id,
                active_tab_id: Some(tab_id),
            }],
        }),
    };
    let mut invalid = ack.clone();
    if let ClientViewAction::Synchronize {
        completion: Some(completion),
        ..
    } = &mut invalid
    {
        let ClientViewCompletionOutcome::NavigationRead { navigation } = &mut completion.outcome
        else {
            panic!()
        };
        navigation.contexts[0].active_tab_id = None;
    }
    assert!(matches!(
        execute(&mut registry, 1, invalid, now),
        ClientViewOutcome::Conflict {}
    ));
    assert!(matches!(
        execute(&mut registry, 2, ack.clone(), now),
        ClientViewOutcome::Conflict {}
    ));
    assert!(matches!(
        execute(&mut registry, 1, ack, now),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    let completed = status(read(&mut registry, 903, now));
    assert!(matches!(
        completed,
        ClientViewControlStatus::NavigationRead { .. }
    ));
    assert!(
        registry.receipts[&operation_id(903)].payload_reservation
            < orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES
    );
    registry.disconnect(owner(1), now);
    assert_eq!(status(execute(&mut registry, 20, submit, now)), completed);
}

#[test]
fn viewport_read_reserves_bounded_memory_and_reads_busy_without_allowing_tab_navigation() {
    use orchbee_workbench_protocol::{
        ClientPanelViewport, ClientViewportObservation, ClientViewportSnapshot,
        ClientViewportUnavailable,
    };
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    let mut busy = sample();
    let ClientViewAction::Synchronize { view, .. } = &mut busy else {
        panic!()
    };
    view.phase = ClientViewPhase::Busy;
    execute(&mut registry, 1, busy.clone(), now);
    let mut blocked = command(907, 1);
    let ClientViewIntent::Panel {
        terminal_context_id,
        tab_id,
        ..
    } = blocked.intent
    else {
        panic!()
    };
    blocked.intent = ClientViewIntent::Tab {
        terminal_context_id,
        tab_id,
    };
    assert!(matches!(
        status(execute(
            &mut registry,
            20,
            ClientViewAction::Submit {
                command: Box::new(blocked)
            },
            now
        )),
        ClientViewControlStatus::Rejected {
            reason: ClientViewControlReason::Busy
        }
    ));
    let mut target = command(908, 1);
    let ClientViewIntent::Panel { panel_id, .. } = target.intent else {
        panic!()
    };
    target.intent = ClientViewIntent::Viewports {
        panel_ids: vec![panel_id.clone()],
    };
    let submit = ClientViewAction::Submit {
        command: Box::new(target),
    };
    assert!(matches!(
        status(execute(&mut registry, 20, submit.clone(), now)),
        ClientViewControlStatus::Queued {}
    ));
    assert_eq!(
        registry.receipts[&operation_id(908)].payload_reservation,
        orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES
    );
    assert!(matches!(
        execute(&mut registry, 1, busy.clone(), now),
        ClientViewOutcome::Synchronized {
            control: Some(_),
            ..
        }
    ));
    let ClientViewAction::Synchronize {
        completion,
        heartbeat,
        ..
    } = &mut busy
    else {
        panic!()
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    *completion = Some(ClientViewCompletion {
        operation_id: operation_id(908),
        outcome: ClientViewCompletionOutcome::ViewportRead {
            viewport: Box::new(ClientViewportSnapshot {
                workbench_revision: WorkbenchRevision::new(12),
                panels: vec![ClientPanelViewport {
                    panel_id,
                    primary_terminal_id: None,
                    observation: ClientViewportObservation::Unavailable {
                        reason: ClientViewportUnavailable::NotAttached,
                    },
                }],
            }),
        },
    });
    assert!(matches!(
        execute(&mut registry, 2, busy.clone(), now),
        ClientViewOutcome::Conflict {}
    ));
    assert!(matches!(
        execute(&mut registry, 1, busy, now),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    let completed = status(read(&mut registry, 908, now));
    assert!(matches!(
        completed,
        ClientViewControlStatus::ViewportRead { .. }
    ));
    assert_eq!(status(execute(&mut registry, 20, submit, now)), completed);
}

#[test]
fn attention_read_is_allowed_while_busy_and_exact_owner_and_sessions_are_required() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    let mut busy = sample();
    let ClientViewAction::Synchronize { view, .. } = &mut busy else {
        panic!("sample")
    };
    view.phase = ClientViewPhase::Busy;
    execute(&mut registry, 1, busy.clone(), now);
    let mut target = command(901, 1);
    target.intent = ClientViewIntent::AgentAttention {
        session_ids: vec![
            orchbee_agent_observation_protocol::AgentSessionId::parse("agent-1").unwrap(),
        ],
    };
    assert!(matches!(
        status(execute(
            &mut registry,
            20,
            ClientViewAction::Submit {
                command: Box::new(target)
            },
            now
        )),
        ClientViewControlStatus::Queued {}
    ));
    assert!(matches!(
        execute(&mut registry, 1, busy.clone(), now),
        ClientViewOutcome::Synchronized {
            control: Some(_),
            ..
        }
    ));
    let attention = orchbee_workbench_protocol::ClientAgentAttentionSnapshot {
        cursor: orchbee_agent_observation_protocol::AgentChangesCursor::parse(&format!(
            "v1.{}.{}",
            "A".repeat(59),
            "A".repeat(43)
        ))
        .unwrap(),
        revision: orchbee_agent_observation_protocol::AgentSafeInteger::new(3),
        outer_focused: Some(false),
        entries: vec![orchbee_workbench_protocol::ClientAgentAttentionEntry {
            session_id: orchbee_agent_observation_protocol::AgentSessionId::parse("wrong").unwrap(),
            state: None,
        }],
    };
    let ClientViewAction::Synchronize {
        completion,
        heartbeat,
        ..
    } = &mut busy
    else {
        panic!("sample")
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    *completion = Some(ClientViewCompletion {
        operation_id: operation_id(901),
        outcome: ClientViewCompletionOutcome::AttentionRead {
            attention: Box::new(attention),
        },
    });
    assert!(matches!(
        execute(&mut registry, 1, busy.clone(), now),
        ClientViewOutcome::Conflict {}
    ));
    let ClientViewAction::Synchronize {
        completion: Some(completion),
        ..
    } = &mut busy
    else {
        panic!("completion")
    };
    let ClientViewCompletionOutcome::AttentionRead { attention } = &mut completion.outcome else {
        panic!("attention")
    };
    attention.entries[0].session_id =
        orchbee_agent_observation_protocol::AgentSessionId::parse("agent-1").unwrap();
    assert!(matches!(
        execute(&mut registry, 2, busy.clone(), now),
        ClientViewOutcome::Conflict {}
    ));
    assert!(matches!(
        execute(&mut registry, 1, busy, now),
        ClientViewOutcome::Synchronized { .. }
    ));
    assert!(matches!(
        status(read(&mut registry, 901, now)),
        ClientViewControlStatus::AttentionRead { .. }
    ));
    assert!(registry.receipts[&operation_id(901)].payload_reservation > 0);
}

#[test]
fn exact_owner_receives_once_and_only_observed_completion_settles() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    execute(&mut registry, 1, sample(), now);
    execute(&mut registry, 2, sample(), now);
    assert!(matches!(
        status(submit(&mut registry, 3, 1, now)),
        ClientViewControlStatus::Queued {}
    ));
    assert!(matches!(
        execute(&mut registry, 2, sample(), now),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    assert!(matches!(
        execute(&mut registry, 2, acknowledge(3), now),
        ClientViewOutcome::Conflict {}
    ));
    let delivered = execute(&mut registry, 1, sample(), now);
    assert!(matches!(
        delivered,
        ClientViewOutcome::Synchronized {
            control: Some(_),
            ..
        }
    ));
    assert!(matches!(
        execute(&mut registry, 1, sample(), now),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    assert!(matches!(
        status(read(&mut registry, 3, now)),
        ClientViewControlStatus::Delivered {}
    ));
    let mut invalid = acknowledge(3);
    if let ClientViewAction::Synchronize {
        view,
        view_revision,
        ..
    } = &mut invalid
    {
        view.workbench_revision = WorkbenchRevision::new(11);
        *view_revision = ClientViewSequence::new(3).unwrap();
    }
    assert!(matches!(
        execute(&mut registry, 1, invalid, now),
        ClientViewOutcome::Conflict {}
    ));
    execute(&mut registry, 1, acknowledge(3), now);
    let applied = status(read(&mut registry, 3, now));
    assert!(matches!(applied, ClientViewControlStatus::Applied { .. }));
    registry.disconnect(owner(1), now);
    assert_eq!(status(submit(&mut registry, 3, 1, now)), applied);
    let mut conflicting = command(3, 1);
    conflicting.minimum_workbench_revision = WorkbenchRevision::new(13);
    assert_eq!(
        execute(
            &mut registry,
            20,
            ClientViewAction::Submit {
                command: Box::new(conflicting)
            },
            now
        ),
        ClientViewOutcome::Conflict {}
    );
}

#[test]
fn idle_connected_view_remains_current_and_revision_changes_fence_delivery() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    execute(&mut registry, 1, sample(), now);
    let later = now + Duration::from_secs(60);
    let ClientViewOutcome::Synchronized { client, .. } = execute(&mut registry, 1, sample(), later)
    else {
        panic!("sync");
    };
    assert_eq!(client.availability, ClientViewAvailability::Current);
    let mut fresh = sample();
    if let ClientViewAction::Synchronize { heartbeat, .. } = &mut fresh {
        *heartbeat = ClientViewSequence::new(2).unwrap();
    }
    execute(&mut registry, 1, fresh.clone(), later);
    submit(&mut registry, 4, 1, later);
    if let ClientViewAction::Synchronize {
        heartbeat,
        view_revision,
        view,
        ..
    } = &mut fresh
    {
        *heartbeat = ClientViewSequence::new(3).unwrap();
        *view_revision = ClientViewSequence::new(3).unwrap();
        view.phase = ClientViewPhase::Busy;
    }
    assert!(matches!(
        execute(&mut registry, 1, fresh, later),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    assert_eq!(
        status(read(&mut registry, 4, later)),
        ClientViewControlStatus::Rejected {
            reason: ClientViewControlReason::StaleView
        }
    );
    assert_eq!(
        execute(&mut registry, 1, sample(), later),
        ClientViewOutcome::Conflict {}
    );
}

#[test]
fn disconnect_distinguishes_undelivered_from_possible_effects_and_preserves_other_clients() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    execute(&mut registry, 1, sample(), now);
    execute(&mut registry, 2, sample(), now);
    submit(&mut registry, 3, 1, now);
    submit(&mut registry, 4, 2, now);
    execute(&mut registry, 1, sample(), now);
    registry.disconnect(owner(1), now);
    assert_eq!(
        status(read(&mut registry, 3, now)),
        ClientViewControlStatus::Incomplete {
            reason: ClientViewControlReason::ClientDisconnected,
            view: None
        }
    );
    assert!(matches!(
        status(read(&mut registry, 4, now)),
        ClientViewControlStatus::Queued {}
    ));
    registry.disconnect(owner(2), now);
    assert_eq!(
        status(read(&mut registry, 4, now)),
        ClientViewControlStatus::Rejected {
            reason: ClientViewControlReason::ClientDisconnected
        }
    );
    assert_eq!(
        execute(&mut registry, 20, ClientViewAction::List {}, now),
        ClientViewOutcome::Listed { clients: vec![] }
    );
}

#[test]
fn completed_receipts_are_never_evicted_early_and_delivered_ownership_does_not_expire() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    execute(&mut registry, 1, sample(), now);
    submit(&mut registry, 1, 1, now);
    execute(&mut registry, 1, sample(), now);
    for id in 2..=256 {
        assert_eq!(
            status(submit(&mut registry, id, 1, now)),
            ClientViewControlStatus::Rejected {
                reason: ClientViewControlReason::Busy
            }
        );
    }
    assert_eq!(
        submit(&mut registry, 257, 1, now),
        unavailable(ClientViewUnavailable::ReceiptCapacity)
    );
    assert!(matches!(
        status(read(
            &mut registry,
            2,
            now + RECEIPT_RETENTION - Duration::from_millis(1)
        )),
        ClientViewControlStatus::Rejected { .. }
    ));
    assert_eq!(
        read(&mut registry, 2, now + RECEIPT_RETENTION),
        ClientViewOutcome::NotFound {}
    );
    assert!(matches!(
        status(read(&mut registry, 1, now + RECEIPT_RETENTION * 2)),
        ClientViewControlStatus::Delivered {}
    ));
    registry.disconnect(owner(1), now + RECEIPT_RETENTION * 2);
    assert!(matches!(
        status(read(&mut registry, 1, now + RECEIPT_RETENTION * 2)),
        ClientViewControlStatus::Incomplete { .. }
    ));
}

#[test]
fn queued_expiry_and_view_capacity_have_exact_boundaries() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    for generation in 1..=32 {
        assert!(matches!(
            execute(&mut registry, generation, sample(), now),
            ClientViewOutcome::Synchronized { .. }
        ));
    }
    assert_eq!(
        execute(&mut registry, 33, sample(), now),
        unavailable(ClientViewUnavailable::ServerBusy)
    );
    submit(&mut registry, 1, 1, now);
    assert!(matches!(
        status(read(
            &mut registry,
            1,
            now + QUEUE_LIFETIME - Duration::from_millis(1)
        )),
        ClientViewControlStatus::Queued {}
    ));
    assert_eq!(
        status(read(&mut registry, 1, now + QUEUE_LIFETIME)),
        ClientViewControlStatus::Rejected {
            reason: ClientViewControlReason::Expired
        }
    );
    registry.disconnect(owner(1), now + QUEUE_LIFETIME);
    execute(&mut registry, 33, sample(), now + QUEUE_LIFETIME);
    assert_eq!(
        execute(
            &mut registry,
            20,
            ClientViewAction::Get {
                client_id: client(1)
            },
            now + QUEUE_LIFETIME
        ),
        ClientViewOutcome::NotFound {}
    );
}

mod traces;

#[test]
fn payload_reservations_bound_pending_payloads_and_shrink_after_valid_completion() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    let layout_command = |id, target| {
        let mut command = command(id, target);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent
        else {
            panic!("panel");
        };
        command.intent = ClientViewIntent::Layout {
            terminal_context_id,
            tab_id,
            panel_id,
        };
        command
    };
    let query = |registry: &mut ClientViewRegistry, id, target| {
        execute(
            registry,
            30,
            ClientViewAction::Submit {
                command: Box::new(layout_command(id, target)),
            },
            now,
        )
    };
    for id in 1..=17 {
        execute(&mut registry, id, sample(), now);
    }
    for id in 1..=16 {
        assert_eq!(
            status(query(&mut registry, 100 + id, id)),
            ClientViewControlStatus::Queued {}
        );
    }
    assert!(matches!(
        query(&mut registry, 117, 17),
        ClientViewOutcome::Unavailable {
            reason: ClientViewUnavailable::ReceiptCapacity
        }
    ));
    assert_eq!(
        registry
            .receipts
            .values()
            .map(|receipt| receipt.payload_reservation)
            .sum::<usize>(),
        MAXIMUM_PAYLOAD_RESERVATION
    );
    assert!(matches!(
        execute(&mut registry, 1, sample(), now),
        ClientViewOutcome::Synchronized {
            control: Some(_),
            ..
        }
    ));

    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let document = &corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "complete-layout")
        .unwrap()["document"];
    let WorkbenchRequest::ClientView(mut request) =
        parse_workbench_request(&serde_json::to_vec(document).unwrap()).unwrap()
    else {
        panic!("view");
    };
    let ClientViewAction::Synchronize {
        heartbeat,
        completion,
        ..
    } = &mut request.action
    else {
        panic!("sync");
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    completion.as_mut().unwrap().operation_id = operation_id(101);
    assert!(matches!(
        execute(&mut registry, 1, request.action, now),
        ClientViewOutcome::Synchronized { .. }
    ));
    let retained = registry.receipts[&operation_id(101)].payload_reservation;
    assert!(retained > 0 && retained < MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES);
    assert!(matches!(
        status(read(&mut registry, 101, now)),
        ClientViewControlStatus::LayoutRead { .. }
    ));
    // The retained small result still consumes bytes: with15 full reservations there is not
    // yet an entire1MiB free. Releasing one pending owner makes the next reservation possible.
    assert!(matches!(
        query(&mut registry, 117, 17),
        ClientViewOutcome::Unavailable {
            reason: ClientViewUnavailable::ReceiptCapacity
        }
    ));
    registry.disconnect(owner(2), now);
    assert_eq!(
        status(query(&mut registry, 117, 17)),
        ClientViewControlStatus::Queued {}
    );

    // A duplicate retains the same observation without reserving a second payload.
    let reserved = registry
        .receipts
        .values()
        .map(|receipt| receipt.payload_reservation)
        .sum::<usize>();
    assert!(matches!(
        status(query(&mut registry, 101, 1)),
        ClientViewControlStatus::LayoutRead { .. }
    ));
    assert_eq!(
        registry
            .receipts
            .values()
            .map(|receipt| receipt.payload_reservation)
            .sum::<usize>(),
        reserved
    );
    registry.disconnect(owner(2), now);
    assert_eq!(registry.receipts[&operation_id(102)].payload_reservation, 0);
    registry.disconnect(owner(1), now);
    assert_eq!(
        registry.receipts[&operation_id(101)].payload_reservation,
        retained
    );
    assert!(matches!(
        read(&mut registry, 101, now + RECEIPT_RETENTION),
        ClientViewOutcome::NotFound {}
    ));
    assert_eq!(
        registry
            .receipts
            .values()
            .map(|receipt| receipt.payload_reservation)
            .sum::<usize>(),
        0
    );
}

#[test]
fn directional_receipts_validate_destination_and_survive_disconnect_without_reexecution() {
    use orchbee_workbench_protocol::{ClientPanelDirection, ClientViewIntent, WorkbenchPanelId};
    for found in [false, true] {
        let now = Instant::now();
        let mut registry = ClientViewRegistry::default();
        execute(&mut registry, 1, sample(), now);
        let mut command = command(3, 1);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent
        else {
            panic!("panel");
        };
        command.intent = ClientViewIntent::FocusDirection {
            terminal_context_id,
            tab_id,
            panel_id,
            direction: ClientPanelDirection::Right,
        };
        let submit = ClientViewAction::Submit {
            command: Box::new(command),
        };
        assert!(matches!(
            status(execute(&mut registry, 20, submit.clone(), now)),
            ClientViewControlStatus::Queued {}
        ));
        assert!(matches!(
            execute(&mut registry, 1, sample(), now),
            ClientViewOutcome::Synchronized {
                control: Some(_),
                ..
            }
        ));
        let mut ack = acknowledge(3);
        let ClientViewAction::Synchronize {
            view,
            view_revision,
            completion,
            ..
        } = &mut ack
        else {
            panic!("sync");
        };
        let destination = WorkbenchPanelId::parse(&format!("{:032x}", 99)).unwrap();
        let outcome = if found {
            view.selection.as_mut().unwrap().panel_id = Some(destination.clone());
            *view_revision = ClientViewSequence::new(3).unwrap();
            ClientViewCompletionOutcome::DirectionFocused {
                panel_id: destination,
            }
        } else {
            ClientViewCompletionOutcome::DirectionUnchanged {
                layout_revision: WorkbenchRevision::new(12),
            }
        };
        completion.as_mut().unwrap().outcome = outcome;
        let mut invalid = ack.clone();
        if let ClientViewAction::Synchronize { completion, .. } = &mut invalid {
            completion.as_mut().unwrap().outcome = ClientViewCompletionOutcome::Applied {};
        }
        assert!(matches!(
            execute(&mut registry, 1, invalid, now),
            ClientViewOutcome::Conflict {}
        ));
        assert!(matches!(
            execute(&mut registry, 2, ack.clone(), now),
            ClientViewOutcome::Conflict {}
        ));
        assert!(matches!(
            execute(&mut registry, 1, ack, now),
            ClientViewOutcome::Synchronized { control: None, .. }
        ));
        let completed = status(read(&mut registry, 3, now));
        assert_eq!(
            matches!(completed, ClientViewControlStatus::DirectionFocused { .. }),
            found
        );
        registry.disconnect(owner(1), now);
        assert_eq!(status(execute(&mut registry, 20, submit, now)), completed);
    }
}

#[test]
fn ratio_receipts_require_exact_owner_target_and_value_and_never_reexecute_on_read() {
    use orchbee_workbench_protocol::{
        ClientLayoutAdjustment, ClientLayoutChange, ClientSplitChange, ClientSplitPath,
        ClientSplitRatio, WorkbenchPanelSlotId, WorkbenchSplitId, WorkbenchTabId,
    };
    for selected in [false, true] {
        let now = Instant::now();
        let mut registry = ClientViewRegistry::default();
        execute(&mut registry, 1, sample(), now);
        let mut command = command(3, 1);
        let ClientViewIntent::Panel {
            terminal_context_id,
            mut tab_id,
            ..
        } = command.intent
        else {
            panic!()
        };
        if !selected {
            tab_id = WorkbenchTabId::parse(&format!("{:032x}", 99)).unwrap();
        }
        command.intent = ClientViewIntent::SetSplitRatio {
            expected_split: None,
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            path: ClientSplitPath::new(vec![]).unwrap(),
            ratio: ClientSplitRatio::new(0.7).unwrap(),
        };
        let submit = ClientViewAction::Submit {
            command: Box::new(command),
        };
        assert!(matches!(
            status(execute(&mut registry, 20, submit.clone(), now)),
            ClientViewControlStatus::Queued {}
        ));
        assert!(matches!(
            execute(&mut registry, 1, sample(), now),
            ClientViewOutcome::Synchronized {
                control: Some(_),
                ..
            }
        ));
        let adjustment = ClientLayoutAdjustment {
            terminal_context_id,
            tab_id,
            workbench_revision: WorkbenchRevision::new(12),
            selected,
            change: ClientLayoutChange::Split {
                split: ClientSplitChange {
                    split_id: WorkbenchSplitId::parse(&format!("{:032x}", 90)).unwrap(),
                    first_slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 91)).unwrap(),
                    second_slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 92)).unwrap(),
                    previous_ratio: ClientSplitRatio::HALF,
                    ratio: ClientSplitRatio::new(0.7).unwrap(),
                },
            },
        };
        let mut ack = acknowledge(3);
        let ClientViewAction::Synchronize { completion, .. } = &mut ack else {
            panic!()
        };
        completion.as_mut().unwrap().outcome = ClientViewCompletionOutcome::LayoutAdjusted {
            adjustment: adjustment.clone(),
        };
        let mut invalid = ack.clone();
        let ClientViewAction::Synchronize { completion, .. } = &mut invalid else {
            panic!()
        };
        let ClientViewCompletionOutcome::LayoutAdjusted { adjustment } =
            &mut completion.as_mut().unwrap().outcome
        else {
            panic!()
        };
        let ClientLayoutChange::Split { split } = &mut adjustment.change else {
            panic!()
        };
        split.ratio = ClientSplitRatio::new(0.8).unwrap();
        assert!(matches!(
            execute(&mut registry, 1, invalid, now),
            ClientViewOutcome::Conflict {}
        ));
        assert!(matches!(
            execute(&mut registry, 2, ack.clone(), now),
            ClientViewOutcome::Conflict {}
        ));
        assert!(matches!(
            execute(&mut registry, 1, ack, now),
            ClientViewOutcome::Synchronized { control: None, .. }
        ));
        let completed = status(read(&mut registry, 3, now));
        assert!(
            matches!(&completed,ClientViewControlStatus::LayoutAdjusted {adjustment,..} if adjustment.selected==selected)
        );
        registry.disconnect(owner(1), now);
        assert_eq!(status(execute(&mut registry, 20, submit, now)), completed);
    }
}

#[test]
fn presentation_read_reserves_bounded_memory_and_reads_busy_without_allowing_tab_navigation() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    let mut busy = sample();
    let ClientViewAction::Synchronize { view, .. } = &mut busy else {
        panic!()
    };
    view.phase = ClientViewPhase::Busy;
    execute(&mut registry, 1, busy.clone(), now);
    let mut blocked = command(907, 1);
    let ClientViewIntent::Panel {
        terminal_context_id,
        tab_id,
        ..
    } = blocked.intent
    else {
        panic!()
    };
    blocked.intent = ClientViewIntent::Tab {
        terminal_context_id,
        tab_id,
    };
    assert!(matches!(
        status(execute(
            &mut registry,
            20,
            ClientViewAction::Submit {
                command: Box::new(blocked)
            },
            now
        )),
        ClientViewControlStatus::Rejected {
            reason: ClientViewControlReason::Busy
        }
    ));
    let mut target = command(908, 1);
    let ClientViewIntent::Panel { panel_id, .. } = target.intent else {
        panic!()
    };
    target.intent = ClientViewIntent::Presentation {
        panel_id: panel_id.clone(),
    };
    let submit = ClientViewAction::Submit {
        command: Box::new(target),
    };
    assert!(matches!(
        status(execute(&mut registry, 20, submit.clone(), now)),
        ClientViewControlStatus::Queued {}
    ));
    assert_eq!(
        registry.receipts[&operation_id(908)].payload_reservation,
        orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES
    );
    assert!(matches!(
        execute(&mut registry, 1, busy.clone(), now),
        ClientViewOutcome::Synchronized {
            control: Some(_),
            ..
        }
    ));
    let ClientViewAction::Synchronize {
        completion,
        heartbeat,
        ..
    } = &mut busy
    else {
        panic!()
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    *completion = Some(ClientViewCompletion {
        operation_id: operation_id(908),
        outcome: ClientViewCompletionOutcome::PresentationRead {
            presentation: Box::new(orchbee_workbench_protocol::ClientPresentationSnapshot {
                workbench_revision: WorkbenchRevision::new(12),
                panel_id,
                presentation: None,
            }),
        },
    });
    assert!(matches!(
        execute(&mut registry, 2, busy.clone(), now),
        ClientViewOutcome::Conflict {}
    ));
    assert!(matches!(
        execute(&mut registry, 1, busy, now),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    let completed = status(read(&mut registry, 908, now));
    assert!(matches!(
        completed,
        ClientViewControlStatus::PresentationRead { .. }
    ));
    assert_eq!(status(execute(&mut registry, 20, submit, now)), completed);
}

#[test]
fn busy_client_delivers_panel_and_directional_focus_but_starting_refuses() {
    for phase in [ClientViewPhase::Busy, ClientViewPhase::Starting] {
        for directional in [false, true] {
            let now = Instant::now();
            let mut registry = ClientViewRegistry::default();
            let mut observed = sample();
            let ClientViewAction::Synchronize { view, .. } = &mut observed else {
                panic!()
            };
            view.phase = phase;
            execute(&mut registry, 1, observed.clone(), now);
            let mut target = command(909, 1);
            if directional {
                let ClientViewIntent::Panel {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = target.intent
                else {
                    panic!()
                };
                target.intent = ClientViewIntent::FocusDirection {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    direction: orchbee_workbench_protocol::ClientPanelDirection::Right,
                };
            }
            let outcome = status(execute(
                &mut registry,
                20,
                ClientViewAction::Submit {
                    command: Box::new(target.clone()),
                },
                now,
            ));
            if phase == ClientViewPhase::Starting {
                assert!(matches!(
                    outcome,
                    ClientViewControlStatus::Rejected {
                        reason: ClientViewControlReason::Busy
                    }
                ));
                assert!(matches!(
                    execute(&mut registry, 1, observed, now),
                    ClientViewOutcome::Synchronized { control: None, .. }
                ));
            } else {
                assert!(matches!(outcome, ClientViewControlStatus::Queued {}));
                let ClientViewOutcome::Synchronized {
                    control: Some(delivered),
                    ..
                } = execute(&mut registry, 1, observed.clone(), now)
                else {
                    panic!("Busy focus must reach the Client preflight")
                };
                assert_eq!(*delivered, target);
                if directional {
                    let ClientViewAction::Synchronize {
                        heartbeat,
                        completion,
                        ..
                    } = &mut observed
                    else {
                        panic!()
                    };
                    *heartbeat = ClientViewSequence::new(2).unwrap();
                    *completion = Some(ClientViewCompletion {
                        operation_id: target.operation_id,
                        outcome: ClientViewCompletionOutcome::DirectionUnchanged {
                            layout_revision: WorkbenchRevision::new(12),
                        },
                    });
                    assert!(matches!(
                        execute(&mut registry, 1, observed, now),
                        ClientViewOutcome::Synchronized { control: None, .. }
                    ));
                    assert!(
                        matches!(status(read(&mut registry, 909, now)), ClientViewControlStatus::DirectionUnchanged { view, .. } if view.phase == ClientViewPhase::Busy)
                    );
                }
            }
        }
    }
}

/// Only the Client knows whether a modal surface, a small window or another not-ready state
/// blocks a popup, so the Server delivers a popup open to a busy Client instead of refusing it as
/// busy itself; a starting Client and the single effect slot still refuse it.
#[test]
fn popup_open_reaches_a_busy_client_so_the_client_names_the_cause() {
    for phase in [
        ClientViewPhase::Ready,
        ClientViewPhase::Busy,
        ClientViewPhase::Starting,
    ] {
        let now = Instant::now();
        let mut registry = ClientViewRegistry::default();
        let mut sample = sample();
        let ClientViewAction::Synchronize { view, .. } = &mut sample else {
            panic!()
        };
        view.phase = phase;
        execute(&mut registry, 1, sample.clone(), now);
        let mut popup = command(990, 1);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = popup.intent
        else {
            panic!()
        };
        popup.intent = ClientViewIntent::PopupOpen {
            spec: Box::new(orchbee_workbench_protocol::PluginPopupSpec {
                selector: orchbee_workbench_protocol::PluginPaneSelector::new(
                    "tools".into(),
                    "dashboard".into(),
                )
                .unwrap(),
                source: orchbee_workbench_protocol::ClientPopupSource {
                    terminal_context_id,
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                    member: serde_json::from_value(serde_json::json!({
                        "presentationId": "3".repeat(32), "selectionGeneration": "1"
                    }))
                    .unwrap(),
                },
                context: Box::new(orchbee_workbench_protocol::PluginInvocationContext {
                    tab_id: Some(tab_id.to_string()),
                    focused_panel_id: Some(panel_id.to_string()),
                    ..Default::default()
                }),
                cwd: None,
                environment: Default::default(),
                width: None,
                height: None,
            }),
        };
        let outcome = execute(
            &mut registry,
            20,
            ClientViewAction::Submit {
                command: Box::new(popup.clone()),
            },
            now,
        );
        if phase == ClientViewPhase::Starting {
            assert_eq!(
                status(outcome),
                ClientViewControlStatus::Rejected {
                    reason: ClientViewControlReason::Busy
                }
            );
            continue;
        }
        assert_eq!(status(outcome), ClientViewControlStatus::Queued {});
        let mut second = popup.clone();
        second.operation_id = operation_id(991);
        assert_eq!(
            status(execute(
                &mut registry,
                20,
                ClientViewAction::Submit {
                    command: Box::new(second)
                },
                now
            )),
            ClientViewControlStatus::Rejected {
                reason: ClientViewControlReason::Busy
            }
        );
        let ClientViewOutcome::Synchronized { control, .. } =
            execute(&mut registry, 1, sample, now)
        else {
            panic!()
        };
        assert_eq!(control, Some(Box::new(popup)));
    }
}
