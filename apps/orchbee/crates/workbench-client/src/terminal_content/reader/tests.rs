use orchbee_terminal_protocol::{TerminalRowPromptClass, TerminalTextWindowRow};

use super::{
    Error, MAXIMUM_BYTES, MAXIMUM_GLYPHS, MAXIMUM_ROWS, paginate, valid_row_range, validate_join,
};
use crate::terminal_copy::{
    CopySearchCase, CopySearchDirection, SearchScan, TerminalTextPoint, TerminalTextRange,
};

#[test]
fn row_bound_is_per_call_and_preserves_the_u32_point_domain() {
    let maximum = MAXIMUM_ROWS as u64;
    assert!(valid_row_range(maximum * 2, &(0..maximum)));
    assert!(valid_row_range(maximum * 2, &(maximum..maximum * 2)));
    assert!(!valid_row_range(maximum + 1, &(0..maximum + 1)));
    assert!(!valid_row_range(maximum, &(0..0)));
    assert!(!valid_row_range(maximum, &(maximum..maximum + 1)));
    let beyond_point_domain = u64::from(u32::MAX) + 2;
    assert!(!valid_row_range(
        beyond_point_domain,
        &(beyond_point_domain - 1..beyond_point_domain),
    ));
}

#[test]
fn independently_read_chunks_require_an_exact_wrap_join() {
    let prior = row(2, true, false);
    let matching = row(3, false, true);
    assert!(validate_join::<()>(std::slice::from_ref(&prior), &[matching]).is_ok());
    let mismatched = row(3, false, false);
    assert!(matches!(
        validate_join::<()>(&[prior], &[mismatched]),
        Err(Error::ProtocolInvalid)
    ));
}

fn row(index: u64, wrap: bool, wrap_continuation: bool) -> TerminalTextWindowRow {
    TerminalTextWindowRow {
        index,
        prompt_class: TerminalRowPromptClass::None,
        wrap,
        wrap_continuation,
        identity_ascii: true,
        text: "x".into(),
        mappings: Vec::new(),
    }
}

#[test]
fn a_scan_past_every_read_bound_covers_the_whole_buffer_in_bounded_pages() {
    // 70,000 rows of 16 columns: past 65,536 rows and past 1,048,576 glyphs.
    const TOTAL: u64 = 70_000;
    const COLUMNS: usize = 16;
    let text = |index: u64| {
        if index == 0 || index == TOTAL - 1 {
            format!("needle{}", "x".repeat(COLUMNS - 6))
        } else {
            "x".repeat(COLUMNS)
        }
    };
    let window = |start: u64, count: u16| {
        let rows = (start..start + u64::from(count))
            .map(|index| {
                let mut row = row(index, false, false);
                row.text = text(index).into();
                row
            })
            .collect::<Vec<_>>();
        let payload = rows.iter().map(|row| 12 + row.text.len()).sum();
        Ok::<_, Error<()>>((rows, payload))
    };
    let scan = |query, case| {
        let mut scan = SearchScan::new(
            COLUMNS as u16,
            query,
            case,
            CopySearchDirection::Forward,
            TerminalTextPoint { row: 0, col: 0 },
            None,
        )
        .unwrap();
        let mut pages = Vec::new();
        paginate(0..TOTAL, 4096, window, |rows| {
            let glyphs = rows.iter().map(|row| row.text.len()).sum::<usize>();
            pages.push((rows.len(), glyphs));
            scan.page(rows).map_err(|_| Error::<()>::LimitExceeded)
        })
        .unwrap();
        (scan.finish().unwrap(), pages)
    };
    let (found, pages) = scan("NEEDLE", CopySearchCase::Insensitive);
    let point = |row, col| TerminalTextPoint { row, col };
    assert_eq!(found.total, 2);
    assert_eq!(
        found.matches,
        vec![
            TerminalTextRange {
                start: point(0, 0),
                end: point(0, 5)
            },
            TerminalTextRange {
                start: point(69_999, 0),
                end: point(69_999, 5)
            },
        ]
    );
    assert!(pages.len() > 1);
    assert_eq!(
        pages.iter().map(|page| page.0).sum::<usize>(),
        TOTAL as usize
    );
    assert!(
        pages
            .iter()
            .all(|&(rows, glyphs)| rows <= MAXIMUM_ROWS && glyphs <= MAXIMUM_GLYPHS)
    );
    let (dense, _) = scan("x", CopySearchCase::Sensitive);
    assert_eq!(
        dense.total,
        (TOTAL - 2) * COLUMNS as u64 + 2 * (COLUMNS as u64 - 6)
    );
}

#[test]
fn a_page_closes_before_a_window_would_pass_the_payload_bound() {
    let window = |start: u64, count: u16| {
        let rows = (start..start + u64::from(count))
            .map(|index| row(index, false, false))
            .collect::<Vec<_>>();
        Ok::<_, Error<()>>((rows, MAXIMUM_BYTES / 3))
    };
    let mut pages = Vec::new();
    paginate(0..10, 1, window, |rows| {
        pages.push(rows.len());
        Ok::<_, Error<()>>(())
    })
    .unwrap();
    assert_eq!(pages, vec![3, 3, 3, 1]);
}
