use super::*;
use crate::{
    client_view::{ClientKey, ClientView},
    localization::Locale,
    terminal_input::{InputDispatcher, PointerProbe, TerminalInputPort, TerminalInputUnavailable},
    tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{io, time::Instant};

/// The Server's acceptance of a set takes every member out of the strip, and in the tick that
/// applies it the selection leaves a selected member for the next Tab outside the set.
#[test]
fn an_accepted_set_leaves_the_strip_and_moves_the_selection_in_the_same_tick() {
    use crate::workbench_state::{CloseOperationKind, CloseResult};
    use orchbee_workbench_protocol::{
        TabSetCloseAction, TabSetCloseMember, TabSetCloseOutcome, TabSetCloseStatus, WorkbenchLabel,
    };
    let (mut ui, mailbox, clock, snapshot) = ready_snapshot(grouped());
    let ids: Vec<_> = snapshot.terminal_contexts[0]
        .tabs
        .iter()
        .map(|tab| tab.tab_id.clone())
        .collect();
    let members = ids[..2].to_vec();
    assert_eq!(
        ui.state.as_ref().unwrap().view().active_tab_id(),
        Some(&ids[0])
    );
    assert!(ui.request_close_set(&members).is_some());
    draw(&mut ui, Locale::EnUs, 120, 40, None);
    ui.capture_confirmation_key(ClientKey::Enter);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Close(marker) = lease.record() else {
        panic!("the set Start")
    };
    let operation = marker.operation().clone();
    let CloseOperationKind::TabSet(TabSetCloseAction::Start {
        terminal_context_id,
        condition,
        tab_ids,
        ..
    }) = operation.kind().clone()
    else {
        panic!("several Tabs close as one Server operation")
    };
    assert!(members.iter().all(|tab| !ui.view().tab_leaving(tab)));
    assert_eq!(
        mailbox.mark_close_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let accepted = CloseResult::TabSet(TabSetCloseOutcome::Available {
        terminal_context_id,
        condition,
        status: TabSetCloseStatus::Pending {},
        members: tab_ids
            .into_iter()
            .map(|tab_id| TabSetCloseMember {
                tab_id,
                label: WorkbenchLabel::parse("member").unwrap(),
                close: None,
                absent: false,
            })
            .collect(),
    });
    let wait = mailbox
        .publish_close_result_before(operation.clone(), accepted, operation.deadline(), &clock)
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(members.iter().all(|tab| ui.view().tab_leaving(tab)));
    let mut switched = None;
    while let Some(lease) = mailbox.try_recv().unwrap() {
        if let QueuedCommand::Switch(marker) = lease.record() {
            switched = Some(marker.target().selected().key().tab_id().clone());
            break;
        }
        mailbox.finish_lease(lease).unwrap();
    }
    assert_eq!(switched.as_ref(), Some(&ids[2]));
}

struct Port<'a> {
    ui: &'a mut WorkbenchClientUi,
    forwarded: &'a mut Vec<u8>,
}
impl TerminalInputPort for Port<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        self.ui.capture_claims()
    }
    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        self.ui.capture_context(owner)
    }
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        self.ui.pointer_probe()
    }
    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        Some(self.ui.apply_client_action(command))
    }
    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::TabClose { .. }
            | SurfaceInput::PanelClose { .. }
            | SurfaceInput::WorkspaceClose { .. }
            | SurfaceInput::ActionMenu { .. } => self.ui.surface_input(input),
            _ => false,
        }
    }
    fn open_action_menu(
        &mut self,
        target: MenuTarget,
        anchor: Option<crate::action_menu::MenuAnchor>,
        released: bool,
    ) -> bool {
        self.ui.open_action_menu(target, anchor, released)
    }
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{MenuAction, SurfaceAction};
        match action {
            SurfaceAction::ActionMenu(MenuAction::Row { .. })
            | SurfaceAction::ActionMenu(MenuAction::Control { .. }) => {
                self.ui.surface_action(action)
            }
            _ => None,
        }
    }
    fn scroll_list(
        &mut self,
        target: &crate::tui_hit_map::ListScrollTarget,
        up: bool,
        lines: usize,
    ) -> Option<crate::tui_hit_map::ListScrollTarget> {
        let &crate::tui_hit_map::ListScrollTarget::Menu(token) = target else {
            return None;
        };
        self.ui
            .scroll_action_menu(token, up, lines)
            .map(crate::tui_hit_map::ListScrollTarget::Menu)
    }
    fn take_action_menu_action(
        &mut self,
        released: bool,
    ) -> Option<crate::workbench_client::InputAction> {
        self.ui.take_action_menu_action(released)
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut WorkbenchClientUi,
    forwarded: &mut Vec<u8>,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            &mut Port { ui, forwarded },
            Instant::now(),
            |_, _| Ok::<_, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

fn draw(
    ui: &mut WorkbenchClientUi,
    locale: Locale,
    width: u16,
    height: u16,
    capture: Option<&str>,
) -> (String, FrameHitMap) {
    draw_themed(
        ui,
        locale,
        width,
        height,
        capture,
        &Theme::terminal_native(),
    )
}

/// The presented dialog's frame, read from its corner glyphs.
fn dialog_area(buffer: &ratatui::buffer::Buffer) -> Rect {
    let area = buffer.area;
    let symbol = |x, y| buffer[(x, y)].symbol();
    let (left, top) = area
        .positions()
        .map(|position| (position.x, position.y))
        .find(|(x, y)| matches!(symbol(*x, *y), "┌" | "╭"))
        .unwrap();
    let right = (left + 1..area.right())
        .find(|x| matches!(symbol(*x, top), "┐" | "╮"))
        .unwrap();
    let bottom = (top + 1..area.bottom())
        .find(|y| matches!(symbol(left, *y), "└" | "╰"))
        .unwrap();
    Rect::new(left, top, right - left + 1, bottom - top + 1)
}

fn draw_themed(
    ui: &mut WorkbenchClientUi,
    locale: Locale,
    width: u16,
    height: u16,
    capture: Option<&str>,
    theme: &Theme,
) -> (String, FrameHitMap) {
    let (screen, hits, _) = draw_frame(ui, locale, width, height, capture, theme);
    (screen, hits)
}

fn draw_frame(
    ui: &mut WorkbenchClientUi,
    locale: Locale,
    width: u16,
    height: u16,
    capture: Option<&str>,
    theme: &Theme,
) -> (String, FrameHitMap, ratatui::buffer::Buffer) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = None;
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            receipt = Some(crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(width),
                locale,
                Some(ui.view()),
                theme,
                &mut hits,
            ));
        })
        .unwrap();
    if let Some(name) = capture {
        crate::tui_render::test_capture::capture_screen(&mut terminal, name, locale, theme);
    }
    let text = rendered_text(terminal.backend().buffer());
    ui.after_close_draw(receipt.unwrap());
    (text, hits.finish(), terminal.backend().buffer().clone())
}

#[test]
fn close_set_maximum_names_are_bounded_previews_with_full_warning_and_frozen_targets() {
    use crate::terminal_text::{display_width, sanitize_terminal_text, truncate_display};
    use crate::workbench_state::{CloseOperationKind, TabSelectionChange};
    use orchbee_workbench_protocol::TabSetCloseAction;
    let names = [
        "A".repeat(512),
        "界".repeat(512),
        "e\u{301}👩\u{200d}💻".chars().cycle().take(512).collect(),
        "last captured Tab".to_owned(),
    ];
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24)] {
            for (capture, theme) in [
                (true, Theme::terminal_native()),
                (false, Theme::rgb_test_dark()),
            ] {
                let mut snapshot = super::super::super::tests::membership::snapshot(4, 1);
                for (tab, name) in snapshot.terminal_contexts[0].tabs.iter_mut().zip(&names) {
                    tab.label.clone_from(name);
                }
                snapshot.terminal_contexts[0].tabs[0].pinned = true;
                let ids: Vec<_> = snapshot.terminal_contexts[0]
                    .tabs
                    .iter()
                    .map(|tab| tab.tab_id.clone())
                    .collect();
                let (mut ui, mailbox, _, _) = ready_snapshot(snapshot.clone());
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                ui.state
                    .as_mut()
                    .unwrap()
                    .mark_tabs(&ids[0], TabSelectionChange::All)
                    .unwrap();
                ui.apply_client_action(ClientAction::TabMenu {
                    action: MenuAction::CloseSelectedTabs,
                    tab_id: Some(ids[0].clone()),
                });
                let view = ui.view();
                let confirmation = view.close_confirmation().unwrap();
                assert_eq!(confirmation.set_count(), Some(4));
                assert_eq!(
                    confirmation.captured_set_labels().unwrap(),
                    &names
                        .iter()
                        .enumerate()
                        .map(|(index, name)| (index + 1, name.clone()))
                        .collect::<Vec<_>>()
                );
                // No complete draw yet: the bounded preview does not weaken the token fence.
                assert!(ui.capture_confirmation_key(ClientKey::Enter));
                assert!(mailbox.try_recv().unwrap().is_none());
                let name = format!("{locale:?}-{width}x{height}-close-long-names");
                let (screen, _, buffer) = draw_frame(
                    &mut ui,
                    locale,
                    width,
                    height,
                    capture.then_some(name.as_str()),
                    &theme,
                );
                // Each preview truncates at the dialog's content width.
                let content = usize::from(dialog_area(&buffer).width - 6);
                for (index, label) in names.iter().take(3).enumerate() {
                    let prefix = format!("#{} · ", index + 1);
                    let preview = format!(
                        "{prefix}{}",
                        truncate_display(
                            &sanitize_terminal_text(label),
                            content - display_width(&prefix),
                            theme.glyphs.ellipsis
                        )
                    );
                    assert!(preview.ends_with(theme.glyphs.ellipsis));
                    assert!(screen.contains(&preview), "{screen}");
                    assert!(!screen.contains(label));
                }
                let compact = |text: &str| {
                    text.chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect::<String>()
                };
                for required in [
                    locale.close_set_title(4),
                    locale.close_set_more(1),
                    // No terminal here has settled, so every one counts as running.
                    locale.close_warning(crate::localization::CloseImpact {
                        running: 4,
                        ended: 0,
                    }),
                ] {
                    assert!(compact(&screen).contains(&compact(&required)), "{screen}");
                }
                assert!(!screen.contains(locale.presentation_notice(false)));
                assert!(ui.capture_confirmation_key(ClientKey::Enter));
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::Close(operation) = lease.record() else {
                    panic!("Close admission")
                };
                assert!(matches!(
                    operation.operation().kind(),
                    CloseOperationKind::TabSet(TabSetCloseAction::Start {
                        tab_ids,
                        expected_revision,
                        ..
                    }) if tab_ids == &ids && *expected_revision == snapshot.revision
                ));
                assert!(mailbox.try_recv().unwrap().is_none());
                mailbox.close(CarrierFailure::Closed);
            }
        }
    }
}

fn cell(
    hits: &FrameHitMap,
    width: u16,
    height: u16,
    predicate: impl Fn(&HitTarget) -> bool,
) -> (u16, u16) {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .find(|(x, y)| hits.hit_target_at(*x, *y).is_some_and(&predicate))
        .unwrap()
}

#[test]
fn tab_close_confirm_uses_shared_destructive_intent_without_changing_cancel_or_admission() {
    use crate::tui_theme::{ColorLevel, Role};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(80, 24), (120, 40)] {
            for base in [
                Theme::terminal_native(),
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
            ] {
                for level in [
                    ColorLevel::Truecolor,
                    ColorLevel::Ansi256,
                    ColorLevel::Ansi16,
                    ColorLevel::Monochrome,
                ] {
                    let theme = base.quantize(level);
                    let (mut ui, mailbox, _, snapshot) = ready_snapshot(grouped());
                    ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                    ui.apply_client_action(ClientAction::TabMenu {
                        action: MenuAction::CloseSelectedTabs,
                        tab_id: Some(snapshot.terminal_contexts[0].tabs[1].tab_id.clone()),
                    });
                    assert!(ui.view().close_confirmation().is_some());
                    assert!(mailbox.try_recv().unwrap().is_none());
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let mut receipt = None;
                    terminal
                        .draw(|frame| {
                            receipt = Some(crate::tui_render::render_terminal_shell(
                                frame,
                                &ClientView::new(width),
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            ));
                        })
                        .unwrap();
                    let map = hits.finish();
                    let (x, y) = cell(&map, width, height, |target| {
                        matches!(target, HitTarget::TabClose { confirm: true, .. })
                    });
                    let label_x = x + 2;
                    assert_eq!(
                        terminal.backend().buffer()[(label_x, y)].fg,
                        theme.style(Role::AccentDestructive).fg.unwrap(),
                        "Close uses the existing destructive accent, never status-error"
                    );
                    let cancel_x = (0..width)
                        .find(|candidate| {
                            matches!(
                                map.hit_target_at(*candidate, y),
                                Some(HitTarget::TabClose { confirm: false, .. })
                            )
                        })
                        .unwrap();
                    assert_eq!(
                        terminal.backend().buffer()[(cancel_x + label_x - x, y)].fg,
                        theme.style(Role::TextPrimary).fg.unwrap(),
                        "Cancel keeps its ordinary secondary-action tone"
                    );
                    ui.after_close_draw(receipt.unwrap());
                    assert!(ui.capture_confirmation_key(ClientKey::Escape));
                    assert!(ui.view().close_confirmation().is_none());
                    assert_eq!(
                        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
                        &snapshot
                    );
                    assert!(
                        mailbox.try_recv().unwrap().is_none(),
                        "painting intent cannot submit Close"
                    );
                }
            }
        }
    }
}

#[test]
fn close_set_menu_keyboard_and_pointer_routes_capture_exact_ids_and_one_drawn_confirmation() {
    use crate::workbench_state::CloseOperationKind;
    use orchbee_workbench_protocol::TabSetCloseAction;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24)] {
            for pointer in [false, true] {
                for (action, target, expected) in [
                    (MenuAction::CloseSelectedTabs, 2, vec![0, 2]),
                    (MenuAction::CloseOtherTabs, 2, vec![1]),
                    (MenuAction::CloseTrailingTabs, 0, vec![1, 2]),
                    (MenuAction::CloseGroup, 1, vec![1, 2]),
                ] {
                    let mut snapshot = grouped();
                    snapshot.terminal_contexts[0].tabs[0].pinned = true;
                    let ids: Vec<_> = snapshot.terminal_contexts[0]
                        .tabs
                        .iter()
                        .map(|tab| tab.tab_id.clone())
                        .collect();
                    let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
                    let (mut ui, mailbox, _, _) = ready_snapshot(snapshot.clone());
                    ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                    let desired = ui.state.as_ref().unwrap().desired().cloned();
                    if target == 2 {
                        ui.state
                            .as_mut()
                            .unwrap()
                            .mark_tabs(&ids[2], crate::workbench_state::TabSelectionChange::Toggle)
                            .unwrap();
                    }
                    ui.apply_client_action(ClientAction::ToggleTabGroup(group));
                    let mut dispatcher = InputDispatcher::default();
                    let mut view = ClientView::new(width);
                    let mut forwarded = Vec::new();
                    dispatcher.set_pointer_capture(
                        &mut view,
                        &mut Port {
                            ui: &mut ui,
                            forwarded: &mut forwarded,
                        },
                        true,
                    );
                    if pointer && action == MenuAction::CloseGroup {
                        let (_, hits) = draw(&mut ui, locale, width, height, None);
                        let (x, y) = cell(&hits, width, height, |hit| {
                            matches!(hit, HitTarget::TabGroup { .. })
                        });
                        dispatcher.publish_hit_map(hits);
                        dispatch(
                            &mut dispatcher,
                            &mut view,
                            &mut ui,
                            &mut forwarded,
                            format!("\x1b[<2;{};{}M\x1b[<2;{};{}m", x + 1, y + 1, x + 1, y + 1)
                                .as_bytes(),
                        );
                    } else {
                        let target = if action == MenuAction::CloseGroup {
                            MenuTarget::TabGroups {
                                tab_id: ids[target].clone(),
                                colors: false,
                            }
                        } else if action == MenuAction::CloseTrailingTabs {
                            // The global row acts on the current, first Tab.
                            MenuTarget::Global
                        } else {
                            MenuTarget::Tab(ids[target].clone())
                        };
                        assert!(ui.open_action_menu(target, None, false));
                    }
                    let row = ui
                        .view()
                        .action_menu()
                        .unwrap()
                        .actions
                        .iter()
                        .position(|candidate| *candidate == action)
                        .unwrap();
                    // A searchable menu takes each key only after the previous one is painted.
                    for _ in 0..row {
                        let (_, hits) = draw(&mut ui, locale, width, height, None);
                        dispatcher.publish_hit_map(hits);
                        dispatch(
                            &mut dispatcher,
                            &mut view,
                            &mut ui,
                            &mut forwarded,
                            b"\x1b[B",
                        );
                    }
                    let (menu, hits) = draw(&mut ui, locale, width, height, None);
                    let label = {
                        let view = ui.view();
                        let projection = view.action_menu().unwrap();
                        let scope = projection.tabs.unwrap().rows[row];
                        if projection.source.context_menu() {
                            locale.menu_object_action(&action, Some(scope))
                        } else {
                            locale.menu_tab_action(&action, scope)
                        }
                    };
                    assert!(menu.contains(&label), "{menu}");
                    let token = ui.view().action_menu().unwrap().token;
                    let (x, y) = cell(
                        &hits,
                        width,
                        height,
                        |hit| matches!(hit, HitTarget::ActionMenuRow { token: actual, row: actual_row } if *actual == token && *actual_row == row),
                    );
                    dispatcher.publish_hit_map(hits);
                    if pointer {
                        dispatch(
                            &mut dispatcher,
                            &mut view,
                            &mut ui,
                            &mut forwarded,
                            format!(
                                "\x1b[<0;{};{}M\x1b[<0;{};{}myTAIL",
                                x + 1,
                                y + 1,
                                x + 1,
                                y + 1
                            )
                            .as_bytes(),
                        );
                    } else {
                        dispatch(
                            &mut dispatcher,
                            &mut view,
                            &mut ui,
                            &mut forwarded,
                            b"\ryTAIL",
                        );
                    }
                    assert_eq!(
                        ui.view().close_confirmation().unwrap().set_count(),
                        Some(expected.len())
                    );
                    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
                    assert!(
                        mailbox.try_recv().unwrap().is_none(),
                        "a menu choice cannot authorize an unseen prompt"
                    );
                    let name = format!("{locale:?}-{width}x{height}-close-group");
                    let (prompt, hits) = draw(
                        &mut ui,
                        locale,
                        width,
                        height,
                        (pointer && action == MenuAction::CloseGroup).then_some(name.as_str()),
                    );
                    let compact = |text: &str| {
                        text.chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>()
                    };
                    assert!(
                        compact(&prompt).contains(&compact(&locale.close_warning(
                            crate::localization::CloseImpact {
                                running: expected.len(),
                                ended: 0,
                            }
                        ))),
                        "{prompt}"
                    );
                    assert!(prompt.contains(&locale.close_set_title(expected.len())));
                    dispatcher.publish_hit_map(hits);
                    dispatch(
                        &mut dispatcher,
                        &mut view,
                        &mut ui,
                        &mut forwarded,
                        b"\rTRAILING\x02N",
                    );
                    let lease = mailbox.try_recv().unwrap().unwrap();
                    let QueuedCommand::Close(operation) = lease.record() else {
                        panic!("one Close lane admission");
                    };
                    let members: Vec<_> =
                        expected.iter().map(|index| ids[*index].clone()).collect();
                    // One Tab closes as the key does; several close as one Server operation.
                    match operation.operation().kind() {
                        CloseOperationKind::Close => {
                            assert_eq!(
                                std::slice::from_ref(operation.operation().key().target().tab_id()),
                                members.as_slice()
                            );
                        }
                        CloseOperationKind::TabSet(TabSetCloseAction::Start {
                            tab_ids,
                            expected_revision,
                            ..
                        }) => {
                            assert!(members.len() > 1);
                            assert_eq!(tab_ids, &members);
                            assert_eq!(*expected_revision, snapshot.revision);
                        }
                        kind => panic!("not a Close of the captured set: {kind:?}"),
                    }
                    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
                    assert!(ui.view().close_confirmation().is_none());
                    assert!(ui.view().setup_draft().is_none());
                    assert!(
                        forwarded.is_empty(),
                        "menu and confirmation packet tails cannot reach execution"
                    );
                    assert!(mailbox.try_recv().unwrap().is_none());
                    mailbox.close(CarrierFailure::Closed);
                }
            }
        }
    }
}

#[test]
fn close_set_prompt_cancel_resize_and_small_suspension_never_submit() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for action in ["cancel", "resize", "small"] {
            let (mut ui, mailbox, _, _) = ready_snapshot(grouped());
            ui.apply_client_action(ClientAction::TabMenu {
                action: MenuAction::CloseTrailingTabs,
                tab_id: None,
            });
            let before = mailbox.cursors();
            draw(&mut ui, locale, 120, 40, None);
            let token = ui.view().close_confirmation().unwrap().token();
            if action == "resize" {
                ui.invalidate_close_display();
            }
            if action == "small" {
                draw(&mut ui, locale, 22, 7, None);
                let prompt = ui.view().close_confirmation().unwrap();
                assert_eq!(prompt.token(), token);
                assert!(!prompt.interactive());
                assert_eq!(
                    prompt.presentation(),
                    crate::tui_presentation::PresentationPhase::Suspended
                );
            }
            let mut dispatcher = InputDispatcher::default();
            let mut view = ClientView::new(120);
            let mut forwarded = Vec::new();
            dispatch(
                &mut dispatcher,
                &mut view,
                &mut ui,
                &mut forwarded,
                if action == "cancel" {
                    b"\x1b"
                } else {
                    b"\rTAIL"
                },
            );
            if action == "cancel" {
                dispatcher
                    .synchronize(
                        &mut view,
                        &mut Port {
                            ui: &mut ui,
                            forwarded: &mut forwarded,
                        },
                        Instant::now() + std::time::Duration::from_secs(1),
                        &mut |_| Ok::<_, io::Error>(()),
                    )
                    .unwrap();
            }
            assert!(forwarded.is_empty());
            assert_eq!(ui.view().close_confirmation().is_some(), action != "cancel");
            if action == "small" {
                draw(&mut ui, locale, 120, 40, None);
                let resumed = ui.view().close_confirmation().unwrap();
                assert_eq!(resumed.token(), token);
                assert!(resumed.interactive());
            }
            assert_eq!(mailbox.cursors(), before);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}
