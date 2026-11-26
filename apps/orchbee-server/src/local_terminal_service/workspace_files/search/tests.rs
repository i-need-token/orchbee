//! Paging what a search holds: the multiplex sends a stream update only when an answer differs
//! from the last, so a page must repeat exactly while nothing has come, and pages must carry every
//! entry once, in order, with the final state only after the last of them.
use super::{PAGE_BYTES, Search, next_page};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkspaceFilesOutcome as Outcome, WorkspaceSearchFile, WorkspaceSearchLine,
    WorkspaceSearchRange, WorkspaceSearchState as State,
};
use std::collections::VecDeque;

fn entry(path: &str, line: u64) -> WorkspaceSearchFile {
    WorkspaceSearchFile {
        path: path.to_owned(),
        lines: vec![WorkspaceSearchLine {
            line,
            preview_start: 0,
            preview: "needle".to_owned(),
            matches: vec![WorkspaceSearchRange { start: 0, end: 6 }],
        }],
    }
}

fn search() -> Search {
    Search {
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        lane_id: 1,
        incarnation: 1,
        held: VecDeque::new(),
        delivered: 0,
        ended: None,
        last: None,
    }
}

fn parts(page: &Outcome) -> (u64, Vec<(String, u64)>, bool, State) {
    let Outcome::SearchResults {
        from,
        files,
        more,
        state,
    } = page
    else {
        panic!("not a search page");
    };
    let files = files
        .iter()
        .map(|file| (file.path.clone(), file.lines[0].line))
        .collect();
    (*from, files, *more, *state)
}

#[test]
fn pages_repeat_while_nothing_comes_and_end_after_the_last_entry() {
    let mut search = search();
    let first = next_page(&mut search);
    assert_eq!(parts(&first), (0, vec![], false, State::Searching));
    assert_eq!(next_page(&mut search), first);

    search.held.push_back((entry("a", 1), 100));
    search.held.push_back((entry("a", 2), 100));
    let second = next_page(&mut search);
    assert_eq!(
        parts(&second),
        (
            0,
            vec![("a".into(), 1), ("a".into(), 2)],
            false,
            State::Searching
        )
    );
    assert_eq!(next_page(&mut search), second);

    // A page stops at its byte budget; the rest follows at once, the state only after it.
    search.held.push_back((entry("b", 1), PAGE_BYTES));
    search.held.push_back((entry("c", 1), 10));
    search.ended = Some(State::Complete);
    let third = next_page(&mut search);
    assert_eq!(
        parts(&third),
        (2, vec![("b".into(), 1)], true, State::Searching)
    );
    let fourth = next_page(&mut search);
    assert_eq!(
        parts(&fourth),
        (3, vec![("c".into(), 1)], false, State::Complete)
    );
    assert_eq!(next_page(&mut search), fourth);
}
