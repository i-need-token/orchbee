use super::*;
use crate::terminal_session::state::RefreshDecision;
use orchbee_workbench_protocol::{PanelNode, WorkbenchSnapshotOutcome};

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
            WireKind::Obcs,
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
    let old = drive_open_snapshot(&mut harness, source.clone())?;
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
