//! Actual renderer/source preparation and the shared local acknowledgement for divider releases.
use super::*;
use crate::{
    client_view::{ClientView, Notice, PanelDirection},
    localization::Locale,
    terminal_session::TerminalSessionView,
    tui_layout::{DividerDrag, DrawnDivider},
    tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget},
};
use orchbee_workbench_protocol::{
    ClientLayoutChange, ClientSplitRatio, PanelNode, WorkbenchAction, WorkbenchRevision,
    WorkbenchSnapshotOutcome,
};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn divider_unavailable_controller_is_failed_instead_of_perpetually_busy() {
    let (mut native, _, clock, _) = ready_split_ui();
    let view = ClientView::new(120);
    let target = painted(&native, &view, 120, 40).1.remove(0);
    let mut control = controller(clock);
    control.observe_geometry(target.viewport, view.sidebar_surface());
    control.tick(&mut native, false);
    assert!(control.divider_ready(&native, &target).is_ok());
    control.close();
    assert_eq!(
        control.divider_ready(&native, &target),
        Err(Notice::ResizeFailed)
    );
}

#[test]
fn divider_busy_refusal_supersedes_submission_clear_in_the_same_input_batch() {
    let (mut native, _, clock, _) = ready_split_ui();
    let view = ClientView::new(120);
    let target = painted(&native, &view, 120, 40).1.remove(0);
    let mut control = controller(clock);
    control.observe_geometry(target.viewport, view.sidebar_surface());
    control.tick(&mut native, false);
    control.resize_divider(&native, &target, ClientSplitRatio::new(0.7).unwrap());
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_some()
    );
    assert_eq!(
        control.divider_ready(&native, &target),
        Err(Notice::ResizeBusy)
    );
    control.tick(&mut native, false);
    assert!(matches!(
        control.take_local_resize_feedback(),
        Some(LocalResizeFeedback::Notice(Notice::ResizeBusy))
    ));
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_none()
    );
}

fn painted(
    native: &NativeUi,
    view: &ClientView,
    width: u16,
    height: u16,
) -> (FrameHitMap, Vec<DrawnDivider>) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                view,
                Locale::EnUs,
                Some(TerminalSessionView::Native(native.view())),
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    let map = hits.finish();
    let mut targets = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if let Some(HitTarget::Divider(target)) = map.hit_target_at(x, y)
                && !targets.contains(target)
            {
                targets.push(target.clone());
            }
        }
    }
    (map, targets)
}

#[test]
fn divider_native_preparation_pins_every_painted_identity_and_geometry_without_effects() {
    let (native, mailbox, _, _) = ready_split_ui();
    let poll = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(poll).unwrap();
    let view = ClientView::new(120);
    let target = painted(&native, &view, 120, 40).1.remove(0);
    let viewport = Some((target.viewport, view.sidebar_surface()));
    let before = native.control_observation(false).unwrap();
    let (_, adjustment, observed) = native
        .prepare_divider_adjustment(&target, ClientSplitRatio::new(0.7).unwrap(), viewport)
        .unwrap();
    assert_eq!(observed, before.1);
    let ClientLayoutChange::Split { split } = adjustment.change else {
        panic!()
    };
    assert_eq!(
        (
            split.split_id,
            split.first_slot_id,
            split.second_slot_id,
            split.previous_ratio,
            split.ratio
        ),
        (
            target.split.clone(),
            target.first.clone(),
            target.second.clone(),
            target.ratio,
            ClientSplitRatio::new(0.7).unwrap()
        )
    );
    for dimension in 0..13 {
        let mut stale = target.clone();
        let other = &format!("{:032x}", 9999);
        match dimension {
            0 => stale.source.site = orchbee_workbench_protocol::SiteId::parse(other).unwrap(),
            1 => {
                stale.source.context =
                    orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(other).unwrap()
            }
            2 => {
                stale.source.tab = orchbee_workbench_protocol::WorkbenchTabId::parse(other).unwrap()
            }
            3 => {
                stale.source.focus =
                    orchbee_workbench_protocol::WorkbenchPanelId::parse(other).unwrap()
            }
            4 => stale.source.revision = WorkbenchRevision::new(2),
            5 => stale.split = orchbee_workbench_protocol::WorkbenchSplitId::parse(other).unwrap(),
            6 => std::mem::swap(&mut stale.first, &mut stale.second),
            7 => stale.ratio = ClientSplitRatio::new(0.51).unwrap(),
            8 => stale.area.width += 1,
            9 => stale.divider.x += 1,
            10 => stale.viewport.width += 1,
            11 => stale.orientation = orchbee_workbench_protocol::PanelOrientation::Vertical,
            _ => {
                stale.first =
                    orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(other).unwrap()
            }
        }
        assert!(
            native
                .prepare_divider_adjustment(&stale, ClientSplitRatio::new(0.7).unwrap(), viewport)
                .is_err(),
            "dimension {dimension}"
        );
    }
    assert_eq!(native.control_observation(false).unwrap(), before);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn divider_hit_map_and_preview_follow_visible_geometry_locales_and_overlay_cancellation() {
    let (mut native, _, _, _) = ready_split_ui();
    for (width, height) in [(120, 40), (80, 24), (40, 10), (10, 3)] {
        let view = ClientView::new(width);
        let (_, targets) = painted(&native, &view, width, height);
        if width < 60 {
            assert!(targets.is_empty());
            continue;
        }
        assert_eq!(targets.len(), 1);
        let target = targets[0].clone();
        let mut drag =
            DividerDrag::begin(target.clone(), target.divider.x, target.divider.y).unwrap();
        drag.update(target.divider.x + 5, target.divider.y);
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(TerminalSessionView::Native(native.view())),
                        &crate::tui_theme::Theme::terminal_native(),
                        &mut hits,
                    );
                    assert!(crate::tui_render::render_divider_preview(
                        frame,
                        &view,
                        locale,
                        &drag,
                        &crate::tui_theme::Theme::terminal_native(),
                        &hits
                    ));
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .replace(' ', "");
            assert!(text.contains("Esc"));
            assert!(text.contains(if locale == Locale::EnUs {
                "Divider"
            } else {
                "分隔线"
            }));
            assert_eq!(
                terminal.backend().buffer()[(drag.preview.x, drag.preview.y)].symbol(),
                "│"
            );
            assert_eq!(
                hits.finish()
                    .hit_target_at(target.divider.x, target.divider.y),
                Some(&HitTarget::Divider(target.clone()))
            );
        }
    }
    let view = ClientView::new(120);
    native.apply_client_action(ClientAction::ToggleZoom);
    assert!(painted(&native, &view, 120, 40).1.is_empty());
    native.apply_client_action(ClientAction::ToggleZoom);
    let mut view = ClientView::new(120);
    let transition = view.decide(crate::client_view::PrefixInput::Key(
        crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Char('b'),
            crate::client_view::Modifiers::CTRL,
        ),
    ));
    view.commit(
        transition,
        crate::client_view::SessionActionDisposition::KeepNavigator,
    );
    let transition = view.decide(crate::client_view::PrefixInput::Key(
        crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Char('?'),
            crate::client_view::Modifiers::NONE,
        ),
    ));
    view.commit(
        transition,
        crate::client_view::SessionActionDisposition::KeepNavigator,
    );
    assert!(painted(&native, &view, 120, 40).1.is_empty());
}

#[test]
fn divider_preview_emits_over_wide_glyphs_and_restores_visible_underlay() {
    use crate::{
        terminal_text::display_width,
        tui_layout::divide_panes,
        tui_pointer::HitLayer,
        tui_theme::{Role, Theme},
    };
    use orchbee_workbench_protocol::PanelOrientation;
    use ratatui::{
        TerminalOptions, Viewport,
        buffer::{Buffer, CellDiffOption},
        layout::{Margin, Rect},
        style::Modifier,
    };

    let (native, _, _, _) = ready_split_ui();
    let view = ClientView::new(80);
    let source = painted(&native, &view, 80, 24).1.remove(0);
    let viewport = Rect::new(3, 2, 80, 24);
    for theme in [
        Theme::terminal_native(),
        Theme::one_dark(),
        Theme::one_light(),
    ] {
        for orientation in [PanelOrientation::Horizontal, PanelOrientation::Vertical] {
            // Isolate composition with a valid inset split, leaving intact glyphs on both sides.
            let mut target = source.clone();
            target.viewport = viewport;
            target.area = viewport.inner(Margin::new(4, 3));
            target.orientation = orientation;
            target.divider = divide_panes(target.area, orientation, target.ratio)
                .unwrap()
                .divider;
            let mut drag =
                DividerDrag::begin(target.clone(), target.divider.x, target.divider.y).unwrap();
            match orientation {
                PanelOrientation::Horizontal => {
                    drag.update(target.divider.x + 5, target.divider.y);
                }
                PanelOrientation::Vertical => {
                    drag.update(target.divider.x, target.divider.y + 4);
                }
            }
            let line = drag.preview;
            let expected = match orientation {
                PanelOrientation::Horizontal => "│",
                PanelOrientation::Vertical => "─",
            };
            for (glyph, right_edge, offset, skip) in [
                ("中", false, -1_i16, true),
                ("🙂", false, -1, true),
                ("中", true, -1, true),
                ("🙂", true, -1, true),
                ("中", false, -2, false),
                ("🙂", true, 0, false),
                ("X", false, 0, true),
            ] {
                let case = format!(
                    "{} {orientation:?} {glyph} {right_edge} {offset}",
                    theme.name
                );
                let edge = if right_edge { line.right() } else { line.x };
                let glyph_x = u16::try_from(i32::from(edge) + i32::from(offset)).unwrap();
                let glyph_right = glyph_x + u16::try_from(display_width(glyph)).unwrap();
                assert!(glyph_x >= viewport.x && glyph_right <= viewport.right());
                let surface = theme
                    .style(Role::SurfaceCanvas)
                    .patch(theme.style(Role::TextPrimary));
                let glyph_style = theme
                    .style(Role::SurfaceChrome)
                    .patch(theme.style(Role::AccentFocus))
                    .add_modifier(Modifier::ITALIC | Modifier::UNDERLINED);
                let mut underlay = Buffer::empty(viewport);
                for y in viewport.y..viewport.bottom() {
                    underlay.set_string(
                        viewport.x,
                        y,
                        "p".repeat(usize::from(viewport.width)),
                        surface,
                    );
                }
                underlay.set_string(glyph_x, line.y, glyph, glyph_style);
                let lead_style = underlay[(glyph_x, line.y)].style();
                let intersects = glyph_x < line.right() && glyph_right > line.x;
                let mut incoming = underlay.clone();
                if skip {
                    // The preview must reclaim both terminal tails and single-cell Skip targets.
                    incoming[(glyph_right - 1, line.y)].set_diff_option(CellDiffOption::Skip);
                }
                let mut terminal = Terminal::with_options(
                    TestBackend::new(viewport.right(), viewport.bottom()),
                    TerminalOptions {
                        viewport: Viewport::Fixed(viewport),
                    },
                )
                .unwrap();
                terminal
                    .draw(|frame| *frame.buffer_mut() = underlay.clone())
                    .unwrap();
                let mut hits = HitMapBuilder::new();
                hits.record(
                    HitLayer::Foreground,
                    target.divider,
                    HitTarget::Divider(target.clone()),
                );
                let preview = terminal
                    .draw(|frame| {
                        *frame.buffer_mut() = incoming.clone();
                        assert!(crate::tui_render::render_divider_preview(
                            frame,
                            &view,
                            Locale::ZhCn,
                            &drag,
                            &theme,
                            &hits,
                        ));
                    })
                    .unwrap()
                    .buffer
                    .clone();
                let emitted = underlay.diff(&preview);
                for y in line.y..line.bottom() {
                    for x in line.x..line.right() {
                        let cell = &preview[(x, y)];
                        assert_eq!(cell.symbol(), expected, "preview {case} ({x},{y})");
                        assert_eq!(cell.diff_option, CellDiffOption::None);
                        let original_style = if y == line.y && x >= glyph_x && x < glyph_right {
                            lead_style
                        } else {
                            underlay[(x, y)].style()
                        };
                        assert_eq!(
                            cell.style(),
                            original_style.patch(
                                theme
                                    .style(Role::AccentSelection)
                                    .add_modifier(Modifier::BOLD)
                            ),
                            "preview style {case} ({x},{y})"
                        );
                        assert!(
                            emitted.iter().any(|(cx, cy, drawn)| {
                                (*cx, *cy) == (x, y) && **drawn == *cell
                            })
                        );
                        assert_eq!(terminal.backend().buffer()[(x, y)], *cell);
                    }
                }
                // Only the preview, its hint row, and intersected glyph remnants may change.
                for y in viewport.y..viewport.bottom() - 1 {
                    for x in viewport.x..viewport.right() {
                        if line.contains((x, y).into()) {
                            continue;
                        }
                        let cell = &preview[(x, y)];
                        if y == line.y && intersects && x >= glyph_x && x < glyph_right {
                            assert_eq!(cell.symbol(), " ", "remnant {case} ({x},{y})");
                            assert_eq!(cell.style(), lead_style);
                            assert_eq!(cell.diff_option, CellDiffOption::None);
                            assert_eq!(terminal.backend().buffer()[(x, y)], *cell);
                        } else {
                            assert_eq!(*cell, underlay[(x, y)], "intact {case} ({x},{y})");
                        }
                    }
                }
                assert_eq!(
                    hits.finish()
                        .hit_target_at(target.divider.x, target.divider.y),
                    Some(&HitTarget::Divider(target.clone()))
                );
                let restored = terminal
                    .draw(|frame| *frame.buffer_mut() = underlay.clone())
                    .unwrap()
                    .buffer
                    .clone();
                let restored_diff = preview.diff(&restored);
                for y in viewport.y..viewport.bottom() {
                    let mut x = viewport.x;
                    while x < viewport.right() {
                        let cell = &underlay[(x, y)];
                        assert_eq!(
                            terminal.backend().buffer()[(x, y)],
                            *cell,
                            "restored visible glyph {case} ({x},{y})"
                        );
                        if preview[(x, y)] != *cell {
                            assert!(restored_diff.iter().any(|(cx, cy, drawn)| {
                                (*cx, *cy) == (x, y) && **drawn == *cell
                            }));
                        }
                        x += u16::try_from(display_width(cell.symbol()).max(1)).unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn divider_nested_vertical_target_uses_its_own_painted_area_and_ordered_slots() {
    use crate::terminal_session::ui::tests::publication_hint::{
        settle_queued_refresh, split_snapshot,
    };
    use orchbee_workbench_protocol::{
        PanelOrientation, PanelSlot, TerminalId, TerminalPresentationId, WorkbenchPanelId,
        WorkbenchPanelSlotId, WorkbenchSplitId, WorkbenchSplitRatio,
    };
    let (mut native, mailbox, clock, _) = ready_split_ui();
    let poll = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(poll).unwrap();
    let mut snapshot = split_snapshot(2);
    let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    let first = children[1].node.clone();
    let mut second = first.clone();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut second
    else {
        panic!()
    };
    *panel_id = WorkbenchPanelId::parse(&format!("{:032x}", 9_001)).unwrap();
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(&format!("{:032x}", 9_002)).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        TerminalId::parse(&format!("{:032x}", 9_003)).unwrap();
    children[1].node = PanelNode::Split {
        split_id: WorkbenchSplitId::parse(&format!("{:032x}", 9_004)).unwrap(),
        orientation: PanelOrientation::Vertical,
        ratio: WorkbenchSplitRatio::new(0.4).unwrap(),
        children: Box::new([
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 9_005)).unwrap(),
                node: first,
            },
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 9_006)).unwrap(),
                node: second,
            },
        ]),
    };
    assert!(matches!(
        native.refresh_layout_adjustment(&mut false),
        ControlStep::Changed
    ));
    settle_queued_refresh(
        &mut native,
        &mailbox,
        &clock,
        WorkbenchSnapshotOutcome::Available { snapshot },
    );
    let view = ClientView::new(120);
    let (_, targets) = painted(&native, &view, 120, 40);
    assert_eq!(targets.len(), 2);
    let nested = targets
        .iter()
        .find(|t| t.orientation == PanelOrientation::Vertical)
        .unwrap();
    let parent = targets
        .iter()
        .find(|t| t.orientation == PanelOrientation::Horizontal)
        .unwrap();
    assert!(nested.area.x > parent.divider.x);
    assert_eq!(nested.divider.width, nested.area.width);
    let mut drag = DividerDrag::begin(nested.clone(), nested.divider.x, nested.divider.y).unwrap();
    drag.update(nested.divider.x, nested.divider.y + 4);
    let expected = nested.ratio.value() + 4.0 / f64::from(nested.area.height - 1);
    assert_eq!(drag.ratio.value(), expected);
    let (_, adjustment, _) = native
        .prepare_divider_adjustment(
            nested,
            drag.ratio,
            Some((nested.viewport, view.sidebar_surface())),
        )
        .unwrap();
    let ClientLayoutChange::Split { split } = adjustment.change else {
        panic!()
    };
    assert_eq!(split.split_id, nested.split);
    assert_eq!(split.first_slot_id, nested.first);
    assert_eq!(split.second_slot_id, nested.second);
    assert_ne!(split.split_id, parent.split);
    assert_eq!(painted(&native, &view, 120, 40).1, targets);
}

#[test]
fn divider_local_commit_waits_for_shared_publication_and_draw_and_competes_with_keyboard() {
    use crate::terminal_session::ui::tests::publication_hint::{
        settle_queued_refresh, split_snapshot,
    };
    let (mut native, mailbox, clock, _) = ready_split_ui();
    let poll = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(poll).unwrap();
    let view = ClientView::new(120);
    let target = painted(&native, &view, 120, 40).1.remove(0);
    let before = native.control_observation(false).unwrap();
    let mut control = controller(clock.clone());
    control.observe_geometry(target.viewport, view.sidebar_surface());
    control.tick(&mut native, false);
    assert!(control.divider_ready(&native, &target).is_ok());
    control.resize_divider(&native, &target, ClientSplitRatio::new(0.7).unwrap());
    let mutation = control
        .lane
        .as_ref()
        .unwrap()
        .take_layout_for_test()
        .unwrap();
    assert_eq!(mutation.expected_revision, target.source.revision);
    assert!(
        matches!(mutation.action, WorkbenchAction::SplitRatioSet { split, previous_ratio, ratio, .. }
        if split.split_id == target.split && split.first_slot_id == target.first && split.second_slot_id == target.second && previous_ratio == target.ratio && ratio.value() == 0.7)
    );
    assert_eq!(
        control.divider_ready(&native, &target),
        Err(Notice::ResizeBusy)
    );
    control.resize(&native, PanelDirection::Right, false);
    control.resize_divider(&native, &target, ClientSplitRatio::new(0.8).unwrap());
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_none()
    );
    assert_eq!(native.control_observation(false).unwrap(), before);
    control
        .lane
        .as_ref()
        .unwrap()
        .finish_layout_for_test(lane::LayoutCompletion {
            operation_id: mutation.request_id,
            reply: Ok(WorkbenchRevision::new(2)),
        });
    control.tick(&mut native, false);
    control.after_draw(&mut native);
    assert!(control.local_resize_pending());
    let mut snapshot = split_snapshot(3);
    let PanelNode::Split { ratio, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    *ratio = ClientSplitRatio::new(0.7).unwrap();
    settle_queued_refresh(
        &mut native,
        &mailbox,
        &clock,
        WorkbenchSnapshotOutcome::Available { snapshot },
    );
    control.tick(&mut native, false);
    assert!(control.local_resize_pending());
    control.after_draw(&mut native);
    assert!(!control.local_resize_pending());
    assert!(completion(&control).is_none());
    assert_eq!(
        native.control_observation(false).unwrap().1.selection,
        before.1.selection
    );
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_none()
    );
}

#[test]
fn divider_unknown_or_rejected_resolution_never_replays_and_reports_the_shared_notice() {
    for unknown in [false, true] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let view = ClientView::new(120);
        let target = painted(&native, &view, 120, 40).1.remove(0);
        let mut control = controller(clock);
        control.observe_geometry(target.viewport, view.sidebar_surface());
        control.tick(&mut native, false);
        control.resize_divider(&native, &target, ClientSplitRatio::new(0.7).unwrap());
        let mutation = control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .unwrap();
        control
            .lane
            .as_ref()
            .unwrap()
            .finish_layout_for_test(lane::LayoutCompletion {
                operation_id: mutation.request_id,
                reply: Err(
                    crate::local_client::workbench::split_ratio::SplitRatioFailure {
                        reason: Reason::StaleView,
                        may_have_committed: unknown,
                    },
                ),
            });
        control.tick(&mut native, false);
        assert!(!control.local_resize_pending());
        assert!(
            matches!(control.take_local_resize_feedback(), Some(LocalResizeFeedback::Notice(notice)) if notice == if unknown { Notice::ResizeUnknown } else { Notice::ResizeFailed })
        );
        assert!(
            control
                .lane
                .as_ref()
                .unwrap()
                .take_layout_for_test()
                .is_none()
        );
        assert!(completion(&control).is_none());
    }
}
