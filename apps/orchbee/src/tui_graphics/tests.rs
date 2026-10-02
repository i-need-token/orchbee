use super::*;
use crate::terminal_graphics::{Probe, Reply};
use orchbee_workbench_protocol::{
    PaneGraphicsLayer, PaneGraphicsPlacement, PaneGraphicsTarget, TerminalPresentationId,
    WorkbenchPanelId,
};

fn canvas(
    revision: u64,
    placement: PaneGraphicsPlacement,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> PaneGraphicsCanvas {
    PaneGraphicsCanvas::fixture(
        PaneGraphicsTarget {
            panel_id: WorkbenchPanelId::parse("22222222222222222222222222222222").unwrap(),
            presentation_id: Some(
                TerminalPresentationId::parse("33333333333333333333333333333333").unwrap(),
            ),
        },
        WorkbenchRevision::new(revision),
        vec![(
            PaneGraphicsLayer {
                layer_id: "primary".into(),
                z_index: 0,
                width,
                height,
                placement,
                resource_revision: WorkbenchRevision::new(revision),
                stream_id: None,
            },
            rgba,
        )],
    )
}
fn facts(now: Instant, pixels: (u16, u16)) -> Facts {
    let mut probe = Probe::default();
    probe.activate(99);
    probe.query((80, 24), now).unwrap();
    probe.reply(Reply::Cell(Some(pixels)), now);
    probe.reply(
        Reply::Kitty {
            image_id: 99,
            placement_id: None,
            success: true,
        },
        now,
    );
    probe.facts()
}
fn stage(surface: &Surface, canvas: &PaneGraphicsCanvas, body: Rect) -> Option<WorkbenchRevision> {
    surface.begin_frame(false);
    let choice = surface.select(
        &SiteId::parse("11111111111111111111111111111111").unwrap(),
        canvas,
        body,
    );
    surface.finish_frame(&[canvas.draw_for_test()], false);
    choice.and_then(|choice| choice.revision)
}
fn flush(surface: &Surface) -> String {
    let mut bytes = Vec::new();
    let deleted = surface
        .write_frame(&mut bytes, Position { x: 7, y: 4 })
        .unwrap();
    surface.flushed(&deleted);
    String::from_utf8(bytes).unwrap()
}
fn reply(id: u32, placement: Option<u32>, success: bool) -> Reply {
    Reply::Kitty {
        image_id: id,
        placement_id: placement,
        success,
    }
}

#[test]
fn upload_and_placement_have_distinct_scoped_receipts_and_preserve_rgba() {
    let now = Instant::now();
    let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
    let host = facts(now, (1, 2));
    surface.update(host, now, Vec::new(), false);
    let canvas = canvas(
        1,
        PaneGraphicsPlacement {
            grid_columns: 1,
            grid_rows: 1,
            ..Default::default()
        },
        1,
        2,
        vec![255, 0, 0, 128, 0, 0, 255, 0],
    );
    let body = Rect::new(5, 3, 3, 2);
    assert_eq!(stage(&surface, &canvas, body), None);
    let upload = flush(&surface);
    assert!(upload.contains("a=t,t=d,f=32,s=1,v=2,i=1,q=0,m=0;/wAAgAAA/wA="));
    assert!(!upload.contains("a=p"));
    surface.update(
        host,
        now,
        vec![reply(2, None, true), reply(1, Some(1), true)],
        false,
    );
    assert_eq!(stage(&surface, &canvas, body), None);
    assert_eq!(flush(&surface), "");
    surface.update(host, now, vec![reply(1, None, true)], false);
    assert_eq!(stage(&surface, &canvas, body), None);
    let placement = flush(&surface);
    assert!(placement.contains("\x1b[4;6H\x1b_Ga=p,i=1,p=1,c=1,r=1,z=0,C=1,q=0;\x1b\\\x1b[5;8H"));
    surface.update(host, now, vec![reply(1, Some(2), true)], false);
    assert_eq!(stage(&surface, &canvas, body), None);
    surface.update(host, now, vec![reply(1, Some(1), true)], false);
    assert_eq!(
        stage(&surface, &canvas, body),
        Some(WorkbenchRevision::new(1))
    );
    assert_eq!(flush(&surface), "");
    surface.begin_frame(true);
    assert!(
        surface
            .select(
                &SiteId::parse("11111111111111111111111111111111").unwrap(),
                &canvas,
                body
            )
            .is_none()
    );
    surface.finish_frame(&[], true);
    assert!(flush(&surface).contains("a=d,d=I,i=1,q=2"));
}

#[test]
fn chunk_budget_freezes_the_whole_candidate_under_an_advancing_producer() {
    let now = Instant::now();
    let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
    let host = facts(now, (8, 16));
    surface.update(host, now, Vec::new(), false);
    let placement = PaneGraphicsPlacement {
        grid_columns: 20,
        grid_rows: 15,
        ..Default::default()
    };
    let first = canvas(1, placement, 1, 1, vec![1, 2, 3, 4]);
    let newer = canvas(2, placement, 1, 1, vec![5, 6, 7, 8]);
    let body = Rect::new(5, 3, 20, 15);
    stage(&surface, &first, body);
    let mut output = flush(&surface);
    assert_eq!(output.matches("\x1b_G").count(), CHUNKS_PER_FRAME);
    assert!(output.contains("m=1"));
    for _ in 0..2 {
        if !output.contains("m=0") {
            assert!(
                surface.update(host, now, Vec::new(), false),
                "an idle input cadence must continue a partial upload"
            );
        } else {
            surface.update(host, now, Vec::new(), false);
        }
        stage(&surface, &newer, body);
        let chunk = flush(&surface);
        assert!(
            !chunk.contains("a=t"),
            "producer revision must not restart a frozen upload"
        );
        assert!(chunk.matches("\x1b_G").count() <= CHUNKS_PER_FRAME);
        output.push_str(&chunk);
    }
    assert!(output.contains("m=0"));
    let decoded = output
        .split("\x1b_G")
        .skip(1)
        .flat_map(|chunk| {
            let payload = chunk
                .split_once(';')
                .unwrap()
                .1
                .split_once("\x1b\\")
                .unwrap()
                .0;
            STANDARD.decode(payload).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(decoded.len(), 20 * 8 * 15 * 16 * 4);
    assert!(decoded.chunks_exact(4).all(|pixel| pixel == [1, 2, 3, 4]));
    surface.update(host, now, vec![reply(1, None, true)], false);
    stage(&surface, &newer, body);
    flush(&surface);
    surface.update(host, now, vec![reply(1, Some(1), true)], false);
    assert_eq!(
        stage(&surface, &newer, body),
        Some(WorkbenchRevision::new(1))
    );
}

#[test]
fn wide_virtual_grid_clips_before_sampling_and_measurement_changes_retire() {
    let now = Instant::now();
    let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
    surface.update(facts(now, (1, 2)), now, Vec::new(), false);
    let canvas = canvas(
        1,
        PaneGraphicsPlacement {
            viewport_column: i32::MIN,
            grid_columns: u32::MAX,
            grid_rows: 1,
            ..Default::default()
        },
        2,
        1,
        vec![255, 0, 0, 255, 0, 0, 255, 255],
    );
    let body = Rect::new(0, 0, 2, 1);
    stage(&surface, &canvas, body);
    let scene = surface.0.borrow().candidate.as_ref().unwrap().scene.clone();
    let mut bytes = [0; 16];
    scene.images[0].fill(&scene.canvas, 0, &mut bytes);
    assert!(bytes.chunks_exact(4).all(|pixel| pixel == [0, 0, 255, 255]));
    surface.update(facts(now, (2, 2)), now, Vec::new(), false);
    stage(&surface, &canvas, body);
    let output = flush(&surface);
    assert!(output.contains("a=d,d=I,i=1"));
    assert!(output.contains("s=4,v=2,i=2"));
}

#[test]
fn missing_late_failed_and_overflowed_replies_select_session_fallback() {
    for mode in 0..4 {
        let now = Instant::now();
        let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
        let host = facts(now, (1, 2));
        surface.update(host, now, Vec::new(), false);
        let canvas = canvas(1, PaneGraphicsPlacement::default(), 1, 1, vec![255; 4]);
        let body = Rect::new(0, 0, 1, 1);
        stage(&surface, &canvas, body);
        flush(&surface);
        match mode {
            0 => {
                surface.update(host, now + REPLY_DEADLINE, Vec::new(), false);
            }
            1 => {
                surface.update(
                    host,
                    now + REPLY_DEADLINE,
                    vec![reply(1, None, true)],
                    false,
                );
            }
            2 => {
                surface.update(host, now, vec![reply(1, None, false)], false);
            }
            _ => {
                surface.update(host, now, Vec::new(), true);
            }
        }
        surface.begin_frame(false);
        assert!(
            surface
                .select(
                    &SiteId::parse("11111111111111111111111111111111").unwrap(),
                    &canvas,
                    body
                )
                .is_none()
        );
        surface.finish_frame(&[canvas.draw_for_test()], false);
        let output = flush(&surface);
        assert!(output.contains("a=d,d=I,i=1"));
        assert!(!output.contains("a=t"));
    }
}

#[test]
fn equivalent_site_revision_reuses_pixels_but_a_new_site_cannot_reuse_the_same_ids() {
    let now = Instant::now();
    let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
    let host = facts(now, (1, 2));
    surface.update(host, now, Vec::new(), false);
    let canvas = canvas(1, PaneGraphicsPlacement::default(), 1, 1, vec![255; 4]);
    let body = Rect::new(0, 0, 1, 1);
    stage(&surface, &canvas, body);
    // Allocated, unissued IDs cannot accept either kind of acknowledgement.
    surface.update(
        host,
        now,
        vec![reply(1, None, false), reply(1, None, true)],
        false,
    );
    assert!(!surface.0.borrow().disabled);
    flush(&surface);
    surface.update(host, now, vec![reply(1, None, true)], false);
    stage(&surface, &canvas, body);
    flush(&surface);
    surface.update(host, now, vec![reply(1, Some(1), true)], false);
    let revised = canvas.revised_for_test(WorkbenchRevision::new(100));
    assert_eq!(
        stage(&surface, &revised, body),
        Some(WorkbenchRevision::new(100))
    );
    assert_eq!(flush(&surface), "");
    assert!(surface.0.borrow().candidate.is_none());
    surface.begin_frame(false);
    assert!(
        surface
            .select(
                &SiteId::parse("44444444444444444444444444444444").unwrap(),
                &revised,
                body
            )
            .is_none()
    );
    surface.finish_frame(&[revised.draw_for_test()], false);
    let output = flush(&surface);
    assert!(output.contains("a=d,d=I,i=1"));
    assert!(output.contains("i=2"));
    surface.update(host, now, vec![reply(1, Some(1), true)], false);
    assert!(surface.0.borrow().active.is_empty());
}

#[test]
fn maximum_derived_axes_and_cell_pixels_do_not_overflow_sampling() {
    let placement = PaneGraphicsPlacement {
        grid_rows: u32::MAX,
        ..Default::default()
    };
    let canvas = canvas(1, placement, 4096, 1, [1, 2, 3, 4].repeat(4096));
    let scene = geometry::Scene::new(
        &SiteId::parse("11111111111111111111111111111111").unwrap(),
        &canvas,
        Rect::new(0, 0, 1, 1),
        orchbee_workbench_protocol::PaneGraphicsCellPixels {
            width: 1,
            height: u16::MAX,
        },
    )
    .unwrap();
    let mut pixel = [0; 4];
    scene.images[0].fill(&canvas, 0, &mut pixel);
    assert_eq!(pixel, [1, 2, 3, 4]);
    // Exercise the complementary omitted axis with maximum physical columns.
    let canvas = self::canvas(
        2,
        PaneGraphicsPlacement {
            grid_columns: u32::MAX,
            ..Default::default()
        },
        1,
        4096,
        [5, 6, 7, 8].repeat(4096),
    );
    let scene = geometry::Scene::new(
        &SiteId::parse("11111111111111111111111111111111").unwrap(),
        &canvas,
        Rect::new(0, 0, 1, 1),
        orchbee_workbench_protocol::PaneGraphicsCellPixels {
            width: u16::MAX,
            height: 1,
        },
    )
    .unwrap();
    scene.images[0].fill(&canvas, 0, &mut pixel);
    assert_eq!(pixel, [5, 6, 7, 8]);
}

#[test]
fn all_layers_upload_and_place_before_a_canvas_can_acknowledge_its_revision() {
    let now = Instant::now();
    let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
    let host = facts(now, (1, 2));
    surface.update(host, now, Vec::new(), false);
    let base = canvas(1, PaneGraphicsPlacement::default(), 1, 1, vec![1, 2, 3, 4]);
    let mut back = base.layers()[0].metadata().clone();
    back.layer_id = "back".into();
    back.z_index = -1;
    let mut front = back.clone();
    front.layer_id = "front".into();
    front.z_index = 2;
    let canvas = PaneGraphicsCanvas::fixture(
        PaneGraphicsTarget {
            panel_id: WorkbenchPanelId::parse("22222222222222222222222222222222").unwrap(),
            presentation_id: Some(
                TerminalPresentationId::parse("33333333333333333333333333333333").unwrap(),
            ),
        },
        WorkbenchRevision::new(1),
        vec![(back, vec![1, 2, 3, 4]), (front, vec![5, 6, 7, 8])],
    );
    let body = Rect::new(0, 0, 1, 1);
    stage(&surface, &canvas, body);
    assert_eq!(flush(&surface).matches("a=t").count(), 2);
    surface.update(host, now, vec![reply(1, None, true)], false);
    assert!(surface.0.borrow().active.is_empty());
    assert!(surface.0.borrow().candidate.is_some());
    surface.update(host, now, vec![reply(2, None, true)], false);
    assert_eq!(stage(&surface, &canvas, body), None);
    let output = flush(&surface);
    assert_eq!(output.matches("a=p").count(), 2);
    assert!(output.contains("z=-1"));
    assert!(output.contains("z=2"));
    surface.update(host, now, vec![reply(1, Some(1), true)], false);
    assert_eq!(stage(&surface, &canvas, body), None);
    surface.update(host, now, vec![reply(2, Some(1), true)], false);
    assert_eq!(
        stage(&surface, &canvas, body),
        Some(WorkbenchRevision::new(1))
    );
}
