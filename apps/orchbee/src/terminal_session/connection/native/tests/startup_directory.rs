use super::{commands::drive_controlling_after_attach, setup::*};
use crate::terminal_session::{
    NativeStartupTarget, StartupDirectory, startup_directory::tests as fixtures,
    state::CarrierFailure,
};
use orchbee_client_server_protocol::*;
use orchbee_workbench_protocol::*;
use std::sync::{Arc, atomic::AtomicBool};

fn harness(steps: Vec<WireKind>, directory: StartupDirectory) -> TestResult<Harness> {
    Harness::start_at(
        steps,
        fixture_geometry()?,
        NativeStartupTarget::LocalDirectory {
            position: None,
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
fn project(harness: &Harness) -> TestResult {
    read(harness, fixtures::empty())?;
    let command = command(harness)?;
    assert_eq!(command.site_id, fixtures::empty().site_id);
    assert_eq!(command.expected_revision, fixtures::empty().revision);
    assert!(
        matches!(&command.action,WorkbenchAction::ProjectCreate {label} if label.as_str()=="Default")
    );
    reply(
        harness,
        &command,
        WorkbenchCommandOutcome::ProjectCreated {
            revision: fixtures::project().revision,
            project_id: fixtures::project().id,
        },
    )
}
fn launch(harness: &Harness) -> TestResult<WorkbenchCommand> {
    project(harness)?;
    read(harness, fixtures::project_only())?;
    let command = command(harness)?;
    assert_eq!(command.expected_revision, fixtures::project_only().revision);
    assert!(
        matches!(&command.action,WorkbenchAction::WorkspaceLaunch {project_id,label,launch:None,profile:None,options,..}
        if project_id==&fixtures::project().id && label.as_str()=="project space" && options.cwd.as_ref().is_some_and(|cwd|cwd.as_str()=="C:/project space"))
    );
    Ok(command)
}
fn workspace(command: &WorkbenchCommand, pending: bool) -> TestResult<WorkbenchCommandOutcome> {
    let WorkbenchAction::WorkspaceLaunch { geometry, .. } = &command.action else {
        return Err(test_error("WorkspaceLaunch geometry"));
    };
    let receipt = fixtures::commit();
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
fn directory_startup_actual_carrier_creates_in_cwd_before_bootstrap_without_standalone_open()
-> TestResult {
    let mut steps = vec![WireKind::Workbench; 5];
    steps.extend([WireKind::Obcs; 5]);
    let mut harness = harness(steps, fixtures::directory())?;
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
        harness.ui.state().cache().terminal_context_id(),
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
    harness.finish()
}

#[test]
fn directory_startup_actual_carrier_retains_project_on_read_failure_and_stops_before_launch()
-> TestResult {
    let directory = fixtures::directory();
    let harness = harness(vec![WireKind::Workbench; 3], directory.clone())?;
    project(&harness)?;
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
        .ok_or("known Project report")?;
    assert!(note.contains(fixtures::project().id.as_str()));
    assert!(!note.contains(fixtures::commit().workspace_id.as_str()));
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_retains_pending_workspace_without_attach_or_replay()
-> TestResult {
    let directory = fixtures::directory();
    let harness = harness(vec![WireKind::Workbench; 4], directory.clone())?;
    let command = launch(&harness)?;
    reply(&harness, &command, workspace(&command, true)?)?;
    harness.finish_after_worker_failure(CarrierFailure::Transport)?;
    let note = directory
        .failure_note(crate::localization::Locale::ZhCn)?
        .ok_or("known Workspace report")?;
    assert!(
        note.contains(fixtures::project().id.as_str())
            && note.contains(fixtures::commit().workspace_id.as_str())
    );
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_unknown_launch_preserves_request_and_never_reads_or_replays()
-> TestResult {
    let directory = fixtures::directory();
    let harness = harness(vec![WireKind::Workbench; 4], directory.clone())?;
    let command = launch(&harness)?;
    harness.carrier.respond_malformed_workbench()?;
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)?;
    let note = directory
        .failure_note(crate::localization::Locale::EnUs)?
        .ok_or("unknown launch report")?;
    assert!(
        note.contains(fixtures::project().id.as_str())
            && note.contains(command.request_id.as_str())
    );
    assert!(!note.contains(fixtures::commit().workspace_id.as_str()));
    Ok(())
}

#[test]
fn directory_startup_actual_carrier_missing_cwd_never_submits_project_or_falls_back_home()
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
