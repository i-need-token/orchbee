use super::panel_split::{close_leaf, dispatch_split, split_command, standalone_leaf};
use super::tab_create::{immediate_completion, pending_first_open};
use super::*;
use orchbee_client_server_protocol::{
    CreateAcceptedState, CreateOutcome, TerminalLaunchFact, TerminalLaunchReadOutcome,
    TerminalLaunchState,
};
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalId as WorkbenchTerminalId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchPanelId, WorkbenchPanelSplitDirection,
    WorkerId as WorkbenchWorkerId,
};

type Outcome = TerminalLaunchReadOutcome;

fn terminal_hex(authorization: TerminalOperationAuthorization) -> String {
    format!(
        "{:032x}",
        u128::from_be_bytes(authorization.terminal_id().as_bytes())
    )
}
fn worker_hex(authorization: TerminalOperationAuthorization) -> String {
    format!(
        "{:032x}",
        u128::from_be_bytes(authorization.worker_id().as_bytes())
    )
}
fn started_open(coordinator: &mut SiteCoordinator, request: u128, reference: u128) -> String {
    let pending = pending_first_open(coordinator, request, reference);
    let terminal_id = terminal_hex(launch_authorization(&pending));
    let completion = immediate_completion(pending.command());
    coordinator
        .complete(pending, completion, Some(worker(1)))
        .unwrap();
    terminal_id
}
fn primary_presentation_id(committed: &Snapshot, context: &str, tab: &str) -> String {
    let tab = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id.as_str() == context)
        .and_then(|item| item.tabs.iter().find(|item| item.tab_id.as_str() == tab))
        .unwrap();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!("simple panel expected")
    };
    primary_presentation.presentation_id.as_str().to_owned()
}

#[test]
fn launch_read_joins_the_committed_route_and_retained_fact() {
    let mut coordinator = site();
    let pending = pending_first_open(&mut coordinator, 3001, 3002);
    let authorization = launch_authorization(&pending);
    let terminal_id = terminal_hex(authorization);
    let worker_id = worker_hex(authorization);

    // Between dispatch and completion the binding is already committed while the
    // retained record is still `creating`.
    let Outcome::Found {
        site_id,
        workbench_revision,
        terminal,
        launch,
    } = coordinator.terminal_launch_read(&terminal_id)
    else {
        panic!("committed binding expected")
    };
    assert_eq!(site_id.as_str(), format!("{:032x}", 999));
    assert_eq!(
        workbench_revision,
        snapshot(&mut coordinator).revision.value()
    );
    assert_eq!(terminal.terminal_id().as_str(), terminal_id);
    assert_eq!(terminal.worker_id().as_str(), worker_id);
    assert_eq!(launch.state(), TerminalLaunchState::Creating);
    assert_eq!(launch.worker_generation(), NonZeroU64::new(1));
    assert_eq!(launch.create_outcome(), None);
    assert_eq!(launch.geometry(), None);

    let committed = snapshot(&mut coordinator);
    let (context, tab, panel) = standalone_leaf(&mut coordinator);
    let bound_to = terminal.bound_to();
    assert_eq!(bound_to.terminal_context_id().as_str(), context.as_str());
    assert_eq!(bound_to.tab_id().as_str(), tab.as_str());
    assert_eq!(bound_to.panel_id().as_str(), panel.as_str());
    assert_eq!(
        bound_to.presentation_id().as_str(),
        primary_presentation_id(&committed, context.as_str(), tab.as_str())
    );

    let completion = immediate_completion(pending.command());
    coordinator
        .complete(pending, completion, Some(worker(1)))
        .unwrap();

    // Completion settles the retained record only; the joined revision is the
    // same committed membership revision the route was resolved from.
    let Outcome::Found {
        workbench_revision: settled_revision,
        terminal: settled_terminal,
        launch: settled_launch,
        ..
    } = coordinator.terminal_launch_read(&terminal_id)
    else {
        panic!("committed binding expected")
    };
    assert_eq!(settled_revision, workbench_revision);
    assert_eq!(settled_terminal, terminal);
    assert_eq!(settled_launch.state(), TerminalLaunchState::Started);
    assert_eq!(settled_launch.worker_generation(), NonZeroU64::new(1));
    assert_eq!(
        settled_launch.create_outcome(),
        Some(CreateOutcome::Accepted {
            state: CreateAcceptedState::Started {
                geometry: geometry()
            }
        })
    );
    assert_eq!(settled_launch.geometry(), Some(geometry()));
}

#[test]
fn launch_read_re_resolves_route_and_revision_after_membership_moves() {
    let mut coordinator = site();
    let terminal_a = started_open(&mut coordinator, 3101, 3102);
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split = split_command(
        &mut coordinator,
        3103,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split, 3104);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch_b) = pending_b.command() else {
        panic!("terminal launch expected")
    };
    let terminal_b = terminal_hex(launch_b.authorization());
    let completion_b = immediate_completion(pending_b.command());
    coordinator
        .complete_terminal_panel_split(pending_b, completion_b, Some(worker(1)))
        .unwrap();

    // Two bindings at two distinct leaves resolve their own routes; both reads
    // carry the same committed revision.
    let Outcome::Found {
        workbench_revision: revision_a,
        terminal: route_a,
        ..
    } = coordinator.terminal_launch_read(&terminal_a)
    else {
        panic!("first binding expected")
    };
    let Outcome::Found {
        workbench_revision: revision_b,
        terminal: route_b,
        launch: launch_fact_b,
        ..
    } = coordinator.terminal_launch_read(&terminal_b)
    else {
        panic!("second binding expected")
    };
    assert_eq!(launch_fact_b.state(), TerminalLaunchState::Started);
    assert_eq!(route_a.bound_to().panel_id().as_str(), panel_a.as_str());
    assert_ne!(route_a.bound_to().panel_id(), route_b.bound_to().panel_id());
    assert_ne!(
        route_a.bound_to().presentation_id(),
        route_b.bound_to().presentation_id()
    );
    assert_eq!(revision_a, revision_b);

    // A same-tab leaf move keeps the bound identities (the leaf moves whole) but
    // commits a new revision; the next read re-joins against it.
    let committed = snapshot(&mut coordinator);
    let movement = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3105)).unwrap(),
        site_id: committed.site_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelMove {
            terminal_context_id: context,
            tab_id: tab,
            panel_id: panel_a.clone(),
            target_panel_id: WorkbenchPanelId::parse(route_b.bound_to().panel_id().as_str())
                .unwrap(),
            direction: WorkbenchPanelSplitDirection::Right,
        },
    };
    let moved = coordinator.execute_terminal_panel_move(movement).unwrap();
    assert!(matches!(
        moved.outcome,
        WorkbenchCommandOutcome::PanelMoved { .. }
    ));
    let Outcome::Found {
        workbench_revision: moved_revision,
        terminal: moved_route,
        launch: moved_launch,
        ..
    } = coordinator.terminal_launch_read(&terminal_a)
    else {
        panic!("moved binding expected")
    };
    assert_eq!(moved_revision, snapshot(&mut coordinator).revision.value());
    assert!(moved_revision > revision_a);
    assert_eq!(moved_route, route_a);
    assert_eq!(moved_launch.state(), TerminalLaunchState::Started);
}

#[test]
fn launch_read_hides_the_orphaned_retained_record_after_close() {
    let mut coordinator = site();
    let terminal_id = started_open(&mut coordinator, 3201, 3202);
    assert!(matches!(
        coordinator.terminal_launch_read(&terminal_id),
        Outcome::Found { .. }
    ));
    let (context, tab, panel) = standalone_leaf(&mut coordinator);
    let (_command, _changed) = close_leaf(&mut coordinator, context, tab, panel, 3203);

    // The coordinator still retains the create fact — membership, not record
    // retention, gates visibility, so the orphaned record reads as not-found.
    assert!(
        coordinator
            .terminals
            .has_retained_binding(&PresentationBinding {
                terminal_id: WorkbenchTerminalId::parse(&terminal_id).unwrap(),
                worker_id: WorkbenchWorkerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes([9; 16])
                ))
                .unwrap(),
            })
    );
    assert_eq!(
        coordinator.terminal_launch_read(&terminal_id),
        Outcome::NotFound
    );
}

#[test]
fn launch_read_reports_not_tracked_for_a_visible_binding_without_a_coordinator_record() {
    let mut coordinator = site();
    // The Server-issued intent entry point — the same one `prepare_open` drives —
    // admits a binding for a terminal the coordinator never launched.
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .commit_standalone_intent(PresentationBinding {
            terminal_id: WorkbenchTerminalId::parse(&format!("{:032x}", 424_242)).unwrap(),
            worker_id: WorkbenchWorkerId::parse(&format!("{:032x}", 777)).unwrap(),
        })
        .unwrap();

    let terminal_id = format!("{:032x}", 424_242);
    let Outcome::Found {
        workbench_revision,
        terminal,
        launch,
        ..
    } = coordinator.terminal_launch_read(&terminal_id)
    else {
        panic!("visible binding expected")
    };
    assert_eq!(
        workbench_revision,
        snapshot(&mut coordinator).revision.value()
    );
    assert_eq!(terminal.terminal_id().as_str(), terminal_id);
    assert_eq!(terminal.worker_id().as_str(), format!("{:032x}", 777));
    assert_eq!(launch.state(), TerminalLaunchState::NotTracked);
    assert_eq!(launch.worker_generation(), None);
    assert_eq!(launch.create_outcome(), None);
    assert_eq!(launch.geometry(), None);
}

fn settled_failed_fact(observed: TerminalLaunchOutcome) -> TerminalLaunchFact {
    let mut coordinator = site();
    let pending = pending_first_open(&mut coordinator, 3301, 3302);
    let authorization = launch_authorization(&pending);
    let terminal_id = terminal_hex(authorization);
    let failed = Completion::Launch(TerminalLaunchResult::new(authorization, observed).unwrap());
    coordinator
        .complete(pending, failed, Some(worker(1)))
        .unwrap();
    let Outcome::Found { launch, .. } = coordinator.terminal_launch_read(&terminal_id) else {
        panic!("failed launch keeps its committed binding")
    };
    *launch
}

#[test]
fn launch_read_reports_the_retained_failed_or_unknown_launch_on_the_still_visible_binding() {
    let epoch = HostEpoch::from_bytes([4; 16]).unwrap();

    // A Host-reported create failure settles to `unavailable`; the committed
    // binding stays visible and the retained outcome echoes the failure.
    let failed = settled_failed_fact(TerminalLaunchOutcome::Failed(
        TerminalLaunchFailure::HostReportedFailure { host_epoch: epoch },
    ));
    assert_eq!(failed.state(), TerminalLaunchState::Unavailable);
    assert_eq!(
        failed.create_outcome(),
        Some(CreateOutcome::Failed {
            reason: CreateFailureReason::TerminalCreateFailed
        })
    );
    assert_eq!(failed.worker_generation(), NonZeroU64::new(1));
    assert_eq!(failed.geometry(), None);

    // An unknown completion settles to `unknown` under the same visible binding.
    let unknown = settled_failed_fact(TerminalLaunchOutcome::OutcomeUnknown { host_epoch: epoch });
    assert_eq!(unknown.state(), TerminalLaunchState::Unknown);
    assert_eq!(
        unknown.create_outcome(),
        Some(CreateOutcome::OutcomeUnknown)
    );
    assert_eq!(unknown.worker_generation(), NonZeroU64::new(1));
    assert_eq!(unknown.geometry(), None);
}
