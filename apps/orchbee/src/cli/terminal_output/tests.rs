use super::*;
fn rows(items: &[&str]) -> Vec<OutputRow> {
    items
        .iter()
        .enumerate()
        .map(|(index, text)| OutputRow {
            index: 10 + index as u64,
            text: (*text).to_owned(),
            from_previous: false,
            into_next: false,
        })
        .collect()
}
fn selection(source: OutputSource, lines: Option<u16>) -> OutputSelection {
    OutputSelection { source, lines }
}

#[test]
fn output_cursor_excludes_existing_content_below_the_cursor_and_alternate_screens() {
    use orchbee_terminal_protocol::{TerminalCommandObservation, decode_terminal_text_frame};
    let mut frame = decode_terminal_text_frame(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    )))
    .unwrap();
    frame.command_observation = Some(TerminalCommandObservation {
        output_epoch: std::num::NonZeroU64::new(1).unwrap(),
        ready: true,
        command: None,
    });
    frame.alternate_screen = false;
    frame.cursor.row = 0;
    let cursor = capture_cursor(&"1".repeat(32), &frame).unwrap();
    assert!(cursor.row >= visible::last_content_row(&frame).unwrap_or(0));
    frame.alternate_screen = true;
    assert!(capture_cursor(&"1".repeat(32), &frame).is_none());
    frame.alternate_screen = false;
    frame.command_observation = None;
    assert!(capture_cursor(&"1".repeat(32), &frame).is_none());
}

#[test]
fn recent_endpoint_respects_content_cursor_empty_and_alternate_screens() {
    let visible = rows(&["a", "b", "", ""]);
    let choose = |source, lines, cursor, alternate, rows: &[OutputRow], capacity| {
        select_range(
            selection(source, lines),
            Endpoint {
                scrollback: 10,
                total: 14,
                cursor,
                alternate,
                last_content_row: rows
                    .iter()
                    .rev()
                    .find(|row| !row.text.trim().is_empty())
                    .map(|row| row.index),
                input_end: None,
            },
            capacity,
        )
    };
    let range = choose(OutputSource::Recent, Some(5), 0, false, &visible, 1000);
    assert_eq!((range.start, range.count), (7, 5));
    assert_eq!(
        choose(OutputSource::Recent, Some(5), 2, false, &visible, 1000).start,
        8
    );
    assert_eq!(
        choose(OutputSource::Recent, Some(5), 0, true, &visible, 1000).start,
        9
    );
    assert_eq!(
        choose(
            OutputSource::Recent,
            Some(5),
            0,
            false,
            &rows(&["", "", "", ""]),
            1000
        )
        .start,
        9
    );
    let capped = choose(OutputSource::Recent, None, 0, false, &visible, 3);
    assert_eq!(
        (capped.start, capped.count, capped.capacity_omitted),
        (9, 3, 9)
    );
    assert_eq!(
        choose(OutputSource::Detection, Some(1), 0, false, &visible, 1000).start,
        8
    );
    assert_eq!(
        choose(OutputSource::Visible, None, 0, false, &visible, 1000).start,
        10
    );
    assert_eq!(
        choose(OutputSource::Recent, Some(0), 0, false, &visible, 1000).count,
        0
    );
}

/// A full-screen Agent's recent range ends at its answer's last row above a placed input area;
/// an unknown area and every other source keep today's endpoint.
#[test]
fn agent_recent_ends_above_a_placed_input_area_only() {
    use orchbee_terminal_protocol::{
        TerminalTextCell, TerminalTextRow, decode_terminal_text_frame,
    };
    let mut frame = decode_terminal_text_frame(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    )))
    .unwrap();
    let style = frame.rows[0].cells[0].style;
    // Rows 100..=107: a header, two answer rows, blank rows, then the input box and its footer.
    let screen = [
        "header",
        "answer 1",
        "answer 2",
        "",
        "",
        "┃",
        "┃ Build",
        "╹▀▀▀",
    ];
    let first_input_row = 5;
    frame.scrollback_rows = 100;
    frame.geometry.rows = 8;
    frame.alternate_screen = true;
    frame.rows = (0_u16..)
        .zip(screen)
        .map(|(index, text)| TerminalTextRow {
            index,
            flags: 0,
            prompt_class: 0,
            cells: text
                .chars()
                .map(|c| TerminalTextCell {
                    text: c.to_string().into(),
                    style,
                    width: 0,
                })
                .collect(),
        })
        .collect();
    let read = |source, lines, input| {
        let range = range(&frame, selection(source, lines), input).unwrap();
        (range.start, range.count)
    };
    for source in [OutputSource::Recent, OutputSource::RecentUnwrapped] {
        assert_eq!(read(source, Some(2), Some(first_input_row)), (101, 2));
        let (start, count) = read(source, None, Some(first_input_row));
        assert_eq!(
            start + u64::from(count),
            103,
            "the default range ends after answer 2"
        );
        assert_eq!(read(source, Some(2), None), (106, 2));
    }
    for source in [OutputSource::Visible, OutputSource::Detection] {
        assert_eq!(
            read(source, None, Some(first_input_row)),
            read(source, None, None)
        );
    }
}

#[test]
fn physical_tails_blank_rows_and_soft_wraps_have_distinct_meanings() {
    let mut sample = rows(&["ab中", "文 cd", "", "  "]);
    sample[0].into_next = true;
    sample[1].from_previous = true;
    prepare_rows(
        &mut sample,
        selection(OutputSource::RecentUnwrapped, Some(4)),
    );
    assert_eq!(join_text(&sample, true).expect("text"), "ab中文 cd\n");
    assert_eq!(join_text(&sample, false).expect("text"), "ab中\n文 cd\n");
    prepare_rows(&mut sample, selection(OutputSource::Detection, Some(1)));
    assert_eq!(sample.len(), 1);
    assert!(sample[0].from_previous);
    assert_eq!(join_text(&sample, true).expect("fragment"), "文 cd\n");
    prepare_rows(&mut sample, selection(OutputSource::Visible, Some(0)));
    assert!(join_text(&sample, false).expect("empty").is_empty());
    let huge = rows(&[&"x".repeat(output::LIMIT)]);
    assert_eq!(
        join_text(&huge, false).expect_err("bound").kind,
        FailureKind::OutputTooLarge
    );
}
