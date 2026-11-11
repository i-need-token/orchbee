use super::*;

const RENAMED_SECOND_TAB_LABEL: &str = "Process-renamed Tab";

fn expected_renamed_snapshot(
    committed: &Snapshot,
    tab: &CommittedTab,
    revision: WorkbenchRevision,
) -> TestResult<Snapshot> {
    let mut expected = committed.clone();
    expected.revision = revision;
    let initial_workspace_id = expected.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = expected
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == initial_workspace_id)
        .ok_or_else(|| io::Error::other("missing expected initial_workspace Terminal Context"))?;
    let expected_tab = context
        .tabs
        .iter_mut()
        .find(|candidate| candidate.tab_id == tab.tab_id)
        .ok_or_else(|| io::Error::other("missing expected renamed Tab"))?;
    expected_tab.label = RENAMED_SECOND_TAB_LABEL.to_owned();
    Ok(expected)
}

#[test]
fn tab_rename_commits_during_parked_create_and_both_replays_keep_their_revision() -> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut viewer, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut creator, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;

    let setup_deadline = deadline();
    let create_request_id = begin_fixture_tab(&mut viewer, &mut worker, setup_deadline)?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(first_launch) =
        read_worker_message(&mut worker, setup_deadline)?
    else {
        return Err(io::Error::other("missing first initial_workspace Launch").into());
    };
    assert_eq!(
        first_launch.authorization().worker_id().as_bytes(),
        FIRST_WORKER.as_bytes()
    );
    let TerminalLaunchMode::Immediate(first_geometry) = first_launch.mode() else {
        return Err(io::Error::other("initial_workspace Open dispatched a deferred Launch").into());
    };
    let first_host = host_identity(3)?;
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            first_launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                first_host,
                HostLaunchState::Started(first_geometry),
            )),
        )?),
        setup_deadline,
    )?;
    let created = read_command_result(&mut viewer, &create_request_id, setup_deadline)?;
    assert!(matches!(
        created.outcome,
        WorkbenchCommandOutcome::TabCreated { .. }
    ));

    let initial = read_snapshot(&mut creator, 101, setup_deadline)?;
    let initial_context = initial_workspace_context(&initial)?;
    assert!(initial_context.groups.is_empty());
    assert_eq!(initial_context.tabs.len(), 1);
    let initial_tab = initial_context
        .tabs
        .first()
        .ok_or_else(|| io::Error::other("missing initial initial_workspace Tab"))?
        .clone();
    let (first_presentation, _) = assert_simple_tab(&initial_tab, "1")?;
    let first_binding = first_presentation
        .binding
        .as_ref()
        .ok_or_else(|| io::Error::other("first initial_workspace Tab was unbound"))?;
    binding_matches_launch(first_binding, &first_launch);

    let attach_request_id = client_id(2)?;
    write_obcs(
        &mut viewer,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            attach_request_id.clone(),
            TerminalViewerAction::AttachBinding {
                session_id: ObcsSessionId::parse(initial.session_id.as_str())?,
                worker_id: ClientWorkerId::parse(first_binding.worker_id.as_str())?,
                terminal_id: ClientTerminalId::parse(first_binding.terminal_id.as_str())?,
            },
        )),
        setup_deadline,
    )?;
    let CapabilityNodeMessage::TerminalViewerRequest(attached) =
        read_worker_message(&mut worker, setup_deadline)?
    else {
        return Err(io::Error::other("missing Worker viewer Attach").into());
    };
    assert_eq!(attached.worker_id, first_launch.authorization().worker_id());
    assert_eq!(
        attached.worker_generation,
        first_launch.authorization().worker_generation()
    );
    assert_eq!(
        attached.action,
        WorkerViewerAction::Attach {
            host_epoch: first_host.host_epoch(),
            terminal_id: first_host.terminal_id(),
        }
    );
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
            request_id: attached.request_id,
            attachment_id: attached.attachment_id,
            outcome: WorkerViewerOutcome::Attached,
        }),
        setup_deadline,
    )?;
    let ServerMessage::TerminalViewerResult(attach_result) =
        read_obcs(&mut viewer, setup_deadline)?
    else {
        return Err(io::Error::other("missing Client viewer Attach result").into());
    };
    assert_eq!(attach_result.request_id(), &attach_request_id);
    let TerminalViewerOutcome::Attached {
        attachment_id,
        host_epoch,
        host_terminal_id,
        ..
    } = attach_result.outcome()
    else {
        return Err(io::Error::other("Client viewer Attach was not accepted").into());
    };
    assert_eq!(
        attachment_id.as_str(),
        hex_identity(attached.attachment_id.as_bytes())
    );
    assert_eq!(
        host_epoch.as_str(),
        hex_identity(first_host.host_epoch().as_bytes())
    );
    assert_eq!(
        host_terminal_id.as_str(),
        hex_identity(first_host.terminal_id().as_bytes())
    );
    require_before(setup_deadline)?;

    let create_request_id = workbench_id(102)?;
    let tab_geometry = workbench_geometry()?;
    let create_request = command(
        create_request_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: initial_context.terminal_context_id.clone(),
            label: WorkbenchLabel::parse(SECOND_TAB_LABEL)?,
            geometry: tab_geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let parked_deadline = deadline();
    write_obwp(&mut creator, &create_request, parked_deadline)?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(second_launch) =
        read_worker_message(&mut worker, parked_deadline)?
    else {
        return Err(io::Error::other("missing tab.create Launch").into());
    };
    assert_launch_geometry(&second_launch, tab_geometry)?;
    assert_eq!(
        second_launch.authorization().worker_id(),
        first_launch.authorization().worker_id()
    );
    assert_eq!(
        second_launch.authorization().worker_generation(),
        first_launch.authorization().worker_generation()
    );
    assert_ne!(
        second_launch.authorization().terminal_id(),
        first_launch.authorization().terminal_id()
    );
    assert_ne!(
        second_launch.authorization().operation_id(),
        first_launch.authorization().operation_id()
    );

    // Receipt of this Launch is the causal checkpoint that tab.create already committed.
    // Its result stays parked while independent Workbench traffic observes and renames it.
    let committed = read_snapshot(&mut observer, 201, parked_deadline)?;
    assert_eq!(committed.revision.value(), initial.revision.value() + 1);
    let committed_context = initial_workspace_context(&committed)?;
    assert!(committed_context.groups.is_empty());
    assert_eq!(
        committed_context.terminal_context_id,
        initial_context.terminal_context_id
    );
    assert_eq!(committed_context.tabs.len(), 2);
    assert_eq!(committed_context.tabs.first(), Some(&initial_tab));
    let second_tab = committed_context
        .tabs
        .get(1)
        .ok_or_else(|| io::Error::other("missing process-created Tab"))?;
    let (second_presentation, second_panel_id) = assert_simple_tab(second_tab, SECOND_TAB_LABEL)?;
    let second_binding = second_presentation
        .binding
        .as_ref()
        .ok_or_else(|| io::Error::other("process-created Tab was unbound"))?
        .clone();
    binding_matches_launch(&second_binding, &second_launch);

    let pending_before_rename = exchange_command(
        &mut observer,
        &create_request,
        &create_request_id,
        parked_deadline,
    )?;
    let committed_tab = pending_tab(&pending_before_rename.outcome)?;
    assert_eq!(committed_tab.revision, committed.revision);
    assert_eq!(
        committed_tab.terminal_context_id,
        committed_context.terminal_context_id
    );
    assert_eq!(committed_tab.tab_id, second_tab.tab_id);
    assert_eq!(committed_tab.panel_id, *second_panel_id);
    assert_eq!(
        committed_tab.presentation_id,
        second_presentation.presentation_id
    );

    let rename_request_id = workbench_id(103)?;
    let rename_request = command(
        rename_request_id.clone(),
        &committed,
        WorkbenchAction::TabRename {
            terminal_context_id: committed_tab.terminal_context_id.clone(),
            tab_id: committed_tab.tab_id.clone(),
            label: WorkbenchLabel::parse(RENAMED_SECOND_TAB_LABEL)?,
        },
    );
    let renamed = exchange_command(
        &mut observer,
        &rename_request,
        &rename_request_id,
        parked_deadline,
    )?;
    let WorkbenchCommandOutcome::Renamed {
        revision: renamed_revision,
    } = &renamed.outcome
    else {
        return Err(io::Error::other(format!(
            "tab.rename did not complete while Launch was parked: {:?}",
            renamed.outcome
        ))
        .into());
    };
    assert_eq!(renamed_revision.value(), committed.revision.value() + 1);

    let after_rename = read_snapshot(&mut observer, 202, parked_deadline)?;
    assert_eq!(
        after_rename,
        expected_renamed_snapshot(&committed, &committed_tab, *renamed_revision)?
    );
    let renamed_context = initial_workspace_context(&after_rename)?;
    assert_eq!(renamed_context.tabs.first(), Some(&initial_tab));
    let renamed_tab = renamed_context
        .tabs
        .get(1)
        .ok_or_else(|| io::Error::other("renamed Tab changed canonical order"))?;
    assert_eq!(renamed_tab.tab_id, committed_tab.tab_id);
    assert_eq!(renamed_tab.label, RENAMED_SECOND_TAB_LABEL);
    let (renamed_presentation, renamed_panel_id) = primary_presentation(renamed_tab)?;
    assert_eq!(renamed_panel_id, &committed_tab.panel_id);
    assert_eq!(
        renamed_presentation.presentation_id,
        committed_tab.presentation_id
    );
    assert_eq!(renamed_presentation.label, SECOND_TAB_LABEL);
    assert_eq!(renamed_presentation.binding.as_ref(), Some(&second_binding));

    let pending_after_rename = exchange_command(
        &mut observer,
        &create_request,
        &create_request_id,
        parked_deadline,
    )?;
    // A fixed pending replay after Renamed proves both the shared gate and the parked create
    // ledger remain responsive without mutating the original revision or member tuple.
    assert_eq!(pending_after_rename, pending_before_rename);
    require_before(parked_deadline)?;

    let settlement_deadline = deadline();
    let second_host = host_identity(6)?;
    assert_ne!(second_host, first_host);
    let TerminalLaunchMode::Immediate(second_geometry) = second_launch.mode() else {
        unreachable!("tab.create geometry was checked above")
    };
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            second_launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                second_host,
                HostLaunchState::Started(second_geometry),
            )),
        )?),
        settlement_deadline,
    )?;
    let original_create =
        read_command_result(&mut creator, &create_request_id, settlement_deadline)?;
    assert_eq!(final_tab(&original_create.outcome)?, committed_tab);
    let replayed_create = exchange_command(
        &mut observer,
        &create_request,
        &create_request_id,
        settlement_deadline,
    )?;
    assert_eq!(replayed_create, original_create);
    let replayed_rename = exchange_command(
        &mut observer,
        &rename_request,
        &rename_request_id,
        settlement_deadline,
    )?;
    assert_eq!(replayed_rename, renamed);
    assert_eq!(
        read_snapshot(&mut observer, 203, settlement_deadline)?,
        after_rename
    );

    // The preattached Poll is a positive FIFO boundary: any replayed Launch would arrive first.
    let poll_request_id = client_id(3)?;
    write_obcs(
        &mut viewer,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            poll_request_id.clone(),
            TerminalViewerAction::Poll {
                attachment_id: attachment_id.clone(),
            },
        )),
        settlement_deadline,
    )?;
    let next = read_worker_message(&mut worker, settlement_deadline)?;
    let CapabilityNodeMessage::TerminalViewerRequest(polled) = next else {
        return Err(io::Error::other(format!(
            "expected viewer Poll after rename replays, received {next:?}"
        ))
        .into());
    };
    assert_eq!(polled.action, WorkerViewerAction::Poll);
    assert_eq!(polled.worker_id, attached.worker_id);
    assert_eq!(polled.worker_generation, attached.worker_generation);
    assert_eq!(polled.attachment_id, attached.attachment_id);
    assert_ne!(polled.request_id, attached.request_id);
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
            request_id: polled.request_id,
            attachment_id: polled.attachment_id,
            outcome: WorkerViewerOutcome::Empty,
        }),
        settlement_deadline,
    )?;
    let ServerMessage::TerminalViewerResult(poll_result) =
        read_obcs(&mut viewer, settlement_deadline)?
    else {
        return Err(io::Error::other("missing Client viewer Poll result").into());
    };
    assert_eq!(poll_result.request_id(), &poll_request_id);
    assert_eq!(poll_result.outcome(), &TerminalViewerOutcome::Empty {});
    require_before(settlement_deadline)?;
    Ok(())
}
