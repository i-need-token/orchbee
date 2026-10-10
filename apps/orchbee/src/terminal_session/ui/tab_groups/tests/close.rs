use super::*;
use crate::{
    client_view::{ClientKey, ClientView, KeyEvent},
    localization::Locale,
    terminal_input::{InputDispatcher, PointerProbe, TerminalInputPort, TerminalInputUnavailable},
    tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{io, time::Instant};

fn halted_close(
    ui: &mut NativeUi,
    target_index: usize,
    status: orchbee_workbench_protocol::WorkbenchTabCloseStatus,
) -> (
    orchbee_workbench_protocol::WorkbenchTabId,
    orchbee_workbench_protocol::WorkbenchRequestId,
) {
    use crate::terminal_session::state::{CloseLeaseAcknowledgement, CloseResult};
    use orchbee_workbench_protocol::{
        WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseReadOutcome,
    };
    let request = |id: u128| WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap();
    let now = ui.clock.now();
    let deadline = now + std::time::Duration::from_secs(50);
    let state = ui.state.as_mut().unwrap();
    let ids: Vec<_> = state
        .cache()
        .tabs()
        .iter()
        .skip(target_index)
        .map(|tab| tab.tab_id().clone())
        .collect();
    state.open_close_set(&ids).unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let start = state
        .plan_close_confirmation(request(61000), deadline)
        .unwrap();
    state.begin_close_exchange(&start, now).unwrap();
    let known = request(61001);
    state
        .settle_close_exchange(
            &start,
            &CloseResult::Accepted(crate::local_client::workbench::WorkbenchTabCloseAccepted {
                close_request_id: known.clone(),
                terminal_context_id: start.key().target().terminal_context_id().clone(),
                tab_id: ids[0].clone(),
                accepted_revision: start.selection().base_revision(),
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(start.key().clone()));
    let read = state
        .plan_close_read(request(61002), deadline, now)
        .unwrap();
    state.begin_close_exchange(&read, now).unwrap();
    state
        .settle_close_exchange(
            &read,
            &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id: known.clone(),
                terminal_context_id: start.key().target().terminal_context_id().clone(),
                tab_id: ids[0].clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(7).unwrap(),
                status,
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(read.key().clone()));
    assert!(state.close_set_status().unwrap().halted);
    (ids[0].clone(), known)
}

#[test]
fn close_set_menu_resumes_the_presented_exact_close_without_navigation_or_set_restart() {
    use crate::terminal_session::state::{CloseConfirmationKind, CloseOperationKind};
    use orchbee_workbench_protocol::{WorkbenchTabCloseBlockedReason, WorkbenchTabCloseStatus};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height, target_index) in [(120, 40, 0), (80, 24, 0), (120, 40, 1), (80, 24, 1)]
        {
            let (mut ui, mailbox, _, _) = ready_snapshot(grouped());
            let original = ui.state.as_ref().unwrap().desired().cloned();
            let (target, known) = halted_close(
                &mut ui,
                target_index,
                WorkbenchTabCloseStatus::CleanupBlocked {
                    reason: WorkbenchTabCloseBlockedReason::HostRefused,
                    resumable: true,
                },
            );
            let state = ui.state.as_ref().unwrap();
            assert!(
                !menu_actions(state, &state.cache().tabs()[target_index], None)
                    .contains(&MenuAction::ResumeTabClose)
            );
            draw_themed(
                &mut ui,
                locale,
                width,
                height,
                Some(&format!(
                    "{locale:?}-{width}x{height}-close-resume-{target_index}-status"
                )),
                &Theme::terminal_native(),
            );
            let state = ui.state.as_ref().unwrap();
            assert!(state.tab_close_resume_available(&target));
            assert!(
                !menu_actions(state, &state.cache().tabs()[2], None)
                    .contains(&MenuAction::ResumeTabClose)
            );
            assert!(ui.open_action_menu(MenuTarget::Tab(target.clone()), None, false));
            // A real menu draw must not revoke the recovery status behind its body.
            draw_themed(
                &mut ui,
                locale,
                width,
                height,
                None,
                &Theme::terminal_native(),
            );
            let view = ui.view();
            let menu = view.action_menu().unwrap();
            let row = menu
                .actions
                .iter()
                .position(|action| *action == MenuAction::ResumeTabClose)
                .unwrap();
            ui.click_action_menu(menu.token, row);
            let action = ui
                .take_action_menu_action(false)
                .expect("painted recovery menu handoff");
            ui.apply_client_action(action);
            assert_eq!(
                ui.view().close_confirmation().unwrap().kind(),
                CloseConfirmationKind::Resume
            );
            assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
            // Menu acceptance cannot substitute for the new confirmation's complete draw.
            ui.capture_confirmation_key(ClientKey::Plain('y'));
            assert!(mailbox.try_recv().unwrap().is_none());
            let (screen, _) = draw_themed(
                &mut ui,
                locale,
                width,
                height,
                Some(&format!(
                    "{locale:?}-{width}x{height}-close-resume-{target_index}-confirmation"
                )),
                &Theme::terminal_native(),
            );
            let compact = |text: &str| {
                text.chars()
                    .filter(|ch| !ch.is_whitespace() && *ch != '│')
                    .collect::<String>()
            };
            assert!(
                compact(&screen)
                    .contains(&compact(&locale.close_set_status(
                        ui.state.as_ref().unwrap().close_set_status().unwrap(),
                    ))),
                "{locale:?} {width}: {screen}"
            );
            ui.capture_confirmation_key(ClientKey::Plain('y'));
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::Close(operation) = lease.record() else {
                panic!("existing Close owner")
            };
            let operation = operation.operation();
            assert_eq!(operation.key().target().tab_id(), &target);
            assert!(
                matches!(operation.kind(), CloseOperationKind::Resume { operation, generation }
        if operation == &known && generation.value() == 7)
            );
            assert!(
                ui.state
                    .as_ref()
                    .unwrap()
                    .close_set_status()
                    .unwrap()
                    .halted
            );
            assert!(!ui.state.as_ref().unwrap().close_set_ready());
        }
    }
}

#[test]
fn close_set_menu_cannot_resume_an_unrelated_target_without_a_record_or_change_selection() {
    use orchbee_workbench_protocol::{WorkbenchTabCloseBlockedReason, WorkbenchTabCloseStatus};
    let (mut ui, mailbox, _, snapshot) = ready_snapshot(grouped());
    let original = ui.state.as_ref().unwrap().desired().cloned();
    halted_close(
        &mut ui,
        0,
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: WorkbenchTabCloseBlockedReason::HostRefused,
            resumable: true,
        },
    );
    draw_themed(
        &mut ui,
        Locale::EnUs,
        120,
        40,
        None,
        &Theme::terminal_native(),
    );
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::ResumeTabClose,
        tab_id: Some(snapshot.terminal_contexts[0].tabs[1].tab_id.clone()),
    });
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
    assert!(ui.view().close_confirmation().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn close_set_menu_never_offers_resume_for_unknown_close_facts() {
    use orchbee_workbench_protocol::{
        WorkbenchTabCloseStatus, WorkbenchTabCloseUnknownPhase, WorkbenchTabCloseUnknownReason,
    };
    let (mut ui, mailbox, _, _) = ready_snapshot(grouped());
    let (target, _) = halted_close(
        &mut ui,
        0,
        WorkbenchTabCloseStatus::OutcomeUnknown {
            phase: WorkbenchTabCloseUnknownPhase::Dispose,
            reason: WorkbenchTabCloseUnknownReason::TransportLost,
        },
    );
    draw_themed(
        &mut ui,
        Locale::ZhCn,
        120,
        40,
        None,
        &Theme::terminal_native(),
    );
    let state = ui.state.as_ref().unwrap();
    assert!(
        !menu_actions(state, &state.cache().tabs()[0], None).contains(&MenuAction::ResumeTabClose)
    );
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::ResumeTabClose,
        tab_id: Some(target),
    });
    assert!(ui.view().close_confirmation().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn close_set_keyboard_recovery_addresses_the_presented_background_member() {
    use crate::terminal_session::state::CloseOperationKind;
    use orchbee_workbench_protocol::{WorkbenchTabCloseBlockedReason, WorkbenchTabCloseStatus};
    let (mut ui, mailbox, _, _) = ready_snapshot(grouped());
    let original = ui.state.as_ref().unwrap().desired().cloned();
    let (target, known) = halted_close(
        &mut ui,
        1,
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: WorkbenchTabCloseBlockedReason::HostRefused,
            resumable: true,
        },
    );
    draw(&mut ui, Locale::EnUs, 120, 40, None);
    assert_eq!(
        ui.request_close_confirmation(),
        SessionActionDisposition::DismissNavigator
    );
    draw(&mut ui, Locale::EnUs, 120, 40, None);
    ui.capture_confirmation_key(ClientKey::Plain('y'));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Close(marker) = lease.record() else {
        panic!("existing Close owner")
    };
    assert_eq!(marker.operation().key().target().tab_id(), &target);
    assert!(
        matches!(marker.operation().kind(), CloseOperationKind::Resume { operation, .. }
        if operation == &known)
    );
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
    assert!(
        ui.state
            .as_ref()
            .unwrap()
            .close_set_status()
            .unwrap()
            .halted
    );
}

struct Port<'a> {
    ui: &'a mut NativeUi,
    forwarded: &'a mut Vec<u8>,
}
impl TerminalInputPort for Port<'_> {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        self.ui.pointer_probe()
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        Some(self.ui.apply_client_action(action))
    }
    fn capture_confirmation_key(&mut self, key: ClientKey) -> bool {
        self.ui.capture_confirmation_key(key)
    }
    fn action_menu_active(&self) -> bool {
        self.ui.action_menu_active()
    }
    fn open_action_menu(
        &mut self,
        target: MenuTarget,
        anchor: Option<(u16, u16)>,
        released: bool,
    ) -> bool {
        self.ui.open_action_menu(target, anchor, released)
    }
    fn capture_action_menu_key(&mut self, key: KeyEvent) {
        self.ui.capture_action_menu_key(key);
    }
    fn click_action_menu(&mut self, token: crate::action_menu::MenuToken, row: usize) {
        self.ui.click_action_menu(token, row);
    }
    fn take_action_menu_action(&mut self, released: bool) -> Option<ClientAction> {
        self.ui.take_action_menu_action(released)
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut NativeUi,
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
    ui: &mut NativeUi,
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

fn draw_themed(
    ui: &mut NativeUi,
    locale: Locale,
    width: u16,
    height: u16,
    capture: Option<&str>,
    theme: &Theme,
) -> (String, FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = None;
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            receipt = Some(crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(width),
                locale,
                Some(TerminalSessionView::Native(ui.view())),
                theme,
                &mut hits,
            ));
        })
        .unwrap();
    if let Some(name) = capture {
        capture_screen(terminal.backend().buffer(), name);
    }
    let text = rendered_text(terminal.backend().buffer());
    ui.after_close_draw(receipt.unwrap());
    (text, hits.finish())
}

#[test]
fn close_set_maximum_names_are_bounded_previews_with_full_warning_and_frozen_targets() {
    use crate::terminal_session::state::TabSelectionChange;
    use crate::terminal_text::{display_width, sanitize_terminal_text, truncate_display};
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
                let mut snapshot = super::super::super::membership_tests::snapshot(4, 1);
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
                    confirmation.set_labels().unwrap(),
                    &names
                        .iter()
                        .enumerate()
                        .map(|(index, name)| (index + 1, name.clone()))
                        .collect::<Vec<_>>()
                );
                // No complete draw yet: the bounded preview does not weaken the token fence.
                assert!(ui.capture_confirmation_key(ClientKey::Plain('y')));
                assert!(mailbox.try_recv().unwrap().is_none());
                let name = format!("{locale:?}-{width}x{height}-close-long-names");
                let (screen, _) = draw_themed(
                    &mut ui,
                    locale,
                    width,
                    height,
                    capture.then_some(name.as_str()),
                    &theme,
                );
                for (index, label) in names.iter().take(3).enumerate() {
                    let prefix = format!("#{} · ", index + 1);
                    let preview = format!(
                        "{prefix}{}",
                        truncate_display(
                            &sanitize_terminal_text(label),
                            64 - display_width(&prefix),
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
                    locale.close_set_warning().to_owned(),
                    locale.close_actions().to_owned(),
                ] {
                    assert!(compact(&screen).contains(&compact(&required)), "{screen}");
                }
                assert!(!screen.contains(locale.close_resize_hint()));
                assert!(ui.capture_confirmation_key(ClientKey::Plain('y')));
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::Close(operation) = lease.record() else {
                    panic!("Close admission")
                };
                assert_eq!(operation.operation().set_source(), Some(&snapshot));
                assert_eq!(operation.operation().key().target().tab_id(), &ids[0]);
                assert_eq!(ui.view().close_set_status().unwrap().total, 4);
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
fn close_set_menu_keyboard_and_pointer_routes_capture_exact_ids_and_one_drawn_confirmation() {
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
                            .mark_tabs(
                                &ids[2],
                                crate::terminal_session::state::TabSelectionChange::Toggle,
                            )
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
                    let (_, hits) = draw(&mut ui, locale, width, height, None);
                    dispatcher.publish_hit_map(hits);
                    dispatch(
                        &mut dispatcher,
                        &mut view,
                        &mut ui,
                        &mut forwarded,
                        b"\x1b[B".repeat(row).as_slice(),
                    );
                    let (menu, hits) = draw(&mut ui, locale, width, height, None);
                    assert!(menu.contains(locale.menu_action(action)), "{menu}");
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
                        compact(&prompt).contains(&compact(locale.close_set_warning())),
                        "{prompt}"
                    );
                    assert!(prompt.contains(&locale.close_set_title(expected.len())));
                    dispatcher.publish_hit_map(hits);
                    dispatch(
                        &mut dispatcher,
                        &mut view,
                        &mut ui,
                        &mut forwarded,
                        b"yTRAILING\x02N",
                    );
                    let lease = mailbox.try_recv().unwrap().unwrap();
                    let QueuedCommand::Close(operation) = lease.record() else {
                        panic!("one Close lane admission");
                    };
                    assert_eq!(
                        operation.operation().key().target().tab_id(),
                        &ids[expected[0]]
                    );
                    assert_eq!(operation.operation().set_source(), Some(&snapshot));
                    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
                    assert_eq!(ui.view().close_set_status().unwrap().total, expected.len());
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
fn close_set_prompt_resize_small_draw_and_cancellation_never_submit() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for action in ["cancel", "resize", "small"] {
            let (mut ui, mailbox, _, _) = ready_snapshot(grouped());
            ui.apply_client_action(ClientAction::TabMenu {
                action: MenuAction::CloseTrailingTabs,
                tab_id: None,
            });
            let before = mailbox.cursors();
            draw(&mut ui, locale, 120, 40, None);
            if action == "resize" {
                ui.invalidate_close_display();
            }
            if action == "small" {
                draw(&mut ui, locale, 24, 7, None);
            }
            if action == "small" {
                assert!(ui.view().close_confirmation().is_none());
            } else {
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
                        b"yTAIL"
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
                assert_eq!(ui.view().close_confirmation().is_some(), action == "resize");
            }
            assert_eq!(mailbox.cursors(), before);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}
