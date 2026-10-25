//! Public grammar and receipt compatibility are checked independently of UI implementation.
use super::*;
use crate::invocation::{self, Request};
use serde_json::{Value, json};

#[test]
fn editor_grammar_is_windows_targeted_bounded_and_rejects_effect_authority_flags() {
    let target = "11111111111111111111111111111111";
    let args = |tail: &[&str]| {
        let mut v = vec!["terminal", "edit-scrollback", target];
        v.extend_from_slice(tail);
        v.into_iter()
            .map(Into::into)
            .collect::<Vec<std::ffi::OsString>>()
    };
    assert!(matches!(
        invocation::parse_request(args(&[]).into_iter()),
        Ok(Request::TerminalEditor {
            timeout_ms: 30000,
            client: None,
            ..
        })
    ));
    assert!(matches!(
        invocation::parse_request(
            args(&["--client", target, "--timeout", "1", "--format", "json"]).into_iter()
        ),
        Ok(Request::TerminalEditor {
            timeout_ms: 1,
            client: Some(_),
            format: OutputFormat::Json,
            ..
        })
    ));
    for extra in [
        &["--client", "invalid"][..],
        &["--timeout", "0"],
        &["--timeout", "3600001"],
        &["--takeover"],
        &["--cwd", "C:/tmp"],
        &["--env", "EDITOR=x"],
        &["--cols", "80"],
        &["--input-file", "-"],
        &["extra"],
    ] {
        assert!(
            invocation::parse_request(args(extra).into_iter()).is_err(),
            "{extra:?}"
        );
    }
    let help = invocation::parse_request(
        ["help", "terminal", "edit-scrollback"]
            .map(std::ffi::OsString::from)
            .into_iter(),
    )
    .unwrap();
    assert!(matches!(help, Request::Help(_)));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = crate::cli::help(
            invocation::HelpTopic::Command(invocation::CommandPath::TerminalEditScrollback),
            locale,
        );
        assert_eq!(text.status, 0);
        assert!(String::from_utf8(text.stdout).unwrap().contains("30000"));
    }
}

#[test]
fn editor_cli_native_pipe_checks_target_and_view_and_retains_one_submission()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::local_client::tests::server::FakeServer;
    use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
    use orchbee_workbench_protocol::{
        ClientPresentationSnapshot, ClientViewOperation, ClientViewRecord, ClientViewSequence,
        WorkbenchClientViewRequest, WorkbenchClientViewResult, WorkbenchRequest, WorkbenchResponse,
        read_workbench_request, write_workbench_response,
    };
    use std::{
        io::{self, Read},
        num::NonZeroU64,
    };
    let id = |n: u64| format!("{n:032x}");
    let snapshot: Snapshot = serde_json::from_value(
        json!({"protocolVersion":1,"siteId":id(2),"revision":"12",
        "navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"shell","pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":id(6),"primaryPresentation":{"customLabel":null,"presentationId":id(7),"label":"shell","binding":{"workerId":id(8),"terminalId":id(9)}},
            "childPresentations":[{"customLabel":null,"presentationId":id(11),"label":"other","binding":{"workerId":id(8),"terminalId":id(10)}}]}}]}]}),
    )?;
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))?;
    let opened = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "editor-opened-acknowledged-source")
        .unwrap();
    let member: orchbee_workbench_protocol::ClientPresentationSelection = serde_json::from_value(
        opened["document"]["outcome"]["operation"]["command"]["intent"]["spec"]["source"]["member"]
            .clone(),
    )?;
    let record: ClientViewRecord = serde_json::from_value(
        json!({"clientId":id(7),"heartbeat":"2","viewRevision":"2","availability":"current",
        "view":{"workbenchRevision":"12","phase":"ready","selection":{"terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"zoomed":false,"panelCount":1}}}),
    )?;
    let read_request =
        |stream: &mut orchbee_local_ipc::LocalStream| -> io::Result<WorkbenchClientViewRequest> {
            match read_workbench_request(stream).map_err(io::Error::other)? {
                WorkbenchRequest::ClientView(request) => Ok(request),
                other => panic!("CLI must use only passive/control Client requests: {other:?}"),
            }
        };
    let reply = |stream: &mut orchbee_local_ipc::LocalStream,
                 request: WorkbenchClientViewRequest,
                 outcome: ClientViewOutcome|
     -> io::Result<()> {
        write_workbench_response(
            stream,
            &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                protocol_version: orchbee_workbench_protocol::ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome,
            }),
        )
        .map_err(io::Error::other)
    };
    // 0 ready; 1 mismatched target; 2 changed view; 3 lost answer; 4 wrong command echo.
    for mode in 0..=4 {
        let selected = record.clone();
        let source = member.clone();
        let server = FakeServer::start(move |mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::MIN,
                },
            )
            .map_err(io::Error::other)?;
            let request = read_request(&mut stream)?;
            assert!(matches!(request.action, ClientViewAction::List {}));
            reply(
                &mut stream,
                request,
                ClientViewOutcome::Listed {
                    clients: vec![selected.clone()],
                },
            )?;
            let request = read_request(&mut stream)?;
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("passive Presentation")
            };
            assert!(
                matches!(&command.intent,ClientViewIntent::Presentation { panel_id } if panel_id.as_str()==id(6))
            );
            let operation = ClientViewOperation {
                command: command.as_ref().clone(),
                status: ClientViewControlStatus::PresentationRead {
                    presentation: Box::new(ClientPresentationSnapshot {
                        workbench_revision: orchbee_workbench_protocol::WorkbenchRevision::new(12),
                        panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(6))
                            .unwrap(),
                        presentation: Some(source.clone()),
                    }),
                    view: selected.view.clone(),
                },
            };
            reply(
                &mut stream,
                request,
                ClientViewOutcome::Operation {
                    operation: Box::new(operation),
                },
            )?;
            if mode == 1 {
                assert_eq!(stream.read(&mut [0; 1])?, 0);
                return Ok(());
            }
            let request = read_request(&mut stream)?;
            assert!(
                matches!(&request.action,ClientViewAction::Get { client_id } if client_id==&selected.client_id)
            );
            let mut current = selected.clone();
            if mode == 2 {
                current.view_revision = ClientViewSequence::new(3).unwrap();
                current.heartbeat = current.view_revision;
            }
            reply(
                &mut stream,
                request,
                ClientViewOutcome::Found {
                    client: Box::new(current),
                },
            )?;
            if mode == 2 {
                assert_eq!(stream.read(&mut [0; 1])?, 0);
                return Ok(());
            }
            let request = read_request(&mut stream)?;
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one editor Submit")
            };
            assert!(
                matches!(&command.intent,ClientViewIntent::EditorOpen { spec } if spec.source.member==source)
            );
            let mut command = command.as_ref().clone();
            let operation_id = command.operation_id.clone();
            if mode == 3 {
                return Ok(());
            }
            if mode == 4 {
                let ClientViewIntent::EditorOpen { spec } = &mut command.intent else {
                    panic!()
                };
                let orchbee_workbench_protocol::ClientPresentationSelection::Bound {
                    incarnation,
                    ..
                } = &mut spec.source.member
                else {
                    panic!()
                };
                *incarnation += 1;
            }
            let mut view = selected.view.clone();
            view.phase = ClientViewPhase::Busy;
            reply(
                &mut stream,
                request,
                ClientViewOutcome::Operation {
                    operation: Box::new(ClientViewOperation {
                        command,
                        status: ClientViewControlStatus::PopupOpened {
                            popup_id: operation_id,
                            view,
                        },
                    }),
                },
            )?;
            assert_eq!(stream.read(&mut [0; 1])?, 0);
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), "editor-cli-test")?;
        let mut signals = ArmedSignals::arm_interrupts()?;
        let target = Target::Id(id(if mode == 1 { 10 } else { 9 }));
        let answer = execute_admitted(
            &mut client,
            &snapshot,
            &target,
            Some(&record.client_id),
            Instant::now() + Duration::from_secs(3),
            &mut signals,
        );
        drop(client);
        if mode == 1 || mode == 2 {
            assert!(matches!(
                answer,
                Err(Failure {
                    kind: FailureKind::Rejected,
                    ..
                })
            ));
        } else {
            let answer = answer.unwrap();
            assert_eq!(answer.exit_code(), if mode == 0 { 0 } else { 8 });
            assert!(answer.document()["operationId"].is_string());
            assert_eq!(
                answer.document()["observation"]["kind"] == "unknown",
                mode != 0
            );
            if mode == 0 {
                assert_eq!(answer.document()["version"], 10);
            }
        }
        signals.finish()?;
        server.finish()?;
    }
    Ok(())
}
