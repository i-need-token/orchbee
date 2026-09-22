use super::super::{ConnectionSlots, handle_candidate};
use crate::{
    agent_service::{AgentPolicy, AgentService},
    client_session_registry::ClientSessionRegistry,
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_local_client_protocol::{ClientHello, LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_workbench_protocol::{
    WorkbenchCommandOutcome, WorkbenchResponse, WorkbenchSnapshotOutcome, parse_workbench_request,
    read_workbench_response, write_workbench_request,
};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[test]
fn an_oblc_admitted_session_with_read_only_policy_cannot_mutate_or_discover_ledger_state()
-> io::Result<()> {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: false,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::clone(&terminals),
    );
    let agents = AgentService::open(AgentPolicy { read: true })?;
    let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
    let slots = Arc::new(ConnectionSlots::new());
    let slot = slots.acquire().unwrap();
    let (mut client, server) = UnixStream::pair()?;
    client.set_read_timeout(Some(Duration::from_secs(3)))?;
    client.set_write_timeout(Some(Duration::from_secs(3)))?;
    let server_thread = thread::spawn(move || {
        handle_candidate(
            server, &registry, "test", slot, &terminals, &service, &agents,
        )
    });
    write_message(
        &mut client,
        &LocalClientMessage::ClientHello(ClientHello::new("test").unwrap()),
    )
    .unwrap();
    assert!(matches!(
        read_message(&mut client).unwrap(),
        LocalClientMessage::ClientAccepted { .. }
    ));
    let read = parse_workbench_request(br#"{"kind":"workbench.snapshot.read","protocolVersion":1,"requestId":"11111111111111111111111111111111"}"#).unwrap();
    write_workbench_request(&mut client, &read).unwrap();
    let WorkbenchResponse::Snapshot(initial) = read_workbench_response(&mut client).unwrap() else {
        panic!("expected snapshot")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = &initial.outcome else {
        panic!("expected available")
    };
    let json = format!(
        r#"{{"kind":"workbench.command","protocolVersion":1,"requestId":"22222222222222222222222222222222","siteId":"{}","expectedRevision":"0","action":{{"kind":"project.create","label":"Forbidden"}}}}"#,
        snapshot.site_id
    );
    let command = parse_workbench_request(json.as_bytes()).unwrap();
    for _replay in 0..2 {
        write_workbench_request(&mut client, &command).unwrap();
        let WorkbenchResponse::Command(result) = read_workbench_response(&mut client).unwrap()
        else {
            panic!("expected command")
        };
        assert_eq!(result.outcome, WorkbenchCommandOutcome::Denied {});
    }

    use orchbee_workbench_protocol::{
        ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandResult,
        WorkbenchRequest, WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration,
        WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseResume,
        WorkbenchTabCloseResumeOutcome, WorkbenchTabId,
    };
    let close_read = WorkbenchTabCloseRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("33333333333333333333333333333333").unwrap(),
        site_id: snapshot.site_id.clone(),
        terminal_context_id: snapshot.navigation.standalone_terminal_context_id.clone(),
        tab_id: WorkbenchTabId::parse("44444444444444444444444444444444").unwrap(),
    };
    let close = WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: close_read.request_id.clone(),
        site_id: close_read.site_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: close_read.terminal_context_id.clone(),
            tab_id: close_read.tab_id.clone(),
        },
    });
    write_workbench_request(&mut client, &close).unwrap();
    assert!(matches!(
        read_workbench_response(&mut client).unwrap(),
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    write_workbench_request(
        &mut client,
        &WorkbenchRequest::TabCloseRead(close_read.clone()),
    )
    .unwrap();
    let WorkbenchResponse::TabCloseRead(read_result) =
        read_workbench_response(&mut client).unwrap()
    else {
        panic!("Close read result expected")
    };
    assert_eq!(read_result.request_id, close_read.request_id);
    assert_eq!(
        read_result.outcome,
        WorkbenchTabCloseReadOutcome::NotClosing {}
    );
    let resume = WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: close_read.request_id.clone(),
        site_id: close_read.site_id,
        terminal_context_id: close_read.terminal_context_id,
        tab_id: close_read.tab_id,
        close_request_id: WorkbenchRequestId::parse("55555555555555555555555555555555").unwrap(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
    };
    write_workbench_request(
        &mut client,
        &WorkbenchRequest::TabCloseResume(resume.clone()),
    )
    .unwrap();
    let WorkbenchResponse::TabCloseResume(resumed) = read_workbench_response(&mut client).unwrap()
    else {
        panic!("Resume result expected")
    };
    assert_eq!(resumed.request_id, resume.request_id);
    assert_eq!(resumed.site_id, resume.site_id);
    assert_eq!(resumed.terminal_context_id, resume.terminal_context_id);
    assert_eq!(resumed.tab_id, resume.tab_id);
    assert_eq!(resumed.close_request_id, resume.close_request_id);
    assert_eq!(resumed.outcome, WorkbenchTabCloseResumeOutcome::Denied {});
    write_workbench_request(&mut client, &read).unwrap();
    assert_eq!(
        read_workbench_response(&mut client).unwrap(),
        WorkbenchResponse::Snapshot(initial)
    );
    drop(client);
    server_thread.join().unwrap();
    Ok(())
}
