use super::*;
use orchbee_workbench_protocol::{PanelLabelUpdate, Presentation, WorkbenchPanelId};

fn primary(snapshot: &mut Snapshot, tab: usize) -> &mut Presentation {
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut snapshot.terminal_contexts[0].tabs[tab].root
    else {
        panic!("leaf")
    };
    primary_presentation
}
fn rename(snapshot: &Snapshot, tab: usize, label: Option<&str>) -> WorkbenchAction {
    let context = &snapshot.terminal_contexts[0];
    let tab = &context.tabs[tab];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("leaf")
    };
    WorkbenchAction::PanelRename {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: tab.tab_id.clone(),
        panel_id: panel_id.clone(),
        label: PanelLabelUpdate::parse(label).unwrap(),
    }
}

#[test]
fn panel_labels_change_only_the_override_replay_normalized_input_and_survive_pending_creation() {
    let (mut authority, grant) = authority();
    let (_, second, pending) = install_two_tabs(&mut authority);
    let before = snapshot(&mut authority, &grant);
    let cursor = authority.last_identity;
    let first = command(1, 2, rename(&before, 1, Some("  标题\u{85}")));
    let accepted = submit(&mut authority, &grant, first);
    assert_eq!(
        accepted.outcome,
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(3)
        }
    );
    let mut expected = before.clone();
    expected.revision = WorkbenchRevision::new(3);
    primary(&mut expected, 1).custom_label = PanelLabelUpdate::parse(Some("标题"))
        .unwrap()
        .custom_label()
        .cloned();
    assert_eq!(snapshot(&mut authority, &grant), expected);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(1, 2, rename(&before, 1, Some("标题")))
        ),
        accepted
    );
    assert!(matches!(
        submit(
            &mut authority,
            &grant,
            command(1, 3, rename(&before, 1, None))
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(2, 3, rename(&before, 1, Some("标题")))
        )
        .outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(3)
        }
    );
    let settlement = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: second.geometry(),
            },
        )
        .unwrap()
        .commit();
    assert!(matches!(
        settlement.outcome,
        WorkbenchCommandOutcome::TabCreated { .. }
    ));
    assert_eq!(snapshot(&mut authority, &grant), expected);
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(3, 3, rename(&before, 1, Some(" \u{85}")))
        )
        .outcome,
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(4)
        }
    );
    let mut cleared = before.clone();
    cleared.revision = WorkbenchRevision::new(4);
    assert_eq!(snapshot(&mut authority, &grant), cleared);
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(4, 4, rename(&before, 1, None))
        )
        .outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(4)
        }
    );
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(1, 2, rename(&before, 1, Some("标题")))
        ),
        accepted
    );
    assert_eq!(authority.last_identity, cursor);
}

#[test]
fn panel_labels_refuse_scope_authorization_and_capacity_before_any_change() {
    let (mut authority, grant) = authority();
    install_two_tabs(&mut authority);
    let before = snapshot(&mut authority, &grant);
    let cursor = authority.last_identity;
    let request = command(1, 2, rename(&before, 0, Some("x")));
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());
    assert!(matches!(
        submit(&mut authority, &reader, request.clone()).outcome,
        WorkbenchCommandOutcome::Denied {}
    ));
    for (index, reason) in [
        WorkbenchRejectionReason::TerminalContextUnavailable,
        WorkbenchRejectionReason::TabUnavailable,
        WorkbenchRejectionReason::PanelUnavailable,
        WorkbenchRejectionReason::RevisionMismatch,
    ]
    .into_iter()
    .enumerate()
    {
        let mut invalid = command(10 + index as u128, 2, rename(&before, 0, Some("x")));
        let WorkbenchAction::PanelRename {
            terminal_context_id,
            tab_id,
            panel_id,
            ..
        } = &mut invalid.action
        else {
            panic!("rename")
        };
        match index {
            0 => *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(999)).unwrap(),
            1 => *tab_id = before.terminal_contexts[0].tabs[1].tab_id.clone(),
            2 => *panel_id = WorkbenchPanelId::parse(&identity(999)).unwrap(),
            _ => invalid.expected_revision = WorkbenchRevision::new(1),
        }
        // Existing sibling Tab with the first Panel is a scoped Panel miss.
        let reason = if index == 1 {
            WorkbenchRejectionReason::PanelUnavailable
        } else {
            reason
        };
        assert!(
            matches!(submit(&mut authority, &grant, invalid).outcome, WorkbenchCommandOutcome::Rejected { reason: actual, .. } if actual == reason)
        );
        assert_eq!(snapshot(&mut authority, &grant), before);
    }
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let ledger_count = authority.ledger.len();
    assert_eq!(
        submit(&mut authority, &grant, request).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.ledger.len(), ledger_count);
    assert_eq!(snapshot(&mut authority, &grant), before);
    assert_eq!(authority.last_identity, cursor);
    authority.ledger_bytes = 0;
    let mut exhausted = before.clone();
    exhausted.revision = WorkbenchRevision::new(u64::MAX);
    authority
        .projection
        .advance_projection(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(exhausted.clone())).unwrap(),
        )
        .unwrap();
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(50, u64::MAX, rename(&before, 0, None))
        )
        .outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(u64::MAX)
        }
    );
    assert!(matches!(
        submit(
            &mut authority,
            &grant,
            command(51, u64::MAX, rename(&before, 0, Some("x")))
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert_eq!(snapshot(&mut authority, &grant), exhausted);
}
