use super::*;
use crate::local_client::workbench::layout::{LayoutRequest, LayoutSubmission};
use orchbee_workbench_protocol::WorkbenchPanelTerminalTarget;

fn layout_command() -> WorkbenchCommand {
    workbench_command(FIRST_REQUEST_ID,7,WorkbenchAction::TabCreateLayout { terminal_context_id: context_id(30),
        label: workbench_label(),automatic_label:false,
        layout: serde_json::from_value(serde_json::json!({"kind":"panel","primary":{"label":"Empty"},"children":[],"rightClick":"orchbee"})).unwrap() })
}
fn bind_command() -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        7,
        WorkbenchAction::PanelCreateTerminal {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            target: WorkbenchPanelTerminalTarget::Unbound {
                presentation_id: TerminalPresentationId::parse(&id(33)).unwrap(),
            },
            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    )
}
fn pending() -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelTerminalPending {
        revision: WorkbenchRevision::new(8),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(33)).unwrap(),
    }
}
#[test]
fn layout_transport_preserves_exact_commits_and_definite_rejections() -> TestResult {
    let cases = [
        (
            layout_command(),
            WorkbenchCommandOutcome::TabLayoutCreated {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            },
        ),
        (bind_command(), pending()),
        (
            bind_command(),
            WorkbenchCommandOutcome::PanelTerminalCreated {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
                panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
                presentation_id: TerminalPresentationId::parse(&id(33)).unwrap(),
                terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            },
        ),
        (layout_command(), WorkbenchCommandOutcome::Denied {}),
        (
            layout_command(),
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(99),
                reason: WorkbenchRejectionReason::RevisionMismatch,
            },
        ),
    ];
    for (command, outcome) in cases {
        let sent = command.clone();
        let reply = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 61)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(sent)
            );
            write_workbench_response(&mut stream, &command_response(FIRST_REQUEST_ID, reply))
                .map_err(io::Error::other)?;
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let answer = client.submit_workbench_layout(
            LayoutRequest::try_from(command).unwrap(),
            Instant::now() + Duration::from_secs(1),
            None,
        );
        assert!(matches!(answer,LayoutSubmission::Outcome(value) if *value==outcome));
        assert!(client.connection.is_some());
        drop(client);
        server.finish()?;
    }
    Ok(())
}
#[test]
fn layout_transport_retires_mismatched_or_lost_replies_without_replay() -> TestResult {
    for mode in 0..6 {
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 61)?;
            let _ = read_workbench_request(&mut stream).map_err(io::Error::other)?;
            if mode == 5 {
                return Ok(());
            }
            let mut reply = command_response(FIRST_REQUEST_ID, pending());
            let WorkbenchResponse::Command(result) = &mut reply else {
                panic!()
            };
            match mode {
                0 => result.request_id = WorkbenchRequestId::parse(&id(99)).unwrap(),
                1 => {
                    if let WorkbenchCommandOutcome::PanelTerminalPending { revision, .. } =
                        &mut result.outcome
                    {
                        *revision = WorkbenchRevision::new(9);
                    }
                }
                2 => {
                    if let WorkbenchCommandOutcome::PanelTerminalPending {
                        presentation_id, ..
                    } = &mut result.outcome
                    {
                        *presentation_id = TerminalPresentationId::parse(&id(99)).unwrap();
                    }
                }
                3 => {
                    if let WorkbenchCommandOutcome::PanelTerminalPending { panel_id, .. } =
                        &mut result.outcome
                    {
                        *panel_id = WorkbenchPanelId::parse(&id(99)).unwrap();
                    }
                }
                4 => {
                    result.outcome = WorkbenchCommandOutcome::TabLayoutCreated {
                        revision: WorkbenchRevision::new(8),
                        terminal_context_id: context_id(30),
                        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
                    }
                }
                _ => unreachable!(),
            }
            write_workbench_response(&mut stream, &reply).map_err(io::Error::other)?;
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = Instant::now() + Duration::from_secs(1);
        assert!(matches!(
            client.submit_workbench_layout(
                LayoutRequest::try_from(bind_command()).unwrap(),
                deadline,
                None
            ),
            LayoutSubmission::Unknown { .. }
        ));
        assert!(client.connection.is_none());
        assert!(matches!(
            client.submit_workbench_layout(
                LayoutRequest::try_from(bind_command()).unwrap(),
                deadline,
                None
            ),
            LayoutSubmission::NotSubmitted { .. }
        ));
        drop(client);
        server.finish()?;
    }
    Ok(())
}
