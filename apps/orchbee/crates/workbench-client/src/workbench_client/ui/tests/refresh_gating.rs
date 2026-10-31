//! Background publication reads, as another Workspace's continuous output keeps them in flight,
//! never disable, drop, or misroute what the user asked for.
use super::super::*;
use super::context_selection::snapshot;
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::ContextNavigationAdmission as Admission;
use crate::workbench_client::mailbox::{CommandLease, EventDeadline};
use crate::workbench_projection::ClientContextTarget;
use orchbee_client_server_protocol::ControllerId;
use orchbee_workbench_protocol::*;
use std::sync::atomic::AtomicU64;

fn controlling() -> (WorkbenchClientUi, Arc<CommandMailbox>, LocalOperationClock) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(&snapshot(false))
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let mode = ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, mode.clone(), Some(geometry)));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(1), mode, Some(geometry)));
    (ui, mailbox, clock)
}

fn target(ui: &WorkbenchClientUi, index: usize) -> ClientContextTarget {
    ClientContextTarget {
        session_id: ui.view().navigation_session().unwrap().clone(),
        location: ui.view().context_at(index).unwrap().location().clone(),
    }
}

/// Starts the publication read that another Workspace's output keeps in flight.
pub(in crate::workbench_client::ui) fn background_read(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
) -> CommandLease {
    ui.state.as_mut().unwrap().note_publication_hint();
    ui.tick();
    let lease = mailbox
        .try_recv()
        .unwrap()
        .expect("background publication read");
    assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
    lease
}

/// Settles a leased background read. While output continues, the next commit's hint is already
/// pending when the result applies, so another read is due at once.
pub(in crate::workbench_client::ui) fn settle_background_read(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    lease: CommandLease,
    snapshot: Snapshot,
    output_continues: bool,
) {
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("publication read");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available { snapshot },
            marker.deadline(),
            clock,
        )
        .unwrap();
    if output_continues {
        ui.state.as_mut().unwrap().note_publication_hint();
    }
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
}

/// O-9: with output in another Workspace, a background read is in flight whenever a sidebar click
/// arrives, and a new one used to start the moment the previous one settled. The Shell retries
/// the queued click every loop; it must be admitted once the read in flight settles.
#[test]
fn continuous_background_reads_never_starve_a_requested_switch() {
    let (mut ui, mailbox, clock) = controlling();
    let destination = target(&ui, 1);
    let read = background_read(&mut ui, &mailbox);
    assert_eq!(ui.navigate_context(&destination), Admission::Busy);
    settle_background_read(&mut ui, &mailbox, &clock, read, snapshot(false), true);
    assert_eq!(
        ui.navigate_context(&destination),
        Admission::Admitted,
        "no further background read may start ahead of the waiting switch"
    );
    let lease = mailbox.try_recv().unwrap().expect("the switch's own read");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("context selection is Snapshot-only");
    };
    assert!(
        marker.context_deadlines().is_some(),
        "the slot went to the switch, not to another background read"
    );
}

/// 问题 5 and O-2: a background read is not the user's refresh. Divider drags and Worktree
/// sources stay available through it; only the Client's own refresh reports Busy.
#[test]
fn background_reads_keep_divider_and_worktree_sources_available() {
    use orchbee_workbench_protocol::ClientViewPhase;
    let (mut ui, mailbox, clock) = controlling();
    let phase = |ui: &WorkbenchClientUi| ui.control_observation(false).unwrap().1.phase;
    assert_eq!(phase(&ui), ClientViewPhase::Ready);
    let read = background_read(&mut ui, &mailbox);
    assert_eq!(
        phase(&ui),
        ClientViewPhase::Ready,
        "a divider drag is not refused as Busy while another Workspace prints"
    );
    assert!(
        ui.state.as_ref().unwrap().worktree_origin().is_some(),
        "a Worktree listing reply is not reported as a changed source"
    );
    settle_background_read(&mut ui, &mailbox, &clock, read, snapshot(false), false);
    // The Client's own refresh still owns the slot and is Busy.
    assert_eq!(ui.navigate_context(&target(&ui, 1)), Admission::Admitted);
    assert_eq!(phase(&ui), ClientViewPhase::Busy);
    assert!(ui.state.as_ref().unwrap().worktree_origin().is_none());
}
