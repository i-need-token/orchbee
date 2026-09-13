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
    let initial = read_snapshot(&mut observer, 300, deadline())?;
    let context_id = initial.navigation.standalone_terminal_context_id.clone();

    let first_id = workbench_id(301)?;
    let first_request = command(
        first_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("settled source")?,
            geometry: workbench_geometry()?,
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
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("pending during split")?,
            geometry: workbench_geometry()?,
        },
    );
    write_obwp(&mut first_client, &second_request, deadline())?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(second_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("second Launch expected".into());
    };
    let committed = read_snapshot(&mut observer, 304, deadline())?;
    assert_eq!(standalone_context(&committed)?.tabs.len(), 2);
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
            terminal_context_id: context_id.clone(),
            tab_id: first_tuple.tab_id.clone(),
            panel_id: first_tuple.panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("split peer")?,
            geometry: workbench_geometry()?,
        },
    );
    write_obwp(&mut second_client, &split_request, deadline())?;
    // One immutable absolute cap bounds the whole wait: every observer read
    // shares it, so a server that keeps answering snapshots without ever
    // committing the split cannot renew the deadline per successful read.
    let split_wait_deadline = deadline();
    let mut multileaf = read_snapshot(&mut observer, 306, split_wait_deadline)?;
    loop {
        let source = standalone_context(&multileaf)?
            .tabs
            .iter()
            .find(|tab| tab.tab_id == first_tuple.tab_id)
            .ok_or("split source Tab missing")?;
        if matches!(&source.root, PanelNode::Split { children, .. } if children.len() == 2) {
            break;
        }
        assert!(
            Instant::now() < split_wait_deadline,
            "split did not commit while create was parked"
        );
        thread::yield_now();
        multileaf = read_snapshot(&mut observer, 306, split_wait_deadline)?;
    }

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
    let settled_context = standalone_context(&settled)?;
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

#[test]
fn never_split_observer_wait_terminates_at_the_shared_deadline() -> TestResult {
    // Bounded negative diagnostic: the server keeps answering observer
    // snapshots, but the split is never sent. The shared-cap wait must end in
    // the deadline failure instead of living forever on per-read deadlines.
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first_client, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read_snapshot(&mut observer, 311, deadline())?;
    let context_id = initial.navigation.standalone_terminal_context_id.clone();

    let first_id = workbench_id(312)?;
    let first_request = command(
        first_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            terminal_context_id: context_id,
            label: WorkbenchLabel::parse("never split")?,
            geometry: workbench_geometry()?,
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

    let cap = deadline();
    let start = Instant::now();
    let wait = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> TestResult {
        let mut snapshot = read_snapshot(&mut observer, 313, cap)?;
        loop {
            let source = standalone_context(&snapshot)?
                .tabs
                .iter()
                .find(|tab| tab.tab_id == first_tuple.tab_id)
                .ok_or("split source Tab missing")?;
            if matches!(&source.root, PanelNode::Split { children, .. } if children.len() == 2) {
                break;
            }
            assert!(
                Instant::now() < cap,
                "split did not commit while create was parked"
            );
            thread::yield_now();
            snapshot = read_snapshot(&mut observer, 313, cap)?;
        }
        Ok(())
    }));
    let elapsed = start.elapsed();
    match wait {
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&str>()
                        .map(|text| (*text).to_string())
                })
                .unwrap_or_default();
            assert!(
                message.contains("split did not commit"),
                "unexpected panic payload: {message}"
            );
        }
        // The shared cap may surface through the bounded read itself as a
        // socket timeout instead of the in-loop assertion.
        Ok(Err(_)) => {}
        Ok(Ok(())) => return Err("never-split wait returned success".into()),
    }
    assert!(
        elapsed >= PROCESS_TIMEOUT - std::time::Duration::from_millis(500),
        "wait terminated before the shared cap: {elapsed:?}"
    );
    assert!(
        elapsed <= PROCESS_TIMEOUT + std::time::Duration::from_secs(2),
        "fixed-cap wait overran its bound: {elapsed:?}"
    );
    Ok(())
}
