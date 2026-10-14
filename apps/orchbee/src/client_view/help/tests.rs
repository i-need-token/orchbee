use super::*;

#[test]
fn a_new_episode_rejects_the_previous_frames_controls() {
    let mut help = State::default();
    assert!(help.open());
    let old = help.candidate().presented(0, 2).unwrap();
    help.settle(Some(old), true);
    assert_eq!(
        help.activate(&DrawnHelpControl::new(old, HelpControl::Next)),
        Some(Activation::Page(1))
    );

    assert!(help.open());
    assert_eq!(
        help.activate(&DrawnHelpControl::new(old, HelpControl::Close)),
        None
    );
}

#[test]
fn presented_pages_clamp_exactly_and_page_changes_revoke_the_proof() {
    let mut help = State::default();
    assert!(help.open());
    help.request_page(8, 20);
    let candidate = help.candidate();
    assert!(candidate.presented(3, 3).is_some());
    assert!(candidate.presented(2, 3).is_none());

    let source = candidate.presented(3, 3).unwrap();
    help.settle(Some(source), true);
    assert_eq!(help.page(), 3);
    assert_eq!(help.scroll(&source, false), None);
    assert_eq!(help.scroll(&source, true), Some(2));
    help.request_page(2, 20);
    assert_eq!(help.scroll(&source, true), None);
}

#[test]
fn refused_frames_authorize_only_their_exact_close() {
    let mut help = State::default();
    assert!(help.open());
    help.request_page(2, 20);
    let source = help.candidate().refused();
    help.settle(Some(source), true);
    assert_eq!(
        help.activate(&DrawnHelpControl::new(source, HelpControl::Close)),
        Some(Activation::Close)
    );
    assert_eq!(
        help.activate(&DrawnHelpControl::new(source, HelpControl::Previous)),
        None
    );
    assert_eq!(help.scroll(&source, true), None);
    help.settle(None, true);
    assert_eq!(
        help.activate(&DrawnHelpControl::new(source, HelpControl::Close)),
        None
    );
}
