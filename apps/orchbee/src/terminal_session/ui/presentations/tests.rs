use super::*;
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, TerminalId, WorkerId,
};
#[test]
fn prefix_choice_queues_exact_child_and_does_not_display_primary_report() {
    let mut source = super::super::membership_tests::snapshot(2, 1);
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    let child = TerminalPresentationId::parse(&format!("{:032x}", 9000)).unwrap();
    child_presentations.push(Presentation {
        presentation_id: child.clone(),
        label: "child shell".into(),
        custom_label: None,
        binding: Some(PresentationBinding {
            worker_id: WorkerId::parse(&format!("{:032x}", 9001)).unwrap(),
            terminal_id: TerminalId::parse(&format!("{:032x}", 9002)).unwrap(),
        }),
    });
    let (mut ui, mailbox, _, _) =
        super::super::move_membership_tests::ready_ui_with_snapshot(source);
    assert_eq!(
        ui.apply_client_action(ClientAction::NextPresentation),
        SessionActionDisposition::DismissNavigator
    );
    let command = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(
        command.record(),
        crate::terminal_session::mailbox::QueuedCommand::Switch(_)
    ));
    let view = ui.view();
    let panel = view.tab(0).unwrap().panel(0).unwrap();
    assert_eq!(view.selected_presentation(panel).presentation_id(), &child);
    assert!(matches!(
        view.panel_report(panel),
        crate::terminal_session::PanelReportView::Pending
    ));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &crate::client_view::ClientView::new(120),
                crate::localization::Locale::EnUs,
                Some(crate::terminal_session::TerminalSessionView::Native(
                    ui.view(),
                )),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut crate::tui_pointer::HitMapBuilder::new(),
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(
        text.contains("child shell") && text.contains("[2/2]"),
        "{text}"
    );
}

fn child_source(split: bool) -> orchbee_workbench_protocol::Snapshot {
    let mut source = if split {
        super::super::publication_hint_tests::split_snapshot(1)
    } else {
        super::super::membership_tests::snapshot(2, 1)
    };
    fn add(node: &mut PanelNode, id: &mut u128) {
        match node {
            PanelNode::Panel {
                child_presentations,
                ..
            } => {
                for _ in 0..2 {
                    child_presentations.push(Presentation {
                        presentation_id: TerminalPresentationId::parse(&format!("{:032x}", *id))
                            .unwrap(),
                        label: "child".into(),
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            worker_id: WorkerId::parse(&format!("{:032x}", *id + 1)).unwrap(),
                            terminal_id: TerminalId::parse(&format!("{:032x}", *id + 2)).unwrap(),
                        }),
                    });
                    *id += 10;
                }
            }
            PanelNode::Split { children, .. } => {
                for slot in children.iter_mut() {
                    add(&mut slot.node, id);
                }
            }
        }
    }
    add(&mut source.terminal_contexts[0].tabs[0].root, &mut 9000);
    source
}
fn painted_selector(
    ui: &NativeUi,
    width: u16,
    height: u16,
) -> (
    ratatui::buffer::Buffer,
    crate::tui_pointer::FrameHitMap,
    usize,
) {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &crate::client_view::ClientView::new(width),
                crate::localization::Locale::EnUs,
                Some(crate::terminal_session::TerminalSessionView::Native(
                    ui.view(),
                )),
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    let panes = hits.painted_panes().count();
    (terminal.backend().buffer().clone(), hits.finish(), panes)
}
struct PointerUi<'a>(&'a mut NativeUi);
impl crate::terminal_input::TerminalInputPort for PointerUi<'_> {
    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("member navigation never forwards terminal bytes")
    }
    fn pointer_probe(&mut self) -> crate::terminal_input::PointerProbe {
        self.0.pointer_probe()
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        Some(self.0.apply_client_action(action))
    }
}
#[test]
fn painted_member_arrows_select_exact_children_in_single_split_and_zoom_views() {
    use crate::terminal_input::{MouseAction, MouseButton, MouseEvent};
    use crate::tui_pointer::{HitTarget, PointerOutcome, PointerState};
    for layout in 0..3 {
        let (mut ui, mailbox, clock, _) =
            super::super::move_membership_tests::ready_ui_with_snapshot(child_source(layout != 0));
        if layout != 0 {
            super::super::publication_hint_tests::attach_split_peer(&mut ui, &mailbox, &clock);
        }
        if layout == 2 {
            ui.apply_client_action(ClientAction::ToggleZoom);
        }
        let (buffer, hits, painted_panes) = painted_selector(&ui, 120, 40);
        let count = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                matches!(
                    hits.hit_target_at(x, y),
                    Some(HitTarget::Presentation { .. })
                )
            })
            .count();
        assert_eq!(count, if layout == 1 { 12 } else { 6 });
        if layout == 1 {
            assert_eq!(
                painted_panes, 2,
                "header regions must not duplicate painted terminal receipts"
            );
        }
        let view = ui.view();
        let target_panel = view
            .tab(0)
            .unwrap()
            .panel(usize::from(layout == 1))
            .unwrap();
        let panel_id = target_panel.panel_id().clone();
        let destination = target_panel
            .presentation(if layout == 2 { 2 } else { 1 })
            .unwrap()
            .presentation_id()
            .clone();
        let (x,y) = (0..40).flat_map(|y| (0..120).map(move |x| (x,y))).find(|&(x,y)| {
            matches!(hits.hit_target_at(x,y), Some(HitTarget::Presentation { presentation, .. }) if presentation == &destination)
        }).unwrap();
        // The whole three-cell [>] glyph is clickable; surrounding cells cannot select.
        assert_eq!(buffer[(x, y)].symbol(), "[");
        assert_eq!(
            buffer[(x + 1, y)].symbol(),
            if layout == 2 { "<" } else { ">" }
        );
        assert_eq!(buffer[(x + 2, y)].symbol(), "]");
        for offset in 0..3 {
            assert!(
                matches!(hits.hit_target_at(x+offset,y), Some(HitTarget::Presentation { presentation, .. }) if presentation == &destination)
            );
        }
        for (cx, cy) in [(x - 1, y), (x + 3, y), (x, y + 1), (x, y - 1)] {
            assert!(!matches!(
                hits.hit_target_at(cx, cy),
                Some(HitTarget::Presentation { .. })
            ));
        }
        assert!(
            hits.hit_target_at(if layout == 2 { x + 4 } else { x - 2 }, y)
                .is_none(),
            "the counter is inert chrome"
        );
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        pointer.publish(hits);
        let outcome = pointer.event(
            &mut crate::client_view::ClientView::new(120),
            &mut PointerUi(&mut ui),
            std::time::Instant::now(),
            MouseEvent {
                action: MouseAction::Press,
                button: MouseButton::Left,
                mods: crate::client_view::Modifiers::NONE,
                column: x + 1,
                row: y,
            },
        );
        assert_eq!(outcome, PointerOutcome::RegionsChanged);
        let command = mailbox.try_recv().unwrap().unwrap();
        assert!(matches!(
            command.record(),
            crate::terminal_session::mailbox::QueuedCommand::Switch(_)
        ));
        let view = ui.view();
        let panel = view
            .tab(0)
            .unwrap()
            .panel(usize::from(layout == 1))
            .unwrap();
        assert_eq!(view.focus_panel_id(), Some(&panel_id));
        assert_eq!(
            view.selected_presentation(panel).presentation_id(),
            &destination
        );
    }
}
#[test]
fn selector_degrades_in_minimal_view_and_rejects_missing_foreign_or_selected_members() {
    use crate::tui_pointer::HitTarget;
    let (mut ui, mailbox, _, _) =
        super::super::move_membership_tests::ready_ui_with_snapshot(child_source(false));
    let (_, hits, _) = painted_selector(&ui, 59, 11);
    assert!((0..11).all(|y| (0..59).all(|x| !matches!(
        hits.hit_target_at(x, y),
        Some(HitTarget::Presentation { .. })
    ))));
    let view = ui.view();
    let tab = view.tab(0).unwrap();
    let primary = tab.panel(0).unwrap();
    let panel_id = primary.panel_id().clone();
    let primary_id = primary.presentation_id().clone();
    let other = view.tab(1).unwrap().panel(0).unwrap();
    let foreign_panel = other.panel_id().clone();
    let foreign_id = other.presentation_id().clone();
    for (panel, presentation) in [
        (panel_id.clone(), primary_id),
        (
            panel_id.clone(),
            TerminalPresentationId::parse(&"f".repeat(32)).unwrap(),
        ),
        (foreign_panel, foreign_id.clone()),
        (panel_id, foreign_id),
    ] {
        assert_eq!(
            ui.apply_client_action(ClientAction::SelectPresentation {
                panel,
                presentation
            }),
            SessionActionDisposition::KeepNavigator
        );
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn narrow_split_headers_drop_controls_without_covering_title_or_terminal_content() {
    use crate::tui_pointer::HitTarget;
    use orchbee_workbench_protocol::{
        PanelOrientation, PanelSlot, WorkbenchPanelSlotId, WorkbenchSplitId,
    };
    let mut source = child_source(true);
    let extra = source.terminal_contexts[0].tabs.remove(1).root;
    let PanelNode::Split { children, .. } = &mut source.terminal_contexts[0].tabs[0].root else {
        unreachable!()
    };
    let small_panel = match &children[0].node {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        _ => unreachable!(),
    };
    children[0].node = PanelNode::Split {
        split_id: WorkbenchSplitId::parse(&format!("{:032x}", 20000)).unwrap(),
        orientation: PanelOrientation::Horizontal,
        children: Box::new([
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 20001)).unwrap(),
                node: children[0].node.clone(),
            },
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 20002)).unwrap(),
                node: extra,
            },
        ]),
    };
    let (mut ui, mailbox, _, _) =
        super::super::move_membership_tests::ready_ui_with_snapshot(source);
    let tab = &ui.state.as_ref().unwrap().cache().tabs()[0];
    let observed: Vec<_> = (0..tab.panel_count())
        .map(|index| {
            (
                tab.panel(index).unwrap().panel_id().clone(),
                index,
                Geometry::new(20, 8, 8, 16).unwrap(),
            )
        })
        .collect();
    ui.observe_panes(&observed);
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    let (_, hits, painted_panes) = painted_selector(&ui, 60, 12);
    assert_eq!(painted_panes, 3);
    let mut buttons = 0;
    let mut small_header = false;
    for y in 0..12 {
        for x in 0..60 {
            match hits.hit_target_at(x, y) {
                Some(HitTarget::Presentation { pane, .. }) => {
                    assert_ne!(pane.pane().panel(), &small_panel);
                    buttons += 1;
                }
                Some(HitTarget::Pane { pane, content })
                    if pane.pane().panel() == &small_panel && y < content.y =>
                {
                    small_header = true
                }
                _ => {}
            }
        }
    }
    assert_eq!(buttons, 6, "only the wider sibling retains controls");
    assert!(small_header, "narrow pane header retains its focus region");
}
