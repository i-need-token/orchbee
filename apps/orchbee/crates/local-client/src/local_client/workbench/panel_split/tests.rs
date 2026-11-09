use super::*;

fn id(value: u128) -> String {
    format!("{value:032x}")
}

fn request() -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: orchbee_workbench_protocol::ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(1)).unwrap(),
        session_id: orchbee_workbench_protocol::SessionId::parse(&id(2)).unwrap(),
        expected_revision: WorkbenchRevision::new(7),
        action: WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
            tab_id: WorkbenchTabId::parse(&id(4)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(5)).unwrap(),
            direction: orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Right,
            label: orchbee_workbench_protocol::WorkbenchLabel::parse("new").unwrap(),
            custom_label: None,
            geometry: WorkbenchTerminalGeometry::new(120, 40, 10, 20).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    }
}

fn committed_fields() -> (
    WorkbenchRevision,
    WorkbenchTerminalContextId,
    WorkbenchTabId,
    WorkbenchPanelId,
    WorkbenchPanelId,
    WorkbenchSplitId,
    WorkbenchPanelSlotId,
    WorkbenchPanelSlotId,
    TerminalPresentationId,
) {
    (
        WorkbenchRevision::new(8),
        WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
        WorkbenchTabId::parse(&id(4)).unwrap(),
        WorkbenchPanelId::parse(&id(5)).unwrap(),
        WorkbenchPanelId::parse(&id(6)).unwrap(),
        WorkbenchSplitId::parse(&id(7)).unwrap(),
        WorkbenchPanelSlotId::parse(&id(8)).unwrap(),
        WorkbenchPanelSlotId::parse(&id(9)).unwrap(),
        TerminalPresentationId::parse(&id(10)).unwrap(),
    )
}

#[test]
fn request_only_accepts_panel_split() {
    assert!(WorkbenchPanelSplitRequest::try_from(request()).is_ok());
    let mut other = request();
    other.action = WorkbenchAction::TabMove {
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
        tab_id: WorkbenchTabId::parse(&id(4)).unwrap(),
        direction: orchbee_workbench_protocol::WorkbenchTabMoveDirection::Next,
    };
    assert!(WorkbenchPanelSplitRequest::try_from(other).is_err());
}

#[test]
fn classifies_pending_and_created_results_with_exact_identity_and_geometry() {
    let command = request();
    let WorkbenchAction::PanelSplit {
        terminal_context_id,
        tab_id,
        panel_id,
        geometry,
        ..
    } = command.action
    else {
        unreachable!();
    };
    let (
        revision,
        context,
        committed_tab,
        committed_panel,
        new_panel,
        split,
        first_slot,
        second_slot,
        presentation,
    ) = committed_fields();
    let pending = classify_outcome(
        WorkbenchRevision::new(7),
        &terminal_context_id,
        &tab_id,
        &panel_id,
        geometry,
        WorkbenchCommandOutcome::PanelSplitPending {
            revision,
            terminal_context_id: context.clone(),
            tab_id: committed_tab.clone(),
            panel_id: committed_panel.clone(),
            new_panel_id: new_panel.clone(),
            split_id: split.clone(),
            first_slot_id: first_slot.clone(),
            second_slot_id: second_slot.clone(),
            presentation_id: presentation.clone(),
        },
    )
    .unwrap();
    assert!(matches!(
        pending,
        WorkbenchPanelSplitSubmission::Committed(WorkbenchPanelSplitCommit::Pending { .. })
    ));

    let created = classify_outcome(
        WorkbenchRevision::new(7),
        &terminal_context_id,
        &tab_id,
        &panel_id,
        geometry,
        WorkbenchCommandOutcome::PanelSplitCreated {
            revision,
            terminal_context_id: context,
            tab_id: committed_tab,
            panel_id: committed_panel,
            new_panel_id: new_panel,
            split_id: split,
            first_slot_id: first_slot,
            second_slot_id: second_slot,
            presentation_id: presentation,
            terminal_outcome: WorkbenchTerminalOutcome::Started { geometry },
        },
    )
    .unwrap();
    assert!(matches!(
        created,
        WorkbenchPanelSplitSubmission::Committed(WorkbenchPanelSplitCommit::Created { .. })
    ));
}

#[test]
fn mismatched_panel_or_started_geometry_is_unknown() {
    let command = request();
    let WorkbenchAction::PanelSplit {
        terminal_context_id,
        tab_id,
        panel_id,
        geometry,
        ..
    } = command.action
    else {
        unreachable!();
    };
    let (
        revision,
        context,
        committed_tab,
        _committed_panel,
        new_panel,
        split,
        first_slot,
        second_slot,
        presentation,
    ) = committed_fields();
    let wrong_panel = WorkbenchPanelId::parse(&id(99)).unwrap();
    assert!(matches!(
        classify_outcome(
            WorkbenchRevision::new(7),
            &terminal_context_id,
            &tab_id,
            &panel_id,
            geometry,
            WorkbenchCommandOutcome::PanelSplitPending {
                revision,
                terminal_context_id: context.clone(),
                tab_id: committed_tab.clone(),
                panel_id: wrong_panel,
                new_panel_id: new_panel.clone(),
                split_id: split.clone(),
                first_slot_id: first_slot.clone(),
                second_slot_id: second_slot.clone(),
                presentation_id: presentation.clone(),
            },
        ),
        Err(WorkbenchPanelSplitOutcomeUnknownReason::PanelMismatch { .. })
    ));
    let wrong_geometry = WorkbenchTerminalGeometry::new(119, 40, 10, 20).unwrap();
    let (
        revision,
        context,
        committed_tab,
        committed_panel,
        new_panel,
        split,
        first_slot,
        second_slot,
        presentation,
    ) = committed_fields();
    assert!(matches!(
        classify_outcome(
            WorkbenchRevision::new(7),
            &terminal_context_id,
            &tab_id,
            &panel_id,
            geometry,
            WorkbenchCommandOutcome::PanelSplitCreated {
                revision,
                terminal_context_id: context,
                tab_id: committed_tab,
                panel_id: committed_panel,
                new_panel_id: new_panel,
                split_id: split,
                first_slot_id: first_slot,
                second_slot_id: second_slot,
                presentation_id: presentation,
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: wrong_geometry,
                },
            },
        ),
        Err(WorkbenchPanelSplitOutcomeUnknownReason::StartedGeometryMismatch { .. })
    ));
}
