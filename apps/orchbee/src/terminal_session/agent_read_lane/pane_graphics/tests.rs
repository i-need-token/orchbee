use super::*;
use crate::local_client::tests::server::FakeServer;
use crate::terminal_session::agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::*;
use std::{
    io::{self, Read},
    num::NonZeroU64,
    time::Duration,
};

fn fixture() -> (Snapshot, Source, PaneGraphicsLayer) {
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    let target = PaneGraphicsTarget {
        panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
        presentation_id: Some(TerminalPresentationId::parse(&format!("{:032x}", 5)).unwrap()),
    };
    let snapshot = Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        revision: WorkbenchRevision::new(5),
        navigation: Navigation {
            project_order: Vec::new(),
            directories: Vec::new(),
            workspaces: Vec::new(),
            projects: vec![],
            standalone_terminal_context_id: context.clone(),
            workspace_order: vec![],
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: context,
            groups: vec![],
            tabs: vec![Tab {
                automatic_label: false,
                group_id: None,
                label: "Tab".into(),
                pinned: false,
                tab_id: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
                root: PanelNode::Panel {
                    right_click: PanelRightClickTarget::default(),
                    panel_id: target.panel_id.clone(),
                    child_presentations: vec![],
                    primary_presentation: Presentation {
                        presentation_id: target.presentation_id.clone().unwrap(),
                        binding: None,
                        custom_label: None,
                        label: "Panel".into(),
                        launch_failure: None,
                    },
                },
            }],
        }],
    };
    let metadata = PaneGraphicsLayer {
        layer_id: "primary".into(),
        z_index: 0,
        width: 256,
        height: 257,
        placement: PaneGraphicsPlacement::default(),
        resource_revision: WorkbenchRevision::new(1),
        stream_id: None,
    };
    (
        snapshot,
        Source {
            target,
            binding: None,
        },
        metadata,
    )
}

#[test]
fn actual_read_connection_stages_every_chunk_and_rejects_catalog_source_and_offset_changes() {
    // 0 full resource, 1 wrong offset, 2 changed catalog, 3 changed binding,
    // 4 wrong Site, 5 shared cached resource, 6 unrelated Site graphics revision advance.
    for fault in 0..=6 {
        let server = FakeServer::start_connections(2, move |index, mut stream| {
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
            if index == 0 {
                return Ok(());
            }
            let (snapshot, source, metadata) = fixture();
            let (mut snapshots, mut catalogs, mut chunks) = (0, 0, 0);
            loop {
                let request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
                let mut last = false;
                let response = match request {
                    WorkbenchRequest::SnapshotRead(request) => {
                        snapshots += 1;
                        last = snapshots == 2 || fault == 4;
                        let mut snapshot = snapshot.clone();
                        if fault == 4 {
                            snapshot.site_id = SiteId::parse(&format!("{:032x}", 99)).unwrap();
                        }
                        if fault == 3 && snapshots == 2 {
                            let PanelNode::Panel {
                                primary_presentation,
                                ..
                            } = &mut snapshot.terminal_contexts[0].tabs[0].root
                            else {
                                unreachable!()
                            };
                            primary_presentation.binding = Some(PresentationBinding {
                                worker_id: WorkerId::parse(&format!("{:032x}", 7)).unwrap(),
                                terminal_id: TerminalId::parse(&format!("{:032x}", 8)).unwrap(),
                            });
                        }
                        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
                        })
                    }
                    WorkbenchRequest::PaneGraphics(request) => {
                        let outcome = match request.action {
                            Action::Catalog { target } => {
                                assert_eq!(target, source.target);
                                catalogs += 1;
                                if fault == 2 && catalogs == 2 {
                                    last = true;
                                }
                                Outcome::Catalog {
                                    target,
                                    revision: WorkbenchRevision::new(
                                        if (fault == 2 || fault == 6) && catalogs == 2 {
                                            2
                                        } else {
                                            1
                                        },
                                    ),
                                    layers: vec![if fault == 2 && catalogs == 2 {
                                        PaneGraphicsLayer {
                                            resource_revision: WorkbenchRevision::new(2),
                                            ..metadata.clone()
                                        }
                                    } else {
                                        metadata.clone()
                                    }],
                                }
                            }
                            Action::Pixels {
                                target,
                                layer_id,
                                resource_revision,
                                offset,
                                maximum_bytes,
                            } => {
                                assert_eq!(target, source.target);
                                assert_eq!(resource_revision, metadata.resource_revision);
                                assert_eq!(maximum_bytes as usize, MAXIMUM_GRAPHICS_CHUNK_BYTES);
                                assert_eq!(offset as usize, chunks * MAXIMUM_GRAPHICS_CHUNK_BYTES);
                                chunks += 1;
                                let total =
                                    graphics_rgba_bytes(metadata.width, metadata.height).unwrap();
                                let count =
                                    MAXIMUM_GRAPHICS_CHUNK_BYTES.min(total - offset as usize);
                                if fault == 1 {
                                    last = true;
                                }
                                Outcome::Pixels {
                                    target,
                                    layer_id,
                                    resource_revision,
                                    width: metadata.width,
                                    height: metadata.height,
                                    offset: if fault == 1 { offset + 4 } else { offset },
                                    total_bytes: total as u32,
                                    data_base64: base64::engine::general_purpose::STANDARD
                                        .encode(vec![7; count]),
                                    complete: offset as usize + count == total,
                                }
                            }
                            _ => panic!(
                                "read lane cannot upload, observe or change terminal ownership"
                            ),
                        };
                        WorkbenchResponse::PaneGraphics(PaneGraphicsResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            site_id: request.site_id,
                            outcome,
                        })
                    }
                    _ => panic!("only canonical and graphics reads"),
                };
                write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
                if last {
                    break;
                }
            }
            if fault == 0 {
                assert_eq!(chunks, 2);
            }
            if fault == 5 {
                assert_eq!(chunks, 0);
            }
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let origin = client.origin();
        let clock = client.operation_clock();
        drop(client);
        let (snapshot, source, metadata) = fixture();
        let old = Layer::new(
            metadata.clone(),
            vec![7; graphics_rgba_bytes(metadata.width, metadata.height).unwrap()],
        )
        .unwrap();
        let mut previous = BTreeMap::new();
        if fault == 5 {
            previous.insert(
                source.target.panel_id.clone(),
                Canvas::new(source.clone(), WorkbenchRevision::new(1), vec![old.clone()]).unwrap(),
            );
        }
        let mut lane = AgentReadLane::spawn(origin, clock.clone()).unwrap();
        assert!(lane.send(LaneCommand::Graphics {
            epoch: 7,
            deadline: clock.now() + Duration::from_secs(5),
            job: Job {
                scope: crate::terminal_session::pane_graphics::Scope {
                    site: snapshot.site_id,
                    revision: snapshot.revision,
                    sources: vec![source.clone()]
                },
                previous
            }
        }));
        let LaneOutcome::Graphics { epoch, result } =
            lane.outcomes.recv_timeout(Duration::from_secs(4)).unwrap()
        else {
            panic!("graphics completion");
        };
        assert_eq!(epoch, 7);
        if fault == 0 || fault == 5 || fault == 6 {
            let round = result.unwrap();
            let layer = &round.rows[&source.target.panel_id].layers()[0];
            assert_eq!(layer.pixels(), old.pixels());
            if fault == 5 {
                assert_eq!(layer.pixels().as_ptr(), old.pixels().as_ptr());
            }
        } else {
            assert!(result.is_err());
        }
        lane.interrupt();
        lane.close();
        server.finish().unwrap();
    }
}
