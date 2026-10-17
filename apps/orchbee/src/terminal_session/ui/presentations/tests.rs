use super::super::{
    AttachmentKey, ClientAction, ControllerMode, FrameIdentity, Geometry, WorkerEvent,
};
use super::*;
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, TerminalId, WorkerId,
};

#[test]
fn automatic_tab_chrome_uses_localized_positions_and_keeps_fixed_names_and_pane_titles() {
    use crate::{
        client_view::ClientView, localization::Locale, tui_pointer::HitMapBuilder, tui_theme::Theme,
    };
    for (locale, first, second) in [
        (Locale::EnUs, "Tab 1", "Tab 2"),
        (Locale::ZhCn, "标签页 1", "标签页 2"),
    ] {
        for fixed in ["3", "Review 中文"] {
            let mut source = super::super::tests::membership::snapshot(3, 1);
            for (index, tab) in source.terminal_contexts[0].tabs.iter_mut().enumerate() {
                tab.automatic_label = index < 2;
                tab.label = if index < 2 {
                    (index + 1).to_string()
                } else {
                    fixed.into()
                };
                let PanelNode::Panel {
                    primary_presentation,
                    ..
                } = &mut tab.root
                else {
                    unreachable!();
                };
                primary_presentation.label = tab.label.clone();
            }
            let (ui, mailbox, _, _) =
                super::super::tests::move_membership::ready_ui_with_snapshot(source);
            assert_eq!(ui.view().tab(2).unwrap().display_label(locale), fixed);
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
                    let mut terminal =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                            .unwrap();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &ClientView::new(width),
                                locale,
                                Some(crate::terminal_session::TerminalSessionView::Native(
                                    ui.view(),
                                )),
                                &theme,
                                &mut HitMapBuilder::new(),
                            );
                        })
                        .unwrap();
                    let row = |y| {
                        let buffer = terminal.backend().buffer();
                        let mut text = String::new();
                        let mut x = 0;
                        while x < width {
                            let symbol = buffer[(x, y)].symbol();
                            text.push_str(symbol);
                            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1))
                                .unwrap();
                        }
                        text
                    };
                    if width >= 60 {
                        let strip = row(0);
                        assert!(strip.contains(first), "{strip}");
                        assert!(strip.contains(second), "{strip}");
                        if width >= 80 || strip.contains(fixed) {
                            assert!(strip.contains(fixed), "{strip}");
                        } else {
                            // Compact chrome may clip a long fixed name. It must
                            // still paint that name's prefix, never an automatic label.
                            let suffix = strip.split_once(second).unwrap().1.trim_start();
                            let prefix = suffix.split_once(theme.glyphs.ellipsis).unwrap().0;
                            assert!(!prefix.is_empty() && fixed.starts_with(prefix), "{strip}");
                        }
                        assert!(!strip.contains(&locale.automatic_tab_label(3)), "{strip}");
                        assert!(
                            row(1).contains(&format!("{} 1", locale.shell().panel_title)),
                            "{}",
                            row(1)
                        );
                    } else {
                        assert!(!row(0).contains(first), "hidden strips cannot paint a Tab");
                    }
                }
            }
            mailbox.close(crate::terminal_session::state::CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn tab_strip_omits_equal_location_but_retains_context_for_a_distinct_tab_label() {
    use crate::{
        client_view::ClientView, localization::Locale, tui_pointer::HitMapBuilder, tui_theme::Theme,
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::one_light()] {
            for repeats_location in [false, true] {
                let location = locale.shell().standalone;
                let label = if repeats_location { location } else { "Editor" };
                let mut source = super::super::tests::membership::snapshot(1, 1);
                source.terminal_contexts[0].tabs[0].label = label.into();
                let (ui, _, _, _) =
                    super::super::tests::move_membership::ready_ui_with_snapshot(source);
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &ClientView::new(120),
                            locale,
                            Some(crate::terminal_session::TerminalSessionView::Native(
                                ui.view(),
                            )),
                            &theme,
                            &mut HitMapBuilder::new(),
                        );
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let mut strip = String::new();
                let mut x = 0;
                while x < 120 {
                    let symbol = buffer[(x, 0)].symbol();
                    strip.push_str(symbol);
                    x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
                }
                assert!(strip.contains(label), "{strip}");
                assert_eq!(strip.matches(location).count(), 1, "{strip}");
                if !repeats_location {
                    assert!(strip.contains("Editor"));
                }
            }
        }
    }
}

#[test]
fn prefix_choice_queues_exact_child_and_does_not_display_primary_report() {
    let mut source = super::super::tests::membership::snapshot(2, 1);
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
        super::super::tests::move_membership::ready_ui_with_snapshot(source);
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
        super::super::tests::publication_hint::split_snapshot(1)
    } else {
        super::super::tests::membership::snapshot(2, 1)
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
    capture: Option<&str>,
) -> (
    ratatui::buffer::Buffer,
    crate::tui_pointer::FrameHitMap,
    usize,
) {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    let locale = crate::localization::Locale::EnUs;
    let theme = crate::tui_theme::Theme::terminal_native();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &crate::client_view::ClientView::new(width),
                locale,
                Some(crate::terminal_session::TerminalSessionView::Native(
                    ui.view(),
                )),
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    if let Some(name) = capture {
        crate::tui_render::test_capture::capture_screen(&mut terminal, name, locale, &theme);
    }
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

fn activate_painted_presentation(
    pointer: &mut crate::tui_pointer::PointerState,
    ui: &mut NativeUi,
    column: u16,
    row: u16,
) -> SessionActionDisposition {
    use crate::{
        terminal_input::{MouseAction, MouseButton, MouseEvent},
        tui_pointer::{InteractionTarget, PointerOutcome, PrimaryContract},
    };
    let mut view = crate::client_view::ClientView::new(120);
    let now = std::time::Instant::now();
    let event = |action| MouseEvent {
        action,
        button: MouseButton::Left,
        mods: crate::client_view::Modifiers::NONE,
        column,
        row,
    };
    assert_eq!(
        pointer.event(
            &mut view,
            &mut PointerUi(ui),
            now,
            event(MouseAction::Press),
        ),
        PointerOutcome::Handled
    );
    let PointerOutcome::Interaction {
        target: InteractionTarget::Presentation(target),
        contract: PrimaryContract::ActivateOnRelease,
    } = pointer.event(
        &mut view,
        &mut PointerUi(ui),
        now,
        event(MouseAction::Release),
    )
    else {
        panic!("matching release must carry the exact painted presentation target")
    };
    ui.activate_drawn_presentation(&target)
}

#[test]
fn drawn_presentation_activation_revalidates_live_member_and_unbound_source() {
    let (mut attached, attached_mailbox, _, _) =
        super::super::tests::move_membership::ready_ui_with_snapshot(child_source(true));
    let (target, foreign_member) = {
        let view = attached.view();
        let tab = view.tab(0).unwrap();
        let panel = tab.panel(0).unwrap();
        let peer = tab.panel(1).unwrap();
        (
            view.drawn_presentation_target(panel, 1).unwrap(),
            peer.presentation(1).unwrap().presentation_id().clone(),
        )
    };
    let wrong_member = match target.clone() {
        DrawnPresentationTarget::Attached { pane, .. } => DrawnPresentationTarget::Attached {
            pane,
            presentation: foreign_member,
        },
        DrawnPresentationTarget::Unbound { .. } => panic!("ready pane must be attached"),
    };
    assert_eq!(
        attached.activate_drawn_presentation(&wrong_member),
        SessionActionDisposition::KeepNavigator
    );
    assert!(attached_mailbox.try_recv().unwrap().is_none());
    assert_eq!(
        attached.activate_drawn_presentation(&target),
        SessionActionDisposition::DismissNavigator
    );
    assert!(matches!(
        attached_mailbox.try_recv().unwrap().unwrap().record(),
        crate::terminal_session::mailbox::QueuedCommand::Switch(_)
    ));

    let (mut unbound, unbound_mailbox, _) = empty_ui();
    let (target, prior_member) = {
        let view = unbound.view();
        let panel = view.tab(0).unwrap().panel(0).unwrap();
        (
            view.drawn_presentation_target(panel, 2).unwrap(),
            panel.presentation(1).unwrap().presentation_id().clone(),
        )
    };
    let stale_source = match target {
        DrawnPresentationTarget::Unbound {
            mut selected,
            presentation,
        } => {
            selected.presentation = prior_member;
            DrawnPresentationTarget::Unbound {
                selected,
                presentation,
            }
        }
        DrawnPresentationTarget::Attached { .. } => panic!("empty slot must be unbound"),
    };
    assert_eq!(
        unbound.activate_drawn_presentation(&stale_source),
        SessionActionDisposition::KeepNavigator
    );
    assert!(unbound_mailbox.try_recv().unwrap().is_none());
}

#[test]
fn painted_member_arrows_select_exact_children_in_single_split_and_zoom_views() {
    use crate::tui_pointer::{HitTarget, PointerState};
    for layout in 0..3 {
        let (mut ui, mailbox, clock, _) =
            super::super::tests::move_membership::ready_ui_with_snapshot(child_source(layout != 0));
        if layout != 0 {
            super::super::tests::publication_hint::attach_split_peer(&mut ui, &mailbox, &clock);
        }
        if layout == 2 {
            ui.apply_client_action(ClientAction::ToggleZoom);
        }
        let (buffer, hits, painted_panes) = painted_selector(&ui, 120, 40, None);
        let count = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|&(x, y)| matches!(hits.hit_target_at(x, y), Some(HitTarget::Presentation(_))))
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
            matches!(hits.hit_target_at(x,y), Some(HitTarget::Presentation(target)) if target.presentation() == &destination)
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
                matches!(hits.hit_target_at(x+offset,y), Some(HitTarget::Presentation(target)) if target.presentation() == &destination)
            );
        }
        for (cx, cy) in [(x - 1, y), (x + 3, y), (x, y + 1), (x, y - 1)] {
            assert!(!matches!(
                hits.hit_target_at(cx, cy),
                Some(HitTarget::Presentation(_))
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
        assert_eq!(
            activate_painted_presentation(&mut pointer, &mut ui, x + 1, y),
            SessionActionDisposition::DismissNavigator
        );
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
        #[cfg(windows)]
        assert_eq!(
            view.focused_member(),
            Some(crate::terminal_session::NativeMemberTarget {
                tab: view.tab(0).unwrap().tab_id().clone(),
                panel: panel_id.clone(),
                presentation: destination.clone(),
            })
        );
        assert_eq!(
            view.selected_presentation(panel).presentation_id(),
            &destination
        );
        if layout == 0 {
            let (selected_buffer, _, _) = painted_selector(
                &ui,
                120,
                40,
                Some("alignment-presentation-en-us-120x40-terminal-native"),
            );
            let selected_text: String = selected_buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(selected_text.contains("child"), "{selected_text}");
        }
    }
}
#[test]
fn selector_degrades_in_minimal_view_and_exact_targets_reject_selected_or_foreign_members() {
    use crate::tui_pointer::HitTarget;
    let (mut ui, mailbox, _, _) =
        super::super::tests::move_membership::ready_ui_with_snapshot(child_source(false));
    let (_, hits, _) = painted_selector(&ui, 59, 11, None);
    assert!((0..11).all(|y| {
        (0..59).all(|x| !matches!(hits.hit_target_at(x, y), Some(HitTarget::Presentation(_))))
    }));
    let view = ui.view();
    let tab = view.tab(0).unwrap();
    let primary = tab.panel(0).unwrap();
    let selected_index = primary
        .presentation_index(view.selected_presentation(primary).presentation_id())
        .unwrap();
    let selected = view
        .drawn_presentation_target(primary, selected_index)
        .unwrap();
    let prior_tab = view
        .drawn_presentation_target(primary, (selected_index + 1) % primary.presentation_count())
        .unwrap();
    assert_eq!(
        ui.activate_drawn_presentation(&selected),
        SessionActionDisposition::KeepNavigator
    );
    assert!(mailbox.try_recv().unwrap().is_none());

    // The same exact target is a foreign source in a client whose selected
    // Tab is the other member. It was minted by a real view while its Tab was
    // current; activation must not reinterpret it against the new selection.
    let mut foreign_source = child_source(false);
    foreign_source.terminal_contexts[0].tabs.swap(0, 1);
    let (mut foreign_ui, foreign_mailbox, _, _) =
        super::super::tests::move_membership::ready_ui_with_snapshot(foreign_source);
    assert_ne!(foreign_ui.view().focus_panel_id(), Some(prior_tab.panel()));
    assert_eq!(
        foreign_ui.activate_drawn_presentation(&prior_tab),
        SessionActionDisposition::KeepNavigator
    );
    assert!(foreign_mailbox.try_recv().unwrap().is_none());
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
        ratio: Default::default(),
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
        super::super::tests::move_membership::ready_ui_with_snapshot(source);
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
    let (_, hits, painted_panes) = painted_selector(&ui, 60, 12, None);
    assert_eq!(painted_panes, 3);
    let mut buttons = 0;
    let mut small_header = false;
    for y in 0..12 {
        for x in 0..60 {
            match hits.hit_target_at(x, y) {
                Some(HitTarget::Presentation(DrawnPresentationTarget::Attached {
                    pane, ..
                })) => {
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

#[cfg(windows)]
#[test]
fn passive_member_read_tracks_child_attachment_and_never_borrows_primary_frame() {
    use orchbee_workbench_protocol::*;
    let source = child_source(false);
    let (mut ui, mailbox, _, _) =
        super::super::tests::move_membership::ready_ui_with_snapshot(source.clone());
    let primary_key =
        super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    let panel_id = primary_key.pane().panel().clone();
    let command = ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 9900)).unwrap(),
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 9901)).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: source.revision,
        intent: ClientViewIntent::Presentation {
            panel_id: panel_id.clone(),
        },
    };
    let inactive_panel = ui.state.as_ref().unwrap().cache().tabs()[1]
        .panel(0)
        .unwrap()
        .panel_id()
        .clone();
    let mut inactive = command.clone();
    inactive.intent = ClientViewIntent::Presentation {
        panel_id: inactive_panel,
    };
    assert!(
        ui.read_client_presentation(&inactive, &source)
            .unwrap()
            .presentation
            .is_none()
    );
    let primary = ui
        .read_client_presentation(&command, &source)
        .unwrap()
        .presentation
        .unwrap();
    assert!(matches!(
        primary.viewport(),
        Some(ClientViewportObservation::Observed { .. })
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    // The public state transition installs a new binding only after its existing cleanup owner.
    let state = ui.state.as_mut().unwrap();
    let (index, panel, child_id) = state.next_presentation(true).unwrap();
    let deadlines = OperationDeadlines::from_sample(std::time::Instant::now()).unwrap();
    let target = state
        .target_for_presentation(index, &panel, &child_id, 80, deadlines)
        .unwrap();
    let marker = target.selected().key().anchor().clone();
    assert!(state.begin_switch(target));
    let prepared = state
        .prepare_membership_candidate(
            state.cache().clone(),
            Some(marker.tab_id()),
            state.active().map(|selected| selected.index()),
            None,
            None,
        )
        .unwrap();
    let cleanup = state
        .stage_switch_membership_cleanup_k(&marker, prepared, deadlines)
        .unwrap();
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    state
        .prepare_cleaned_membership_install(
            &cleanup,
            prepared,
            Some(AttachmentKey::new(81, marker.tab_id().clone())),
        )
        .unwrap()
        .commit();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(2)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(2), ControllerMode::ReadOnly, None));
    let child = ui
        .read_client_presentation(&command, &source)
        .unwrap()
        .presentation
        .unwrap();
    assert_eq!(child.presentation_id(), &child_id);
    assert_eq!(
        child.binding().unwrap().terminal_id.as_str(),
        format!("{:032x}", 9002)
    );
    assert_eq!(
        serde_json::to_value(&child).unwrap()["attachmentGeneration"],
        "81"
    );
    assert!(!primary.same_member(&child));
    assert_eq!(
        child.viewport(),
        Some(&ClientViewportObservation::Unavailable {
            reason: ClientViewportUnavailable::FrameUnavailable
        })
    );
    // A stale primary projection still exists, but it is never relabeled as this child.
    assert!(ui.projections.iter().any(|p| p.key == primary_key));
    super::super::tests::viewport::install_projection(&mut ui, Some((17, false, 24)), 0);
    let installed = ui
        .read_client_presentation(&command, &source)
        .unwrap()
        .presentation
        .unwrap();
    assert!(child.same_member(&installed));
    assert!(
        matches!(installed.viewport(), Some(ClientViewportObservation::Observed { scroll, .. }) if scroll.max_offset_from_bottom == 17)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    let mut rebound = source.clone();
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut rebound.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    child_presentations.clear();
    assert_eq!(
        ui.read_client_presentation(&command, &rebound),
        Err(ClientViewControlReason::ProjectionUnavailable)
    );
    let mut missing = command;
    missing.intent = ClientViewIntent::Presentation {
        panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 9999)).unwrap(),
    };
    assert_eq!(
        ui.read_client_presentation(&missing, &source),
        Err(ClientViewControlReason::TargetUnavailable)
    );
}

fn empty_source() -> orchbee_workbench_protocol::Snapshot {
    let mut source = child_source(false);
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    primary_presentation.binding = None;
    child_presentations[0].binding = None;
    source
}

fn empty_ui() -> (
    NativeUi,
    Arc<CommandMailbox>,
    Arc<std::sync::atomic::AtomicU64>,
) {
    use crate::terminal_session::mailbox::EventDeadline;
    let elapsed = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let clock =
        crate::local_client::tests::server::adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let source = empty_source();
    let cache =
        crate::terminal_session::workbench_projection::project_native_standalone_tabs(&source)
            .unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache,
                selection: None,
            },
            EventDeadline::Startup(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    (ui, mailbox, elapsed)
}

#[test]
fn unbound_bootstrap_paints_empty_guidance_and_clickable_member_selector_without_viewer() {
    use crate::terminal_session::DrawnPresentationTarget;
    use crate::tui_pointer::{HitTarget, PointerState};
    let (mut ui, mailbox, _) = empty_ui();
    assert!(ui.state.as_ref().unwrap().invariant_holds());
    assert!(ui.pointer_probe().panes.is_empty());
    assert_eq!(ui.pointer_probe().unbound.len(), 1);
    assert_eq!(
        ui.control_observation(false).unwrap().1.phase,
        orchbee_workbench_protocol::ClientViewPhase::Ready
    );
    let (buffer, hits, painted) = painted_selector(&ui, 120, 40, None);
    assert_eq!(painted, 0);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(
        text.contains("Empty terminal slot") && text.contains("[1/3]"),
        "{text}"
    );
    assert!(
        crate::localization::Locale::ZhCn
            .native_phase_status(&ui.view())
            .contains("空终端位")
    );
    let (x, y, target) = (0..40)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .find_map(|(x, y)| match hits.hit_target_at(x, y) {
            Some(HitTarget::Presentation(DrawnPresentationTarget::Unbound {
                presentation: id,
                ..
            })) => Some((x, y, id.clone())),
            _ => None,
        })
        .unwrap();
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(hits);
    assert_eq!(
        activate_painted_presentation(&mut pointer, &mut ui, x, y),
        SessionActionDisposition::DismissNavigator
    );
    assert!(matches!(
        mailbox.try_recv().unwrap().unwrap().record(),
        crate::terminal_session::mailbox::QueuedCommand::Switch(_)
    ));
    let view = ui.view();
    let panel = view.tab(0).unwrap().panel(0).unwrap();
    assert_eq!(view.selected_presentation(panel).presentation_id(), &target);
}

#[test]
fn unbound_session_schedules_bounded_refresh_without_terminal_poll_or_repeated_admission() {
    let (mut ui, mailbox, elapsed) = empty_ui();
    assert!(mailbox.try_recv().unwrap().is_none());
    elapsed.store(999, std::sync::atomic::Ordering::SeqCst);
    ui.tick();
    assert!(mailbox.try_recv().unwrap().is_none());
    elapsed.store(1000, std::sync::atomic::Ordering::SeqCst);
    ui.tick();
    let queued = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(
        queued.record(),
        crate::terminal_session::mailbox::QueuedCommand::Refresh(_)
    ));
    ui.tick();
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(ui.pointer_probe().panes.is_empty());
}

#[test]
fn passive_empty_member_read_keeps_identity_and_never_exports_a_pending_switch() {
    use orchbee_workbench_protocol::*;
    let source = empty_source();
    let (mut ui, mailbox, _) = empty_ui();
    let state = ui.state.as_ref().unwrap();
    let panel = state.focus_panel_id().unwrap().clone();
    let command = ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 9200)).unwrap(),
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 9201)).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: source.revision,
        intent: ClientViewIntent::Presentation {
            panel_id: panel.clone(),
        },
    };
    let cursors = mailbox.cursors();
    let first = ui
        .read_client_presentation(&command, &source)
        .unwrap()
        .presentation
        .unwrap();
    let ClientPresentationSelection::Unbound {
        presentation_id,
        selection_generation,
    } = &first
    else {
        panic!("empty member")
    };
    assert_eq!(
        presentation_id,
        state
            .selected_presentation(state.tab(0).unwrap().panel(0).unwrap())
            .presentation_id()
    );
    assert_eq!(
        selection_generation.value(),
        state.desired().unwrap().anchor().generation()
    );
    assert_eq!(mailbox.cursors(), cursors);
    assert!(mailbox.try_recv().unwrap().is_none());

    let mut rebound = source.clone();
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &mut rebound.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    primary_presentation.binding = child_presentations[1].binding.take();
    assert_eq!(
        ui.read_client_presentation(&command, &rebound),
        Err(ClientViewControlReason::ProjectionUnavailable)
    );
    let mut moved = source.clone();
    moved.terminal_contexts[0].tabs[0].tab_id =
        WorkbenchTabId::parse(&format!("{:032x}", 9300)).unwrap();
    assert_eq!(
        ui.read_client_presentation(&command, &moved),
        Err(ClientViewControlReason::ProjectionUnavailable)
    );

    ui.apply_client_action(ClientAction::NextPresentation);
    assert!(
        ui.read_client_presentation(&command, &source)
            .unwrap()
            .presentation
            .is_none()
    );
    let state = ui.state.as_mut().unwrap();
    let marker = state.desired().unwrap().clone();
    let prepared = state
        .prepare_switch_candidate(
            &marker,
            WorkbenchSnapshotOutcome::Available {
                snapshot: source.clone(),
            },
        )
        .unwrap();
    state
        .prepare_switch_membership_install(
            &marker,
            prepared,
            Some(AttachmentKey::new(
                marker.anchor().generation() + 1,
                marker.anchor().tab_id().clone(),
            )),
        )
        .unwrap()
        .commit();
    let child = ui
        .read_client_presentation(&command, &source)
        .unwrap()
        .presentation
        .unwrap();
    assert!(matches!(child, ClientPresentationSelection::Unbound { .. }));
    assert!(!child.same_member(&first));
    assert!(ui.state.as_ref().unwrap().focus_pane_ref().is_none());
}

#[test]
fn unbound_panel_focus_exits_board_without_attaching_a_terminal() {
    use crate::terminal_session::client_control::{ControlStep, StepProgress};
    use orchbee_workbench_protocol::{ClientViewControl, ClientViewIntent, ClientViewSequence};
    let (mut ui, mailbox, _) = empty_ui();
    let (_, before) = ui.control_observation(false).unwrap();
    let selection = before.selection.clone().unwrap();
    let command = ClientViewControl {
        operation_id: serde_json::from_value(serde_json::json!(format!("{:032x}", 91))).unwrap(),
        client_id: serde_json::from_value(serde_json::json!(format!("{:032x}", 92))).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: before.workbench_revision,
        intent: ClientViewIntent::Panel {
            terminal_context_id: selection.terminal_context_id,
            tab_id: selection.tab_id.unwrap(),
            panel_id: selection.panel_id.unwrap(),
        },
    };
    ui.board.open();
    assert!(ui.control_dialog());
    assert_eq!(ui.validate_panel_focus(&command, &empty_source()), Ok(()));
    assert_eq!(ui.dismiss_focus_modes(&command), Ok(true));
    assert!(!ui.control_dialog());
    assert!(matches!(
        ui.control_step(&command, &mut StepProgress::default()),
        ControlStep::Observed
    ));
    assert_eq!(ui.finish_focus(&command), Ok(()));
    assert_eq!(ui.control_observation(false).unwrap().1, before);
    assert!(ui.state.as_ref().unwrap().selected_is_unbound());
    assert!(mailbox.try_recv().unwrap().is_none());
}
