use super::*;

#[test]
fn hover_updates_the_candidate_without_confirmation_or_a_new_row_window() {
    let mut menu = menu();
    let token = menu.view().unwrap().token;
    assert!(
        !menu.hover(token, 1),
        "an undrawn menu has no hover authority"
    );
    present(&mut menu, 0, 2);
    assert!(menu.hover(token, 1));
    let view = menu.view().unwrap();
    assert_eq!((view.selected, view.first, view.token), (1, 0, token));
    assert!(
        !view.follow_selection,
        "hover preserves the scrolled window"
    );
    assert_eq!(menu.confirmed(), None);
    assert!(!menu.confirm(), "Enter awaits the painted candidate");
    assert!(!menu.hover(token, 1), "same-row movement is inert");

    // Multiple motion reports can arrive before the next successful frame.
    assert!(menu.hover(token, 0));
    assert!(menu.hover(token, 1));
    present(&mut menu, 0, 2);
    assert!(menu.confirm());
    assert!(
        !menu.hover(token, 0),
        "a transferring action cannot be retargeted"
    );
    assert_eq!(menu.take(), Some(MenuAction::Detach));
}

#[test]
fn hover_rejects_unpainted_disabled_stale_and_recovery_rows() {
    let mut menu = menu();
    let old = present(&mut menu, 0, 1);
    assert!(!menu.hover(old.token, 1), "clipped rows are not candidates");
    menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(MenuUnavailable::OperationBusy)
    });
    let current = present(&mut menu, 0, 2);
    assert!(!menu.hover(old.token, 0), "replaced sources cannot hover");
    assert!(!menu.hover(current.token, 1), "disabled rows remain inert");
    assert_eq!(menu.view().unwrap().selected, 0);
    menu.not_presented();
    assert!(!menu.hover(current.token, 1));
    assert_eq!(menu.confirmed(), None);
}

#[test]
fn hover_preserves_a_scrolled_viewport_and_keyboard_resumes_from_its_candidate() {
    let mut menu = ActionMenu::default();
    menu.open("Actions".into(), vec![MenuAction::Help; 8], None);
    let draw = present(&mut menu, 0, 3);
    let token = menu.scroll(draw.token, false, 3).unwrap();
    present(&mut menu, 3, 3);
    assert!(menu.hover(token, 4));
    assert_eq!(menu.view().unwrap().first, 3);
    assert_eq!(menu.view().unwrap().first_visible_row(3), 3);
    assert_eq!(menu.view().unwrap().selected, 4);
    assert_eq!(menu.confirmed(), None);
    menu.select(MenuSelection::Next);
    let view = menu.view().unwrap();
    assert_eq!((view.selected, view.first), (5, 3));
    assert!(view.follow_selection);
    assert_ne!(view.token, token);
    assert!(
        !menu.hover(token, 3),
        "keyboard navigation revokes old row authority"
    );
}
