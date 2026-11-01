use super::*;
use crate::action_menu::{ActionMenu, MenuAction, MenuTarget, MenuToken};

fn token() -> MenuToken {
    let mut menu = ActionMenu::default();
    assert!(menu.open_for(
        MenuTarget::Global,
        String::new(),
        vec![MenuAction::Help],
        None
    ));
    menu.view().unwrap().token
}

#[test]
fn a_row_opens_after_its_rest_and_a_parent_row_waits_for_stillness() {
    let token = token();
    let row = |row| InteractionTarget::ActionMenuRow {
        source: MenuTarget::Global,
        token,
        row,
    };
    let control = |control| InteractionTarget::ActionMenuControl {
        source: MenuTarget::Global,
        token,
        control,
    };
    let parent = |row| control(MenuControl::ParentRow(row));
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut state = State::default();

    // A row opens a fixed time after the pointer reaches it, however it moves within the row.
    state.motion(Some(&row(0)), at(0));
    state.motion(Some(&row(0)), at(150));
    assert_eq!(state.due(Some(&row(0)), at(199)), None);
    assert_eq!(state.due(Some(&row(0)), at(200)), Some(row(0)));
    assert_eq!(
        state.due(Some(&row(0)), at(400)),
        None,
        "one rest acts once"
    );

    // Every movement over parent rows restarts the grace, so a path toward the submenu that
    // crosses them keeps it open; stillness returns to the row under the pointer.
    state.motion(Some(&parent(1)), at(400));
    state.motion(Some(&parent(2)), at(650));
    assert_eq!(state.due(Some(&parent(2)), at(900)), None);
    assert_eq!(state.due(Some(&parent(2)), at(950)), Some(parent(2)));

    // Reaching the submenu, the open row or anything else cancels the return.
    for other in [Some(row(3)), Some(control(MenuControl::Back)), None] {
        state.motion(Some(&parent(1)), at(1000));
        state.motion(other.as_ref(), at(1100));
        assert_eq!(state.due(Some(&parent(1)), at(2000)), None, "{other:?}");
    }

    // A frame that no longer shows the target under the pointer drops the rest.
    state.motion(Some(&parent(1)), at(2000));
    assert_eq!(state.due(Some(&parent(2)), at(3000)), None);
    assert_eq!(state.due(Some(&parent(1)), at(3000)), None);
}
