use super::{commands::drive_controlling_ready_from_snapshot, setup::*};
use crate::workbench_client::mailbox::WorkerEvent;
use crate::workbench_state::CarrierFailure;
use crate::workbench_state::workspace_setup::{SetupResult, SetupStatus};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ServerMessage, TerminalControlOutcome, TerminalControlResult,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason, WorkbenchTabGroupId,
};
use std::sync::atomic::Ordering;

fn steps(after: &[WireKind]) -> Vec<WireKind> {
    let mut steps = vec![WireKind::Workbench];
    steps.extend([WireKind::Obcs; 5]);
    steps.extend_from_slice(after);
    steps
}

fn command(harness: &Harness) -> TestResult<WorkbenchCommand> {
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("setup must submit one exact command"));
    };
    Ok(command)
}

fn respond_command(
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

fn respond_snapshot(harness: &Harness, outcome: WorkbenchSnapshotOutcome) -> TestResult {
    let request = harness.carrier.take_read()?;
    harness.carrier.respond_read(&request, outcome)
}

#[test]
fn earlier_input_consumes_setup_budget_without_any_setup_frame() -> TestResult {
    // The script ends at the earlier input. Its trailing-byte check rejects even a partial setup
    // frame, an automatic snapshot, or a replay after the frozen setup budget expires.
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start_with_prewrite_gate(steps(&[WireKind::Obcs]), geometry)?;
    let (old, _) = drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let baseline = harness.ui.state().cache().clone();
    let draft = harness.ui.setup_group_draft("Group after input", true)?;
    harness
        .ui
        .admit_input(ControlInput::new(b"earlier accepted work".to_vec())?)?;
    let ClientMessage::TerminalControl(input) = harness.carrier.take_obcs()? else {
        return Err(test_error("earlier input must drain first"));
    };
    harness.ui.enqueue_setup_token(draft)?;
    harness.elapsed_millis.store(44_999, Ordering::SeqCst);
    harness.arm_prewrite_gate()?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                input.request_id().clone(),
                TerminalControlOutcome::InputAccepted {},
            ),
        ))?;
    harness.await_prewrite_blocked()?;
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    harness.release_prewrite_gate()?;
    let state = harness.finish_with_setup_not_applied(CarrierFailure::Deadline)?;
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.attached_key().map(|pane| pane.anchor()), Some(&old));
    Ok(())
}

#[test]
fn definite_setup_refusal_survives_unconsumed_ui_result_deadline() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    let (old, _) = drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let baseline = harness.ui.state().cache().clone();
    let draft = harness.ui.setup_group_draft("Denied Group", true)?;
    harness.ui.enqueue_setup_token(draft)?;
    let submitted = command(&harness)?;
    assert!(matches!(
        submitted.action,
        WorkbenchAction::TabsConfigure { .. }
    ));
    respond_command(&harness, &submitted, WorkbenchCommandOutcome::Denied {})?;
    let event = harness.ui.take_next()?;
    assert!(
        matches!(event.into_event(), WorkerEvent::WorkspaceSetupResult { operation, outcome: SetupResult::NotApplied } if operation.command == submitted)
    );
    // The UI has taken the event but never deposited its receipt. Only the worker's retained
    // definite evidence may clear the pending operation after this deadline.
    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    let state = harness.finish_with_setup_not_applied(CarrierFailure::Deadline)?;
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.attached_key().map(|pane| pane.anchor()), Some(&old));
    Ok(())
}

#[test]
fn explicit_setup_recovery_reads_only_and_next_group_command_requires_fresh_draw() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench; 4]), geometry)?;
    let (old, _) = drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let baseline = harness.ui.state().cache().clone();
    let draft = harness.ui.setup_group_draft("Created Group", true)?;
    harness.ui.enqueue_setup_token(draft)?;
    let group_command = command(&harness)?;
    let group_id = WorkbenchTabGroupId::parse("00000000000000000000000000000070")?;
    respond_command(
        &harness,
        &group_command,
        WorkbenchCommandOutcome::TabsConfigured {
            revision: WorkbenchRevision::new(2),
            group_id: Some(group_id.clone()),
        },
    )?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    harness.ui.settle_setup_next()?;
    assert_eq!(
        harness.ui.state().setup_status(),
        Some(SetupStatus::TabsCommittedNeedRefresh)
    );
    assert_eq!(harness.ui.state().cache(), &baseline);
    assert_eq!(
        harness.ui.state().attached_key().map(|pane| pane.anchor()),
        Some(&old)
    );
    assert!(harness.ui.state().setup_draft().is_none());

    // An explicit action receives a new operation budget even after the mutation's original
    // budget has expired. The scripted next frame must be SnapshotRead, never TabsConfigure.
    harness.elapsed_millis.store(60_000, Ordering::SeqCst);
    harness.ui.refresh_known_setup()?;
    let mut recovered = snapshot(2, 2);
    recovered.terminal_contexts[0]
        .groups
        .push(orchbee_workbench_protocol::TabGroup {
            group_id: group_id.clone(),
            label: "Created Group".into(),
            color: orchbee_workbench_protocol::TabGroupColor::Blue,
        });
    recovered.terminal_contexts[0].tabs[0].group_id = Some(group_id.clone());
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: recovered,
        },
    )?;
    harness.ui.settle_setup_next()?;
    assert_eq!(
        harness.ui.state().setup_status(),
        Some(SetupStatus::TabsConfigured)
    );
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(2)
    );
    assert_eq!(
        harness.ui.state().attached_key().map(|pane| pane.anchor()),
        Some(&old)
    );
    assert!(harness.ui.state().setup_draft().is_none());
    let next = harness.ui.setup_group_draft("Next Group", false)?;
    assert_ne!(next, draft);
    assert!(harness.ui.enqueue_setup_token(draft).is_err());
    assert!(harness.ui.enqueue_setup_token(next).is_err());
    harness.ui.draw_setup(next);
    harness.ui.enqueue_setup_token(next)?;
    let workspace_command = command(&harness)?;
    assert_ne!(workspace_command.request_id, group_command.request_id);
    assert_eq!(
        workspace_command.expected_revision,
        WorkbenchRevision::new(2)
    );
    assert!(
        matches!(&workspace_command.action, WorkbenchAction::TabsConfigure { operation: orchbee_workbench_protocol::TabConfiguration::CreateGroup { label, .. }, .. } if label.as_str() == "Next Group")
    );
    respond_command(
        &harness,
        &workspace_command,
        WorkbenchCommandOutcome::Denied {},
    )?;
    harness.ui.settle_setup_next()?;
    assert_eq!(
        harness.ui.state().setup_status(),
        Some(SetupStatus::TabsNotApplied)
    );
    assert_eq!(
        harness
            .ui
            .state()
            .cache()
            .canonical_context()
            .unwrap()
            .groups[0]
            .group_id,
        group_id
    );
    harness.finish()
}
