use super::*;
use crate::workbench_state::RefreshDecision;
use orchbee_workbench_protocol::{
    Project, Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace,
};

fn snapshot(empty: bool, unbound: bool) -> Snapshot {
    let mut snapshot = setup::snapshot(3, 2);
    let mut tab = snapshot.terminal_contexts[0].tabs.pop().unwrap();
    tab.pinned = true;
    if unbound {
        let orchbee_workbench_protocol::PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut tab.root
        else {
            panic!("single Panel fixture")
        };
        primary_presentation.binding = None;
    }
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 272)).unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: context.clone(),
        groups: Vec::new(),
        tabs: if empty { Vec::new() } else { vec![tab] },
    });
    snapshot.navigation.workspaces.extend(vec![Workspace {
        project_id: Some((WorkbenchProjectId::parse(&format!("{:032x}", 70)).unwrap()).clone()),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 271)).unwrap(),
        label: "Workspace".into(),
        primary_directory_id: None,
        terminal_context_id: context,
    }]);
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 70)).unwrap(),
        label: "Project".into(),
    });
    snapshot
}

fn return_snapshot(harness: &mut Harness, snapshot: Snapshot) -> TestResult {
    let read = harness.carrier.take_read()?;
    harness
        .carrier
        .respond_read(&read, WorkbenchSnapshotOutcome::Available { snapshot })
}

#[test]
fn actual_context_selection_reads_snapshot_then_detaches_exact_viewer_before_workspace_attach()
-> TestResult {
    for (empty, unbound) in [(false, false), (true, false), (false, true)] {
        let geometry = fixture_geometry()?;
        let mut steps = vec![
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ];
        steps.extend([WireKind::Workbench, WireKind::Workbench]);
        steps.push(WireKind::Obcs);
        if !empty && !unbound {
            steps.extend([WireKind::Obcs; 5]);
        }
        let mut harness = Harness::start(steps, geometry)?;
        let old = drive_controlling_startup(&mut harness, geometry, 2)?;
        let refresh = harness.ui.admit_refresh()?;
        return_snapshot(&mut harness, snapshot(empty, unbound))?;
        assert!(
            matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::Refresh { generation, decision: RefreshDecision::Continue } if generation == refresh)
        );
        assert_eq!(harness.ui.state().view().context_count(), 2);
        let baseline = harness.ui.state().cache().clone();
        let selected = harness.ui.admit_context_selection()?;
        return_snapshot(&mut harness, snapshot(empty, unbound))?;
        let event = harness.ui.settle_next(FrameCut::None)?;
        {
            assert!(
                matches!(event, UiEvent::Refresh { generation, decision: RefreshDecision::Cleanup(_) } if generation == selected)
            );
            assert_eq!(harness.ui.state().cache(), &baseline);
            assert_eq!(
                harness.ui.state().attached_key().map(|pane| pane.anchor()),
                Some(&old)
            );
            let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
                return Err(test_error("exact old Detach"));
            };
            assert_eq!(
                detach.action(),
                &TerminalViewerAction::Detach {
                    attachment_id: attached_viewer_id()?,
                }
            );
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
            if empty || unbound {
                assert!(matches!(
                    event,
                    UiEvent::Refresh {
                        decision: RefreshDecision::SettledWithoutAttachment,
                        ..
                    }
                ));
                assert_eq!(
                    harness.ui.state().phase_kind(),
                    if empty {
                        ClientPhaseKind::Empty
                    } else {
                        ClientPhaseKind::SelectedWithoutAttachment
                    }
                );
                assert!(!harness.ui.state().has_attachments());
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
