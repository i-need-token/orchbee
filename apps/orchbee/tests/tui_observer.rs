//! Cross-platform completed-frame grammar evidence for native TUI process observers.
//! It exercises the same bounded screen model used by Linux PTY scenarios without
//! claiming process, PTY, input, or publication evidence on other hosts.

#[path = "support/tui_observer.rs"]
mod frame_observer;

use std::time::{Duration, Instant};

use frame_observer::{ModalRow, Screen, WorkbenchHeader};

const COLUMNS: usize = 120;
const ROWS: usize = 32;
const SYNCHRONIZED_UPDATE_BEGIN: &str = "\x1b[?2026h";
const SYNCHRONIZED_UPDATE_END: &str = "\x1b[?2026l";
const DRAW_OPEN: &str = "\x1b[?25l\x1b[0m\x1b[0m";
const DRAW_CLOSE: &str = "\x1b[0m\x1b[0m";

fn backend_frame(draws: &[&str]) -> String {
    let mut output = SYNCHRONIZED_UPDATE_BEGIN.to_owned();
    for draw in draws {
        output.push_str(DRAW_OPEN);
        output.push_str(draw);
        output.push_str(DRAW_CLOSE);
    }
    output.push_str(SYNCHRONIZED_UPDATE_END);
    output
}

fn backend_frame_before_flush(draws: &[&str]) -> String {
    let mut output = backend_frame(draws);
    output.truncate(output.len() - SYNCHRONIZED_UPDATE_END.len());
    output
}

fn complete_frame(draw: &str) -> Screen {
    let mut screen = Screen::new(COLUMNS, ROWS);
    screen.feed_all(backend_frame(&[draw]).as_bytes()).unwrap();
    screen
}

fn modal_frame(top_edge: &str, rows: &[&str], right_border: &str) -> Screen {
    let width = ratatui::text::Line::raw(top_edge).width();
    let mut draw = format!("\x1b[2;3H┌{top_edge}┐");
    for (index, content) in rows.iter().enumerate() {
        let content_width = ratatui::text::Line::raw(*content).width();
        assert!(
            content_width <= width,
            "fixture row must fit inside its borders"
        );
        let padding = " ".repeat(width - content_width);
        draw.push_str(&format!(
            "\x1b[{};3H│{content}{padding}{right_border}",
            index + 3
        ));
    }
    draw.push_str(&format!(
        "\x1b[{};3H└{}┘",
        rows.len() + 3,
        "─".repeat(width)
    ));
    complete_frame(&draw)
}

#[test]
fn native_header_requires_the_top_location_and_current_compact_tab_grammar() {
    for (top, header, expected) in [
        (
            "+ New workspace             *1 1                 Workspace",
            WorkbenchHeader::active("Workspace", 1, "1"),
            true,
        ),
        (
            "+ New workspace             *1 1   2 2             Workspace",
            WorkbenchHeader::active("Workspace", 1, "1"),
            true,
        ),
        (
            "+ New workspace             *1  current             Workspace",
            WorkbenchHeader::active("Workspace", 1, " current"),
            true,
        ),
        (
            "+ New workspace                                  Setup process / Chosen workspace",
            WorkbenchHeader::empty("Setup process / Chosen workspace"),
            true,
        ),
        (
            "+ 新建工作区                                      设置进程 / 已选择工作区",
            WorkbenchHeader::empty("设置进程 / 已选择工作区"),
            true,
        ),
        (
            "+ New workspace             [*1 1]               Workspace",
            WorkbenchHeader::active("Workspace", 1, "1"),
            false,
        ),
        (
            "+ New workspace                                  Workspace",
            WorkbenchHeader::active("Workspace", 1, "1"),
            false,
        ),
        (
            "+ New workspace             *1 1  *2 2             Workspace",
            WorkbenchHeader::active("Workspace", 1, "1"),
            false,
        ),
        (
            "+ New workspace             *1  current  *2 2       Workspace",
            WorkbenchHeader::active("Workspace", 1, " current"),
            false,
        ),
        (
            "+ New workspace             *1  current             Workspace",
            WorkbenchHeader::empty("Workspace"),
            false,
        ),
        (
            "+ New workspace             *1 current  2 nested    Workspace",
            WorkbenchHeader::active("Workspace", 1, "current  2 nested"),
            false,
        ),
    ] {
        let screen = complete_frame(&format!("\x1b[1;1H{top}\x1b[2;1H› * 1 Workspace"));
        assert_eq!(
            screen.latest_frame().unwrap().has_native_header(header),
            expected,
            "{top:?}"
        );
    }
}

#[test]
fn screen_observer_keeps_raw_boundaries_and_wide_display_cells_distinct() {
    let mut screen = complete_frame(concat!(
        "\x1b[1;1H *1 1                                      设置进程 / 已选择工作区",
        "\x1b[2;1HPREFIX Ctrl+b"
    ));
    let checkpoint = screen.checkpoint();
    let frame = screen.latest_frame().unwrap();
    assert!(frame.has_active_tab(1, "1"));
    assert!(frame.prefix_active());
    assert_eq!(frame.cell_position("PREFIX"), Some((0, 1)));
    assert_eq!(frame.displayed_rows(), frame.rows());
    assert_eq!(frame.cells()[0][43], "设");
    assert_eq!(frame.cells()[0][44], " ");
    assert_eq!(frame.cells()[0][45], "置");
    assert!(screen.raw_contains("设置进程".as_bytes()));
    assert!(screen.raw_contains_after(0, "已选择工作区".as_bytes()));
    assert!(!screen.raw_contains_after(checkpoint, "设置进程".as_bytes()));

    screen.feed_all(b"tail").unwrap();
    assert!(screen.raw_since(checkpoint).contains("tail"));
}

#[test]
fn ordered_tab_observer_rejects_stale_order_missing_members_and_extra_selection() {
    for (strip, expected) in [
        ("  1 3  *2 1   3 2 ", true),
        (" *1 1   2 2   3 3 ", false),
        ("  1 3  *2 1 ", false),
        ("  1 3  *2 1   3 2  *4 4 ", false),
        ("  1 3  *2 1   3 2   4 4 ", false),
        ("  1 3   2 1  *3 2 ", false),
    ] {
        let screen = complete_frame(&format!("\x1b[1;1H{strip}"));
        assert_eq!(
            screen
                .latest_frame()
                .unwrap()
                .has_ordered_tabs(&["3", "1", "2"], 2),
            expected,
            "{strip:?}"
        );
    }
}

#[test]
fn navigator_observer_keeps_current_state_distinct_from_row_order() {
    for (rows, expected) in [
        (
            ["\x1b[4;1H    1 3", "\x1b[5;1H› * 2 1", "\x1b[6;1H    3 2"],
            true,
        ),
        (
            ["\x1b[4;1H› * 1 1", "\x1b[5;1H    2 2", "\x1b[6;1H    3 3"],
            false,
        ),
        (
            ["\x1b[4;1H    1 3", "\x1b[5;1H  * 2 1", "\x1b[6;1H    3 2"],
            false,
        ),
        (
            ["\x1b[4;1H    1 3", "\x1b[5;1Hx * 2 1", "\x1b[6;1H    3 2"],
            false,
        ),
    ] {
        let screen = complete_frame(&rows.concat());
        assert_eq!(
            screen
                .latest_frame()
                .unwrap()
                .has_ordered_navigator_rows(&["3", "1", "2"], 2),
            expected
        );
    }
}

#[test]
fn modal_observer_requires_the_exact_heading_edge_complete_borders_and_exact_rows() {
    let expected = [
        ModalRow::exact("Close Tab?"),
        ModalRow::exact(""),
        ModalRow::exact("1"),
        ModalRow::exact(""),
        ModalRow::exact("warning"),
        ModalRow::exact(""),
        ModalRow::exact("[ Cancel  Esc ] [ Close  Enter ]"),
    ];
    let rows = [
        "",
        "  1",
        "",
        "  warning",
        "",
        "  [ Cancel  Esc ] [ Close  Enter ]",
    ];
    let edge = |heading: &str, close: &str| format!("─ {heading} {} {close} ─", "─".repeat(16));
    for (top, right, accepted) in [
        (edge("Close Tab?", "[x]"), "│", true),
        (edge("Close Panel?", "[x]"), "│", false),
        (edge("Close Tab?", "[?]"), "│", false),
        (format!("─ Close Tab? {}[x]", "─".repeat(20)), "│", false),
        ("─".repeat(38), "│", false),
        (edge("Close Tab?", "[x]"), " ", false),
    ] {
        let screen = modal_frame(&top, &rows, right);
        assert_eq!(
            screen
                .latest_frame()
                .unwrap()
                .has_bordered_modal_content(&expected),
            accepted,
            "top {top:?}, right border {right:?}"
        );
    }
}

#[test]
fn localized_close_observer_requires_the_full_consequence_target_and_decision_keys() {
    for (title, warning, actions, incomplete_warning) in [
        (
            "Close Tab?",
            "Ends 1 terminal.",
            "[ Cancel  Esc ] [ Close  Enter ]",
            "Ends 1 terminal",
        ),
        (
            "关闭标签页？",
            "将结束 1 个终端。",
            "[ 取消  Esc ] [ 关闭  Enter ]",
            "将结束 1 个终端",
        ),
    ] {
        let expected = [
            ModalRow::exact(title),
            ModalRow::exact(""),
            ModalRow::exact("1"),
            ModalRow::exact(""),
            ModalRow::exact(warning),
            ModalRow::exact(""),
            ModalRow::exact(actions),
        ];
        // This grammar fixture holds the complete consequence and padding;
        // it does not establish product admission.
        let top = format!("─ {title} {} [x] ─", "─".repeat(24));
        let rows = ["", "1", "", warning, "", actions];
        assert!(
            modal_frame(&top, &rows, "│")
                .latest_frame()
                .unwrap()
                .has_bordered_modal_content(&expected),
            "{title}"
        );
        for (index, replacement) in [
            (1, "2".to_owned()),
            (3, incomplete_warning.to_owned()),
            (5, actions.replace("Enter", "")),
            (5, actions.replace("Esc", "")),
        ] {
            let mut changed = rows;
            changed[index] = &replacement;
            assert!(
                !modal_frame(&top, &changed, "│")
                    .latest_frame()
                    .unwrap()
                    .has_bordered_modal_content(&expected),
                "incomplete or changed row {index}: {title}"
            );
        }
    }
}

#[test]
fn modal_field_observer_requires_brackets_padding_and_cursor_end_suffix() {
    let top = "─ Rename Tab ─ [x] ─";
    for (field, expected, accepted) in [
        ("  [ hijk  ]", "abcdefghijk", true),
        ("  [ ghijk ]", "abcdefghijk", false),
        ("  hijk ", "abcdefghijk", false),
        ("  [ hijk ]", "abcdefghijk", false),
    ] {
        let screen = modal_frame(top, &[field], "│");
        assert_eq!(
            screen.latest_frame().unwrap().has_bordered_modal_content(&[
                ModalRow::exact("Rename Tab"),
                ModalRow::field_at_end(expected),
            ]),
            accepted,
            "{field:?}"
        );
    }
}

#[test]
fn fragmented_complete_draw_rejects_a_later_half_draw_hybrid() {
    let mut screen = Screen::new(COLUMNS, ROWS);
    let complete = backend_frame(&[concat!(
        "\x1b[1;1H  1 1  *2 2 ",
        "\x1b[32;1HTab 2 · Ready · controlling"
    )]);
    for byte in complete.as_bytes() {
        screen.feed_all(std::slice::from_ref(byte)).unwrap();
    }
    let checkpoint = screen.checkpoint();
    let frame = screen.latest_frame().unwrap();
    assert_eq!(frame.marker_offset(), complete.len());
    assert!(frame.has_ordered_tabs(&["1", "2"], 2));
    assert!(frame.text().contains("Tab 2 · Ready · controlling"));

    let partial = format!(
        "{SYNCHRONIZED_UPDATE_BEGIN}{DRAW_OPEN}{}",
        concat!("\x1b[1;1H *1 1   2 2 ", "\x1b[32;5H1")
    );
    screen.feed_all(partial.as_bytes()).unwrap();
    assert!(screen.working_text().contains(" *1 1 "));
    assert!(
        screen
            .working_text()
            .contains("Tab 1 · Ready · controlling")
    );
    let mut false_ready = |frame: &frame_observer::CompletedFrame| {
        (frame.has_ordered_tabs(&["1", "2"], 1)
            && frame.text().contains("Tab 1 · Ready · controlling"))
        .then_some(())
    };
    assert!(
        screen
            .observe_completed_before(
                checkpoint,
                Instant::now() + Duration::from_secs(1),
                &mut false_ready,
            )
            .is_none()
    );
}

#[test]
fn complete_frame_snapshot_ignores_next_partial_draw_in_the_same_batch() {
    let mut screen = Screen::new(COLUMNS, ROWS);
    let complete = backend_frame(&[concat!(
        "\x1b[1;1H *1 1   2 2 ",
        "\x1b[32;1HTab 1 · Ready · controlling"
    )]);
    let partial = format!(
        "{SYNCHRONIZED_UPDATE_BEGIN}{DRAW_OPEN}{}",
        concat!("\x1b[1;1H  1 1  *2 2 ", "\x1b[32;5H2")
    );
    screen
        .feed_all(format!("{complete}{partial}").as_bytes())
        .unwrap();

    let frame = screen.latest_frame().unwrap();
    assert_eq!(frame.marker_offset(), complete.len());
    assert!(frame.has_ordered_tabs(&["1", "2"], 1));
    assert!(frame.text().contains("Tab 1 · Ready · controlling"));
    assert_eq!(screen.working_text().lines().next(), Some("  1 1  *2 2"));
    assert!(
        screen
            .working_text()
            .contains("Tab 2 · Ready · controlling")
    );

    screen
        .feed_all(
            format!("\x1b[32;1H\x1b[2KTab 2 - Switching{DRAW_CLOSE}{SYNCHRONIZED_UPDATE_END}")
                .as_bytes(),
        )
        .unwrap();
    let mut stale_ready = |frame: &frame_observer::CompletedFrame| {
        frame
            .text()
            .contains("Tab 1 · Ready · controlling")
            .then_some(())
    };
    assert!(
        screen
            .observe_completed_before(0, Instant::now() + Duration::from_secs(1), &mut stale_ready,)
            .is_none(),
        "an older complete Ready frame must not beat the latest complete frame"
    );
}

#[test]
fn draw_resets_and_fragmented_cell_writes_publish_once_at_flush_boundary() {
    let mut screen = complete_frame("\x1b[1;1Hold");
    let checkpoint = screen.checkpoint();
    let previous_marker = screen.latest_frame().unwrap().marker_offset();

    let before_flush = backend_frame_before_flush(&["\x1b[1;1Hne", "\x1b[1;3Hw"]);
    for fragment in before_flush.as_bytes().chunks(3) {
        screen.feed_all(fragment).unwrap();
    }
    assert_eq!(screen.working_text().lines().next(), Some("new"));
    assert_eq!(
        screen.latest_frame().unwrap().marker_offset(),
        previous_marker
    );
    let mut new_frame = |frame: &frame_observer::CompletedFrame| {
        frame
            .text()
            .lines()
            .next()
            .is_some_and(|row| row == "new")
            .then_some(())
    };
    assert!(
        screen
            .observe_completed_before(
                checkpoint,
                Instant::now() + Duration::from_secs(1),
                &mut new_frame,
            )
            .is_none(),
        "draw reset pairs and partial cell writes cannot publish a frame"
    );

    let flush = SYNCHRONIZED_UPDATE_END.as_bytes();
    screen.feed_all(&flush[..flush.len() - 1]).unwrap();
    assert_eq!(
        screen.latest_frame().unwrap().marker_offset(),
        previous_marker
    );
    screen.feed_all(&flush[flush.len() - 1..]).unwrap();
    assert_eq!(
        screen.latest_frame().unwrap().marker_offset(),
        screen.checkpoint()
    );
    assert_eq!(
        screen.latest_frame().unwrap().text().lines().next(),
        Some("new")
    );

    let published_marker = screen.latest_frame().unwrap().marker_offset();
    screen.feed_all(SYNCHRONIZED_UPDATE_END.as_bytes()).unwrap();
    assert_eq!(
        screen.latest_frame().unwrap().marker_offset(),
        published_marker,
        "an end without a newly opened backend frame is not a publication"
    );
}

#[test]
fn expired_deadline_rejects_a_preloaded_complete_frame() {
    let screen = complete_frame("\x1b[1;1Hready");
    let expired = Instant::now() - Duration::from_secs(1);
    let mut predicate =
        |frame: &frame_observer::CompletedFrame| frame.text().contains("ready").then_some(());
    assert!(
        screen
            .observe_completed_before(0, expired, &mut predicate)
            .is_none()
    );
}
