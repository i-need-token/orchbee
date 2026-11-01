//! Actual renderer/source preparation and the shared local acknowledgement for divider releases.
use super::*;
use crate::{
    client_view::{ClientView, Notice, PanelDirection},
    localization::Locale,
    tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget},
    tui_layout::{DividerDrag, DrawnDivider, ResizeProjection},
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
    // While the release settles, a press on a divider the frame no longer paints stays refused.
    let elsewhere = painted(&native, &view, 112, 32).1.remove(0);
    assert_eq!(
        control.divider_ready(&native, &elsewhere),
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
    native: &WorkbenchClientUi,
    view: &ClientView,
    width: u16,
    height: u16,
) -> (FrameHitMap, Vec<DrawnDivider>) {
    painted_with_resize(native, view, width, height, &ResizeProjection::Canonical)
}

fn painted_with_resize(
    native: &WorkbenchClientUi,
    view: &ClientView,
    width: u16,
    height: u16,
    resize: &ResizeProjection,
) -> (FrameHitMap, Vec<DrawnDivider>) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell_with_resize(
                frame,
                view,
                Locale::EnUs,
                Some(native.view()),
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
                resize,
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
    let viewport = (target.viewport, view.sidebar_surface());
    assert!(native.divider_projection_current(&target, viewport));
    let before = native.control_observation(false).unwrap();
    let (_, adjustment, observed) = native
        .prepare_divider_adjustment(&target, ClientSplitRatio::new(0.7).unwrap(), Some(viewport))
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
            0 => {
                stale.source.session = orchbee_workbench_protocol::SessionId::parse(other).unwrap()
            }
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
        let prepared = native.prepare_divider_adjustment(
            &stale,
            ClientSplitRatio::new(0.7).unwrap(),
            Some(viewport),
        );
        if dimension == 4 {
            // A commit outside the split advances only the revision: the split is the same,
            // and the CAS names the newest projection rather than the painted one.
            assert_eq!(
                prepared.unwrap().1.workbench_revision,
                native.state.as_ref().unwrap().cache().revision()
            );
        } else {
            assert!(prepared.is_err(), "dimension {dimension}");
        }
        assert_eq!(
            native.divider_projection_current(&stale, viewport),
            dimension == 4,
            "projection dimension {dimension}"
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
        let resize = ResizeProjection::Divider {
            source: target.clone(),
            ratio: drag.ratio,
        };
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell_with_resize(
                        frame,
                        &view,
                        locale,
                        Some(native.view()),
                        &crate::tui_theme::Theme::terminal_native(),
                        &mut hits,
                        &resize,
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
                hits.finish().hit_target_at(drag.preview.x, drag.preview.y),
                Some(&HitTarget::Divider(drag.projected_target()))
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
        crate::client_view::ClientActionDisposition::KeepNavigator,
    );
    let transition = view.decide(crate::client_view::PrefixInput::Key(
        crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Char('?'),
            crate::client_view::Modifiers::NONE,
        ),
    ));
    view.commit(
        transition,
        crate::client_view::ClientActionDisposition::KeepNavigator,
    );
    assert!(painted(&native, &view, 120, 40).1.is_empty());
}

#[test]
fn divider_live_frames_share_program_geometry_and_preserve_pane_paint() {
    use crate::{terminal_device::WindowDimensions, tui_theme::Theme};
    let (native, _, _, _) = ready_split_ui();
    for (width, height) in [(120, 40), (80, 24)] {
        let view = ClientView::new(width);
        let target = painted(&native, &view, width, height).1.remove(0);
        let dimensions = WindowDimensions {
            columns: width,
            rows: height,
            pixel_width: 0,
            pixel_height: 0,
        };
        let canonical =
            crate::tui_layout::pane_geometries(dimensions, &view, &native.view()).unwrap();
        for theme in [
            Theme::terminal_native(),
            Theme::one_dark(),
            Theme::one_light(),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut drag =
                DividerDrag::begin(target.clone(), target.divider.x, target.divider.y).unwrap();
            for delta in [5, 10] {
                drag.update(target.divider.x + delta, target.divider.y);
                let resize = ResizeProjection::Divider {
                    source: target.clone(),
                    ratio: drag.ratio,
                };
                let mut projected = native.view();
                projected.set_resize_projection(&resize);
                let panes =
                    crate::tui_layout::pane_geometries(dimensions, &view, &projected).unwrap();
                let displacement = drag.preview.x - target.divider.x;
                assert_eq!(panes.len(), 2);
                assert_eq!(
                    panes[0].2.columns(),
                    canonical[0].2.columns() + displacement
                );
                assert_eq!(
                    panes[1].2.columns(),
                    canonical[1].2.columns() - displacement
                );
                assert_eq!(panes[0].2.rows(), canonical[0].2.rows());
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell_with_resize(
                            frame,
                            &view,
                            Locale::ZhCn,
                            Some(native.view()),
                            &theme,
                            &mut hits,
                            &resize,
                        );
                        let base = frame.buffer_mut().clone();
                        assert!(crate::tui_render::render_divider_preview(
                            frame,
                            &view,
                            Locale::ZhCn,
                            &drag,
                            &theme,
                            &hits,
                        ));
                        // Feedback styles the real border. It cannot replace pane glyphs,
                        // junctions or wide characters with a detached preview line.
                        for y in 0..height - 1 {
                            for x in 0..width {
                                let before = &base[(x, y)];
                                let after = &frame.buffer_mut()[(x, y)];
                                assert_eq!(after.symbol(), before.symbol());
                                if !drag.preview.contains((x, y).into()) {
                                    assert_eq!(after, before);
                                }
                            }
                        }
                    })
                    .unwrap();
                let map = hits.finish();
                assert_eq!(
                    map.hit_target_at(drag.preview.x, drag.preview.y),
                    Some(&HitTarget::Divider(drag.projected_target()))
                );
                assert!(!matches!(
                    map.hit_target_at(target.divider.x, target.divider.y),
                    Some(HitTarget::Divider(_))
                ));
            }
            assert_eq!(
                crate::tui_layout::pane_geometries(dimensions, &view, &native.view()).unwrap(),
                canonical
            );
            assert_eq!(
                painted(&native, &view, width, height).1.as_slice(),
                std::slice::from_ref(&target)
            );
        }
    }
}

#[test]
fn held_viewport_observations_keep_the_cas_source_and_resize_only_its_controller() {
    use crate::workbench_client::mailbox::{CausalSettlement, QueuedCommand};
    use crate::workbench_client::ui::tests::publication_hint::ready_split_ui_with_mode;
    use crate::workbench_state::{ControllerMode, OPERATION_TOTAL};
    for controlling in [false, true] {
        let mode = if controlling {
            ControllerMode::Controlling(
                orchbee_client_server_protocol::ControllerId::parse(&format!("{:032x}", 8))
                    .unwrap(),
            )
        } else {
            ControllerMode::ReadOnly
        };
        let (mut native, mailbox, clock, _) = ready_split_ui_with_mode(mode);
        let poll = mailbox.try_recv().unwrap().unwrap();
        let poll_key = poll.poll_key().unwrap().clone();
        let deadline = clock.now() + OPERATION_TOTAL;
        mailbox
            .publish_poll_result_before(poll_key.clone(), None, false, deadline, &clock)
            .unwrap();
        native.tick();
        assert!(matches!(
            mailbox
                .take_causal_before(&poll_key, deadline, &clock)
                .unwrap(),
            Some(CausalSettlement::NoFeedback)
        ));
        mailbox.finish_lease(poll).unwrap();
        let view = ClientView::new(120);
        let target = painted(&native, &view, 120, 40).1.remove(0);
        let identity = native.view().divider_source().unwrap();
        let focus = native
            .view()
            .pane_ref_for_panel(&target.source.focus)
            .unwrap();
        let dimensions = crate::terminal_device::WindowDimensions {
            columns: 120,
            rows: 40,
            pixel_width: 960,
            pixel_height: 640,
        };
        let mut drag =
            DividerDrag::begin(target.clone(), target.divider.x, target.divider.y).unwrap();
        for delta in [5, 10] {
            drag.update(target.divider.x + delta, target.divider.y);
            let resize = ResizeProjection::Divider {
                source: target.clone(),
                ratio: drag.ratio,
            };
            let mut projected = native.view();
            projected.set_resize_projection(&resize);
            let panes = crate::tui_layout::pane_geometries(dimensions, &view, &projected).unwrap();
            let expected = panes
                .iter()
                .find(|(panel, _, _)| panel == &target.source.focus)
                .unwrap()
                .2;
            native.observe_panes(&panes);
            assert_eq!(native.view().divider_source().unwrap(), identity);
            assert_eq!(
                native
                    .view()
                    .pane_ref_for_panel(&target.source.focus)
                    .unwrap(),
                focus
            );
            assert!(
                native
                    .prepare_divider_adjustment(
                        &target,
                        drag.ratio,
                        Some((target.viewport, view.sidebar_surface()))
                    )
                    .is_ok()
            );
            let mut resizes = Vec::new();
            while let Some(lease) = mailbox.try_recv().unwrap() {
                if let QueuedCommand::Resize(tagged) = lease.record() {
                    resizes.push((tagged.key().clone(), *tagged.value()));
                }
                mailbox.finish_lease(lease).unwrap();
            }
            assert_eq!(
                resizes,
                if controlling {
                    vec![(focus.clone(), expected)]
                } else {
                    Vec::new()
                }
            );
            // The shell can re-observe its retained viewport after zero-sized host samples.
            // The actual controller owner must deduplicate every identical projected geometry.
            for _ in 0..3 {
                native.observe_panes(&panes);
                while let Some(lease) = mailbox.try_recv().unwrap() {
                    assert!(
                        !matches!(lease.record(), QueuedCommand::Resize(_)),
                        "same held frame never resizes twice"
                    );
                    mailbox.finish_lease(lease).unwrap();
                }
            }
            assert_eq!(
                painted_with_resize(&native, &view, 120, 40, &resize).1,
                [drag.projected_target()]
            );
        }
        assert_eq!(painted(&native, &view, 120, 40).1, [target]);
    }
}

#[test]
fn divider_nested_vertical_target_uses_its_own_painted_area_and_ordered_slots() {
    use crate::workbench_client::ui::tests::publication_hint::{
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
    let dimensions = crate::terminal_device::WindowDimensions {
        columns: 120,
        rows: 40,
        pixel_width: 0,
        pixel_height: 0,
    };
    let original = crate::tui_layout::pane_geometries(dimensions, &view, &native.view()).unwrap();
    for delta in [2, 4] {
        drag.update(nested.divider.x, nested.divider.y + delta);
        let resize = ResizeProjection::Divider {
            source: nested.clone(),
            ratio: drag.ratio,
        };
        let drawn = painted_with_resize(&native, &view, 120, 40, &resize).1;
        assert!(drawn.contains(parent), "the ancestor split is unchanged");
        assert!(drawn.contains(&drag.projected_target()));
        let mut projected = native.view();
        projected.set_resize_projection(&resize);
        let panes = crate::tui_layout::pane_geometries(dimensions, &view, &projected).unwrap();
        let displacement = drag.preview.y - nested.divider.y;
        assert_eq!(
            panes[0], original[0],
            "the ancestor's other branch is unchanged"
        );
        assert_eq!(panes[1].2.columns(), original[1].2.columns());
        assert_eq!(panes[1].2.rows(), original[1].2.rows() + displacement);
        assert_eq!(panes[2].2.rows(), original[2].2.rows() - displacement);
        assert!(
            native
                .prepare_divider_adjustment(
                    nested,
                    drag.ratio,
                    Some((nested.viewport, view.sidebar_surface()))
                )
                .is_ok(),
            "own projected geometry does not replace the canonical CAS origin"
        );
    }
    assert_eq!(painted(&native, &view, 120, 40).1, targets);
}

#[test]
fn divider_local_commit_waits_for_shared_publication_and_draw_and_competes_with_keyboard() {
    use crate::workbench_client::ui::tests::publication_hint::{
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
    let released = ResizeProjection::Divider {
        source: target.clone(),
        ratio: ClientSplitRatio::new(0.7).unwrap(),
    };
    assert_eq!(
        control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
        released
    );
    assert!(
        matches!(mutation.action, WorkbenchAction::SplitRatioSet { split, previous_ratio, ratio, .. }
        if split.split_id == target.split && split.first_slot_id == target.first && split.second_slot_id == target.second && previous_ratio == target.ratio && ratio.value() == 0.7)
    );
    // The settling release's draft is what the screen paints; a drag may start on it.
    let drafted = painted_with_resize(&native, &view, 120, 40, &released)
        .1
        .remove(0);
    assert!(control.divider_ready(&native, &drafted).is_ok());
    control.resize(&native, PanelDirection::Right, false);
    control.resize_divider(&native, &drafted, ClientSplitRatio::new(0.8).unwrap());
    let queued = ResizeProjection::Divider {
        source: target.clone(),
        ratio: ClientSplitRatio::new(0.8).unwrap(),
    };
    assert_eq!(
        control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
        queued,
        "a second release shows over the draft and waits for the first"
    );
    // A drag on the waiting release's draft replaces that release from the layout it was dragged
    // on; the draft it covers no longer starts one, which would discard the waiting release.
    let waiting = painted_with_resize(&native, &view, 120, 40, &queued)
        .1
        .remove(0);
    assert!(control.divider_ready(&native, &waiting).is_ok());
    assert_eq!(
        control.divider_ready(&native, &drafted),
        Err(Notice::ResizeBusy)
    );
    control.resize_divider(&native, &waiting, ClientSplitRatio::new(0.75).unwrap());
    let queued = ResizeProjection::Divider {
        source: target.clone(),
        ratio: ClientSplitRatio::new(0.75).unwrap(),
    };
    assert_eq!(
        control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
        queued,
        "the newest release shows over the same draft"
    );
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_none()
    );
    assert_eq!(native.control_observation(false).unwrap(), before);
    control.lane.as_ref().unwrap().finish_layout_for_test(
        crate::workbench_client::client_control::LayoutCompletion {
            operation_id: mutation.request_id,
            reply: Ok(WorkbenchRevision::new(2)),
        },
    );
    control.tick(&mut native, false);
    control.after_draw(&mut native);
    assert!(control.local_resize_pending());
    assert_eq!(
        control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
        queued,
        "a reply before publication cannot snap the panes back"
    );
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
    assert_eq!(
        control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
        ResizeProjection::Divider {
            source: drafted.clone(),
            ratio: ClientSplitRatio::new(0.75).unwrap(),
        },
        "the new canonical publication takes over the first release's position"
    );
    control.after_draw(&mut native);
    assert!(completion(&control).is_none());
    assert_eq!(
        native.control_observation(false).unwrap().1.selection,
        before.1.selection
    );
    // The first release completed; the queued one is submitted against the layout it left.
    assert!(control.local_resize_pending());
    let second = control
        .lane
        .as_ref()
        .unwrap()
        .take_layout_for_test()
        .unwrap();
    assert!(
        matches!(second.action, WorkbenchAction::SplitRatioSet { split, previous_ratio, ratio, .. }
        if split.split_id == target.split && previous_ratio.value() == 0.7 && ratio.value() == 0.75)
    );
}

/// A refusal that is not about an older revision, or an unknown outcome, is never replayed, and a
/// release queued behind it is discarded with it.
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
        assert!(matches!(
            control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
            ResizeProjection::Divider { .. }
        ));
        let mutation = control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .unwrap();
        let released = ResizeProjection::Divider {
            source: target.clone(),
            ratio: ClientSplitRatio::new(0.7).unwrap(),
        };
        let drafted = painted_with_resize(&native, &view, 120, 40, &released)
            .1
            .remove(0);
        control.resize_divider(&native, &drafted, ClientSplitRatio::new(0.8).unwrap());
        control.lane.as_ref().unwrap().finish_layout_for_test(
            crate::workbench_client::client_control::LayoutCompletion {
                operation_id: mutation.request_id,
                reply: Err(
                    crate::local_client::workbench::split_ratio::SplitRatioFailure {
                        reason: if unknown {
                            Reason::StaleView
                        } else {
                            Reason::TargetUnavailable
                        },
                        may_have_committed: unknown,
                    },
                ),
            },
        );
        control.tick(&mut native, false);
        assert!(!control.local_resize_pending());
        assert_eq!(
            control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
            ResizeProjection::Canonical
        );
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

/// Another Client's commit moves the Workbench past the revision a release CAS names, so the
/// Server refuses it without effect. The same move is resubmitted against each newer installed
/// revision while its split keeps the previous ratio, however often other commits outpace it,
/// until a time bound passes, with the draft kept meanwhile.
#[test]
fn a_revision_refusal_resubmits_the_same_divider_move_only_while_its_split_is_unchanged() {
    use crate::workbench_client::ui::tests::publication_hint::{
        settle_queued_refresh, split_snapshot,
    };
    let refuse = |control: &ClientControl, request_id| {
        control.lane.as_ref().unwrap().finish_layout_for_test(
            crate::workbench_client::client_control::LayoutCompletion {
                operation_id: request_id,
                reply: Err(
                    crate::local_client::workbench::split_ratio::SplitRatioFailure {
                        reason: Reason::StaleView,
                        may_have_committed: false,
                    },
                ),
            },
        );
    };
    for split_moved in [false, true] {
        let (mut native, mailbox, clock, elapsed) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let view = ClientView::new(120);
        let target = painted(&native, &view, 120, 40).1.remove(0);
        let mut control = controller(clock.clone());
        control.observe_geometry(target.viewport, view.sidebar_surface());
        control.tick(&mut native, false);
        control.resize_divider(&native, &target, ClientSplitRatio::new(0.7).unwrap());
        let mut mutation = control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .unwrap();
        for revision in [3, 5, 7, 9] {
            refuse(&control, mutation.request_id.clone());
            control.tick(&mut native, false);
            assert!(control.local_resize_pending(), "{split_moved} {revision}");
            assert!(
                control
                    .lane
                    .as_ref()
                    .unwrap()
                    .take_layout_for_test()
                    .is_none(),
                "nothing is resubmitted before the newer revision is installed"
            );
            assert!(matches!(
                control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
                ResizeProjection::Divider { .. }
            ));
            let mut snapshot = split_snapshot(revision);
            if split_moved {
                let PanelNode::Split { ratio, .. } =
                    &mut snapshot.terminal_contexts[0].tabs[0].root
                else {
                    panic!()
                };
                *ratio = ClientSplitRatio::new(0.6).unwrap();
            }
            settle_queued_refresh(
                &mut native,
                &mailbox,
                &clock,
                WorkbenchSnapshotOutcome::Available { snapshot },
            );
            // The installed snapshot schedules the next pane poll; it plays no part here.
            let poll = mailbox.try_recv().unwrap().unwrap();
            mailbox.finish_lease(poll).unwrap();
            control.tick(&mut native, false);
            if split_moved {
                break;
            }
            mutation = control
                .lane
                .as_ref()
                .unwrap()
                .take_layout_for_test()
                .unwrap();
            assert_eq!(mutation.expected_revision, WorkbenchRevision::new(revision));
            assert!(
                matches!(mutation.action, WorkbenchAction::SplitRatioSet { split, previous_ratio, ratio, .. }
                if split.split_id == target.split && previous_ratio == target.ratio && ratio.value() == 0.7)
            );
        }
        if !split_moved {
            // Five seconds after the first refusal, one more ends the move under its notice.
            elapsed.fetch_add(5_000, std::sync::atomic::Ordering::SeqCst);
            refuse(&control, mutation.request_id.clone());
            control.tick(&mut native, false);
        }
        assert!(!control.local_resize_pending());
        let expected = if split_moved {
            Notice::ResizeFailed
        } else {
            Notice::ResizeContended
        };
        assert!(
            matches!(control.take_local_resize_feedback(), Some(LocalResizeFeedback::Notice(notice)) if notice == expected)
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
}

/// A drag pressed on a settling release's draft, or on the draft of a release waiting behind it,
/// outlives that release when it ends without landing. The drag can no longer land on the layout
/// it was pressed on, so the pointer's next check ends it under the notice the release ended with
/// instead of a generic failure that would replace that notice.
#[test]
fn a_drag_on_a_draft_whose_release_ends_unlanded_ends_under_that_release_notice() {
    for waiting in [false, true] {
        let (mut native, mailbox, clock, elapsed) = ready_split_ui();
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
        let released = ResizeProjection::Divider {
            source: target.clone(),
            ratio: ClientSplitRatio::new(0.7).unwrap(),
        };
        let mut pressed = painted_with_resize(&native, &view, 120, 40, &released)
            .1
            .remove(0);
        if waiting {
            control.resize_divider(&native, &pressed, ClientSplitRatio::new(0.8).unwrap());
            let queued = ResizeProjection::Divider {
                source: target.clone(),
                ratio: ClientSplitRatio::new(0.8).unwrap(),
            };
            pressed = painted_with_resize(&native, &view, 120, 40, &queued)
                .1
                .remove(0);
        }
        assert!(
            control.divider_ready(&native, &pressed).is_ok(),
            "{waiting}"
        );
        // Other commits keep outpacing the release until its time bound passes.
        control.lane.as_ref().unwrap().finish_layout_for_test(
            crate::workbench_client::client_control::LayoutCompletion {
                operation_id: mutation.request_id,
                reply: Err(
                    crate::local_client::workbench::split_ratio::SplitRatioFailure {
                        reason: Reason::StaleView,
                        may_have_committed: false,
                    },
                ),
            },
        );
        control.tick(&mut native, false);
        assert!(control.local_resize_pending(), "{waiting}");
        elapsed.fetch_add(5_000, std::sync::atomic::Ordering::SeqCst);
        control.tick(&mut native, false);
        assert!(!control.local_resize_pending(), "{waiting}");
        assert!(
            matches!(
                control.take_local_resize_feedback(),
                Some(LocalResizeFeedback::Notice(Notice::ResizeContended))
            ),
            "{waiting}"
        );
        assert_eq!(
            control.divider_ready(&native, &pressed),
            Err(Notice::ResizeContended),
            "{waiting}"
        );
        assert!(
            matches!(
                control.take_local_resize_feedback(),
                Some(LocalResizeFeedback::Notice(Notice::ResizeContended))
            ),
            "{waiting}"
        );
        // The canonical split it snapped back to admits the next drag as usual.
        assert!(control.divider_ready(&native, &target).is_ok(), "{waiting}");
    }
}

#[test]
fn released_divider_projection_retires_permanently_on_external_owner_or_geometry_loss() {
    use crate::client_view::SidebarSurface;
    for reason in 0..6 {
        let (mut native, _, clock, elapsed) = ready_split_ui();
        let view = ClientView::new(120);
        let target = painted(&native, &view, 120, 40).1.remove(0);
        let mut control = controller(clock);
        control.observe_geometry(target.viewport, view.sidebar_surface());
        control.tick(&mut native, false);
        control.resize_divider(&native, &target, ClientSplitRatio::new(0.7).unwrap());
        assert!(matches!(
            control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
            ResizeProjection::Divider { .. }
        ));
        let mut viewport = target.viewport;
        let mut sidebar = view.sidebar_surface();
        let mut allowed = true;
        match reason {
            0 => allowed = false,
            1 => viewport.width += 1,
            2 => {
                sidebar = SidebarSurface::Hidden;
                assert_ne!(sidebar, view.sidebar_surface());
            }
            3 => {
                native.apply_client_action(ClientAction::ToggleZoom);
            }
            4 => control.interrupt_local_resize(),
            _ => elapsed.store(51_000, std::sync::atomic::Ordering::SeqCst),
        }
        assert_eq!(
            control.resize_projection(&native, viewport, sidebar, allowed),
            ResizeProjection::Canonical,
            "retirement reason {reason}"
        );
        assert_eq!(
            control.resize_projection(&native, target.viewport, view.sidebar_surface(), true),
            ResizeProjection::Canonical,
            "restoring the old presentation cannot revive its lease"
        );
        assert!(
            control.local_resize_pending(),
            "retiring pixels does not cancel or replay the submitted mutation"
        );
        assert!(
            control
                .lane
                .as_ref()
                .unwrap()
                .take_layout_for_test()
                .is_some()
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
}

/// Drives the real renderer, published hit map, input decoder and controller with the byte
/// sequence a terminal sends for press, held drag and release on the painted divider column.
#[test]
fn divider_real_mouse_sequence_on_the_painted_column_submits_one_ratio() {
    use crate::terminal_input::{
        InputDispatcher, LayoutAction, SurfaceAction, TerminalInputPort, TerminalInputUnavailable,
    };
    struct Port<'a> {
        native: &'a WorkbenchClientUi,
        control: &'a mut ClientControl,
        refusals: Vec<Notice>,
        forwarded: Vec<u8>,
    }
    impl TerminalInputPort for Port<'_> {
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.extend_from_slice(bytes);
            Ok(())
        }
        fn pointer_divider_ready(&mut self, target: &DrawnDivider) -> Result<(), Notice> {
            let result = self.control.divider_ready(self.native, target);
            if let Err(notice) = result {
                self.refusals.push(notice);
            }
            result
        }
        fn surface_action(
            &mut self,
            action: SurfaceAction<'_>,
        ) -> Option<crate::client_view::ClientActionDisposition> {
            if let SurfaceAction::Layout(LayoutAction::Divider { target, ratio }) = action {
                self.control.resize_divider(self.native, target, ratio);
            }
            None
        }
    }
    fn sgr(button: u8, column: u16, row: u16, release: bool) -> Vec<u8> {
        format!(
            "\x1b[<{button};{};{}{}",
            column + 1,
            row + 1,
            if release { 'm' } else { 'M' }
        )
        .into_bytes()
    }
    for (width, height) in [(112, 32), (120, 40)] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut view = ClientView::new(width);
        let (map, mut targets) = painted(&native, &view, width, height);
        assert_eq!(targets.len(), 1, "{width}x{height}");
        let target = targets.remove(0);
        let (column, row) = (
            target.divider.x,
            target.divider.y + target.divider.height / 2,
        );
        // A 112-column host with the default sidebar paints the divider in its 72nd column
        // (one-based); the 71st belongs to the left pane and holds its scrollbar track.
        if width == 112 {
            assert_eq!(column + 1, 72);
        }
        // Only the single divider column starts a drag; its neighbours belong to the panes.
        assert!(matches!(
            map.hit_target_at(column, row),
            Some(HitTarget::Divider(painted)) if painted == &target
        ));
        for neighbour in [column - 1, column + 1] {
            assert!(!matches!(
                map.hit_target_at(neighbour, row),
                Some(HitTarget::Divider(_))
            ));
        }
        let mut control = controller(clock);
        control.observe_geometry(target.viewport, view.sidebar_surface());
        control.tick(&mut native, false);
        let mut port = Port {
            native: &native,
            control: &mut control,
            refusals: Vec::new(),
            forwarded: Vec::new(),
        };
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(map);
        let send = |dispatcher: &mut InputDispatcher,
                    view: &mut ClientView,
                    port: &mut Port<'_>,
                    bytes: &[u8]| {
            dispatcher
                .dispatch(
                    view,
                    bytes,
                    port,
                    std::time::Instant::now(),
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        };
        send(
            &mut dispatcher,
            &mut view,
            &mut port,
            &sgr(0, column, row, false),
        );
        assert!(dispatcher.divider_preview().is_some(), "{width}x{height}");
        send(
            &mut dispatcher,
            &mut view,
            &mut port,
            &sgr(32, column + 6, row, false),
        );
        send(
            &mut dispatcher,
            &mut view,
            &mut port,
            &sgr(0, column + 6, row, true),
        );
        assert!(dispatcher.divider_preview().is_none());
        assert!(port.refusals.is_empty(), "{:?}", port.refusals);
        assert!(port.forwarded.is_empty());
        // Until that submission is published and drawn, a press on a split the frame paints
        // still starts a drag, and its changed release waits behind the pending one.
        dispatcher.publish_hit_map(painted(port.native, &view, width, height).0);
        send(
            &mut dispatcher,
            &mut view,
            &mut port,
            &sgr(0, column, row, false),
        );
        assert!(dispatcher.divider_preview().is_some(), "{width}x{height}");
        send(
            &mut dispatcher,
            &mut view,
            &mut port,
            &sgr(32, column + 3, row, false),
        );
        send(
            &mut dispatcher,
            &mut view,
            &mut port,
            &sgr(0, column + 3, row, true),
        );
        assert!(dispatcher.divider_preview().is_none());
        assert!(port.refusals.is_empty(), "{:?}", port.refusals);
        let lane = control.lane.as_ref().unwrap();
        let mutation = lane
            .take_layout_for_test()
            .expect("the release submits one ratio");
        assert!(matches!(
            mutation.action,
            WorkbenchAction::SplitRatioSet { previous_ratio, ratio, .. }
                if previous_ratio == target.ratio && ratio.value() > target.ratio.value()
        ));
        assert!(
            lane.take_layout_for_test().is_none(),
            "the second release is not submitted before the first settles"
        );
    }
}
