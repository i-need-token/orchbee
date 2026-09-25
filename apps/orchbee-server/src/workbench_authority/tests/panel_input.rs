use super::*;
use orchbee_workbench_protocol::{PanelRightClickTarget, WorkbenchPanelId};

fn policy(snapshot: &mut Snapshot, tab: usize) -> &mut PanelRightClickTarget {
    let PanelNode::Panel { right_click, .. } = &mut snapshot.terminal_contexts[0].tabs[tab].root
    else {
        panic!("leaf")
    };
    right_click
}
fn set(snapshot: &Snapshot, tab: usize, right_click: PanelRightClickTarget) -> WorkbenchAction {
    let context = &snapshot.terminal_contexts[0];
    let tab = &context.tabs[tab];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("leaf")
    };
    WorkbenchAction::PanelInputSet {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: tab.tab_id.clone(),
        panel_id: panel_id.clone(),
        right_click,
    }
}

#[test]
fn panel_input_changes_only_policy_and_retains_replay_and_pending_terminal_settlement() {
    let (mut authority, grant) = authority();
    let (_, second, pending) = install_two_tabs(&mut authority);
    let before = snapshot(&mut authority, &grant);
    let cursor = authority.last_identity;
    let request = command(1, 2, set(&before, 1, PanelRightClickTarget::Panel));
    let accepted = submit(&mut authority, &grant, request.clone());
    assert_eq!(
        accepted.outcome,
        WorkbenchCommandOutcome::PanelInputConfigured {
            revision: WorkbenchRevision::new(3)
        }
    );
    let mut expected = before.clone();
    expected.revision = WorkbenchRevision::new(3);
    *policy(&mut expected, 1) = PanelRightClickTarget::Panel;
    assert_eq!(snapshot(&mut authority, &grant), expected);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(submit(&mut authority, &grant, request.clone()), accepted);
    assert!(matches!(
        submit(
            &mut authority,
            &grant,
            command(1, 3, set(&before, 1, PanelRightClickTarget::Orchbee))
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
            command(2, 3, set(&before, 1, PanelRightClickTarget::Panel))
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
            command(3, 3, set(&before, 1, PanelRightClickTarget::Orchbee))
        )
        .outcome,
        WorkbenchCommandOutcome::PanelInputConfigured {
            revision: WorkbenchRevision::new(4)
        }
    );
    let mut restored = before.clone();
    restored.revision = WorkbenchRevision::new(4);
    assert_eq!(snapshot(&mut authority, &grant), restored);
    assert_eq!(submit(&mut authority, &grant, request), accepted);
    assert_eq!(authority.last_identity, cursor);
}

#[test]
fn panel_input_rejects_wrong_scope_stale_base_permission_and_capacity_atomically() {
    let (mut authority, grant) = authority();
    install_two_tabs(&mut authority);
    let before = snapshot(&mut authority, &grant);
    let request = command(1, 2, set(&before, 0, PanelRightClickTarget::Panel));
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());
    assert!(matches!(
        submit(&mut authority, &reader, request.clone()).outcome,
        WorkbenchCommandOutcome::Denied {}
    ));
    for index in 0..4 {
        let mut invalid = command(10 + index, 2, set(&before, 0, PanelRightClickTarget::Panel));
        let WorkbenchAction::PanelInputSet {
            terminal_context_id,
            tab_id,
            panel_id,
            ..
        } = &mut invalid.action
        else {
            panic!("input")
        };
        let reason = match index {
            0 => {
                *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(999)).unwrap();
                WorkbenchRejectionReason::TerminalContextUnavailable
            }
            1 => {
                *tab_id = before.terminal_contexts[0].tabs[1].tab_id.clone();
                WorkbenchRejectionReason::PanelUnavailable
            }
            2 => {
                *panel_id = WorkbenchPanelId::parse(&identity(999)).unwrap();
                WorkbenchRejectionReason::PanelUnavailable
            }
            _ => {
                invalid.expected_revision = WorkbenchRevision::new(1);
                WorkbenchRejectionReason::RevisionMismatch
            }
        };
        assert!(
            matches!(submit(&mut authority, &grant, invalid).outcome, WorkbenchCommandOutcome::Rejected { reason: actual, .. } if actual == reason)
        );
        assert_eq!(snapshot(&mut authority, &grant), before);
    }
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let entries = authority.ledger.len();
    assert_eq!(
        submit(&mut authority, &grant, request).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.ledger.len(), entries);
    assert_eq!(snapshot(&mut authority, &grant), before);
}
