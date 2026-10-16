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

fn present(menu: &mut ActionMenu, first: usize, visible_rows: usize) -> MenuDraw {
    let view = menu.view().unwrap();
    let draw = MenuDraw {
        token: view.token,
        first,
        visible_rows,
        selected: view.selected,
    };
    assert!(menu.presented(draw));
    draw
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
    assert!(!retained.presented(draw), "a pre-cover receipt stays stale");
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
            MenuAction::Reload,
            MenuAction::Settings,
            MenuAction::Machines,
            MenuAction::Sidebar,
        ],
        None,
    ));
    let draw = present(&mut menu, 0, 3);
    let scrolled = menu.scroll(draw.token, false, 3).unwrap();
    let view = menu.view().unwrap();
    assert_eq!(view.token, scrolled);
    assert_eq!(view.selected, 0, "wheel movement never steals selection");
    assert_eq!(view.first, 3);
    assert!(!view.follow_selection);
    assert_eq!(view.first_visible_row(3), 3);

    present(&mut menu, 3, 3);
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
    assert_eq!(view.first_visible_row(3), 0, "a boundary key still reveals");
    let draw = present(&mut menu, 0, 3);
    assert!(menu.scroll(draw.token, false, 3).is_some());
    present(&mut menu, 3, 3);

    menu.select(MenuSelection::Next);
    let view = menu.view().unwrap();
    assert_eq!(view.selected, 1);
    assert!(view.follow_selection);
    let first = view.first_visible_row(3);
    assert!(first <= view.selected && view.selected < first + 3);
    present(&mut menu, first, 3);
    assert!(menu.input_context().unwrap().activation_ready());
}

#[test]
fn wheel_can_reach_a_tail_window_that_contains_more_separators() {
    let mut menu = ActionMenu::default();
    assert!(menu.open(
        "Actions".into(),
        vec![
            MenuAction::Help,
            MenuAction::Detach,
            MenuAction::Reload,
            MenuAction::DuplicateTab,
            MenuAction::PinTab,
            MenuAction::TabTools,
        ],
        None,
    ));
    let draw = present(&mut menu, 0, 3);
    let token = menu.scroll(draw.token, false, usize::MAX).unwrap();
    let view = menu.view().unwrap();
    assert_eq!(view.token, token);
    assert_eq!(view.selected, 0);
    assert_eq!(view.first_visible_row(3), 4);
    let draw = present(&mut menu, 4, 2);
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
    assert!(menu.open("Group".into(), vec![red, blue], None));
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
    let token = menu.view().unwrap().token;
    assert!(!menu.presented(MenuDraw {
        token,
        first: 0,
        visible_rows: 2,
        selected: 1,
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
    let old = MenuDraw {
        token: menu.view().unwrap().token,
        first: 0,
        visible_rows: 1,
        selected: 0,
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
                        token: view.token,
                        first: view.selected,
                        visible_rows: 1,
                    };
                    old = Some(draw);
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
                    expected = Some(view.actions[view.selected].clone());
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
                if let Some(draw) = old {
                    let _ = menu.presented(draw);
                }
            }
        }
    }
    Ok(())
}
