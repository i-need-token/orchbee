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
fn activation_requires_current_successful_draw_and_is_taken_once() {
    let mut menu = menu();
    assert!(!menu.confirm());
    let token = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw { token, rows: 1 }));
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
        token: old,
        rows: 1,
    }));
    menu.select(MenuSelection::Next);
    assert_eq!(menu.view().unwrap().selected, 1);
    assert_eq!(menu.view().unwrap().first, 1);
    assert!(!menu.confirm());
    menu.draw(Some(MenuDraw {
        token: old,
        rows: 1,
    }));
    assert!(!menu.confirm());
    let current = menu.view().unwrap().token;
    menu.draw(Some(MenuDraw {
        token: current,
        rows: 1,
    }));
    assert!(menu.confirm());
    assert_eq!(menu.take(), Some(MenuAction::Detach));
}

#[test]
fn old_episode_draw_cannot_arm_reopened_menu_and_unpresentable_closes() {
    let mut menu = menu();
    let token = menu.view().unwrap().token;
    menu.cancel();
    assert!(menu.open("Global".into(), vec![MenuAction::Help], None));
    menu.draw(Some(MenuDraw { token, rows: 1 }));
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
            steps.push(((random >> 32) % 8) as u8);
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
    for step in steps {
        match step {
            0 => {
                if let Some(view) = menu.view() {
                    old = Some(view.token);
                    menu.draw(Some(MenuDraw {
                        token: view.token,
                        rows: 1,
                    }));
                }
            }
            1 => menu.select(MenuSelection::Next),
            2 => menu.select(MenuSelection::Previous),
            3 => {
                if menu.confirm() {
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
                }
            }
            _ => {
                if let Some(token) = old {
                    menu.draw(Some(MenuDraw { token, rows: 1 }));
                }
            }
        }
    }
    Ok(())
}
