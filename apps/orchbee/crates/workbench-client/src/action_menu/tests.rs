use super::*;

mod hover;

fn menu() -> ActionMenu {
    let mut menu = ActionMenu::default();
    assert!(menu.open(
        "Global".into(),
        vec![MenuAction::Help, MenuAction::Detach],
        None,
    ));
    menu
}

#[test]
fn searchable_escape_cancels_before_draw_after_edit_and_after_cover() {
    let mut pending = menu();
    assert!(pending.active());
    pending.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!pending.active(), "an undrawn root menu can always exit");

    let mut edited = menu();
    present(&mut edited, 0, 2);
    edited.key(KeyEvent::press(KeyCode::Char('x'), Modifiers::NONE));
    assert_eq!(
        edited.input_context().unwrap().readiness(),
        MenuInputReadiness::Continuation
    );
    edited.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(
        !edited.active(),
        "query edits do not defer Esc to a later paint"
    );

    let mut covered = menu();
    present(&mut covered, 0, 2);
    covered.not_presented();
    assert_eq!(
        covered.input_context().unwrap().readiness(),
        MenuInputReadiness::Recovery
    );
    covered.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(
        !covered.active(),
        "an admitted but covered root menu can exit"
    );
}

#[test]
fn searchable_escape_cancels_a_confirmed_transfer_and_ignores_modified_or_release_escape() {
    let mut menu = menu();
    present(&mut menu, 0, 2);
    assert!(!menu.key(KeyEvent {
        code: KeyCode::Esc,
        mods: Modifiers::NONE,
        kind: KeyKind::Release,
    }));
    assert!(menu.active());
    assert!(!menu.key(KeyEvent::press(KeyCode::Esc, Modifiers::CTRL)));
    assert!(menu.active());

    assert!(menu.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE)));
    assert_eq!(menu.confirmed(), Some(MenuAction::Help));
    menu.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!menu.active());
    assert_eq!(menu.confirmed(), None);
}

#[test]
fn searchable_menu_edits_are_bounded_and_clear_the_query_without_navigation_leaks() {
    let mut menu = menu();
    present(&mut menu, 0, 2);
    menu.key(KeyEvent::press(KeyCode::Char('j'), Modifiers::NONE));
    assert_eq!(menu.query(), Some("j"));
    assert_eq!(menu.view().unwrap().selected, 0);

    menu.key(KeyEvent::press(KeyCode::Char('a'), Modifiers::NONE));
    menu.key(KeyEvent::press(KeyCode::Backspace, Modifiers::NONE));
    assert_eq!(menu.query(), Some("j"));
    menu.key(KeyEvent::press(KeyCode::Char('e'), Modifiers::NONE));
    menu.key(KeyEvent::press(KeyCode::Char('\u{301}'), Modifiers::NONE));
    assert_eq!(menu.query(), Some("je\u{301}"));
    menu.key(KeyEvent::press(KeyCode::Backspace, Modifiers::NONE));
    assert_eq!(menu.query(), Some("j"), "Backspace removes one grapheme");
    menu.key(KeyEvent::press(KeyCode::Char('u'), Modifiers::CTRL));
    assert_eq!(menu.query(), Some(""));
    assert_eq!(menu.view().unwrap().result_actions, [0, 1]);

    for _ in 0..MAXIMUM_ACTION_MENU_QUERY_BYTES + 1 {
        menu.key(KeyEvent::press(KeyCode::Char('x'), Modifiers::NONE));
    }
    assert_eq!(menu.query().unwrap().len(), MAXIMUM_ACTION_MENU_QUERY_BYTES);
    let token = menu.view().unwrap().token;
    menu.key(KeyEvent::press(KeyCode::Char('y'), Modifiers::NONE));
    assert_eq!(menu.view().unwrap().token, token, "overflow stays bounded");
}

#[test]
fn filtered_draw_rows_keep_their_original_action_identity_and_availability() {
    let mut menu = ActionMenu::default();
    assert!(menu.open(
        "Actions".into(),
        vec![MenuAction::Help, MenuAction::Detach],
        None,
    ));
    present(&mut menu, 0, 2);
    menu.key(KeyEvent::press(KeyCode::Char('d'), Modifiers::NONE));
    let view = menu.view().unwrap();
    let token = view.token;
    assert!(menu.presented(MenuDraw {
        token,
        first: 0,
        visible_rows: 1,
        selected: 0,
        candidate_visible: true,
        result_actions: vec![1],
    }));
    assert!(menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(MenuUnavailable::OperationBusy)
    }));
    let view = menu.view().unwrap();
    let token = view.token;
    assert!(menu.presented(MenuDraw {
        token,
        first: 0,
        visible_rows: 1,
        selected: 0,
        candidate_visible: true,
        result_actions: vec![1],
    }));
    assert!(
        !menu.confirm(),
        "filtered action stays disabled by its owner reason"
    );
    assert!(!menu.click(token, 0));
}

#[test]
fn restored_action_selection_uses_its_filtered_result_position() {
    let mut menu = ActionMenu::default();
    let actions = vec![
        MenuAction::Help,
        MenuAction::RenameTab,
        MenuAction::ClosePane,
        MenuAction::Detach,
        MenuAction::Zoom,
    ];
    assert!(menu.open("Actions".into(), actions, None));
    menu.set_query(Some("e"));
    menu.select_action(MenuAction::Zoom);
    let token = {
        let view = menu.view().unwrap();
        assert_eq!(view.pending_selected_action, Some(&MenuAction::Zoom));
        view.token
    };
    assert!(menu.presented(MenuDraw {
        token,
        first: 0,
        visible_rows: 3,
        selected: 2,
        candidate_visible: true,
        result_actions: vec![0, 2, 4],
    }));
    assert!(menu.confirm());
    assert_eq!(menu.confirmed(), Some(MenuAction::Zoom));
}

fn present(menu: &mut ActionMenu, first: usize, visible_rows: usize) -> MenuDraw {
    let view = menu.view().unwrap();
    let draw = MenuDraw {
        token: view.token,
        first,
        visible_rows,
        selected: view.selected,
        candidate_visible: !view.candidate_hidden,
        result_actions: view.result_actions.to_vec(),
    };
    assert!(menu.presented(draw.clone()));
    draw
}

fn present_in_terminal_rows(menu: &mut ActionMenu, first: usize, terminal_rows: usize) -> MenuDraw {
    // MenuDraw::visible_rows counts action results; separators consume terminal rows but are not
    // actions. Build receipts with the same result-index geometry the renderer paints.
    let view = menu.view().unwrap();
    let visible_rows = action_rows_in_terminal_rows(&view, first, terminal_rows);
    assert!(visible_rows > 0, "the window paints an action row");
    present(menu, first, visible_rows)
}

fn action_rows_in_terminal_rows(view: &MenuView<'_>, first: usize, terminal_rows: usize) -> usize {
    let mut visible_rows = 0;
    while first + visible_rows < view.result_actions.len()
        && view.result_row_height(
            view.result_actions,
            first,
            first.saturating_add(visible_rows).saturating_add(1),
        ) <= terminal_rows
    {
        visible_rows += 1;
    }
    visible_rows
}

#[test]
fn presentation_lifecycle_refuses_a_first_block_and_suspends_an_admitted_menu() {
    let mut pending = menu();
    assert_eq!(
        pending.input_context().unwrap().phase(),
        PresentationPhase::Pending
    );
    assert_eq!(
        pending.input_context().unwrap().readiness(),
        MenuInputReadiness::Recovery
    );
    assert_eq!(
        pending.geometry_blocked(),
        Some(GeometrySettlement::Refused)
    );
    assert!(!pending.active());

    let mut retained = menu();
    let draw = present(&mut retained, 0, 2);
    assert_eq!(
        retained.input_context().unwrap().phase(),
        PresentationPhase::Admitted
    );
    assert!(retained.input_context().unwrap().activation_ready());

    retained.not_presented();
    let covered = retained.input_context().unwrap();
    assert_ne!(covered.token(), draw.token);
    assert_eq!(covered.phase(), PresentationPhase::Admitted);
    assert_eq!(covered.readiness(), MenuInputReadiness::Recovery);
    assert!(
        !retained.presented(draw.clone()),
        "a pre-cover receipt stays stale"
    );
    assert_eq!(
        retained.input_context().unwrap().readiness(),
        MenuInputReadiness::Recovery
    );
    assert_eq!(
        retained.geometry_blocked(),
        Some(GeometrySettlement::Suspended)
    );
    assert!(retained.active());
    assert_eq!(
        retained.input_context().unwrap().phase(),
        PresentationPhase::Suspended
    );
    assert_eq!(
        retained.input_context().unwrap().readiness(),
        MenuInputReadiness::Recovery
    );

    present(&mut retained, 0, 2);
    assert_eq!(
        retained.input_context().unwrap().phase(),
        PresentationPhase::Admitted
    );
    assert!(retained.input_context().unwrap().activation_ready());
}

#[test]
fn wheel_moves_only_the_viewport_and_keyboard_navigation_reveals_selection() {
    let mut menu = ActionMenu::default();
    assert!(menu.open(
        "Actions".into(),
        vec![
            MenuAction::Help,
            MenuAction::Detach,
            MenuAction::Copy,
            MenuAction::Settings,
            MenuAction::Machines,
            MenuAction::Sidebar,
        ],
        None,
    ));
    let draw = present_in_terminal_rows(&mut menu, 0, 3);
    let scrolled = menu.scroll(draw.token, false, 3).unwrap();
    let view = menu.view().unwrap();
    assert_eq!(view.token, scrolled);
    assert_eq!(view.selected, 0, "wheel movement never steals selection");
    assert_eq!(view.first, 3);
    assert!(!view.follow_selection);
    let first = view.first_visible_result_row(3, view.result_actions, view.selected);
    assert_eq!(
        first, view.first,
        "the rendered window uses result-row indices"
    );
    assert!(first > 0, "wheel navigation moves the viewport");
    assert!(view.selected < first, "the old selection remains offscreen");

    present_in_terminal_rows(&mut menu, first, 3);
    assert_eq!(
        menu.input_context().unwrap().readiness(),
        MenuInputReadiness::Continuation
    );
    assert!(!menu.confirm(), "an offscreen selection cannot activate");

    let offscreen = menu.view().unwrap().token;
    menu.select(MenuSelection::First);
    let view = menu.view().unwrap();
    assert_ne!(view.token, offscreen);
    assert!(view.follow_selection);
    assert_eq!(
        view.first_visible_result_row(3, view.result_actions, view.selected),
        0,
        "a boundary key still reveals"
    );
    let draw = present_in_terminal_rows(&mut menu, 0, 3);
    assert!(menu.scroll(draw.token, false, 3).is_some());
    let view = menu.view().unwrap();
    let first = view.first_visible_result_row(3, view.result_actions, view.selected);
    present_in_terminal_rows(&mut menu, first, 3);

    menu.select(MenuSelection::Next);
    let view = menu.view().unwrap();
    assert_eq!(view.selected, 1);
    assert!(view.follow_selection);
    let first = view.first_visible_result_row(3, view.result_actions, view.selected);
    assert!(first <= view.selected && view.selected < first + 3);
    present_in_terminal_rows(&mut menu, first, 3);
    assert!(menu.input_context().unwrap().activation_ready());
}

#[test]
fn wheel_reaches_a_tail_window_using_terminal_rows_and_result_indices() {
    let mut menu = ActionMenu::default();
    assert!(menu.open(
        "Actions".into(),
        vec![
            MenuAction::Help,
            MenuAction::Detach,
            MenuAction::Machines,
            MenuAction::DuplicateTab,
            MenuAction::PinTab,
            MenuAction::Settings,
        ],
        None,
    ));
    let draw = present_in_terminal_rows(&mut menu, 0, 3);
    let token = menu.scroll(draw.token, false, usize::MAX).unwrap();
    let view = menu.view().unwrap();
    assert_eq!(view.token, token);
    assert_eq!(view.selected, 0);
    let first = view.first_visible_result_row(3, view.result_actions, view.selected);
    assert_eq!(first, view.first, "the tail view uses result-row indices");
    assert!(first > 0, "wheel navigation reaches the tail window");
    let visible = action_rows_in_terminal_rows(&view, first, 3);
    assert!(visible > 0);
    assert!(
        view.result_row_height(view.result_actions, first, first + visible) <= 3,
        "the displayed actions and separators fit the terminal-row budget"
    );
    assert_eq!(
        first + visible,
        view.result_actions.len(),
        "the wheel reaches the final result in its tail viewport"
    );
    if first > 0 {
        assert!(
            view.result_row_height(view.result_actions, first - 1, view.result_actions.len()) > 3,
            "the tail cannot reveal one more result within the same terminal-row budget"
        );
    }
    let draw = present_in_terminal_rows(&mut menu, first, 3);
    assert_eq!(
        menu.scroll(draw.token, false, usize::MAX),
        Some(draw.token),
        "wheeling at the derived tail must not churn the presentation token"
    );
}

#[test]
fn same_surface_metadata_rekeys_keep_the_continuation_lease() {
    let mut menu = menu();
    let draw = present(&mut menu, 0, 2);
    menu.set_labels(|action| (action == MenuAction::Help).then(|| "Help target".into()));
    let context = menu.input_context().unwrap();
    assert_ne!(context.token(), draw.token);
    assert_eq!(context.phase(), PresentationPhase::Admitted);
    assert_eq!(context.readiness(), MenuInputReadiness::Continuation);
    assert!(!menu.confirm());
    present(&mut menu, 0, 2);
    assert!(menu.input_context().unwrap().activation_ready());
}

#[test]
fn changed_tab_scope_or_saved_color_requires_fresh_paint_without_resetting_candidate() {
    use orchbee_workbench_protocol::TabGroupColor;
    let red = MenuAction::GroupColor(TabGroupColor::Red);
    let blue = MenuAction::GroupColor(TabGroupColor::Blue);
    let mut menu = ActionMenu::default();
    assert!(menu.open("Group".into(), vec![red.clone(), blue.clone()], None));
    let scope = TabMenuScope::Group(std::num::NonZeroUsize::new(2).unwrap());
    menu.set_tab_presentation(scope, Some(TabGroupColor::Red), |_| Some(scope));
    let first = present(&mut menu, 0, 2);
    assert!(menu.hover(first.token, 1));
    let candidate = present(&mut menu, 0, 2);
    menu.set_tab_presentation(scope, Some(TabGroupColor::Blue), |_| Some(scope));
    let view = menu.view().unwrap();
    assert_ne!(view.token, candidate.token);
    assert_eq!(view.selected, 1);
    assert!(!view.tabs.unwrap().checked(&red));
    assert!(view.tabs.unwrap().checked(&blue));
    assert_eq!(
        menu.input_context().unwrap().readiness(),
        MenuInputReadiness::Continuation
    );
    assert!(!menu.click(candidate.token, 1));
    assert!(!menu.confirm());
    let current = present(&mut menu, 0, 2);
    menu.set_tab_presentation(scope, Some(TabGroupColor::Blue), |_| Some(scope));
    assert_eq!(
        menu.view().unwrap().token,
        current.token,
        "unchanged facts preserve the exact frame"
    );
    let next_scope = TabMenuScope::Group(std::num::NonZeroUsize::new(3).unwrap());
    menu.set_tab_presentation(next_scope, Some(TabGroupColor::Blue), |_| Some(next_scope));
    assert_ne!(menu.view().unwrap().token, current.token);
    assert_eq!(menu.view().unwrap().selected, 1);
    assert!(!menu.click(current.token, 1));
    assert!(!menu.confirm());
    present(&mut menu, 0, 2);
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(blue));
}

#[test]
fn disabled_pointer_rows_neither_select_nor_confirm() {
    let mut menu = menu();
    assert!(menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(MenuUnavailable::OperationBusy)
    }));
    let draw = present(&mut menu, 0, 2);
    assert!(!menu.click(draw.token, 1));
    assert_eq!(menu.view().unwrap().selected, 0);
    assert_eq!(menu.confirmed(), None);

    menu.select(MenuSelection::Next);
    assert_eq!(menu.view().unwrap().selected, 1);
    present(&mut menu, 0, 2);
    assert!(!menu.confirm());
    assert_eq!(menu.take(), None);

    assert!(menu.refresh_availability(|_| None));
    assert!(!menu.confirm(), "availability changes revoke the old draw");
    assert_eq!(
        menu.input_context().unwrap().readiness(),
        MenuInputReadiness::Continuation
    );
    present(&mut menu, 0, 2);
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Detach));
}

#[test]
fn activation_requires_the_exact_selected_row_and_painted_window() {
    let mut menu = menu();
    let view = menu.view().unwrap();
    let token = view.token;
    let results = view.result_actions.to_vec();
    assert!(!menu.presented(MenuDraw {
        token,
        first: 0,
        visible_rows: 2,
        selected: 1,
        candidate_visible: true,
        result_actions: results,
    }));
    assert!(!menu.confirm(), "a draw for another selection is rejected");
    assert!(!menu.click(token, 2), "an unpainted row cannot activate");

    present(&mut menu, 0, 1);
    assert!(!menu.click(token, 1), "a hidden row cannot activate");
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Help));
    assert_eq!(menu.take(), None);
}

#[test]
fn selection_and_availability_revisions_retire_old_draw_receipts() {
    let mut menu = menu();
    let old = present(&mut menu, 0, 2);
    menu.select(MenuSelection::Next);
    assert_ne!(menu.view().unwrap().token, old.token);
    assert!(!menu.confirm());
    assert!(!menu.presented(old));
    assert!(!menu.confirm());
    assert_eq!(
        menu.input_context().unwrap().readiness(),
        MenuInputReadiness::Continuation,
        "a stale row receipt cannot revoke the admitted whole-surface lease"
    );

    let current = present(&mut menu, 0, 2);
    assert!(menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(MenuUnavailable::OperationBusy)
    }));
    assert_ne!(menu.view().unwrap().token, current.token);
    assert!(!menu.click(current.token, 1));
    assert!(!menu.confirm());
}

#[test]
fn an_old_episode_receipt_cannot_arm_a_reopened_menu() {
    let mut menu = menu();
    let view = menu.view().unwrap();
    let old = MenuDraw {
        token: view.token,
        first: 0,
        visible_rows: 1,
        selected: 0,
        candidate_visible: !view.candidate_hidden,
        result_actions: view.result_actions.to_vec(),
    };
    menu.cancel();
    assert!(menu.open("Reopened".into(), vec![MenuAction::Help], None));
    assert!(!menu.presented(old));
    assert!(!menu.confirm());
    assert!(menu.active());
    present(&mut menu, 0, 1);
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Help));
}

#[test]
fn generated_sequences_never_take_unconfirmed_or_cancelled_intent() {
    for seed in 0..256u64 {
        let mut random = seed + 1;
        let mut steps = Vec::new();
        for _ in 0..80 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            steps.push(((random >> 32) % 14) as u8);
        }
        if let Err(failure) = replay(&steps) {
            let mut minimal = steps;
            let mut width = minimal.len() / 2;
            while width > 0 {
                let mut start = 0;
                while start + width <= minimal.len() {
                    let mut candidate = minimal.clone();
                    candidate.drain(start..start + width);
                    if replay(&candidate) == Err(failure) {
                        minimal = candidate;
                    } else {
                        start += 1;
                    }
                }
                width /= 2;
            }
            panic!("seed={seed} failure={failure} replay={minimal:?}");
        }
    }
}

fn replay(steps: &[u8]) -> Result<(), &'static str> {
    let mut menu = menu();
    let mut expected = None;
    let mut taken = 0;
    let mut old = None;
    let mut disabled = false;
    for step in steps {
        match step {
            0 => {
                if let Some(view) = menu.view() {
                    let draw = MenuDraw {
                        selected: view.selected,
                        candidate_visible: !view.candidate_hidden,
                        token: view.token,
                        first: view.selected,
                        visible_rows: 1,
                        result_actions: view.result_actions.to_vec(),
                    };
                    old = Some(draw.clone());
                    let _ = menu.presented(draw);
                }
            }
            1 => menu.select(MenuSelection::Next),
            2 => menu.select(MenuSelection::Previous),
            3 => {
                if menu.confirm() {
                    if disabled {
                        return Err("disabled confirmation");
                    }
                    let view = menu.view().unwrap();
                    expected = view
                        .result_actions
                        .get(view.selected)
                        .and_then(|index| view.actions.get(*index))
                        .cloned();
                }
            }
            4 => {
                menu.cancel();
                expected = None;
            }
            5 => {
                if let Some(action) = menu.take() {
                    if disabled {
                        return Err("disabled handoff");
                    }
                    if expected != Some(action) {
                        return Err("unconfirmed or changed action");
                    }
                    taken += 1;
                    if taken > 1 {
                        return Err("duplicate handoff");
                    }
                    expected = None;
                }
            }
            6 => {
                if menu.open(
                    "Reopened".into(),
                    vec![MenuAction::Help, MenuAction::Detach],
                    None,
                ) {
                    expected = None;
                    taken = 0;
                    menu.refresh_availability(|_| {
                        disabled.then_some(MenuUnavailable::OperationBusy)
                    });
                }
            }
            7 => {
                menu.not_presented();
            }
            8 | 9 => {
                disabled = *step == 8;
                menu.refresh_availability(|_| disabled.then_some(MenuUnavailable::OperationBusy));
                if !menu.active() {
                    expected = None;
                }
            }
            10 => {
                if let Some(view) = menu.view() {
                    let _ = menu.scroll(view.token, false, 1);
                }
            }
            11 => {
                if menu.geometry_blocked() == Some(GeometrySettlement::Refused) {
                    expected = None;
                }
            }
            _ => {
                if let Some(draw) = old.as_ref() {
                    let _ = menu.presented(draw.clone());
                }
            }
        }
    }
    Ok(())
}

#[test]
fn menu_binding_projection_requires_the_same_typed_target_semantics() {
    use crate::action_projection::{binding_metadata, menu_metadata};

    let global = MenuTarget::Global;
    let cases = [
        (
            MenuAction::SplitPanel(PanelDirection::Up),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Up)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Down),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Down)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Left),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Left)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Right),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Right)),
        ),
        (
            MenuAction::ClosePane,
            PrefixAction::Client(ClientAction::ClosePanel),
        ),
        (
            MenuAction::Zoom,
            PrefixAction::Client(ClientAction::ToggleZoom),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Up),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Up)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Down),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Down)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Left),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Left)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Right),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Right)),
        ),
        (
            MenuAction::FindInTerminal,
            PrefixAction::Client(ClientAction::FindInTerminal),
        ),
        (
            MenuAction::Copy,
            PrefixAction::Client(ClientAction::EnterCopyMode),
        ),
        (
            MenuAction::Resize,
            PrefixAction::Client(ClientAction::EnterResizeMode),
        ),
        (
            MenuAction::RenameTab,
            PrefixAction::Client(ClientAction::RequestRenameTab),
        ),
        (
            MenuAction::Settings,
            PrefixAction::Client(ClientAction::OpenSettings),
        ),
        (MenuAction::Help, PrefixAction::ShowHelp),
        (MenuAction::Sidebar, PrefixAction::SidebarToggle),
        (MenuAction::Detach, PrefixAction::Client(ClientAction::Exit)),
    ];
    for (menu, expected) in cases {
        assert_eq!(menu_binding_action(&global, menu), Some(expected));
    }

    let tab = MenuTarget::Tab(
        orchbee_workbench_protocol::WorkbenchTabId::parse("00000000000000000000000000000001")
            .unwrap(),
    );
    assert_eq!(
        menu_metadata(MenuAction::RenameTab).action,
        binding_metadata(&PrefixAction::Client(ClientAction::RequestRenameTab)).unwrap()
    );
    assert_eq!(menu_binding_action(&tab, MenuAction::RenameTab), None);
    assert_eq!(
        menu_binding_action(&MenuTarget::Create(None), MenuAction::NewWorkspace),
        Some(PrefixAction::Client(ClientAction::RequestWorkspaceCreate))
    );
    assert_eq!(
        menu_binding_action(&MenuTarget::Create(None), MenuAction::Settings),
        None
    );
    assert_eq!(
        menu_binding_action(&tab, MenuAction::SplitPanel(PanelDirection::Up)),
        None,
        "a global current-pane shortcut is not advertised for an exact background target"
    );
}
