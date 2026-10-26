use super::{commands::drive_controlling_after_attach, setup::*};
use crate::workbench_client::{
    NativeStartupTarget, StartupDirectory, startup_directory::tests as fixtures,
    state::CarrierFailure,
};
use orchbee_client_server_protocol::*;
use orchbee_workbench_protocol::*;
use std::sync::{Arc, atomic::AtomicBool};

fn harness(steps: Vec<WireKind>, directory: StartupDirectory) -> TestResult<Harness> {
    harness_at(steps, directory, None)
}
fn harness_at(
    steps: Vec<WireKind>,
    directory: StartupDirectory,
    position: Option<crate::workbench_client::startup_preference::Position>,
) -> TestResult<Harness> {
    Harness::start_at(
        steps,
        fixture_geometry()?,
        NativeStartupTarget::LocalDirectory {
            position,
            fallback: Arc::new(AtomicBool::new(false)),
            directory: Box::new(directory),
        },
    )
}
fn read(harness: &Harness, snapshot: Snapshot) -> TestResult {
    let request = harness.carrier.take_read()?;
    harness
        .carrier
        .respond_read(&request, WorkbenchSnapshotOutcome::Available { snapshot })
}
fn command(harness: &Harness) -> TestResult<WorkbenchCommand> {
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("one startup command"));
    };
    Ok(command)
}
fn reply(
    harness: &Harness,
    command: &WorkbenchCommand,
    outcome: WorkbenchCommandOutcome,
) -> TestResult {
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id.clone(),
            outcome,
        }))
}
fn launch(harness: &Harness) -> TestResult<WorkbenchCommand> {
    launch_from(harness, fixtures::empty())
}
fn launch_from(harness: &Harness, snapshot: Snapshot) -> TestResult<WorkbenchCommand> {
    let site = snapshot.site_id.clone();
    let revision = snapshot.revision;
    read(harness, snapshot)?;
    let command = command(harness)?;
    assert_eq!(command.site_id, site);
    assert_eq!(command.expected_revision, revision);
    assert!(
        matches!(&command.action,WorkbenchAction::WorkspaceLaunch {placement:WorkspacePlacement::Ungrouped {},label,launch:None,profile:None,options,..}
        if label.as_str()=="project space" && options.cwd.as_ref().is_some_and(|cwd|cwd.as_str()=="C:/project space"))
    );
    Ok(command)
}
fn workspace(command: &WorkbenchCommand, pending: bool) -> TestResult<WorkbenchCommandOutcome> {
    let WorkbenchAction::WorkspaceLaunch { geometry, .. } = &command.action else {
        return Err(test_error("WorkspaceLaunch geometry"));
    };
    let mut receipt = fixtures::commit();
    receipt.revision = WorkbenchRevision::new(command.expected_revision.value() + 1);
    Ok(if pending {
        WorkbenchCommandOutcome::WorkspaceLaunchPending {
            revision: receipt.revision,
            project_id: receipt.project_id,
            workspace_id: receipt.workspace_id,
            directory_id: receipt.directory_id,
            terminal_context_id: receipt.terminal_context_id,
            tab_id: receipt.tab_id,
            panel_id: receipt.panel_id,
            presentation_id: receipt.presentation_id,
        }
    } else {
        WorkbenchCommandOutcome::WorkspaceLaunched {
            revision: receipt.revision,
            project_id: receipt.project_id,
            workspace_id: receipt.workspace_id,
            directory_id: receipt.directory_id,
            terminal_context_id: receipt.terminal_context_id,
            tab_id: receipt.tab_id,
            panel_id: receipt.panel_id,
            presentation_id: receipt.presentation_id,
            terminal_outcome: WorkbenchTerminalOutcome::Started {
                geometry: *geometry,
            },
        }
    })
}

#[test]
fn directory_startup_actual_carrier_creates_in_cwd_before_bootstrap_without_initial_workspace_open()
-> TestResult {
    for saved_empty in [false, true] {
        let mut steps = vec![WireKind::Workbench; 3];
        steps.extend([WireKind::Obcs; 5]);
        let position = saved_empty.then(|| crate::workbench_client::startup_preference::Position {
            location: crate::workbench_client::NativeContextLocation::NoWorkspace,
            member: None,
        });
        let mut harness = harness_at(steps, fixtures::directory(), position)?;
        let command = launch(&harness)?;
        reply(&harness, &command, workspace(&command, false)?)?;
        read(&harness, fixtures::created())?;
        let UiEvent::Bootstrap { key } = harness.ui.settle_next(FrameCut::None)? else {
            return Err(test_error("created member Bootstrap"));
        };
        assert_eq!(key.tab_id(), &fixtures::commit().tab_id);
        assert_eq!(
            harness.ui.state().focus_panel_id(),
            Some(&fixtures::commit().panel_id)
        );
        assert_eq!(
            harness.ui.state().cache().terminal_context_id().unwrap(),
            &fixtures::commit().terminal_context_id
        );
        let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
            return Err(test_error("created exact AttachBinding"));
        };
        assert!(
            matches!(request.action(),TerminalViewerAction::AttachBinding {worker_id,terminal_id,..}
        if worker_id.as_str()==format!("{:032x}",99) && terminal_id.as_str()==format!("{:032x}",112))
        );
        harness
            .carrier
            .respond_obcs(ServerMessage::TerminalViewerResult(
                TerminalViewerResult::new(request.request_id().clone(), attached_outcome()?),
            ))?;
        assert!(matches!(
            harness.ui.settle_next(FrameCut::None)?,
            UiEvent::Attached { .. }
        ));
        drive_controlling_after_attach(&mut harness, &key, fixture_geometry()?)?;
        assert!(harness.ui.state().ready().is_some());
        harness.finish()?;
    }
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_empty_project_does_not_couple_launch_and_read_failure_retains_workspace()
-> TestResult {
    let directory = fixtures::directory();
    let harness = harness(vec![WireKind::Workbench; 3], directory.clone())?;
    let command = launch_from(&harness, fixtures::project_only())?;
    reply(&harness, &command, workspace(&command, false)?)?;
    let request = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    harness.finish_after_worker_failure(CarrierFailure::Transport)?;
    let note = directory
        .failure_note(crate::localization::Locale::EnUs)?
        .ok_or("known Workspace report")?;
    assert!(note.contains(fixtures::commit().workspace_id.as_str()));
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_retains_pending_workspace_without_attach_or_replay()
-> TestResult {
    let directory = fixtures::directory();
    let harness = harness(vec![WireKind::Workbench; 2], directory.clone())?;
    let command = launch(&harness)?;
    reply(&harness, &command, workspace(&command, true)?)?;
    harness.finish_after_worker_failure(CarrierFailure::Transport)?;
    let note = directory
        .failure_note(crate::localization::Locale::ZhCn)?
        .ok_or("known Workspace report")?;
    assert!(note.contains(fixtures::commit().workspace_id.as_str()));
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_unknown_launch_preserves_request_and_never_reads_or_replays()
-> TestResult {
    let directory = fixtures::directory();
    let harness = harness(vec![WireKind::Workbench; 2], directory.clone())?;
    let command = launch(&harness)?;
    harness.carrier.respond_malformed_workbench()?;
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)?;
    let note = directory
        .failure_note(crate::localization::Locale::EnUs)?
        .ok_or("unknown launch report")?;
    assert!(note.contains(command.request_id.as_str()));
    assert!(!note.contains(fixtures::commit().workspace_id.as_str()));
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_missing_cwd_never_submits_launch_or_falls_back_home()
-> TestResult {
    let directory = StartupDirectory::new(None, crate::localization::Locale::EnUs, true);
    let harness = harness(vec![WireKind::Workbench], directory.clone())?;
    read(&harness, fixtures::empty())?;
    harness.finish_after_worker_failure(CarrierFailure::Transport)?;
    assert!(
        directory
            .failure_note(crate::localization::Locale::EnUs)?
            .is_none()
    );
    Ok(())
}

#[test]
fn directory_startup_existing_workspace_without_saved_position_never_opens_compatibility_terminal()
-> TestResult {
    let mut steps = vec![WireKind::Workbench];
    steps.extend([WireKind::Obcs; 5]);
    let mut harness = harness(steps, fixtures::directory())?;
    read(&harness, fixtures::created())?;
    let UiEvent::Bootstrap { key } = harness.ui.settle_next(FrameCut::None)? else {
        return Err(test_error("existing Workspace Bootstrap"));
    };
    assert_eq!(key.tab_id(), &fixtures::commit().tab_id);
    assert_eq!(
        harness.ui.state().cache().terminal_context_id().unwrap(),
        &fixtures::commit().terminal_context_id
    );
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("existing Workspace AttachBinding"));
    };
    assert!(matches!(
        request.action(),
        TerminalViewerAction::AttachBinding { worker_id, terminal_id, .. }
            if worker_id.as_str() == format!("{:032x}", 99)
                && terminal_id.as_str() == format!("{:032x}", 112)
    ));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(request.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { .. }
    ));
    drive_controlling_after_attach(&mut harness, &key, fixture_geometry()?)?;
    assert!(harness.ui.state().ready().is_some());
    harness.finish()
}

#[test]
fn directory_startup_empty_workspace_and_corrupt_restore_bootstrap_without_open_or_launch()
-> TestResult {
    for canonical_workspace in [false, true] {
        let directory =
            StartupDirectory::new(None, crate::localization::Locale::EnUs, canonical_workspace);
        let mut snapshot = if canonical_workspace {
            fixtures::created()
        } else {
            fixtures::empty()
        };
        for context in &mut snapshot.terminal_contexts {
            context.tabs.clear();
        }
        let expected = canonical_workspace.then(|| fixtures::commit().terminal_context_id);
        let mut harness = harness(vec![WireKind::Workbench], directory)?;
        read(&harness, snapshot)?;
        assert!(matches!(
            harness.ui.settle_next(FrameCut::None)?,
            UiEvent::BootstrapEmpty
        ));
        assert!(harness.ui.state().desired().is_none());
        if let Some(expected) = expected {
            assert_eq!(
                harness.ui.state().cache().terminal_context_id().unwrap(),
                &expected
            );
        } else {
            assert_eq!(harness.ui.state().view().context_count(), 0);
        }
        harness.finish()?;
    }
    Ok(())
}
