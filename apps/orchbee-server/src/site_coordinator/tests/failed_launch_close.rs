//! Failed launch removal uses real admission/settlement/publication with synthetic Worker replies.
mod idle;
use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion, tab_command,
};
use super::*;
use orchbee_server::CloseContinuationNext;
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchTabCloseStatus,
};
use std::time::Instant;

fn failed_completion(command: &CapabilityNodeMessage) -> Completion {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = command else {
        panic!("launch");
    };
    Completion::Launch(
        TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure {
                host_epoch: identity().host_epoch(),
            }),
        )
        .unwrap(),
    )
}

fn close_command(
    coordinator: &mut SiteCoordinator,
    request: u128,
    action: WorkbenchAction,
) -> WorkbenchCommand {
    let current = snapshot(coordinator);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        site_id: current.site_id,
        expected_revision: current.revision,
        action,
    }
}

fn publish_without_host(coordinator: &mut SiteCoordinator, close: &WorkbenchCommand) {
    let now = Instant::now();
    let step = coordinator
        .close_authority()
        .unwrap()
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    assert_eq!(step.next(), CloseContinuationNext::Publication);
    let _changed = coordinator
        .publish_terminal_tab_close(&close.request_id, now)
        .unwrap();
    assert!(
        matches!(coordinator.close_authority().unwrap().terminal_tab_close_status(&close.request_id),
        Some(WorkbenchTabCloseStatus::Closed { revision }) if revision.value() == close.expected_revision.value() + 1)
    );
}

#[test]
fn failed_tab_closes_without_worker_and_replays_original_acknowledgment() {
    let mut coordinator = site();
    let create = tab_command(&coordinator, 100, 0, "failed");
    let pending = dispatch_tab(&mut coordinator, create, 101);
    let completion = failed_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, _) = super::panel_split::workspace_leaf(&mut coordinator);
    let close = close_command(
        &mut coordinator,
        102,
        WorkbenchAction::TabClose {
            terminal_context_id: context.clone(),
            tab_id: tab,
        },
    );
    let accepted = coordinator
        .execute_terminal_tab_close(close.clone(), None, true, Instant::now())
        .unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    assert!(
        coordinator
            .close_authority()
            .unwrap()
            .terminal_tab_close_target(&close.request_id)
            .unwrap()
            .runtime()
            .is_none()
    );
    publish_without_host(&mut coordinator, &close);
    assert!(
        snapshot(&mut coordinator)
            .terminal_contexts
            .iter()
            .find(|item| item.terminal_context_id == context)
            .unwrap()
            .tabs
            .is_empty()
    );
    assert_eq!(
        coordinator
            .execute_terminal_tab_close(close, None, false, Instant::now())
            .unwrap(),
        accepted
    );
}

#[test]
fn unknown_launch_and_stopped_maintenance_cannot_remove_the_tab() {
    for unknown in [false, true] {
        let mut coordinator = site();
        let create = tab_command(&coordinator, 100, 0, "failed");
        let pending = dispatch_tab(&mut coordinator, create, 101);
        coordinator
            .complete_terminal_tab_create(
                pending,
                if unknown {
                    Completion::Unknown
                } else {
                    Completion::Unavailable
                },
                Some(worker(1)),
            )
            .unwrap();
        let (context, tab, _) = super::panel_split::workspace_leaf(&mut coordinator);
        let close = close_command(
            &mut coordinator,
            102,
            WorkbenchAction::TabClose {
                terminal_context_id: context,
                tab_id: tab,
            },
        );
        let before = snapshot(&mut coordinator);
        let result = coordinator
            .execute_terminal_tab_close(close.clone(), None, unknown, Instant::now())
            .unwrap();
        assert!(
            matches!(result.outcome, WorkbenchCommandOutcome::Unavailable { reason } if reason == if unknown { WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable } else { WorkbenchCommandUnavailableReason::ServerBusy })
        );
        assert_eq!(snapshot(&mut coordinator), before);
        assert!(
            coordinator
                .close_authority()
                .unwrap()
                .terminal_tab_close_status(&close.request_id)
                .is_none()
        );
    }
}

#[test]
fn failed_split_panel_closes_without_disposing_its_running_sibling() {
    let mut coordinator = site();
    let create = tab_command(&coordinator, 100, 0, "running");
    let pending = dispatch_tab(&mut coordinator, create, 101);
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel) = super::panel_split::workspace_leaf(&mut coordinator);
    let split = super::panel_split::split_command(
        &mut coordinator,
        102,
        context.clone(),
        tab.clone(),
        panel.clone(),
        orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Right,
    );
    let pending = super::panel_split::dispatch_split(&mut coordinator, split, 103);
    let completion = failed_completion(pending.command());
    coordinator
        .complete_terminal_panel_create(pending, completion, Some(worker(1)))
        .unwrap();
    let members = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .terminal_tree_membership(&context)
        .unwrap();
    let failed = members
        .members()
        .iter()
        .find(|member| member.panel_id() != &panel)
        .unwrap()
        .panel_id()
        .clone();
    let close = close_command(
        &mut coordinator,
        104,
        WorkbenchAction::PanelClose {
            terminal_context_id: context,
            tab_id: tab,
            panel_id: failed,
        },
    );
    assert!(matches!(
        coordinator
            .execute_terminal_tab_close(close.clone(), None, true, Instant::now())
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    publish_without_host(&mut coordinator, &close);
    let (_, _, surviving_panel) = super::panel_split::workspace_leaf(&mut coordinator);
    assert_eq!(surviving_panel, panel);
}

#[test]
fn workspace_close_keeps_failed_members_and_requires_live_members_before_admission() {
    for mixed in [false, true] {
        let mut coordinator = site();
        let context = configure_workspace(&mut coordinator, "C:/workspace");
        for (request, successful) in [(100, false), (110, mixed)] {
            let revision = snapshot(&mut coordinator).revision.value();
            let create =
                context_tab_command(&coordinator, request, revision, context.clone(), "member");
            let pending = dispatch_tab(&mut coordinator, create, request + 1);
            let completion = if successful {
                immediate_completion(pending.command())
            } else {
                failed_completion(pending.command())
            };
            coordinator
                .complete_terminal_tab_create(pending, completion, Some(worker(1)))
                .unwrap();
        }
        let before = snapshot(&mut coordinator);
        let close = close_command(
            &mut coordinator,
            120,
            WorkbenchAction::WorkspaceClose {
                workspace_id: before.navigation.workspaces[0].workspace_id.clone(),
            },
        );
        let result = coordinator
            .execute_workspace_close(close.clone(), None, true, Instant::now())
            .unwrap();
        if mixed {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable
                }
            ));
            assert_eq!(snapshot(&mut coordinator), before);
            assert!(
                coordinator
                    .close_authority()
                    .unwrap()
                    .terminal_tab_close_status(&close.request_id)
                    .is_none()
            );
            assert!(matches!(
                coordinator
                    .execute_workspace_close(close.clone(), Some(worker(1)), true, Instant::now())
                    .unwrap()
                    .outcome,
                WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
            ));
            let _changed =
                super::multileaf_membership::drive_close_to_publication(&mut coordinator, &close);
        } else {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
            ));
            publish_without_host(&mut coordinator, &close);
        }
        assert!(snapshot(&mut coordinator).navigation.workspaces.is_empty());
        assert!(matches!(
            coordinator
                .close_authority()
                .unwrap()
                .terminal_tab_close_status(&close.request_id),
            Some(WorkbenchTabCloseStatus::Closed { .. })
        ));
    }
}
