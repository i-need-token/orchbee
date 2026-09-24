use super::*;
use crate::terminal_session::ui::{
    membership_tests::snapshot,
    move_membership_tests::ready_ui,
    publication_hint_tests::{ready_split_ui, split_snapshot},
};
use orchbee_workbench_protocol::{
    ClientViewSequence, WorkbenchClientId, WorkbenchRequestId, WorkbenchRevision,
};

fn query(snapshot: &Snapshot, tab_index: usize) -> ClientViewControl {
    let cache = project_native_standalone_tabs(snapshot).unwrap();
    let tab = &cache.tabs()[tab_index];
    ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 801)).unwrap(),
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 802)).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::Layout {
            terminal_context_id: cache.terminal_context_id().clone(),
            tab_id: tab.tab_id().clone(),
            panel_id: tab.panel(0).unwrap().panel_id().clone(),
        },
    }
}

#[test]
fn selected_and_inactive_reads_preserve_selection_cache_and_empty_command_queue() {
    let (native, mailbox, _clock, _elapsed) = ready_ui();
    let before = native.control_observation(false).unwrap();
    for tab in [0, 1] {
        let snapshot = snapshot(2, 1);
        let result = native
            .read_client_layout(
                &query(&snapshot, tab),
                &snapshot,
                Rect::new(0, 0, 120, 40),
                SidebarSurface::Hidden,
            )
            .unwrap();
        assert_eq!(result.selected, tab == 0);
        assert_eq!(result.focused_panel_id.is_some(), tab == 0);
        assert_eq!(result.area, cell(Rect::new(1, 2, 118, 36)));
        assert_eq!(result.panels.len(), 1);
        assert_eq!(result.panels[0].displayed, Some(result.area));
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "reads must not queue attachment, refresh or resize"
        );
    }
    // A read cannot install a newer Snapshot into the selected Tab's rendered membership.
    let newer = snapshot(2, 2);
    assert_eq!(
        native.read_client_layout(
            &query(&newer, 0),
            &newer,
            Rect::new(0, 0, 120, 40),
            SidebarSurface::Hidden
        ),
        Err(Reason::ProjectionUnavailable)
    );
    let result = native
        .read_client_layout(
            &query(&newer, 1),
            &newer,
            Rect::new(0, 0, 120, 40),
            SidebarSurface::Hidden,
        )
        .unwrap();
    assert_eq!(result.workbench_revision, WorkbenchRevision::new(2));
    assert!(!result.selected);
    assert_eq!(
        native.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(1)
    );
    assert_eq!(native.control_observation(false).unwrap(), before);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn actual_chrome_rounding_zoom_and_minimal_collapse_are_observed_without_mutation() {
    let (mut native, mailbox, _clock, _elapsed) = ready_split_ui();
    // The shared ready fixture leaves one ordinary Poll queued. Retire that fixture work first;
    // subsequent observations must enqueue nothing of their own.
    let initial = mailbox
        .try_recv()
        .unwrap()
        .expect("fixture's ordinary Poll");
    assert!(matches!(
        initial.record(),
        crate::terminal_session::mailbox::QueuedCommand::Poll(_)
    ));
    mailbox.finish_lease(initial).unwrap();
    let snapshot = split_snapshot(1);
    let command = query(&snapshot, 0);
    let result = native
        .read_client_layout(
            &command,
            &snapshot,
            Rect::new(0, 0, 120, 40),
            SidebarSurface::Hidden,
        )
        .unwrap();
    assert_eq!(result.panels.len(), 2);
    assert_eq!(
        result.panels[0].unzoomed,
        Some(cell(Rect::new(1, 2, 59, 36)))
    );
    assert_eq!(
        result.panels[0].terminal,
        Some(cell(Rect::new(1, 3, 59, 35)))
    );
    assert_eq!(
        result.panels[1].unzoomed,
        Some(cell(Rect::new(61, 2, 58, 36)))
    );
    assert_eq!(
        result.splits[0].divider,
        Some(cell(Rect::new(60, 2, 1, 36)))
    );
    assert_eq!(
        result.neighbors.right.as_ref(),
        Some(&result.panels[1].panel_id)
    );
    assert!(result.neighbors.left.is_none());

    native.toggle_zoom();
    let before = native.control_observation(false).unwrap();
    let zoomed = native
        .read_client_layout(
            &command,
            &snapshot,
            Rect::new(0, 0, 120, 40),
            SidebarSurface::Hidden,
        )
        .unwrap();
    assert!(zoomed.zoomed);
    assert_eq!(zoomed.panels[0].unzoomed, result.panels[0].unzoomed);
    assert_eq!(zoomed.panels[0].displayed, Some(zoomed.area));
    assert_eq!(zoomed.panels[1].displayed, None);
    assert_eq!(zoomed.neighbors, result.neighbors);
    assert_eq!(native.control_observation(false).unwrap(), before);

    let minimal = native
        .read_client_layout(
            &command,
            &snapshot,
            Rect::new(0, 0, 50, 10),
            SidebarSurface::Hidden,
        )
        .unwrap();
    assert_eq!(
        minimal.panels[0].unzoomed,
        Some(cell(Rect::new(0, 0, 50, 9)))
    );
    assert_eq!(minimal.panels[1].unzoomed, None);
    assert_eq!(minimal.splits[0].area, None);
    assert!(minimal.neighbors.right.is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn an_unvisited_workspace_is_projected_without_entering_it() {
    use orchbee_workbench_protocol::{
        Project, TerminalContext, WorkbenchProjectId, WorkbenchTerminalContextId,
        WorkbenchWorkspaceId, Workspace,
    };
    let (native, mailbox, _clock, _elapsed) = ready_ui();
    let before = native.control_observation(false).unwrap();
    let mut snapshot = snapshot(2, 2);
    let mut command = query(&snapshot, 1);
    let context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 991)).unwrap();
    let ClientViewIntent::Layout {
        terminal_context_id,
        ..
    } = &mut command.intent
    else {
        panic!("layout");
    };
    *terminal_context_id = context_id.clone();
    let tab = snapshot.terminal_contexts[0].tabs.pop().unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context_id.clone(),
        groups: Vec::new(),
        tabs: vec![tab],
    });
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 992)).unwrap(),
        label: "Unvisited".into(),
        directories: Vec::new(),
        workspaces: vec![Workspace {
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 993)).unwrap(),
            terminal_context_id: context_id.clone(),
            label: "New workspace".into(),
            primary_directory_id: None,
        }],
    });
    let result = native
        .read_client_layout(
            &command,
            &snapshot,
            Rect::new(0, 0, 120, 40),
            SidebarSurface::Docked,
        )
        .unwrap();
    assert_eq!(result.terminal_context_id, context_id);
    assert_eq!(result.workbench_revision, WorkbenchRevision::new(2));
    assert_eq!(result.area, cell(Rect::new(31, 2, 88, 36)));
    assert!(!result.selected);
    assert!(result.focused_panel_id.is_none());
    assert_eq!(native.control_observation(false).unwrap(), before);
    assert_eq!(native.state.as_ref().unwrap().cache().navigation().len(), 1);
    assert!(mailbox.try_recv().unwrap().is_none());
}
