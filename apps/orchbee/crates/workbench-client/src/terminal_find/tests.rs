use super::*;
use crate::terminal_copy::TerminalTextPoint;

fn range(row: u32) -> TerminalTextRange {
    TerminalTextRange {
        start: TerminalTextPoint { row, col: 0 },
        end: TerminalTextPoint { row, col: 2 },
    }
}

fn result(rows: &[u32], current: usize, first: u64, total: u64) -> CopySearchResult {
    CopySearchResult {
        matches: rows.iter().copied().map(range).collect(),
        total,
        current: Some(current),
        current_global: Some(first + current as u64),
    }
}

fn running(bar: &mut FindBar, now: Instant) -> FindRead {
    bar.take_due(now + SEARCH_DELAY)
        .unwrap()
        .expect("a typed query searches once its delay passes")
}

#[test]
fn typing_waits_for_the_delay_and_a_newer_edit_restarts_it() {
    let now = Instant::now();
    let mut bar = FindBar::new(&FindMemory::default(), now);
    assert_eq!(bar.edit(SearchEdit::Insert("er"), now), Ok(true));
    assert_eq!(bar.view().counter, FindCounter::Searching);
    let later = now + SEARCH_DELAY / 2;
    assert_eq!(bar.take_due(later), Ok(None));
    assert_eq!(bar.edit(SearchEdit::Insert("r"), later), Ok(true));
    assert_eq!(bar.take_due(now + SEARCH_DELAY), Ok(None));
    let read = running(&mut bar, later);
    assert_eq!(&*read.text, "err");
    assert_eq!(read.case, CopySearchCase::Insensitive);
    assert_eq!(read.direction, CopySearchDirection::Backward);
    assert_eq!(read.previous, None);
    assert_eq!(bar.edit(SearchEdit::Backspace, now), Ok(true));
    assert!(!bar.settle(read.query, &result(&[1], 0, 0, 1)));
}

#[test]
fn next_and_previous_wrap_and_read_past_the_retained_window() {
    let now = Instant::now();
    let mut bar = FindBar::new(&FindMemory::default(), now);
    bar.edit(SearchEdit::Insert("x"), now).unwrap();
    let read = running(&mut bar, now);
    assert!(bar.settle(read.query, &result(&[3, 7, 9], 2, 0, 3)));
    assert_eq!(
        bar.view().counter,
        FindCounter::Position {
            current: 3,
            total: 3
        }
    );
    assert_eq!(bar.step(true, now), Ok(FindStep::Moved));
    assert_eq!(bar.matches().unwrap().position(), 1);
    assert_eq!(bar.step(false, now), Ok(FindStep::Moved));
    assert_eq!(bar.matches().unwrap().position(), 3);

    bar.toggle_match_case(now);
    let read = running(&mut bar, now);
    assert_eq!(read.case, CopySearchCase::Sensitive);
    assert!(bar.settle(read.query, &result(&[3, 7], 1, 10, 20)));
    let Ok(FindStep::Read(read)) = bar.step(true, now) else {
        panic!("stepping out of the window reads from the active match");
    };
    assert_eq!(read.direction, CopySearchDirection::Forward);
    assert_eq!(read.previous, Some(range(7)));
}
