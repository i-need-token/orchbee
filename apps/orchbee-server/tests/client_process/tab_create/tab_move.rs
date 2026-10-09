use super::*;
use orchbee_workbench_protocol::{WorkbenchRejectionReason, WorkbenchTabMoveDirection};

fn wait_for_members(client: &mut UnixStream, count: usize, limit: Instant) -> TestResult<Snapshot> {
    // SnapshotRead is the observable membership checkpoint while the first Worker launch is parked.
    let mut request = 8000;
    loop {
        let snapshot = read_snapshot(client, request, limit)?;
        let actual = standalone_context(&snapshot)?.tabs.len();
        if actual == count {
            return Ok(snapshot);
        }
        assert!(
            actual < count,
            "unexpected membership while awaiting commit"
        );
        require_before(limit)?;
        request += 1;
        std::thread::yield_now();
    }
}

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
fn tab_move_preserves_pending_first_create_and_exact_replay_after_lost_response() -> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first_client, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second_client, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut lost_response, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read_snapshot(&mut observer, 100, deadline())?;
    let first_id = workbench_id(101)?;
    let first_request = command(
        first_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            placement: None,
            terminal_context_id: initial.navigation.standalone_terminal_context_id.clone(),
            label: WorkbenchLabel::parse("A creation")?,
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
    let parked_deadline = Instant::now() + Duration::from_secs(5);
    let one = read_snapshot(&mut observer, 102, parked_deadline)?;
    let first_pending =
        exchange_command(&mut observer, &first_request, &first_id, parked_deadline)?;
    let first_tuple = pending_tab(&first_pending.outcome)?;
    let first_binding = primary_presentation(&standalone_context(&one)?.tabs[0])?
        .0
        .binding
        .as_ref()
        .ok_or("first binding missing")?;
    binding_matches_launch(first_binding, &first_launch);
    let second_id = workbench_id(103)?;
    let second_request = command(
        second_id.clone(),
        &one,
        WorkbenchAction::TabCreate {
            placement: None,
            terminal_context_id: one.navigation.standalone_terminal_context_id.clone(),
            label: WorkbenchLabel::parse("B creation")?,
            geometry: workbench_geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    write_obwp(&mut second_client, &second_request, parked_deadline)?;
    let two = wait_for_members(&mut observer, 2, parked_deadline)?;
    let second_pending =
        exchange_command(&mut observer, &second_request, &second_id, parked_deadline)?;
    let second_tuple = pending_tab(&second_pending.outcome)?;
    let move_id = workbench_id(104)?;
    let move_request = command(
        move_id.clone(),
        &two,
        WorkbenchAction::TabMove {
            terminal_context_id: first_tuple.terminal_context_id.clone(),
            tab_id: first_tuple.tab_id.clone(),
            direction: WorkbenchTabMoveDirection::Last,
        },
    );
    write_obwp(&mut lost_response, &move_request, parked_deadline)?;
    // The response remains unread. A different admitted Client observes the committed order.
    let mut moved = read_snapshot(&mut observer, 105, parked_deadline)?;
    while moved.revision == two.revision {
        moved = read_snapshot(&mut observer, 105, parked_deadline)?;
    }
    let mut expected = two.clone();
    expected.revision = WorkbenchRevision::new(3);
    expected.terminal_contexts[0].tabs.reverse();
    assert_eq!(moved, expected);
    drop(lost_response);
    let replay = exchange_command(&mut observer, &move_request, &move_id, parked_deadline)?;
    assert_eq!(
        replay.outcome,
        WorkbenchCommandOutcome::TabMoved {
            revision: WorkbenchRevision::new(3),
            terminal_context_id: first_tuple.terminal_context_id.clone(),
            tab_id: first_tuple.tab_id.clone(),
        }
    );
    assert_eq!(
        exchange_command(&mut observer, &first_request, &first_id, parked_deadline)?,
        first_pending
    );
    assert_eq!(
        exchange_command(&mut observer, &second_request, &second_id, parked_deadline)?,
        second_pending
    );
    assert_eq!(read_snapshot(&mut observer, 106, parked_deadline)?, moved);
    require_before(parked_deadline)?;

    accept_launch(&mut worker, &first_launch, 3, deadline())?;
    let first_final = read_command_result(&mut first_client, &first_id, deadline())?;
    assert_eq!(final_tab(&first_final.outcome)?, first_tuple);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(second_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("second Launch expected".into());
    };
    let second_binding = primary_presentation(&standalone_context(&two)?.tabs[1])?
        .0
        .binding
        .as_ref()
        .ok_or("second binding missing")?;
    binding_matches_launch(second_binding, &second_launch);
    assert_ne!(
        first_launch.authorization().terminal_id(),
        second_launch.authorization().terminal_id()
    );
    accept_launch(&mut worker, &second_launch, 6, deadline())?;
    let second_final = read_command_result(&mut second_client, &second_id, deadline())?;
    assert_eq!(final_tab(&second_final.outcome)?, second_tuple);
    assert_eq!(
        exchange_command(&mut observer, &first_request, &first_id, deadline())?,
        first_final
    );
    assert_eq!(
        exchange_command(&mut observer, &second_request, &second_id, deadline())?,
        second_final
    );

    let third_id = workbench_id(107)?;
    let third_request = command(
        third_id.clone(),
        &moved,
        WorkbenchAction::TabCreate {
            placement: None,
            terminal_context_id: moved.navigation.standalone_terminal_context_id.clone(),
            label: WorkbenchLabel::parse("C creation")?,
            geometry: workbench_geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    write_obwp(&mut first_client, &third_request, deadline())?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(third_launch) =
        read_worker_message(&mut worker, deadline())?
    else {
        return Err("third Launch expected".into());
    };
    accept_launch(&mut worker, &third_launch, 9, deadline())?;
    let third_final = read_command_result(&mut first_client, &third_id, deadline())?;
    let third_tuple = final_tab(&third_final.outcome)?;
    assert_ne!(
        third_launch.authorization().terminal_id(),
        first_launch.authorization().terminal_id()
    );
    assert_ne!(
        third_launch.authorization().terminal_id(),
        second_launch.authorization().terminal_id()
    );
    let three = read_snapshot(&mut observer, 108, deadline())?;
    let original_tabs = standalone_context(&three)?.tabs.clone(); // B, A, C
    assert_eq!(original_tabs[2].tab_id, third_tuple.tab_id);
    let third_binding = primary_presentation(&original_tabs[2])?
        .0
        .binding
        .as_ref()
        .ok_or("third binding missing")?;
    binding_matches_launch(third_binding, &third_launch);
    let mut current = three.clone();
    for (index, (direction, order)) in [
        (WorkbenchTabMoveDirection::Previous, [1, 0, 2]),
        (WorkbenchTabMoveDirection::Next, [0, 1, 2]),
        (WorkbenchTabMoveDirection::Last, [0, 2, 1]),
        (WorkbenchTabMoveDirection::First, [1, 0, 2]),
    ]
    .into_iter()
    .enumerate()
    {
        let request_id = workbench_id(120 + index as u128)?;
        let request = command(
            request_id.clone(),
            &current,
            WorkbenchAction::TabMove {
                terminal_context_id: first_tuple.terminal_context_id.clone(),
                tab_id: first_tuple.tab_id.clone(),
                direction,
            },
        );
        let result = exchange_command(&mut second_client, &request, &request_id, deadline())?;
        let revision = WorkbenchRevision::new(5 + index as u64);
        assert_eq!(
            result.outcome,
            WorkbenchCommandOutcome::TabMoved {
                revision,
                terminal_context_id: first_tuple.terminal_context_id.clone(),
                tab_id: first_tuple.tab_id.clone()
            }
        );
        current = three.clone();
        current.revision = revision;
        current.terminal_contexts[0].tabs =
            order.map(|index| original_tabs[index].clone()).to_vec();
        assert_eq!(
            read_snapshot(&mut observer, 140 + index as u128, deadline())?,
            current
        );
    }
    assert_eq!(
        exchange_command(&mut observer, &move_request, &move_id, deadline())?,
        replay
    );
    assert_eq!(read_snapshot(&mut observer, 150, deadline())?, current);
    for (index, missing_context) in [false, true].into_iter().enumerate() {
        let request_id = workbench_id(160 + index as u128)?;
        let context = if missing_context {
            WorkbenchTerminalContextId::parse(&format!("{:032x}", 999))?
        } else {
            first_tuple.terminal_context_id.clone()
        };
        let request = command(
            request_id.clone(),
            &current,
            WorkbenchAction::TabMove {
                terminal_context_id: context,
                tab_id: WorkbenchTabId::parse(&format!("{:032x}", 999))?,
                direction: WorkbenchTabMoveDirection::First,
            },
        );
        let result = exchange_command(&mut observer, &request, &request_id, deadline())?;
        assert_eq!(
            result.outcome,
            WorkbenchCommandOutcome::Rejected {
                revision: current.revision,
                reason: if missing_context {
                    WorkbenchRejectionReason::TerminalContextUnavailable
                } else {
                    WorkbenchRejectionReason::TabUnavailable
                }
            }
        );
    }
    assert_eq!(read_snapshot(&mut observer, 170, deadline())?, current);
    worker.set_nonblocking(true)?;
    let mut byte = [0];
    assert!(
        matches!(worker.read(&mut byte), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
        "Move or replay dispatched another Worker message"
    );
    Ok(())
}
