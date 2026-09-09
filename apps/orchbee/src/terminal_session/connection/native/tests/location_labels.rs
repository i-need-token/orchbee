use super::{
    commands::{drive_controlling_after_attach, drive_controlling_ready_from_snapshot},
    setup::*,
};
use crate::terminal_session::state::{
    RefreshDecision,
    workspace_setup::{NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupStatus},
};
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerResult,
};
use orchbee_workbench_protocol::{
    Project, ProtocolVersion, Snapshot, TerminalContext, WorkbenchAction, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchProjectId, WorkbenchRequest, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSnapshotUnavailableReason, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    Workspace,
};
use std::sync::atomic::Ordering;

fn workspace_snapshot() -> Snapshot {
    let mut snapshot = snapshot(2, 1);
    let tab = snapshot.terminal_contexts[0].tabs.pop().unwrap();
    let context = WorkbenchTerminalContextId::parse("00000000000000000000000000000110").unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context.clone(),
        groups: Vec::new(),
        tabs: vec![tab],
    });
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse("00000000000000000000000000000070").unwrap(),
        label: "Original Project".into(),
        directories: Vec::new(),
        workspaces: vec![Workspace {
            workspace_id: WorkbenchWorkspaceId::parse("00000000000000000000000000000071").unwrap(),
            label: "Original Workspace".into(),
            primary_directory_id: None,
            terminal_context_id: context,
        }],
    });
    snapshot
}

fn respond_snapshot(harness: &Harness, outcome: WorkbenchSnapshotOutcome) -> TestResult {
    let WorkbenchRequest::SnapshotRead(request) = harness.carrier.take_workbench()? else {
        return Err(test_error(
            "location rename may only observe its known result",
        ));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome,
        }))
}

#[test]
fn location_rename_and_noop_recover_by_snapshot_only_without_replacing_terminal_identity()
-> TestResult {
    for target in [
        NativeLocationLabelTarget::Project,
        NativeLocationLabelTarget::Workspace,
    ] {
        for unchanged in [false, true] {
            let geometry = fixture_geometry()?;
            let mut steps = vec![WireKind::Obcs, WireKind::Workbench];
            steps.extend([WireKind::Obcs; 5]);
            steps.push(WireKind::Workbench); // Explicit Workspace selection.
            steps.extend([WireKind::Obcs; 6]); // Old Detach and Workspace Attach/Acquire/Resize/Poll/Feedback.
            steps.extend([WireKind::Workbench; 3]); // One rename, failed snapshot, explicit snapshot recovery.
            let mut harness = Harness::start(steps, geometry)?;
            let initial = workspace_snapshot();
            drive_controlling_ready_from_snapshot(&mut harness, initial.clone(), geometry)?;
            harness.ui.admit_context_selection()?;
            respond_snapshot(
                &harness,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: initial.clone(),
                },
            )?;
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::Refresh {
                    decision: RefreshDecision::Cleanup(_),
                    ..
                }
            ));
            let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
                return Err(test_error("selection must detach original Viewer"));
            };
            assert_eq!(detach.action(), &TerminalViewerAction::Detach {});
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        detach.request_id().clone(),
                        TerminalViewerOutcome::Detached {},
                    ),
                ))?;
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::Refresh {
                    decision: RefreshDecision::AttachRecovered(_),
                    ..
                }
            ));
            let old = drive_attach(&mut harness, 1)?;
            drive_controlling_after_attach(&mut harness, &old, geometry)?;
            let baseline = harness.ui.state().cache().clone();
            let controller = harness.ui.state().view().controller_mode().cloned();
            let draft = harness
                .ui
                .location_label_draft(target, (!unchanged).then_some("Exact renamed label"))?;
            harness.ui.enqueue_setup_token(draft)?;
            let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
                return Err(test_error("expected one location rename command"));
            };
            assert_eq!(command.site_id, initial.site_id);
            assert_eq!(command.expected_revision, initial.revision);
            let project = &initial.navigation.projects[0];
            let original_label = match target {
                NativeLocationLabelTarget::Project => &project.label,
                NativeLocationLabelTarget::Workspace => &project.workspaces[0].label,
            };
            let expected_label = if unchanged {
                original_label.as_str()
            } else {
                "Exact renamed label"
            };
            match (&command.action, target) {
                (
                    WorkbenchAction::ProjectRename { project_id, label },
                    NativeLocationLabelTarget::Project,
                ) => {
                    assert_eq!(project_id, &project.project_id);
                    assert_eq!(label.as_str(), expected_label);
                }
                (
                    WorkbenchAction::WorkspaceRename {
                        project_id,
                        workspace_id,
                        label,
                    },
                    NativeLocationLabelTarget::Workspace,
                ) => {
                    assert_eq!(project_id, &project.project_id);
                    assert_eq!(workspace_id, &project.workspaces[0].workspace_id);
                    assert_eq!(label.as_str(), expected_label);
                }
                _ => return Err(test_error("rename target was silently retargeted")),
            }
            let revision = WorkbenchRevision::new(if unchanged { 1 } else { 2 });
            let outcome = if unchanged {
                WorkbenchCommandOutcome::Unchanged { revision }
            } else {
                WorkbenchCommandOutcome::Renamed { revision }
            };
            harness
                .carrier
                .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome,
                }))?;
            respond_snapshot(
                &harness,
                WorkbenchSnapshotOutcome::Unavailable {
                    reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            )?;
            harness.ui.settle_setup_next()?;
            assert_eq!(
                harness.ui.state().setup_status(),
                Some(NativeSetupStatus::LocationLabel {
                    target,
                    status: NativeLocationLabelStatus::CommittedNeedRefresh
                })
            );
            assert_eq!(harness.ui.state().cache(), &baseline);
            assert_eq!(harness.ui.state().attached_key(), Some(&old));
            harness.elapsed_millis.store(60_000, Ordering::SeqCst);
            harness.ui.refresh_known_setup()?;
            let mut recovered = initial.clone();
            recovered.revision = revision;
            match target {
                NativeLocationLabelTarget::Project => {
                    recovered.navigation.projects[0].label = expected_label.into()
                }
                NativeLocationLabelTarget::Workspace => {
                    recovered.navigation.projects[0].workspaces[0].label = expected_label.into()
                }
            }
            respond_snapshot(
                &harness,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: recovered.clone(),
                },
            )?;
            harness.ui.settle_setup_next()?;
            assert_eq!(
                harness.ui.state().setup_status(),
                Some(NativeSetupStatus::LocationLabel {
                    target,
                    status: if unchanged {
                        NativeLocationLabelStatus::Unchanged
                    } else {
                        NativeLocationLabelStatus::Renamed
                    }
                })
            );
            assert_eq!(harness.ui.state().cache().revision(), revision);
            assert_eq!(
                harness.ui.state().cache().projects(),
                recovered.navigation.projects.as_slice()
            );
            assert_eq!(harness.ui.state().cache().tabs(), baseline.tabs());
            assert_eq!(harness.ui.state().attached_key(), Some(&old));
            assert_eq!(
                harness.ui.state().view().controller_mode(),
                controller.as_ref()
            );
            assert!(harness.ui.state().setup_draft().is_none());
            // The script admits no Viewer or Control frames after rename: no detach, attach,
            // shell start, mutation replay, or implicit second observation can pass unnoticed.
            harness.finish()?;
        }
    }
    Ok(())
}
