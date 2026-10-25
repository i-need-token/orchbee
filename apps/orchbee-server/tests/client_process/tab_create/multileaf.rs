use super::*;
use orchbee_workbench_protocol::WorkbenchPanelSplitDirection;
use std::thread;

fn accept_launch(
    worker: &mut UnixStream,
    launch: &AuthorizedTerminalLaunch,
    seed: u8,
    limit: Instant,
) -> TestResult {
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        return Err("immediate launch expected".into());
    };
    write_worker_message(
        worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                host_identity(seed)?,
                HostLaunchState::Started(geometry),
            )),
        )?),
        limit,
    )
}

#[test]
fn parked_tab_create_completion_survives_an_interleaved_split_on_one_connection() -> TestResult {
    // The Worker socket is the controlled completion seam: this test holds the second
    // create's Launch result while an interleaved split commits multileaf membership,
    // then releases it. Dispatch, ledger, and completion paths are real; only the
    // Launch result timing is staged.
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first_client, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second_client, _) = connect_bounded_client(server.client_endpoint())?;
    // Snapshot reads are FIFO behind each connection's parked command response, so a
    // dedicated observer connection watches commits.
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = create_workspace(&mut observer, 0x100_000, "/tmp", deadline())?;
    let context_id = initial.navigation.workspaces[0].terminal_context_id.clone();

    let first_id = workbench_id(301)?;
    let first_request = command(
        first_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("settled source")?,
            geometry: workbench_geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    write_obwp(&mut first_client, &first_request, deadline())?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(first_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("first Launch expected".into());
    };
    accept_launch(&mut worker, &first_launch, 11, deadline())?;
    let first_result = read_command_result(&mut first_client, &first_id, deadline())?;
    let first_tuple = final_tab(&first_result.outcome)?;

    let second_id = workbench_id(302)?;
    let one = read_snapshot(&mut observer, 303, deadline())?;
    let second_request = command(
        second_id.clone(),
        &one,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("pending during split")?,
            geometry: workbench_geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    write_obwp(&mut first_client, &second_request, deadline())?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(second_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("second Launch expected".into());
    };
    let committed = read_snapshot(&mut observer, 304, deadline())?;
    assert_eq!(initial_workspace_context(&committed)?.tabs.len(), 2);
    let second_tuple = pending_tab(
        &exchange_command(&mut observer, &second_request, &second_id, deadline())?.outcome,
    )?;

    // The interleaved split commits a second member under the first Tab while the second
    // create's completion is still parked at the Worker seam. The split's own Launch is
    // queued behind the parked exchange, so the split result is only readable after the
    // parked create settles.
    let split_id = workbench_id(305)?;
    let split_request = command(
        split_id.clone(),
        &committed,
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: context_id.clone(),
            tab_id: first_tuple.tab_id.clone(),
            panel_id: first_tuple.panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("split peer")?,
            geometry: workbench_geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    write_obwp(&mut second_client, &split_request, deadline())?;
    // One immutable absolute cap bounds the whole wait: every observer read
    // shares it, so a server that keeps answering snapshots without ever
    // committing the split cannot renew the deadline per successful read.
    let split_wait_deadline = deadline();
    let mut split_wait_replies = 0u32;
    wait_for_split_commit(
        &mut observer,
        306,
        &first_tuple.tab_id,
        split_wait_deadline,
        &mut split_wait_replies,
    )?;

    // Releasing the parked completion must settle against tree membership: the context
    // is multileaf now, and a one-leaf re-check would have retired this authority.
    accept_launch(&mut worker, &second_launch, 15, deadline())?;
    let second_result = read_command_result(&mut first_client, &second_id, deadline())?;
    assert_eq!(final_tab(&second_result.outcome)?, second_tuple);

    // The queued split Launch now reaches the Worker; completing it settles the split.
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(split_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("queued split Launch expected".into());
    };
    assert_ne!(
        split_launch.authorization().terminal_id(),
        second_launch.authorization().terminal_id()
    );
    accept_launch(&mut worker, &split_launch, 17, deadline())?;
    let split_result = read_command_result(&mut second_client, &split_id, deadline())?;
    let WorkbenchCommandOutcome::PanelSplitCreated {
        terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
        ..
    } = split_result.outcome
    else {
        return Err(format!("panel.split did not start: {:?}", split_result.outcome).into());
    };

    assert_eq!(
        exchange_command(&mut observer, &second_request, &second_id, deadline())?,
        second_result
    );
    let settled = read_snapshot(&mut observer, 307, deadline())?;
    assert_eq!(settled, read_snapshot(&mut first_client, 308, deadline())?);
    let settled_context = initial_workspace_context(&settled)?;
    assert_eq!(settled_context.tabs.len(), 2);
    let second_tab = settled_context
        .tabs
        .iter()
        .find(|tab| tab.tab_id == second_tuple.tab_id)
        .ok_or("settled second Tab missing")?;
    binding_matches_launch(
        primary_presentation(second_tab)?
            .0
            .binding
            .as_ref()
            .ok_or("settled second binding missing")?,
        &second_launch,
    );

    // No fence was declared: settling the parked create dispatched nothing extra.
    worker.set_nonblocking(true)?;
    let mut byte = [0];
    assert!(
        matches!(worker.read(&mut byte), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
        "settling the parked create dispatched another Worker message"
    );
    Ok(())
}

// The single wait path shared by the positive witness and the negative
// diagnostic. The absolute deadline bounds every snapshot read and the
// in-loop check, so a responsive-but-never-split server ends this wait in
// the deadline error instead of renewing it per successful reply.
// `successful_replies` makes the number of answered snapshots observable to
// the caller even when the wait fails.
fn wait_for_split_commit(
    observer: &mut UnixStream,
    request_value: u128,
    tab_id: &WorkbenchTabId,
    deadline: Instant,
    successful_replies: &mut u32,
) -> TestResult<Snapshot> {
    let mut snapshot = read_snapshot(observer, request_value, deadline)?;
    *successful_replies += 1;
    loop {
        let source = initial_workspace_context(&snapshot)?
            .tabs
            .iter()
            .find(|tab| tab.tab_id == *tab_id)
            .ok_or("split source Tab missing")?;
        if matches!(&source.root, PanelNode::Split { children, .. } if children.len() == 2) {
            return Ok(snapshot);
        }
        check_deadline(deadline)?;
        thread::yield_now();
        snapshot = read_snapshot(observer, request_value, deadline)?;
        *successful_replies += 1;
    }
}

// The absolute-deadline criterion: "process witness deadline" is emitted only
// by `remaining`/`check_deadline` once Instant::now() >= deadline. It reaches
// the caller either as a bare io::Error (deadline check before/inside the
// loop) or wrapped in the wire protocol's Io(...) variant (a read was in
// flight when the bound passed), so matching the marker is the criterion —
// not the wrapping type.
fn is_process_deadline(error: &(dyn std::error::Error + 'static)) -> bool {
    error.to_string().contains("process witness deadline")
}

// Shared fixture for the never-split diagnostics: one real Server, one
// controlled Worker connection, one commanding Client and a dedicated
// observer Client, with a single settled initial_workspace Tab as the split source.
fn never_split_fixture() -> TestResult<(
    RunningServer,
    UnixStream,
    UnixStream,
    UnixStream,
    CommittedTab,
)> {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first_client, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = create_workspace(&mut observer, 0x100_000, "/tmp", deadline())?;
    let context_id = initial.navigation.workspaces[0].terminal_context_id.clone();

    let first_id = workbench_id(312)?;
    let first_request = command(
        first_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: context_id,
            label: WorkbenchLabel::parse("never split")?,
            geometry: workbench_geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    write_obwp(&mut first_client, &first_request, deadline())?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(first_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("first Launch expected".into());
    };
    accept_launch(&mut worker, &first_launch, 11, deadline())?;
    let first_result = read_command_result(&mut first_client, &first_id, deadline())?;
    let first_tuple = final_tab(&first_result.outcome)?;
    Ok((server, worker, first_client, observer, first_tuple))
}

#[test]
fn never_split_observer_wait_terminates_at_the_shared_deadline() -> TestResult {
    // Bounded negative diagnostic over the same wait path as the positive
    // witness: the server keeps answering observer snapshots, but the split
    // is never sent. The wait must end in the absolute-deadline error after
    // at least two successful replies — successful responses must not renew
    // the shared cap, and termination must be the deadline criterion itself,
    // not any arbitrary error.
    let (_server, _worker, _first_client, mut observer, first_tuple) = never_split_fixture()?;

    let cap = deadline();
    let start = Instant::now();
    let mut successful_replies = 0u32;
    let outcome = wait_for_split_commit(
        &mut observer,
        313,
        &first_tuple.tab_id,
        cap,
        &mut successful_replies,
    );
    let elapsed = start.elapsed();
    let Err(error) = outcome else {
        return Err("never-split wait returned success".into());
    };
    assert!(
        is_process_deadline(&*error),
        "never-split wait ended on a non-deadline error: {error}"
    );
    eprintln!(
        "never-split wait: successful_replies={successful_replies} elapsed={elapsed:?} \
         error={error}"
    );
    assert!(
        successful_replies >= 2,
        "deadline evidence needs at least two successful snapshot replies, got {successful_replies}"
    );
    assert!(
        elapsed >= PROCESS_TIMEOUT - Duration::from_millis(500),
        "wait terminated before the shared cap: {elapsed:?}"
    );
    assert!(
        elapsed <= PROCESS_TIMEOUT + Duration::from_secs(2),
        "fixed-cap wait overran its bound: {elapsed:?}"
    );
    Ok(())
}

#[test]
fn renewed_deadline_wait_outlives_the_shared_cap_under_an_outer_bound() -> TestResult {
    // Expected-negative control for the original renewal bug shape: every
    // successful snapshot read starts a fresh PROCESS_TIMEOUT deadline, so a
    // never-split server keeps this copy alive past the point where the
    // shared-cap path already ended. `shared_cap` is sampled once and `outer`
    // bounds every read and the loop. The oracle only accepts the outcome as
    // the bounded negative result once the wait actually reached outer — via
    // the loop check or a deadline-marker error at-or-past outer — and it
    // requires at least one successful, still-unsplit reply after shared_cap.
    // Substituting one shared read cap for the renewed per-read caps fails
    // both assertions instead of passing this control.
    let (_server, _worker, _first_client, mut observer, first_tuple) = never_split_fixture()?;

    let start = Instant::now();
    let shared_cap = start + PROCESS_TIMEOUT;
    let outer = shared_cap + Duration::from_secs(3);
    let mut successful_replies = 0u32;
    let mut post_cap_replies = 0u32;
    let mut snapshot = read_snapshot(&mut observer, 321, deadline().min(outer))?;
    successful_replies += 1;
    if Instant::now() >= shared_cap {
        post_cap_replies += 1;
    }
    loop {
        let source = initial_workspace_context(&snapshot)?
            .tabs
            .iter()
            .find(|tab| tab.tab_id == first_tuple.tab_id)
            .ok_or("split source Tab missing")?;
        if matches!(&source.root, PanelNode::Split { children, .. } if children.len() == 2) {
            return Err("renewal-form diagnostic unexpectedly committed a split".into());
        }
        if Instant::now() >= outer {
            break;
        }
        thread::yield_now();
        // The renewed per-read deadline reproduces the bug under observation;
        // the outer bound still caps each socket read so nothing can hang.
        let read_cap = deadline().min(outer);
        match read_snapshot(&mut observer, 321, read_cap) {
            Ok(next) => {
                snapshot = next;
                successful_replies += 1;
                if Instant::now() >= shared_cap {
                    post_cap_replies += 1;
                }
            }
            Err(error) => {
                // A read in flight across the outer bound returns the deadline
                // marker; an earlier io/deadline error is not the outer end
                // state and fails this oracle.
                assert!(
                    is_process_deadline(&*error),
                    "renewal-form wait ended on a non-deadline error: {error}"
                );
                assert!(
                    Instant::now() >= outer,
                    "deadline fired before the outer bound — not the outer end state"
                );
                break;
            }
        }
    }
    let elapsed = start.elapsed();
    eprintln!(
        "renewed wait: successful_replies={successful_replies} \
         post_cap_replies={post_cap_replies} elapsed={elapsed:?}"
    );
    assert!(
        successful_replies >= 2,
        "renewal evidence needs at least two successful snapshot replies, got {successful_replies}"
    );
    assert!(
        post_cap_replies >= 1,
        "no successful unsplit snapshot arrived after the shared cap"
    );
    assert!(
        elapsed >= PROCESS_TIMEOUT + Duration::from_secs(3),
        "renewal-form wait ended before the outer bound: {elapsed:?}"
    );
    assert!(
        elapsed <= PROCESS_TIMEOUT + Duration::from_secs(3) + Duration::from_secs(2),
        "outer-bound diagnostic overran its bound: {elapsed:?}"
    );
    Ok(())
}
