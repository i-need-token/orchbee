use super::*;
use crate::terminal_session::state::RefreshDecision;
use orchbee_workbench_protocol::{
    Project, Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace,
};

fn snapshot(empty: bool, unsupported: bool) -> Snapshot {
    let mut snapshot = setup::snapshot(3, 2);
    let mut tab = snapshot.terminal_contexts[0].tabs.pop().unwrap();
    tab.pinned = unsupported;
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 272)).unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context.clone(),
        groups: Vec::new(),
        tabs: if empty { Vec::new() } else { vec![tab] },
    });
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 70)).unwrap(),
        label: "Project".into(),
        directories: Vec::new(),
        workspaces: vec![Workspace {
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 71)).unwrap(),
            label: "Workspace".into(),
            primary_directory_id: None,
            terminal_context_id: context,
        }],
    });
    snapshot
}

fn return_snapshot(harness: &mut Harness, snapshot: Snapshot) -> TestResult {
    let WorkbenchRequest::SnapshotRead(read) = harness.carrier.take_workbench()? else {
        return Err(test_error("only SnapshotRead may precede context cleanup"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: read.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }))
}

#[test]
fn actual_context_selection_reads_snapshot_then_detaches_exact_viewer_before_workspace_attach()
-> TestResult {
    for (empty, unsupported) in [(false, false), (true, false), (false, true)] {
        let geometry = fixture_geometry()?;
        let mut steps = vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ];
        steps.extend([WireKind::Workbench, WireKind::Workbench]);
        if !unsupported {
            steps.push(WireKind::Obcs);
            if !empty {
                steps.extend([WireKind::Obcs; 5]);
            }
        }
        let mut harness = Harness::start(steps, geometry)?;
        let old = drive_controlling_startup(&mut harness, geometry, 2)?;
        let refresh = harness.ui.admit_refresh()?;
        return_snapshot(&mut harness, snapshot(empty, unsupported))?;
        assert!(
            matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::Refresh { generation, decision: RefreshDecision::Continue } if generation == refresh)
        );
        assert_eq!(harness.ui.state().view().context_count(), 2);
        let baseline = harness.ui.state().cache().clone();
        let selected = harness.ui.admit_context_selection()?;
        return_snapshot(&mut harness, snapshot(empty, unsupported))?;
        let event = harness.ui.settle_next(FrameCut::None)?;
        if unsupported {
            assert!(
                matches!(event, UiEvent::Refresh { generation, decision: RefreshDecision::Continue } if generation == selected)
            );
            assert_eq!(harness.ui.state().cache(), &baseline);
            assert_eq!(harness.ui.state().attached_key(), Some(&old));
        } else {
            assert!(
                matches!(event, UiEvent::Refresh { generation, decision: RefreshDecision::Cleanup(_) } if generation == selected)
            );
            assert_eq!(harness.ui.state().cache(), &baseline);
            assert_eq!(harness.ui.state().attached_key(), Some(&old));
            let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
                return Err(test_error("exact old Detach"));
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
            let event = harness.ui.settle_next(FrameCut::None)?;
            assert_eq!(harness.ui.state().view().active_context_position(), Some(2));
            if empty {
                assert!(matches!(
                    event,
                    UiEvent::Refresh {
                        decision: RefreshDecision::SettledWithoutAttachment,
                        ..
                    }
                ));
                assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Empty);
            } else {
                assert!(matches!(
                    event,
                    UiEvent::Refresh {
                        decision: RefreshDecision::AttachRecovered(_),
                        ..
                    }
                ));
                let key = drive_attach(&mut harness, 2)?;
                assert!(key.generation() > old.generation());
                super::commands::drive_controlling_after_attach(&mut harness, &key, geometry)?;
                assert!(harness.ui.state().ready().is_some());
            }
        }
        // The strict mixed-wire script admits no navigation command, Create, Start or Dispose.
        harness.finish()?;
    }
    Ok(())
}
