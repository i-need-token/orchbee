use super::*;
use orchbee_workbench_protocol::{
    ProtocolVersion, SiteId, WorkbenchClientViewResult, WorkbenchRequest, WorkbenchRevision,
    parse_workbench_request,
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
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
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
fn frozen_ui_never_renews_freshness_and_revision_changes_fence_delivery() {
    let now = Instant::now();
    let mut registry = ClientViewRegistry::default();
    execute(&mut registry, 1, sample(), now);
    let later = now + FRESHNESS;
    let ClientViewOutcome::Synchronized { client, .. } = execute(&mut registry, 1, sample(), later)
    else {
        panic!("sync");
    };
    assert_eq!(client.availability, ClientViewAvailability::Stale);
    assert_eq!(
        status(submit(&mut registry, 3, 1, later)),
        ClientViewControlStatus::Rejected {
            reason: ClientViewControlReason::StaleView
        }
    );
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
fn layout_reservations_bound_pending_payloads_and_shrink_after_valid_completion() {
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
            .map(|receipt| receipt.layout_reservation)
            .sum::<usize>(),
        MAXIMUM_LAYOUT_RESERVATION
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
    let retained = registry.receipts[&operation_id(101)].layout_reservation;
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
        .map(|receipt| receipt.layout_reservation)
        .sum::<usize>();
    assert!(matches!(
        status(query(&mut registry, 101, 1)),
        ClientViewControlStatus::LayoutRead { .. }
    ));
    assert_eq!(
        registry
            .receipts
            .values()
            .map(|receipt| receipt.layout_reservation)
            .sum::<usize>(),
        reserved
    );
    registry.disconnect(owner(2), now);
    assert_eq!(registry.receipts[&operation_id(102)].layout_reservation, 0);
    registry.disconnect(owner(1), now);
    assert_eq!(
        registry.receipts[&operation_id(101)].layout_reservation,
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
            .map(|receipt| receipt.layout_reservation)
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
