//! Immediate admitted reorder, refusal rollback and authoritative handoff through the real UI lane.
use super::*;
use crate::machine_picker::{Choice, Picker, SidebarRow};
use orchbee_workbench_protocol::{NavigationConfiguration, configure_navigation};

fn ready_sidebar() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let mut snapshot = super::super::membership::snapshot(1, 1);
    for n in [800, 801] {
        let terminal_context_id =
            WorkbenchTerminalContextId::parse(&format!("{:032x}", n + 10)).unwrap();
        snapshot.navigation.workspaces.push(Workspace {
            terminal_directory: None,
            automatic_label: false,
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{n:032x}")).unwrap(),
            terminal_context_id: terminal_context_id.clone(),
            project_id: None,
            label: "same workspace".into(),
            primary_directory_id: None,
        });
        snapshot.terminal_contexts.push(TerminalContext {
            terminal_context_id,
            tabs: vec![],
            groups: vec![],
        });
    }
    ready_snapshot(snapshot)
}

fn order(picker: &Picker) -> Vec<crate::machine_picker::SidebarTarget> {
    picker
        .sidebar_navigation_rows()
        .iter()
        .map(SidebarRow::target)
        .collect()
}

#[test]
fn reorder_projects_before_dequeue_and_settles_without_a_metadata_poll() {
    for committed in [false, true] {
        let (mut ui, mailbox, clock, snapshot) = ready_sidebar();
        let mut picker = Picker::default();
        picker.available = true;
        picker.observe_agent_sidebar(true);
        assert!(picker.observe_native_navigation(Choice::Local, ui.view().sidebar_navigation()));
        let original_order = order(&picker);
        let selection = ui.state.as_ref().unwrap().desired().cloned();
        let operation = NavigationConfiguration::MoveWorkspace {
            workspace_id: snapshot.navigation.workspaces[2].workspace_id.clone(),
            project_id: None,
            before_workspace_id: Some(snapshot.navigation.workspaces[1].workspace_id.clone()),
        };
        let source = ui.sidebar_reorder_source().unwrap();
        ui.commit_sidebar_reorder(&source, operation.clone());
        assert!(
            ui.sidebar_reorder_source().is_none(),
            "a second move cannot use the speculative order"
        );
        assert!(picker.observe_native_navigation(Choice::Local, ui.view().sidebar_navigation()));
        let projected_order = order(&picker);
        assert_ne!(
            projected_order, original_order,
            "visible feedback precedes any worker execution"
        );
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );
        assert_eq!(ui.state.as_ref().unwrap().desired(), selection.as_ref());
        assert!(!picker.observe_native_navigation(Choice::Local, ui.view().sidebar_navigation()));

        let wakeup = crate::ui_wakeup::UiWakeup::current();
        ui.set_ui_wakeup(Some(wakeup.clone()));
        wakeup.take();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
            panic!("reorder command")
        };
        assert_eq!(
            mailbox.mark_setup_in_flight(&marker.0).unwrap(),
            MutationResult::Applied
        );
        let wait = if committed {
            let mut after = snapshot.clone();
            after.revision = WorkbenchRevision::new(2);
            after.navigation = configure_navigation(&snapshot.navigation, &operation)
                .unwrap()
                .unwrap();
            place(
                &mailbox,
                &clock,
                &lease,
                WorkbenchSetupCommit::NavigationConfigured {
                    revision: after.revision,
                    unchanged: false,
                },
                after,
            )
        } else {
            assert!(mailbox.retain_setup_not_applied(&marker.0));
            mailbox
                .publish_setup_result_before(marker.0.clone(), SetupResult::NotApplied, &clock)
                .unwrap()
        };
        assert!(wakeup.take(), "the result interrupts the existing UI wait");
        ui.tick();
        mailbox.take_deposited_reply(&wait).unwrap().unwrap();
        picker.observe_native_navigation(Choice::Local, ui.view().sidebar_navigation());
        assert_eq!(
            order(&picker),
            if committed {
                projected_order
            } else {
                original_order
            }
        );
        assert_eq!(ui.state.as_ref().unwrap().desired(), selection.as_ref());
        assert!(picker.request.is_none());
        let settled_order = order(&picker);
        picker.observe_native_navigation(Choice::Local, None);
        assert_eq!(
            order(&picker),
            settled_order,
            "carrier loss cannot restore an older observer topology"
        );
        mailbox.finish_lease(lease).unwrap();
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "settlement does not replay a move"
        );
        mailbox.close(CarrierFailure::WorkerExited);
    }
}
