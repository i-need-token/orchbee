//! Exercise the conditional owner through actual retained Create records.
use super::*;
use orchbee_workbench_protocol::{WorkbenchRejectionReason, WorkbenchRevision};

#[test]
fn conditional_delete_distinguishes_never_created_from_live_unknown_pending_and_missing_worker() {
    for project in [false, true] {
        for kind in 0..6 {
            let mut coordinator = site();
            let context = configure_workspace(&mut coordinator, "C:/workspace");
            let revision = snapshot(&mut coordinator).revision.value();
            let create =
                context_tab_command(&coordinator, 100, revision, context, "captured member");
            let pending = dispatch_tab(&mut coordinator, create, 101);
            if kind != 4 {
                let completion = match kind {
                    0 => failed_completion(pending.command()),
                    1 => Completion::Unavailable,
                    2 | 5 => immediate_completion(pending.command()),
                    3 => Completion::Unknown,
                    _ => unreachable!(),
                };
                coordinator
                    .complete_terminal_tab_create(pending, completion, Some(worker(1)))
                    .unwrap();
            }
            let before = snapshot(&mut coordinator);
            let action = if project {
                WorkbenchAction::ProjectDissolveIfIdle {
                    project_id: before.navigation.projects[0].project_id.clone(),
                }
            } else {
                WorkbenchAction::WorkspaceCloseIfIdle {
                    workspace_id: before.navigation.workspaces[0].workspace_id.clone(),
                }
            };
            let command = close_command(&mut coordinator, 102, action);
            let current_worker = (kind != 5).then(|| worker(1));
            let result = if project {
                coordinator.execute_project_dissolve_if_idle(command.clone(), current_worker)
            } else {
                coordinator.execute_workspace_close(
                    command.clone(),
                    current_worker,
                    true,
                    Instant::now(),
                )
            }
            .unwrap();
            match kind {
                0 | 1 => {
                    if project {
                        assert!(matches!(
                            result.outcome,
                            WorkbenchCommandOutcome::ProjectDissolved { .. }
                        ));
                        let mut expected = before.clone();
                        expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
                        expected.navigation.projects.clear();
                        expected.navigation.project_order.clear();
                        for workspace in &mut expected.navigation.workspaces {
                            workspace.project_id = None;
                        }
                        assert_eq!(
                            snapshot(&mut coordinator),
                            expected,
                            "organization only; failed member retained"
                        );
                    } else {
                        assert!(matches!(
                            result.outcome,
                            WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
                        ));
                        assert_eq!(
                            snapshot(&mut coordinator),
                            before,
                            "acceptance is a retained fence"
                        );
                        publish_without_host(&mut coordinator, &command);
                        assert!(snapshot(&mut coordinator).navigation.workspaces.is_empty());
                    }
                }
                2 => assert!(matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::Rejected {
                        reason: WorkbenchRejectionReason::ConfirmationRequired,
                        ..
                    }
                )),
                3 | 5 => assert!(matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::Unavailable {
                        reason: WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable
                    }
                )),
                4 => assert!(matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::Unavailable {
                        reason: WorkbenchCommandUnavailableReason::TerminalBusy
                    }
                )),
                _ => unreachable!(),
            }
            if kind >= 2 {
                assert_eq!(snapshot(&mut coordinator), before);
                assert!(
                    coordinator
                        .close_authority()
                        .unwrap()
                        .terminal_tab_close_status(&command.request_id)
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn busy_project_keeps_preflight_classification_even_when_runtime_authority_becomes_unavailable() {
    for has_worker in [false, true] {
        let mut coordinator = site();
        let context = configure_workspace(&mut coordinator, "C:/workspace");
        let revision = snapshot(&mut coordinator).revision.value();
        let create = context_tab_command(&coordinator, 100, revision, context, "live member");
        let pending = dispatch_tab(&mut coordinator, create, 101);
        let completion = immediate_completion(pending.command());
        coordinator
            .complete_terminal_tab_create(pending, completion, Some(worker(1)))
            .unwrap();
        let before = snapshot(&mut coordinator);
        let close = close_command(
            &mut coordinator,
            102,
            WorkbenchAction::WorkspaceClose {
                workspace_id: before.navigation.workspaces[0].workspace_id.clone(),
            },
        );
        assert!(matches!(
            coordinator
                .execute_workspace_close(close, Some(worker(1)), true, Instant::now())
                .unwrap()
                .outcome,
            WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
        ));
        let request = close_command(
            &mut coordinator,
            103,
            WorkbenchAction::ProjectDissolveIfIdle {
                project_id: before.navigation.projects[0].project_id.clone(),
            },
        );
        let result = coordinator
            .execute_project_dissolve_if_idle(request.clone(), has_worker.then(|| worker(1)))
            .unwrap();
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            }
        ));
        assert_eq!(snapshot(&mut coordinator), before);
        assert_eq!(
            coordinator
                .execute_project_dissolve_if_idle(request, None)
                .unwrap(),
            result
        );
    }
}
