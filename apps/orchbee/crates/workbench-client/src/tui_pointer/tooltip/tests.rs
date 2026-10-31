use super::*;
use crate::tui_hit_map::TooltipContent;
use crate::tui_hit_map::{HitLayer, HitMapBuilder, HitTarget};
use crate::tui_pointer::HelpControl;
use ratatui::layout::Rect;

const ROW: Rect = Rect::new(0, 4, 20, 1);
const CLOSE: Rect = Rect::new(16, 4, 3, 1);
const NEXT: Rect = Rect::new(0, 5, 20, 1);

fn target(action: HelpControl) -> HitTarget {
    HitTarget::Help { page: 1, action }
}

fn content(label: &str) -> TooltipContent {
    TooltipContent::Action {
        label: label.to_owned(),
        key: None,
    }
}

/// A row with a close child recorded before it, and an unrelated row below, all with tooltips.
fn regions(rows: &[(Rect, HelpControl)]) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    for (rect, action) in rows {
        hits.record(HitLayer::Chrome, *rect, target(*action));
        hits.record_tooltip(HitLayer::Chrome, *rect, target(*action), content("tip"));
    }
    hits.finish()
}

fn sidebar() -> FrameHitMap {
    regions(&[
        (CLOSE, HelpControl::Close),
        (ROW, HelpControl::Previous),
        (NEXT, HelpControl::Next),
    ])
}

fn shown(state: &State) -> Option<(Rect, (u16, u16))> {
    state
        .projection()
        .map(|shown| (shown.key.rect(), shown.pointer))
}

fn ms(start: Instant, millis: u64) -> Instant {
    start.checked_add(Duration::from_millis(millis)).unwrap()
}

#[test]
fn only_a_rest_after_real_motion_opens_and_the_box_stays_while_the_pointer_moves_within() {
    let map = sidebar();
    let start = Instant::now();
    let mut state = State::default();
    state.motion(Some(&map), (2, 4), start);
    assert!(!state.tick(Some(&map), ms(start, 299), true));
    assert_eq!(shown(&state), None);
    assert!(state.tick(Some(&map), ms(start, 300), true));
    assert_eq!(shown(&state), Some((ROW, (2, 4))));

    // Movement within the target keeps the box where it opened.
    state.motion(Some(&map), (9, 4), ms(start, 400));
    assert!(!state.tick(Some(&map), ms(start, 800), true));
    assert_eq!(shown(&state), Some((ROW, (2, 4))));

    // Nothing opens while the screen belongs to another owner, and the rest does not survive it.
    let mut state = State::default();
    state.motion(Some(&map), (2, 4), start);
    assert!(!state.tick(Some(&map), ms(start, 300), false));
    assert!(!state.tick(Some(&map), ms(start, 600), true));
    assert_eq!(shown(&state), None);

    // A rest spent over dead space is consumed: content that later moves under the still pointer
    // never opens a tooltip.
    let mut state = State::default();
    let empty = regions(&[]);
    state.motion(Some(&empty), (2, 4), start);
    assert!(!state.tick(Some(&empty), ms(start, 300), true));
    state.publish(&map);
    assert!(!state.tick(Some(&map), ms(start, 900), true));
    assert_eq!(shown(&state), None);
}

#[test]
fn leaving_a_target_opens_the_next_at_once_but_a_press_quiets_it_until_the_pointer_leaves() {
    let map = sidebar();
    let start = Instant::now();
    let mut state = State::default();
    state.motion(Some(&map), (2, 4), start);
    state.tick(Some(&map), ms(start, 300), true);

    // The adjacent close opens at once, from where the pointer arrived.
    state.motion(Some(&map), (17, 4), ms(start, 310));
    assert_eq!(shown(&state), Some((CLOSE, (17, 4))));
    // Off every target, the grace runs out after 400 ms.
    state.motion(Some(&map), (30, 9), ms(start, 320));
    assert_eq!(shown(&state), None);
    assert!(state.tick(Some(&map), ms(start, 730), true));
    state.motion(Some(&map), (2, 5), ms(start, 731));
    assert_eq!(shown(&state), None);
    assert!(state.tick(Some(&map), ms(start, 1031), true));
    assert_eq!(shown(&state), Some((NEXT, (2, 5))));

    // A press closes it and keeps that target quiet, however long the pointer rests there.
    state.press(Some(&map), (2, 5));
    assert_eq!(shown(&state), None);
    state.motion(Some(&map), (3, 5), ms(start, 1100));
    // The pending redraw removes the closed box; nothing reopens.
    assert!(state.tick(Some(&map), ms(start, 2000), true));
    assert!(!state.tick(Some(&map), ms(start, 2100), true));
    assert_eq!(shown(&state), None);
    // Leaving and returning restores it.
    state.motion(Some(&map), (3, 4), ms(start, 2001));
    state.motion(Some(&map), (3, 5), ms(start, 2002));
    state.tick(Some(&map), ms(start, 2302), true);
    assert_eq!(shown(&state), Some((NEXT, (3, 5))));

    // A key closes it without warming the next target.
    state.dismiss();
    state.motion(Some(&map), (3, 4), ms(start, 2310));
    assert_eq!(shown(&state), None);
}

#[test]
fn a_published_frame_closes_a_moved_target_but_retargets_what_the_same_arrival_reveals() {
    let start = Instant::now();
    let mut state = State::default();
    let map = sidebar();
    state.motion(Some(&map), (2, 4), start);
    state.tick(Some(&map), ms(start, 300), true);
    state.publish(&map);
    assert!(!state.take_redraw_after_publish());

    // The row moved down a line beneath the still pointer.
    let moved = regions(&[
        (Rect::new(16, 5, 3, 1), HelpControl::Close),
        (NEXT, HelpControl::Previous),
    ]);
    state.publish(&moved);
    assert!(state.take_redraw_after_publish());
    assert_eq!(shown(&state), None);

    // Arriving on the row reveals its close beneath the pointer in the next frame.
    let mut state = State::default();
    let unrevealed = regions(&[(ROW, HelpControl::Previous)]);
    state.motion(Some(&unrevealed), (2, 4), start);
    state.tick(Some(&unrevealed), ms(start, 300), true);
    state.motion(Some(&unrevealed), (17, 4), ms(start, 350));
    assert_eq!(shown(&state), Some((ROW, (2, 4))));
    state.publish(&map);
    assert_eq!(shown(&state), Some((CLOSE, (17, 4))));
    assert!(state.take_redraw_after_publish());

    // Losing focus, capture or any-motion reporting forgets the pointer altogether.
    state.clear_transport();
    assert_eq!(shown(&state), None);
    state.publish(&map);
    assert!(state.tick(Some(&map), ms(start, 2000), true));
    assert!(!state.tick(Some(&map), ms(start, 2400), true));
    assert_eq!(shown(&state), None);
}

#[test]
fn a_pane_title_offers_one_over_its_divider_or_dead_frame_but_never_under_chrome() {
    use crate::workbench_state::{AttachmentKey, PaneKey, PaneRef};
    use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
    let pane = PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(0, WorkbenchTabId::parse(&format!("{:032x}", 10)).unwrap()),
            WorkbenchPanelId::parse(&format!("{:032x}", 11)).unwrap(),
        ),
        0,
    );
    // A divider line carries one title; another title sits on the frame above, over no region.
    let divider = Rect::new(0, 4, 40, 1);
    let title = Rect::new(1, 4, 12, 1);
    let framed = Rect::new(1, 0, 12, 1);
    let chrome = Rect::new(8, 4, 3, 1);
    let mut hits = HitMapBuilder::new();
    hits.record(HitLayer::Chrome, chrome, target(HelpControl::Close));
    hits.record_pane_title_tooltip(title, pane.clone(), content("title"));
    hits.record_pane_title_tooltip(framed, pane.clone(), content("frame"));
    hits.record(HitLayer::Foreground, divider, target(HelpControl::Next));
    let map = hits.finish();
    // Pressing a title still reaches the divider beneath it.
    assert_eq!(map.hit_target_at(2, 4), Some(&target(HelpControl::Next)));
    let at = |column, row| map.tooltip_at(column, row);
    assert_eq!(at(2, 4), Some(Key::pane_title(title, pane.clone())));
    assert_eq!(at(2, 0), Some(Key::pane_title(framed, pane)));
    // Chrome in front occludes it, and the divider beyond the text offers none.
    assert_eq!(at(9, 4), None);
    assert_eq!(at(20, 4), None);
}
