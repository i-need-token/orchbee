use super::*;
use crate::local_client::{
    LocalClient,
    test_server::{FakeServer, adjustable_operation_clock},
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use std::{
    io::{self, Read},
    num::NonZeroU64,
};

fn admit(stream: &mut LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}
fn id(value: u64) -> String {
    format!("{value:032x}")
}
fn pane() -> PluginPaneInfo {
    serde_json::from_value(serde_json::json!({"selector":{"pluginId":"tools","entrypointId":"dashboard"},"terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"presentationId":id(7),"binding":{"workerId":id(8),"terminalId":id(9)}})).unwrap()
}
fn job(clock: &LocalOperationClock) -> OverlayJob {
    let pane = pane();
    OverlayJob {
        site: SiteId::parse(&id(2)).unwrap(),
        context: pane.terminal_context_id,
        tab: pane.tab_id,
        member: ClientOverlayMember {
            panel_id: pane.panel_id,
            presentation_id: pane.presentation_id,
            binding: Some(pane.binding),
        },
        id: WorkbenchRequestId::parse(&id(10)).unwrap(),
        deadline: clock.now() + Duration::from_secs(3),
    }
}
fn reply(
    stream: &mut LocalStream,
    request: &PluginRequest,
    outcome: PluginOutcome,
) -> io::Result<()> {
    write_workbench_response(
        stream,
        &WorkbenchResponse::Plugin(PluginResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome,
        }),
    )
    .map_err(io::Error::other)
}

#[test]
fn overlay_lane_only_closes_known_settlement_and_retains_the_actual_or_uncertain_close_id()
-> Result<(), Box<dyn std::error::Error>> {
    for case in ["running", "unavailable", "rebound", "settled", "lost-close"] {
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            admit(&mut stream)?;
            if index == 0 {
                return Ok(());
            }
            let WorkbenchRequest::Plugin(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("plugin read")
            };
            assert!(matches!(request.action, PluginAction::PaneGet { .. }));
            let mut current = pane();
            if case == "rebound" {
                current.presentation_id = TerminalPresentationId::parse(&id(99)).unwrap();
            }
            reply(
                &mut stream,
                &request,
                PluginOutcome::PaneFound {
                    revision: WorkbenchRevision::new(12),
                    pane: Box::new(current),
                },
            )?;
            if case != "rebound" {
                let WorkbenchRequest::Plugin(request) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("lifecycle read")
                };
                assert!(
                    matches!(&request.action, PluginAction::PaneObserve { pane: expected } if **expected == pane())
                );
                let outcome = if case == "unavailable" {
                    PluginOutcome::Failed {
                        reason: PluginFailure::Unavailable,
                    }
                } else {
                    PluginOutcome::PaneObserved {
                        revision: WorkbenchRevision::new(13),
                        pane: Box::new(pane()),
                        settled: case != "running",
                    }
                };
                reply(&mut stream, &request, outcome)?;
            }
            if matches!(case, "settled" | "lost-close") {
                let WorkbenchRequest::Command(command) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("one canonical close")
                };
                assert_eq!(command.expected_revision, WorkbenchRevision::new(13));
                assert_eq!(command.request_id.as_str(), id(10));
                let WorkbenchAction::PanelClose {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = command.action
                else {
                    panic!("panel close")
                };
                assert_eq!(panel_id, pane().panel_id);
                if case == "lost-close" {
                    return Ok(());
                }
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: command.request_id,
                        outcome: WorkbenchCommandOutcome::PanelCloseAccepted {
                            close_request_id: WorkbenchRequestId::parse(&id(11)).unwrap(),
                            terminal_context_id,
                            tab_id,
                            panel_id,
                            accepted_revision: WorkbenchRevision::new(13),
                        },
                    }),
                )
                .map_err(io::Error::other)?;
            }
            assert_eq!(stream.read(&mut [0; 1])?, 0, "no repeat or guessed close");
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "overlay-test")?;
        let origin = client.origin();
        drop(client);
        let clock = adjustable_operation_clock(Arc::new(std::sync::atomic::AtomicU64::new(0)));
        let (wake, _wake_writer) = LocalStream::pair()?;
        let result = execute(
            &origin,
            &clock,
            &job(&clock),
            &wake,
            &AtomicBool::new(false),
        );
        match result {
            OverlayResult::Waiting => assert!(!matches!(case, "settled" | "lost-close")),
            OverlayResult::CloseOwned { operation_id } => assert_eq!(
                operation_id.as_str(),
                id(if case == "settled" { 11 } else { 10 })
            ),
        }
        server.finish()?;
    }
    Ok(())
}

#[test]
fn overlay_lane_shutdown_interrupts_a_pending_read_and_joins_its_owned_thread()
-> Result<(), Box<dyn std::error::Error>> {
    let (started, waiting) = std::sync::mpsc::channel();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        admit(&mut stream)?;
        if index == 0 {
            return Ok(());
        }
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Plugin(_)
        ));
        started.send(()).map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0; 1])?, 0);
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "overlay-test")?;
    let origin = client.origin();
    drop(client);
    let clock = adjustable_operation_clock(Arc::new(std::sync::atomic::AtomicU64::new(0)));
    let lane = Lane::spawn(origin, clock.clone())?;
    lane.sender.as_ref().unwrap().send(job(&clock)).unwrap();
    waiting.recv_timeout(Duration::from_secs(3))?;
    let before = Instant::now();
    drop(lane);
    assert!(before.elapsed() < Duration::from_secs(2));
    server.finish()?;
    Ok(())
}
