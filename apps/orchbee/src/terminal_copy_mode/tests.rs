use super::*;
use serde_json::json;

fn id(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}

fn surface(start: u64, revision: u64) -> Surface {
    Surface::checked(TerminalScreen::Primary, id(7), revision, 8, 40, start, 4).unwrap()
}

fn point(row: u32, col: u16) -> TerminalTextPoint {
    TerminalTextPoint { row, col }
}

fn mode() -> Mode {
    Mode::new(EpisodeId::new(id(3)), surface(20, 11), point(22, 2)).unwrap()
}

#[test]
fn movement_coalesces_to_the_latest_endpoint_across_viewport_and_selection_flights() {
    let mut copy = mode();
    copy.toggle_selection().unwrap();
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::Start(point(22, 2))))
    );
    assert_eq!(copy.view().status, CopyModeStatus::Selecting);

    copy.request_local_target(point(23, 3)).unwrap();
    copy.request_local_target(point(30, 4)).unwrap();
    assert_eq!(copy.desired(), point(30, 4));
    assert_eq!(copy.take_effect(), None);

    copy.selection_settled(true).unwrap();
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Navigate(Navigation::Reveal(point(30, 4))))
    );
    copy.request_local_target(point(31, 5)).unwrap();
    copy.viewport_settled(surface(29, 12)).unwrap();
    assert_eq!(copy.cursor(), point(31, 5));
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::Update(point(31, 5))))
    );
}

#[test]
fn viewport_settlement_reissues_navigation_for_a_newer_off_window_endpoint() {
    let mut copy = mode();
    copy.request_local_target(point(10, 2)).unwrap();
    assert_eq!(copy.view().cursor, None);
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Navigate(Navigation::Reveal(point(10, 2))))
    );
    copy.request_local_target(point(5, 3)).unwrap();
    copy.viewport_settled(surface(8, 12)).unwrap();
    assert_eq!(copy.desired(), point(5, 3));
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Navigate(Navigation::Reveal(point(5, 3))))
    );
}

#[test]
fn viewport_settlement_rejects_a_resize_instead_of_reinterpreting_the_cursor() {
    let mut copy = mode();
    copy.request_local_target(point(10, 2)).unwrap();
    copy.take_effect();
    let resized = Surface::checked(TerminalScreen::Primary, id(7), 12, 8, 40, 8, 5).unwrap();
    assert_eq!(copy.viewport_settled(resized), Err(Refusal::Invalid));
}

#[test]
fn final_retained_row_remains_an_explicit_copy_navigation_target() {
    let mut copy = mode();
    copy.request_local_target(point(39, 1)).unwrap();
    assert_eq!(copy.view().cursor, None);
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Navigate(Navigation::Reveal(point(39, 1))))
    );
}

#[test]
fn search_editing_is_grapheme_atomic_bounded_and_empty_submit_reads_nothing() {
    let mut copy = mode();
    copy.begin_search(CopySearchDirection::Forward).unwrap();
    copy.edit_search(SearchEdit::Insert("a👨‍👩‍👧b")).unwrap();
    copy.edit_search(SearchEdit::Left).unwrap();
    copy.edit_search(SearchEdit::Backspace).unwrap();
    let view = copy.view();
    let search = view.search.unwrap();
    assert_eq!(search.draft, "ab");
    assert_eq!(search.cursor, 1);

    assert_eq!(
        copy.edit_search(SearchEdit::Insert("\u{1b}[31m")),
        Err(Refusal::Limit)
    );
    assert_eq!(copy.view().search.unwrap().draft, "ab");

    let too_large = "x".repeat(MAXIMUM_SEARCH_SCALARS);
    assert_eq!(
        copy.edit_search(SearchEdit::Insert(&too_large)),
        Err(Refusal::Limit)
    );
    assert_eq!(copy.view().search.unwrap().draft, "ab");

    copy.edit_search(SearchEdit::Backspace).unwrap();
    copy.edit_search(SearchEdit::Delete).unwrap();
    assert!(!copy.submit_search().unwrap());
    assert_eq!(copy.take_effect(), None);
}

#[test]
fn stale_query_and_clipboard_completions_cannot_change_an_episode() {
    let mut copy = mode();
    copy.request_motion(CopyMotion::NextWordStart).unwrap();
    let Effect::Motion { identity, .. } = copy.take_effect().unwrap() else {
        panic!("motion effect")
    };
    let stale = QueryId::new(id(99));
    assert_ne!(stale, identity);
    assert_eq!(
        copy.query_motion_completed(stale, point(23, 0)),
        Err(Refusal::Invalid)
    );
    assert_eq!(copy.cursor(), point(22, 2));
    copy.query_motion_completed(identity, point(23, 0)).unwrap();
    assert_eq!(copy.cursor(), point(23, 0));

    copy.toggle_selection().unwrap();
    copy.take_effect();
    copy.selection_settled(true).unwrap();
    copy.request_copy(true).unwrap();
    copy.take_effect();
    copy.export_ready(id(9)).unwrap();
    assert!(!copy.clipboard_not_delivered(id(8)));
    assert_eq!(copy.clipboard_identity(), Some(id(9)));
    assert!(copy.clipboard_not_delivered(id(9)));
    assert_eq!(copy.view().status, CopyModeStatus::Ready);
}

#[test]
fn revision_rebases_primary_cursor_by_local_row_and_drops_search_match() {
    let mut copy = mode();
    let next = surface(23, 12);
    let (cursor, desired) = copy.revision_points(next).unwrap();
    copy.revision(next, cursor, desired).unwrap();
    assert_eq!(copy.cursor(), point(25, 2));
    assert_eq!(copy.desired(), point(25, 2));
}

fn complete_search(copy: &mut Mode, expected: CopySearchDirection) {
    let Effect::Search {
        identity,
        query,
        direction,
        ..
    } = copy.take_effect().unwrap()
    else {
        panic!("search effect")
    };
    assert_eq!(direction, expected);
    copy.query_search_completed(
        identity,
        query,
        CopySearchResult {
            matches: vec![crate::terminal_copy::TerminalTextRange {
                start: point(22, 2),
                end: point(22, 3),
            }],
            total: 1,
            current: Some(0),
            current_global: Some(0),
        },
    )
    .unwrap();
}

#[test]
fn repeat_search_keeps_the_submitted_base_direction_across_n_and_shift_n() {
    let mut copy = mode();
    copy.begin_search(CopySearchDirection::Forward).unwrap();
    copy.edit_search(SearchEdit::Insert("foo")).unwrap();
    assert!(copy.submit_search().unwrap());
    complete_search(&mut copy, CopySearchDirection::Forward);

    copy.repeat_search(true).unwrap();
    complete_search(&mut copy, CopySearchDirection::Backward);
    copy.repeat_search(true).unwrap();
    complete_search(&mut copy, CopySearchDirection::Backward);
    copy.repeat_search(false).unwrap();
    let Effect::Search { direction, .. } = copy.take_effect().unwrap() else {
        panic!("forward repeat search effect")
    };
    assert_eq!(direction, CopySearchDirection::Forward);
}

#[test]
fn cancelling_a_selection_drives_the_latest_endpoint_after_host_settlement() {
    let mut copy = mode();
    copy.toggle_selection().unwrap();
    copy.take_effect();
    copy.selection_settled(true).unwrap();
    copy.toggle_selection().unwrap();
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::Cancel))
    );
    copy.request_local_target(point(23, 4)).unwrap();
    copy.selection_settled(false).unwrap();
    assert_eq!(copy.cursor(), point(23, 4));
    assert_eq!(copy.take_effect(), None);
}

#[test]
fn retained_export_failure_toggles_the_owned_selection_off_instead_of_restarting_it() {
    let mut copy = mode();
    copy.toggle_selection().unwrap();
    copy.take_effect();
    copy.selection_settled(true).unwrap();
    copy.request_copy(true).unwrap();
    assert!(matches!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::End(_)))
    ));
    assert!(copy.export_failed(true));

    copy.toggle_selection().unwrap();
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::Cancel))
    );
}

#[test]
fn no_selection_failure_drops_the_marker_so_the_next_toggle_starts_fresh() {
    let mut copy = mode();
    copy.toggle_selection().unwrap();
    copy.take_effect();
    copy.selection_settled(true).unwrap();
    copy.request_copy(false).unwrap();
    copy.take_effect();
    assert!(copy.export_failed(false));

    copy.toggle_selection().unwrap();
    assert!(matches!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::Start(_)))
    ));
}

#[test]
fn revision_rebase_updates_a_settled_host_selection_endpoint() {
    let mut copy = mode();
    copy.toggle_selection().unwrap();
    copy.take_effect();
    copy.selection_settled(true).unwrap();
    let next = surface(23, 12);
    let (cursor, desired) = copy.revision_points(next).unwrap();
    copy.revision(next, cursor, desired).unwrap();
    assert_eq!(copy.cursor(), point(25, 2));
    assert_eq!(
        copy.take_effect(),
        Some(Effect::Selection(SelectionEffect::Update(point(25, 2))))
    );
}

#[derive(Clone, Copy, Debug)]
enum TraceOp {
    Target { row: u8, col: u8 },
    Toggle,
    Settle,
}

impl TraceOp {
    fn json(self) -> serde_json::Value {
        match self {
            Self::Target { row, col } => json!({"target":{"row":row,"col":col}}),
            Self::Toggle => json!("toggle"),
            Self::Settle => json!("settle"),
        }
    }
}

#[derive(Clone, Copy)]
struct Oracle {
    cursor: TerminalTextPoint,
    desired: TerminalTextPoint,
    selecting: bool,
    selection_flight: bool,
}

/// An intentionally small independent oracle for the coalescing rule. It has
/// no effects queue and does not call production transition helpers.
fn replay(trace: &[TraceOp]) -> Result<(), String> {
    let mut actual = mode();
    let mut oracle = Oracle {
        cursor: point(22, 2),
        desired: point(22, 2),
        selecting: false,
        selection_flight: false,
    };
    for op in trace {
        match *op {
            TraceOp::Target { row, col } => {
                let target = point(20 + u32::from(row % 4), u16::from(col % 8));
                actual.request_local_target(target).unwrap();
                oracle.desired = target;
                if !oracle.selection_flight && oracle.cursor != target {
                    oracle.cursor = target;
                    oracle.selection_flight = oracle.selecting;
                }
            }
            TraceOp::Toggle if !oracle.selection_flight => {
                actual.toggle_selection().unwrap();
                oracle.selecting = !oracle.selecting;
                oracle.selection_flight = true;
            }
            TraceOp::Toggle => {}
            TraceOp::Settle if oracle.selection_flight => {
                actual.selection_settled(oracle.selecting).unwrap();
                oracle.selection_flight = false;
                if oracle.cursor != oracle.desired {
                    oracle.cursor = oracle.desired;
                    oracle.selection_flight = oracle.selecting;
                }
            }
            TraceOp::Settle => {}
        }
        let _ = actual.take_effect();
        let observed = (
            actual.cursor(),
            actual.desired(),
            actual.view().selecting,
            matches!(actual.flight, Flight::Selection),
        );
        let expected = (
            oracle.cursor,
            oracle.desired,
            oracle.selecting,
            oracle.selection_flight,
        );
        if observed != expected {
            return Err(format!("observed={observed:?} expected={expected:?}"));
        }
    }
    Ok(())
}

#[test]
fn reduced_cancel_trace_drives_the_latest_endpoint_in_the_independent_oracle() {
    replay(&[
        TraceOp::Toggle,
        TraceOp::Settle,
        TraceOp::Toggle,
        TraceOp::Target { row: 147, col: 8 },
        TraceOp::Settle,
    ])
    .unwrap();
}

#[test]
fn reduced_noop_target_trace_does_not_invent_a_selection_flight() {
    replay(&[
        TraceOp::Toggle,
        TraceOp::Target { row: 246, col: 192 },
        TraceOp::Settle,
        TraceOp::Settle,
        TraceOp::Target { row: 238, col: 152 },
    ])
    .unwrap();
}

fn shrink(mut trace: Vec<TraceOp>) -> Vec<TraceOp> {
    let mut index = 0;
    while index < trace.len() {
        let mut candidate = trace.clone();
        candidate.remove(index);
        if replay(&candidate).is_err() {
            trace = candidate;
        } else {
            index += 1;
        }
    }
    trace
}

#[test]
fn generated_coalescing_traces_match_an_independent_oracle_and_serialize_on_failure() {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for _case in 0..128 {
        let mut trace = Vec::new();
        for _step in 0..96 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            trace.push(match state % 5 {
                0 => TraceOp::Toggle,
                1 => TraceOp::Settle,
                _ => TraceOp::Target {
                    row: (state >> 8) as u8,
                    col: (state >> 24) as u8,
                },
            });
        }
        if let Err(error) = replay(&trace) {
            let minimal = shrink(trace);
            let serialized = serde_json::to_string(
                &minimal
                    .iter()
                    .copied()
                    .map(TraceOp::json)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            panic!("{error}; replay={serialized}");
        }
    }
}
