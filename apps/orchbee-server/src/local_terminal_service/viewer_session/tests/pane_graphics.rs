//! Production Workbench connection and observer ownership around visual transactions.
use super::fixture::{Effect, Fixture, read_snapshot};
use crate::workbench_service::{
    WorkbenchClientSession, WorkbenchExecution, WorkbenchPolicy, WorkbenchService,
};
use orchbee_workbench_protocol::{
    PaneGraphicsAction as Action, PaneGraphicsFailure as Failure, PaneGraphicsFormat,
    PaneGraphicsFrame, PaneGraphicsOutcome as Outcome, PaneGraphicsPlacement, PaneGraphicsRequest,
    PaneGraphicsStreamImage, PaneGraphicsTarget, ProtocolVersion, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision,
};
use std::{num::NonZeroU64, sync::Arc};
fn id(n: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn target(fixture: &Fixture) -> PaneGraphicsTarget {
    let snapshot = read_snapshot(&fixture.service);
    let orchbee_workbench_protocol::PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!("Panel fixture");
    };
    PaneGraphicsTarget {
        panel_id: panel_id.clone(),
        presentation_id: Some(primary_presentation.presentation_id.clone()),
    }
}
fn service(fixture: &Fixture, write: bool) -> WorkbenchService {
    WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: write,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::clone(&fixture.service),
    )
}
fn execute(session: &WorkbenchClientSession<'_>, fixture: &Fixture, action: Action) -> Outcome {
    let request = PaneGraphicsRequest {
        protocol_version: ProtocolVersion,
        request_id: id(10000),
        site_id: read_snapshot(&fixture.service).site_id,
        action,
    };
    let WorkbenchExecution::Response(WorkbenchResponse::PaneGraphics(result)) =
        session.execute(WorkbenchRequest::PaneGraphics(request.clone()))
    else {
        panic!("graphics result");
    };
    assert!(result.answers(&request));
    result.outcome
}
fn frame() -> PaneGraphicsFrame {
    PaneGraphicsFrame {
        format: PaneGraphicsFormat::Rgba,
        width: 1,
        height: 1,
        data_base64: "AQIDBA==".into(),
        placement: PaneGraphicsPlacement::default(),
    }
}
fn synchronize(
    session: &WorkbenchClientSession<'_>,
    fixture: &Fixture,
) -> orchbee_workbench_protocol::ClientViewRecord {
    use orchbee_workbench_protocol::*;
    let snapshot = read_snapshot(&fixture.service);
    let selected = target(fixture);
    let request = WorkbenchClientViewRequest {
        protocol_version: ProtocolVersion,
        request_id: id(11000),
        site_id: snapshot.site_id,
        action: ClientViewAction::Synchronize {
            heartbeat: ClientViewSequence::new(1).unwrap(),
            view_revision: ClientViewSequence::new(1).unwrap(),
            completion: None,
            view: Box::new(ClientViewObservation {
                workbench_revision: snapshot.revision,
                split_revision: None,
                phase: ClientViewPhase::Ready,
                selection: Some(ClientViewSelection {
                    terminal_context_id: snapshot.terminal_contexts[0].terminal_context_id.clone(),
                    tab_id: Some(snapshot.terminal_contexts[0].tabs[0].tab_id.clone()),
                    panel_id: Some(selected.panel_id),
                    zoomed: false,
                    panel_count: 1,
                }),
            }),
        },
    };
    let WorkbenchExecution::Response(WorkbenchResponse::ClientView(result)) =
        session.execute(WorkbenchRequest::ClientView(request))
    else {
        panic!("synchronized view");
    };
    let ClientViewOutcome::Synchronized { client, .. } = result.outcome else {
        panic!("registered Client");
    };
    *client
}

#[test]
fn graphics_facts_require_the_publishing_connection_and_an_actual_matching_draw_revision() {
    use orchbee_workbench_protocol::{
        PaneGraphicsCapabilities, PaneGraphicsClientObservation, PaneGraphicsClientPane,
        PaneGraphicsTransport,
    };
    let fixture = Fixture::new();
    let selected = target(&fixture);
    let service = service(&fixture, true);
    let writer = service.client_session(NonZeroU64::MIN);
    execute(&writer, &fixture, set(&selected));
    let observer = service.client_session(NonZeroU64::new(2).unwrap());
    let client = synchronize(&observer, &fixture);
    let source = fixture
        .service
        .graphics_source(&read_snapshot(&fixture.service).site_id, &selected)
        .unwrap();
    let observation = PaneGraphicsClientObservation {
        workbench_revision: client.view.workbench_revision,
        capabilities: PaneGraphicsCapabilities {
            transport: PaneGraphicsTransport::CellRaster,
            cell_pixels: None,
            pixel_mouse: false,
        },
        complete: true,
        panes: vec![PaneGraphicsClientPane {
            target: selected.clone(),
            binding: source.binding,
            drawn_revision: Some(WorkbenchRevision::new(1)),
        }],
    };
    let observe = |observation| Action::Observe {
        client_id: client.client_id.clone(),
        heartbeat: client.heartbeat,
        observation: Box::new(observation),
    };
    let stranger = service.client_session(NonZeroU64::new(3).unwrap());
    assert_eq!(
        execute(&stranger, &fixture, observe(observation.clone())),
        Outcome::Failed {
            reason: Failure::Denied
        }
    );
    let mut ahead = observation.clone();
    ahead.panes[0].drawn_revision = Some(WorkbenchRevision::new(99));
    assert_eq!(
        execute(&observer, &fixture, observe(ahead)),
        Outcome::Failed {
            reason: Failure::SourceChanged
        }
    );
    assert!(matches!(
        execute(&observer, &fixture, observe(observation)),
        Outcome::Observed { .. }
    ));
    let info = || {
        execute(
            &writer,
            &fixture,
            Action::Info {
                target: selected.clone(),
                client_id: None,
            },
        )
    };
    assert!(
        matches!(info(), Outcome::Info { pane_visible: Some(true), drawn_revision: Some(revision), capabilities: Some(caps), .. }
        if revision == WorkbenchRevision::new(1) && caps.transport == PaneGraphicsTransport::CellRaster && caps.cell_pixels.is_none() && !caps.pixel_mouse)
    );
    let mut image = frame();
    image.data_base64 = "BQYHCA==".into();
    execute(
        &writer,
        &fixture,
        Action::Set {
            target: selected.clone(),
            layer_id: "static".into(),
            z_index: 0,
            image,
        },
    );
    assert!(
        matches!(info(), Outcome::Info { revision, drawn_revision: Some(drawn), .. } if revision == WorkbenchRevision::new(2) && drawn == WorkbenchRevision::new(1))
    );
    synchronize(&writer, &fixture);
    assert!(matches!(
        info(),
        Outcome::Info {
            client_id: None,
            capabilities: None,
            pane_visible: None,
            drawn_revision: None,
            ..
        }
    ));
    assert!(matches!(
        execute(
            &writer,
            &fixture,
            Action::Info {
                target: selected.clone(),
                client_id: Some(client.client_id.clone())
            }
        ),
        Outcome::Info {
            pane_visible: Some(true),
            ..
        }
    ));
    drop(observer);
    assert_eq!(
        execute(
            &writer,
            &fixture,
            Action::Info {
                target: selected,
                client_id: Some(client.client_id)
            }
        ),
        Outcome::Failed {
            reason: Failure::NotFound
        }
    );
    assert!(fixture.barrier().unwrap().iter().all(|effect| !matches!(
        effect,
        Effect::Attach(..) | Effect::Input(..) | Effect::Resize(..)
    )));
}
fn set(target: &PaneGraphicsTarget) -> Action {
    Action::Set {
        target: target.clone(),
        layer_id: "static".into(),
        z_index: 0,
        image: frame(),
    }
}
#[test]
fn pane_graphics_actual_session_grants_receipts_and_eof_preserve_other_layers() {
    let fixture = Fixture::new();
    let target = target(&fixture);
    let readonly = service(&fixture, false);
    let reader = readonly.client_session(NonZeroU64::new(2).unwrap());
    assert_eq!(
        execute(&reader, &fixture, set(&target)),
        Outcome::Failed {
            reason: Failure::Denied
        }
    );
    let service = service(&fixture, true);
    let writer = service.client_session(NonZeroU64::MIN);
    assert!(matches!(
        execute(&writer, &fixture, set(&target)),
        Outcome::Applied { changed: true, .. }
    ));
    assert!(matches!(
        execute(
            &writer,
            &fixture,
            Action::StreamOpen {
                stream_id: id(10001),
                target: target.clone(),
                layer_id: "live".into(),
                z_index: 1
            }
        ),
        Outcome::StreamOpened { .. }
    ));
    assert_eq!(
        execute(
            &reader,
            &fixture,
            Action::StreamClose {
                stream_id: id(10001)
            }
        ),
        Outcome::Failed {
            reason: Failure::Denied
        }
    );
    assert!(
        matches!(execute(&writer,&fixture,Action::StreamFrame{stream_id:id(10001),sequence:WorkbenchRevision::new(1),revision:WorkbenchRevision::new(u64::MAX),image:PaneGraphicsStreamImage::Inline{image:frame()},damage:vec![]}),Outcome::FrameApplied{frame_revision,..} if frame_revision.value()==u64::MAX)
    );
    drop(writer);
    let Outcome::Catalog { layers, .. } = execute(
        &reader,
        &fixture,
        Action::Catalog {
            target: target.clone(),
        },
    ) else {
        panic!("catalog");
    };
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].layer_id, "static");
    assert!(fixture.barrier().unwrap().iter().all(|effect| !matches!(
        effect,
        Effect::Attach(..) | Effect::Input(..) | Effect::Resize(..)
    )));
}
#[test]
fn pane_graphics_png_source_refusal_settles_one_temporary_viewer_and_keeps_old_resource() {
    let fixture = Fixture::new();
    let target = target(&fixture);
    let service = service(&fixture, true);
    let writer = service.client_session(NonZeroU64::MIN);
    execute(&writer, &fixture, set(&target));
    let before = execute(
        &writer,
        &fixture,
        Action::Catalog {
            target: target.clone(),
        },
    );
    let mut image = frame();
    image.format = PaneGraphicsFormat::Png;
    image.data_base64="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".into();
    assert_eq!(
        execute(
            &writer,
            &fixture,
            Action::Set {
                target: target.clone(),
                layer_id: "static".into(),
                z_index: 0,
                image
            }
        ),
        Outcome::Failed {
            reason: Failure::SourceUnavailable
        }
    );
    assert_eq!(
        execute(&writer, &fixture, Action::Catalog { target }),
        before
    );
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::GraphicsDecode(..)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Detach(..)))
            .count(),
        1
    );
    assert!(effects.iter().all(|e| !matches!(
        e,
        Effect::Input(..) | Effect::Resize(..) | Effect::Clear(..)
    )));
}
