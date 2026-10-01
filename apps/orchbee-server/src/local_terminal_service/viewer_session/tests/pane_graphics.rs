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
