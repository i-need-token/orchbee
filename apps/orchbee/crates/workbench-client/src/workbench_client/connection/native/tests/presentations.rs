use super::*;
use crate::workbench_state::RefreshDecision;
use orchbee_workbench_protocol::{PanelNode, WorkbenchSnapshotOutcome};

#[test]
fn direct_agent_switch_detaches_unexpected_host_before_poll_frame_or_control() -> TestResult {
    let geometry = fixture_geometry()?;
    let source = setup::snapshot(2, 1);
    let panel = source.terminal_contexts[0].tabs[0].root.clone();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = panel
    else {
        return Err(test_error("target panel"));
    };
    let mut expected = crate::machine_agents::test_support::physical(1);
    let TerminalViewerOutcome::Attached { host_epoch, .. } = attached_outcome()? else {
        return Err(test_error("Host fixture"));
    };
    expected.host_epoch =
        orchbee_agent_observation_protocol::AgentHostEpoch::parse(host_epoch.as_str())?;
    expected.host_terminal_id =
        orchbee_agent_observation_protocol::AgentHostTerminalId::parse(&format!("{:032x}", 999))?;
    let mut harness = Harness::start(
        vec![
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let old = drive_snapshot(&mut harness, source.clone())?;
    drive_attach(&mut harness, 0)?;
    super::commands::drive_controlling_after_attach(&mut harness, &old, geometry)?;
    harness.ui.admit_fenced_presentation(
        0,
        panel_id,
        primary_presentation.presentation_id,
        expected,
    )?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available { snapshot: source },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("old viewer detach"));
    };
    assert!(matches!(
        detach.action(),
        TerminalViewerAction::Detach { .. }
    ));
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
        UiEvent::SwitchOldDetached {
            continuation: SwitchDetachContinuation::Attach(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("fenced attach"));
    };
    assert!(matches!(
        attach.action(),
        TerminalViewerAction::AttachBinding { .. }
    ));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    let ClientMessage::TerminalViewer(cleanup) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "unexpected Host must detach before any control request",
        ));
    };
    assert!(
        matches!(cleanup.action(), TerminalViewerAction::Detach { .. }),
        "the first action on the unexpected Host must be cleanup"
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                cleanup.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CleanupSettled {
            continuation: CleanupContinuation::Stable,
            ..
        }
    ));
    assert!(harness.ui.state().ready().is_none());
    harness.finish()
}

#[test]
fn actual_child_switch_detaches_old_viewer_before_attaching_exact_child_binding() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut source = setup::snapshot(3, 1);
    let third = source.terminal_contexts[0].tabs.pop().unwrap();
    let PanelNode::Panel {
        primary_presentation: child,
        ..
    } = third.root
    else {
        unreachable!()
    };
    let member = child.presentation_id.clone();
    let PanelNode::Panel {
        child_presentations,
        panel_id,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    let panel = panel_id.clone();
    child_presentations.push(child);
    let mut harness = Harness::start(
        vec![
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let old = drive_snapshot(&mut harness, source.clone())?;
    assert_eq!(drive_attach(&mut harness, 0)?, old);
    super::commands::drive_controlling_after_attach(&mut harness, &old, geometry)?;
    harness
        .ui
        .admit_presentation(0, panel.clone(), member.clone())?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available { snapshot: source },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("old viewer Detach"));
    };
    assert_eq!(
        detach.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?
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
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchOldDetached {
            continuation: SwitchDetachContinuation::Attach(_),
            ..
        }
    ));
    // Binding 2 belongs to the child; it is not a third Tab or a new process.
    let key = drive_attach(&mut harness, 2)?;
    assert!(key.generation() > old.generation());
    super::commands::drive_controlling_after_attach(&mut harness, &key, geometry)?;
    let state = harness.ui.state();
    assert!(state.ready().is_some());
    assert_eq!(state.cache().tabs().len(), 2);
    let leaf = state.tab(0).unwrap().panel(0).unwrap();
    assert_eq!(state.selected_presentation(leaf).presentation_id(), &member);
    assert_eq!(leaf.panel_id(), &panel);
    assert_eq!(state.cache().revision().value(), 1);
    harness.finish()?;
    Ok(())
}

#[test]
fn unbound_bootstrap_does_not_attach_and_refresh_attaches_only_the_new_real_binding() -> TestResult
{
    let geometry = fixture_geometry()?;
    let bound = setup::snapshot(1, 2);
    let mut empty = bound.clone();
    empty.revision = orchbee_workbench_protocol::WorkbenchRevision::new(1);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut empty.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    primary_presentation.binding = None;
    // The first request after Bootstrap must be a Workbench read, never OBCS Attach.
    let mut harness = Harness::start(
        vec![
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let old = drive_snapshot(&mut harness, empty)?;
    assert_eq!(
        harness.ui.state().phase_kind(),
        ClientPhaseKind::SelectedWithoutAttachment
    );
    assert!(harness.ui.state().pane_facts().next().is_none());
    harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available { snapshot: bound },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let attached = drive_attach(&mut harness, 0)?;
    assert!(attached.generation() > old.generation());
    super::commands::drive_controlling_after_attach(&mut harness, &attached, geometry)?;
    assert!(harness.ui.state().ready().is_some() && harness.ui.state().invariant_holds());
    harness.finish()
}

#[test]
fn switching_to_unbound_child_detaches_before_empty_selection_without_new_attach() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut source = setup::snapshot(2, 1);
    let other = source.terminal_contexts[0].tabs.pop().unwrap();
    let PanelNode::Panel {
        primary_presentation: mut child,
        ..
    } = other.root
    else {
        unreachable!()
    };
    child.binding = None;
    let member = child.presentation_id.clone();
    let PanelNode::Panel {
        panel_id,
        child_presentations,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    let panel = panel_id.clone();
    child_presentations.push(child);
    let mut harness = Harness::start(
        vec![
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let old = drive_snapshot(&mut harness, source.clone())?;
    assert_eq!(drive_attach(&mut harness, 0)?, old);
    super::commands::drive_controlling_after_attach(&mut harness, &old, geometry)?;
    harness
        .ui
        .admit_presentation(0, panel.clone(), member.clone())?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: source.clone(),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("old viewer Detach"));
    };
    assert!(matches!(
        detach.action(),
        TerminalViewerAction::Detach { .. }
    ));
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
        UiEvent::SwitchOldDetached {
            continuation: SwitchDetachContinuation::SettledWithoutAttachment,
            ..
        }
    ));
    let state = harness.ui.state();
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::SelectedWithoutAttachment
    );
    assert!(state.pane_facts().next().is_none() && state.invariant_holds());
    assert_eq!(
        state
            .selected_presentation(state.tab(0).unwrap().panel(0).unwrap())
            .presentation_id(),
        &member
    );
    // Prove the loop remains usable and the next wire effect is read-only, not Attach.
    harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available { snapshot: source },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh {
            decision: RefreshDecision::Continue,
            ..
        }
    ));
    harness.finish()
}
