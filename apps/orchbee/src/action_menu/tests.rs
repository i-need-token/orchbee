use super::*;

fn menu() -> ActionMenu {
    let mut menu = ActionMenu::default();
    assert!(menu.open(
        "Global".into(),
        vec![MenuAction::Help, MenuAction::Detach],
        None
    ));
    menu
}

#[test]
fn hover_preserves_painted_pointer_rows_but_requires_painted_keyboard_focus() {
    let mut menu = ActionMenu::default();
    assert!(menu.open("Actions".into(), GLOBAL_ACTIONS.to_vec(), None));
    let token = menu.view().unwrap().token;
    menu.hover(token, 1);
    assert_eq!(
        menu.view().unwrap().selected,
        0,
        "undrawn motion is ignored"
    );
    menu.draw(Some(MenuDraw {
        token,
        rows: 3,
        selected: 0,
    }));
    for row in [3, usize::MAX] {
        menu.hover(token, row);
        assert_eq!(menu.view().unwrap().selected, 0, "hidden or invalid row");
    }
    for row in [2, 1, 2] {
        menu.hover(token, row);
        assert_eq!(menu.view().unwrap().selected, row);
        assert_eq!(menu.view().unwrap().first, 0);
        assert_eq!(
            menu.view().unwrap().token,
            token,
            "hover preserves row authority"
        );
        assert_eq!(menu.confirmed(), None);
        assert!(!menu.confirm());
    }
    menu.draw(Some(MenuDraw {
        token,
        rows: 3,
        selected: 0,
    }));
    assert!(
        !menu.confirm(),
        "an old draw cannot arm the hovered selection"
    );
    menu.draw(Some(MenuDraw {
        token,
        rows: 3,
        selected: 2,
    }));
    assert!(menu.confirm());
    menu.hover(token, 1);
    assert_eq!(menu.take(), Some(GLOBAL_ACTIONS[2]));
    assert_eq!(menu.take(), None);
}

#[test]
fn hover_respects_disabled_rows_keyboard_scrolling_and_episode_retirement() {
    let mut menu = menu();
    menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(MenuUnavailable::OperationBusy)
    });
    let old = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: old,
        rows: 2,
    }));
    menu.hover(old, 1);
    let selected = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: selected,
        rows: 2,
    }));
    assert!(!menu.confirm());
    assert!(!menu.click(selected, 1));
    assert!(menu.active());
    menu.select(MenuSelection::First);
    menu.hover(selected, 1);
    assert_eq!(
        menu.view().unwrap().selected,
        0,
        "keyboard retires the hover map"
    );
    let current = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: current,
        rows: 1,
    }));
    menu.hover(current, 1);
    assert_eq!(menu.view().unwrap().selected, 0);
    menu.refresh_availability(|_| None);
    menu.hover(current, 1);
    assert_eq!(menu.view().unwrap().selected, 0);
    menu.cancel();
    assert!(menu.open("Reopened".into(), TAB_ACTIONS.to_vec(), None));
    let reopened = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: reopened,
        rows: 3,
    }));
    menu.hover(current, 1);
    assert_eq!(menu.view().unwrap().selected, 0);
    assert_eq!(menu.confirmed(), None);
}

#[test]
fn ux_menu_disabled_activation_selects_a_reason_without_confirmation() {
    let mut menu = menu();
    assert!(menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(MenuUnavailable::OperationBusy)
    }));
    let token = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token,
        rows: 2,
    }));
    assert!(!menu.click(token, 1));
    let view = menu.view().unwrap();
    assert_eq!(view.selected, 1);
    assert_eq!(view.unavailable[1], Some(MenuUnavailable::OperationBusy));
    assert_ne!(view.token, token);
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: view.token,
        rows: 2,
    }));
    assert!(!menu.confirm());
    assert_eq!(menu.take(), None);
    assert!(menu.active());
    assert!(menu.refresh_availability(|_| None));
    assert!(!menu.confirm(), "a changed prerequisite needs a fresh draw");
    let token = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token,
        rows: 2,
    }));
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Detach));
}

#[test]
fn activation_requires_current_successful_draw_and_is_taken_once() {
    let mut menu = menu();
    assert!(!menu.confirm());
    let token = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token,
        rows: 1,
    }));
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Help));
    assert_eq!(menu.take(), None);
    assert!(!menu.active());
}

#[test]
fn selection_revokes_old_receipt_and_scrolls_without_wrapping() {
    let mut menu = menu();
    let old = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: old,
        rows: 1,
    }));
    menu.select(MenuSelection::Next);
    assert_eq!(menu.view().unwrap().selected, 1);
    assert_eq!(menu.view().unwrap().first, 1);
    assert!(!menu.confirm());
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: old,
        rows: 1,
    }));
    assert!(!menu.confirm());
    let current = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: current,
        rows: 1,
    }));
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Detach));
}

#[test]
fn ux_menu_resize_commits_the_painted_window_and_rejects_hidden_clicks() {
    let mut menu = ActionMenu::default();
    assert!(menu.open("Global".into(), GLOBAL_ACTIONS.to_vec(), None));
    let last = GLOBAL_ACTIONS.len() - 1;
    let total = GLOBAL_ACTIONS.len();
    let old = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: old,
        rows: total,
    }));
    menu.select(MenuSelection::Last);
    let current = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: current,
        rows: 3,
    }));
    assert_eq!(menu.view().unwrap().selected, last);
    assert_eq!(menu.view().unwrap().first, total - 3);
    assert!(!menu.click(old, last));
    assert!(!menu.click(current, 0), "unpainted rows cannot activate");
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: current,
        rows: total,
    }));
    assert_eq!(menu.view().unwrap().first, 0);
    assert_eq!(menu.view().unwrap().selected, last);
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token: current,
        rows: 3,
    }));
    assert!(
        menu.click(current, last),
        "the painted last row can activate"
    );
    assert_eq!(menu.take(), Some(MenuAction::Detach));
}

#[test]
fn old_episode_draw_cannot_arm_reopened_menu_and_unpresentable_closes() {
    let mut menu = menu();
    let token = menu.view().unwrap().token;
    menu.cancel();
    assert!(menu.open("Global".into(), vec![MenuAction::Help], None));
    menu.draw(Some(MenuDraw {
        selected: menu.view().unwrap().selected,
        token,
        rows: 1,
    }));
    assert!(!menu.confirm());
    menu.draw(None);
    assert!(!menu.active());
    assert_eq!(menu.take(), None);
}

#[test]
fn generated_sequences_never_take_unconfirmed_or_cancelled_intent() {
    // Reopen and stale draws exercise episodes, with a directly replayable minimized witness.
    for seed in 0..256u64 {
        let mut random = seed + 1;
        let mut steps = Vec::new();
        for _ in 0..80 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            steps.push(((random >> 32) % 12) as u8);
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
                        rows: 1,
                    };
                    old = Some(draw);
                    menu.draw(Some(draw));
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
                    expected = Some(view.actions[view.selected]);
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
            8 | 9 => {
                disabled = *step == 8;
                menu.refresh_availability(|_| disabled.then_some(MenuUnavailable::OperationBusy));
                if !menu.active() {
                    expected = None;
                }
            }
            10 | 11 => {
                if let Some(draw) = old {
                    let confirmed = menu.confirmed();
                    menu.hover(draw.token, usize::from(*step - 10));
                    if menu.confirmed() != confirmed {
                        return Err("hover confirmed an action");
                    }
                }
            }
            _ => {
                if let Some(draw) = old {
                    menu.draw(Some(draw));
                }
            }
        }
    }
    Ok(())
}
