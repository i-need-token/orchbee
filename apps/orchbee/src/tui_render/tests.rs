use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend};

use crate::{
    client_view::{
        BackendAction, ClientView, KeyCode, KeyEvent, Modifiers, PrefixInput,
        SessionActionDisposition,
    },
    localization::Locale,
    tui_layout::ViewportTier,
};

use super::render_shell;

#[test]
fn close_request_notice_does_not_replace_the_exact_status_text() {
    let mut terminal = infallible(Terminal::new(TestBackend::new(100, 1)));
    let view = ClientView::new(100);
    let mut visible = None;
    infallible(terminal.draw(|frame| {
        visible = super::render_status(
            frame,
            frame.area(),
            &view,
            Locale::EnUs,
            Locale::EnUs.shell(),
            "terminal · Close cleanup blocked",
            Some("Close request was not applied"),
            false,
            ViewportTier::Full,
            &crate::tui_theme::Theme::rgb_test_dark(),
        );
    }));
    assert!(visible.is_some());
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("terminal · Close cleanup blocked"));
    assert!(text.contains("Close request was not applied"));
}

fn rendered_text(width: u16, height: u16, view: &ClientView, locale: Locale) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = infallible(Terminal::new(backend));
    infallible(terminal.draw(|frame| {
        render_shell(
            frame,
            view,
            locale,
            &crate::tui_theme::Theme::rgb_test_dark(),
        );
    }));
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

fn rendered_rows(width: u16, height: u16, view: &ClientView, locale: Locale) -> Vec<String> {
    rendered_text(width, height, view, locale)
        .split('\n')
        .map(str::to_owned)
        .collect()
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

fn without_cell_padding(value: &str) -> String {
    value.replace(' ', "")
}

fn plain(c: char) -> PrefixInput {
    PrefixInput::Key(KeyEvent::press(KeyCode::Char(c), Modifiers::NONE))
}

fn ctrl_b() -> PrefixInput {
    PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL))
}

fn prefix(view: &mut ClientView, input: PrefixInput) {
    commit(view, ctrl_b());
    commit(view, input);
}

fn commit(view: &mut ClientView, input: PrefixInput) {
    let transition = view.decide(input);
    view.commit(transition, SessionActionDisposition::KeepNavigator);
}

#[test]
fn wide_render_exposes_the_business_sidebar_and_unavailable_workbench_truth() {
    let view = ClientView::new(120);
    let rendered = rendered_text(120, 40, &view, Locale::EnUs);

    assert!(rendered.contains("ORCHBEE"));
    assert!(rendered.contains("Projects"));
    assert!(rendered.contains("WORKBENCH"));
    assert!(rendered.contains("Terminal unavailable"));
    assert!(rendered.contains("No Site or Client Session is connected."));
    assert!(rendered.contains("Ctrl+B · shortcuts"));
}

#[test]
fn narrow_render_hides_then_overlays_the_business_sidebar() {
    let mut view = ClientView::new(70);
    let hidden = rendered_text(70, 24, &view, Locale::EnUs);
    assert!(!hidden.contains("Projects"));
    assert!(hidden.contains("WORKBENCH"));

    prefix(&mut view, plain('b'));
    let overlaid = rendered_text(70, 24, &view, Locale::EnUs);
    assert!(overlaid.contains("Projects"));
    assert!(overlaid.contains("Backend unavailable"));
}

#[test]
fn renders_visible_prefix_and_localized_shortcut_overlays() {
    let mut view = ClientView::new(120);
    commit(&mut view, ctrl_b());
    let prefix_mode = rendered_text(120, 32, &view, Locale::ZhCn);
    assert!(without_cell_padding(&prefix_mode).contains("前缀Ctrl+B"));

    commit(&mut view, plain('?'));
    let help = rendered_text(120, 32, &view, Locale::ZhCn);
    let compact_help = without_cell_padding(&help);
    assert!(compact_help.contains("键盘快捷键"));
    assert!(help.contains("Ctrl+B c"));
    assert!(help.contains("Ctrl+B ,"));
    assert!(help.contains("Ctrl+B Ctrl+B"));
    assert!(compact_help.contains("Ctrl+B?关闭此帮助"));
    assert!(compact_help.contains("输入仍会传递到终端"));
}

#[test]
fn renders_backend_action_results_as_no_command_sent() {
    let mut view = ClientView::new(120);
    view.report_backend_unavailable(BackendAction::CreateTab);
    let english = rendered_text(120, 40, &view, Locale::EnUs);
    let chinese = rendered_text(120, 40, &view, Locale::ZhCn);

    assert!(english.contains("Tab creation unavailable · no command was sent"));
    assert!(without_cell_padding(&chinese).contains("无法创建Tab·未发送命令"));
}

#[test]
fn reserves_exactly_one_status_row_at_every_tier() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
        let view = ClientView::new(width);
        let rows = rendered_rows(width, height, &view, Locale::EnUs);
        assert_eq!(rows.len(), height as usize + 1, "{width}x{height}");
        let status = &rows[height as usize - 1];
        assert!(
            status.contains("Ctrl+B") || status.contains("unavailable"),
            "status row {width}x{height}: {status:?}"
        );
    }
}

#[test]
fn overlays_never_cover_the_status_row() {
    let mut view = ClientView::new(80);
    prefix(&mut view, plain('?'));
    let rows = rendered_rows(80, 24, &view, Locale::EnUs);
    let status = &rows[23];
    assert!(status.contains("unavailable"), "{status:?}");
    for row in &rows[..23] {
        assert!(!row.contains("Backend unavailable ·"), "{row:?}");
    }

    let mut view = ClientView::new(60);
    prefix(&mut view, plain('w'));
    let rows = rendered_rows(60, 12, &view, Locale::ZhCn);
    assert!(rows[11].contains("快捷键") || rows[11].contains("Site"));
}

#[test]
fn degrades_to_minimal_chrome_below_sixty_columns_or_twelve_rows() {
    let view = ClientView::new(50);
    let rendered = rendered_text(50, 10, &view, Locale::EnUs);
    assert!(rendered.contains("Ctrl+B q · exit"));
    assert!(!rendered.contains("ORCHBEE"));
    assert!(!rendered.contains("WORKBENCH · "));

    // Width alone qualifies as Full; height forces Minimal.
    let view = ClientView::new(120);
    let short = rendered_text(120, 10, &view, Locale::EnUs);
    assert!(short.contains("Ctrl+B q · exit"));
    assert!(!short.contains("ORCHBEE"));

    let mut toggled = ClientView::new(50);
    prefix(&mut toggled, plain('b'));
    let toggled_render = rendered_text(50, 10, &toggled, Locale::EnUs);
    assert!(!toggled_render.contains("ORCHBEE"));
}

#[test]
fn paints_only_the_localized_too_small_notice_and_the_status_line() {
    let view = ClientView::new(15);
    let english = rendered_rows(15, 4, &view, Locale::EnUs);
    assert!(english.iter().any(|row| row.contains("Too small")));
    assert!(english[3].contains("Ctrl+B q"));
    assert!(!english.iter().any(|row| row.contains("WORKBENCH")));

    let chinese = rendered_rows(15, 4, &view, Locale::ZhCn);
    assert!(
        chinese
            .iter()
            .any(|row| without_cell_padding(row).contains("窗口太小"))
    );
    assert!(chinese[3].contains("Ctrl+B q"));

    // Width below 20 alone also triggers the too-small surface.
    let narrow = rendered_rows(19, 30, &view, Locale::EnUs);
    assert!(narrow.iter().any(|row| row.contains("Too small")));
}

fn buffer_rows(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn help_paginates_wrapped_rows_and_clamps_the_requested_page() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(60, 12)));
    let mut placed = None;

    // Wrapped shortcut rows in an 8-line page budget -> several pages; the
    // requested page 99 clamps to the last one and the footer stays visible.
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 99, &theme);
    }));
    let (_, page) = placed.expect("help must fit in 60x12");
    assert!(page >= 1, "wrapped help must need more than one page");
    let last = page + 1;
    let rows = buffer_rows(&terminal, 60, 12);
    assert!(rows[9].contains(&format!("{last}/{last}")), "{:?}", rows[9]);
    assert!(rows[8].contains("Ctrl+B ? closes"), "{:?}", rows[8]);
    // The last page holds the trailing "other keys" registry row, whose
    // description wrapped onto an aligned continuation line.
    assert!(rows.iter().any(|row| row.contains("terminal")), "{rows:?}");
    assert!(
        rows.iter().any(|row| row.contains("input port")),
        "{rows:?}"
    );
    // Rows from earlier pages are not drawn.
    assert!(!rows.iter().any(|row| row.contains("Ctrl+B a")));

    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    let (_, page) = placed.expect("help must fit in 60x12");
    assert_eq!(page, 0);
    let rows = buffer_rows(&terminal, 60, 12);
    assert!(rows.iter().any(|row| row.contains("Ctrl+B a")), "{rows:?}");
    assert!(rows[9].contains("1/"), "{:?}", rows[9]);
}

#[test]
fn help_wraps_long_descriptions_instead_of_clipping_them() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    // A 46-column overlay leaves ~28 cells for labels: long descriptions
    // must wrap onto continuation lines rather than clip at the right edge.
    let mut terminal = infallible(Terminal::new(TestBackend::new(46, 20)));
    let mut placed = None;
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    placed.expect("46x20 help must fit with wrapping");
    let rows = buffer_rows(&terminal, 46, 20);
    let joined = rows.join("\n");
    // The longest English descriptions appear whole across wrapped lines.
    for word in ["Workspace", "directory", "Project"] {
        assert!(joined.contains(word), "clipped {word}: {joined}");
    }
    // Continuation lines align under the description column: after the left
    // border they begin with the key-column padding and still carry text.
    assert!(
        rows.iter().any(|row| {
            let inner = row.trim_start().trim_start_matches(['│', '|', '+']);
            inner.starts_with("              ") && !inner[14..].trim().is_empty()
        }),
        "no aligned continuation line: {rows:?}"
    );
}

#[test]
fn help_wraps_chinese_descriptions_on_grapheme_boundaries() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::ZhCn.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(46, 20)));
    let mut placed = None;
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    placed.expect("zh help must fit with wrapping");
    let rows = buffer_rows(&terminal, 46, 20);
    // A wide-character description hard-wraps without dropping its tail.
    let tail = rows.concat();
    assert!(tail.contains('页'), "wrapped zh tail lost: {rows:?}");
    assert!(
        rows.iter().any(|row| row.contains("PgUp/PgDn")),
        "paging footer missing: {rows:?}"
    );
}

#[test]
fn wrap_display_refuses_widths_that_cannot_hold_a_grapheme() {
    // Zero budget is unrenderable for real text; empty text needs no cell.
    assert_eq!(super::wrap_display("ab", 0), None);
    assert_eq!(super::wrap_display("", 0), Some(vec![String::new()]));
    // A one-cell line can hold ASCII but never a two-cell CJK grapheme —
    // the honest answer is `None`, not a forged wider line.
    assert_eq!(
        super::wrap_display("ab", 1),
        Some(vec!["a".into(), "b".into()])
    );
    assert_eq!(super::wrap_display("中", 1), None);
    assert_eq!(super::wrap_display("a中", 1), None);
    assert_eq!(super::wrap_display("中", 2), Some(vec!["中".into()]));
    // A combining sequence is one grapheme: it fits a 1-cell line whole.
    assert_eq!(
        super::wrap_display("a\u{301}", 1),
        Some(vec!["a\u{301}".into()])
    );
    // An empty word between spaces still returns at least one line.
    assert_eq!(super::wrap_display("", 5), Some(vec![String::new()]));
}

#[test]
fn help_footer_falls_back_to_the_explicit_short_form_then_refuses() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    // 46x20: the long dismiss line (56 cells) exceeds the 40-cell inner
    // width, so the localized short form renders — close chord, typing
    // purpose, full paging chord, and page number all still visible.
    let mut terminal = infallible(Terminal::new(TestBackend::new(46, 20)));
    let mut placed = None;
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    placed.expect("46x20 must fit the short footer");
    let rows = buffer_rows(&terminal, 46, 20).concat();
    assert!(
        rows.contains("Ctrl+B ? closes · typing continues"),
        "{rows}"
    );
    assert!(rows.contains("Ctrl+B PgUp/PgDn turns pages"), "{rows}");

    // 38 columns: even the 34-cell short dismiss line cannot fit the
    // 32-cell inner width, so the overlay refuses — no keyless footer.
    let mut terminal = infallible(Terminal::new(TestBackend::new(38, 12)));
    let mut placed = Some((ratatui::layout::Rect::default(), 0usize));
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    assert_eq!(
        placed, None,
        "38-wide help must refuse, not clip the footer"
    );
}

#[test]
fn help_paging_line_leads_with_the_page_number() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    // `page/total` always leads the paging line, ahead of the full paging
    // chord — it can never be clipped away by a right-edge cutoff.
    let mut terminal = infallible(Terminal::new(TestBackend::new(60, 12)));
    infallible(terminal.draw(|frame| {
        super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    let rows = buffer_rows(&terminal, 60, 12);
    let paging = rows
        .iter()
        .find(|row| row.contains("PgUp"))
        .expect("paging footer line missing")
        .trim_start_matches([' ', '│', '|', '+'])
        .to_owned();
    assert!(
        paging.starts_with("1/"),
        "page number not leading: {paging:?}"
    );
    assert!(paging.contains("Ctrl+B PgUp/PgDn"), "{paging:?}");
}

#[test]
fn paginate_help_rows_groups_whole_rows_into_page_budgets() {
    // Uniform one-line rows paginate exactly like the old row division.
    assert_eq!(
        super::paginate_help_rows(&[1; 21], 6),
        Some(vec![(0, 6), (6, 12), (12, 18), (18, 21)])
    );
    assert_eq!(super::paginate_help_rows(&[1; 5], 5), Some(vec![(0, 5)]));
    assert_eq!(super::paginate_help_rows(&[], 4), Some(vec![(0, 0)]));
    // A multi-line row counts every rendered line and is never split.
    assert_eq!(
        super::paginate_help_rows(&[3, 3, 1, 4], 5),
        Some(vec![(0, 1), (1, 3), (3, 4)])
    );
    // A row taller than the page refuses placement instead of clipping.
    assert_eq!(super::paginate_help_rows(&[1, 6, 1], 4), None);
    assert_eq!(super::paginate_help_rows(&[2], 0), None);
}

#[test]
fn help_refuses_placement_that_would_clip_rows_or_footer() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(40, 7)));
    let mut placed = Some((ratatui::layout::Rect::default(), 0usize));

    // A 5-row placed overlay leaves 3 inner rows — only 1 row per page, below
    // the 2-row minimum, so the overlay refuses instead of clipping.
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(frame, frame.area(), text, 0, &theme);
    }));
    assert_eq!(placed, None);
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.trim().is_empty(), "refused overlay drew: {text:?}");
}

#[test]
fn sidebar_status_truncates_with_ellipsis_on_grapheme_boundaries() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(30, 12)));
    let sidebar = super::SidebarArea {
        area: ratatui::layout::Rect::new(0, 0, 30, 12),
        layer: super::SidebarLayer::Docked,
    };
    infallible(terminal.draw(|frame| {
        super::render_sidebar(
            frame,
            sidebar,
            text,
            "Tab 1 · Ready · controlling",
            None,
            &theme,
        );
    }));
    let rows = buffer_rows(&terminal, 30, 12);
    let joined = rows.concat();
    // The 29-cell status exceeds the 26-cell inner width: it must end with
    // the ellipsis marker instead of a half-clipped word.
    assert!(joined.contains('…'), "no ellipsis marker: {rows:?}");
    assert!(!joined.contains("controlling"), "status not cut: {rows:?}");
    assert!(joined.contains("contr"), "{rows:?}");
}

#[test]
fn the_zoom_badge_rides_the_status_label_in_both_locales() {
    for (locale, needle) in [
        (Locale::EnUs, "zoomed · Ctrl+B z restores"),
        (Locale::ZhCn, "已放大 · Ctrl+B z 恢复"),
    ] {
        let mut terminal = infallible(Terminal::new(TestBackend::new(120, 1)));
        let view = ClientView::new(120);
        infallible(terminal.draw(|frame| {
            super::render_status(
                frame,
                frame.area(),
                &view,
                locale,
                locale.shell(),
                "terminal · Ready · controlling",
                None,
                true,
                ViewportTier::Full,
                &crate::tui_theme::Theme::rgb_test_dark(),
            );
        }));
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        // Cell padding splits wide glyphs — compare on collapsed text.
        assert!(
            without_cell_padding(&text).contains(&without_cell_padding(needle)),
            "{locale:?} missing badge: {text:?}"
        );
    }
}

// ===== Retained-history pane surface =====

use std::num::NonZeroU64;

use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, TerminalFrameRoute, TerminalInteraction, TerminalPixelGeometry,
    TerminalRowPromptClass, TerminalTextCell, TerminalTextCellMapping, TerminalTextCursor,
    TerminalTextFrame, TerminalTextRow, TerminalTextRowWindow, TerminalTextStyle,
    TerminalTextWindowRow,
};
use ratatui::layout::Rect;

use crate::terminal_session::NativeViewportSurface;

/// A validated row window with identity-ASCII rows — the shape the core
/// fences before it ever reaches this painter.
fn history_window(texts: &[&str], columns: u16) -> TerminalTextRowWindow {
    TerminalTextRowWindow {
        attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
        host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
        sequence: NonZeroU64::new(1).expect("sequence"),
        revision: 1,
        generation: NonZeroU64::new(1).expect("generation"),
        total_rows: 200,
        start_row: 42,
        columns,
        ignored_optional_flags: 0,
        rows: texts
            .iter()
            .enumerate()
            .map(|(offset, text)| TerminalTextWindowRow {
                index: 42 + offset as u64,
                prompt_class: TerminalRowPromptClass::None,
                wrap: false,
                wrap_continuation: false,
                identity_ascii: true,
                text: (*text).into(),
                mappings: Vec::<TerminalTextCellMapping>::new(),
            })
            .collect(),
    }
}

fn render_viewport(
    width: u16,
    height: u16,
    surface: NativeViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    locale: Locale,
) -> Vec<String> {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
    infallible(terminal.draw(|frame| {
        super::viewport::render(
            frame,
            Rect::new(0, 0, width, height),
            surface,
            live_frame,
            false,
            locale,
            &theme,
        );
    }));
    buffer_rows(&terminal, width, height)
}

// A validated window paints its retained rows under the mode badge, with
// the position readout and the persistent return hint on the bottom line.
#[test]
fn history_window_paints_rows_position_and_hint() {
    let window = history_window(&["row-one", "row-two"], 40);
    let rows = render_viewport(
        40,
        8,
        NativeViewportSurface::Window {
            window: &window,
            position: Some((42, 200)),
        },
        None,
        Locale::EnUs,
    );
    assert!(rows[0].contains("row-one"), "{rows:?}");
    assert!(
        rows[0].contains("HISTORY"),
        "badge on the first row: {rows:?}"
    );
    assert!(rows[1].contains("row-two"), "{rows:?}");
    assert!(rows[7].contains("Row 42 of 200"), "{rows:?}");
    assert!(rows[7].contains("Esc back to live"), "{rows:?}");
}

// The bottom line and indicators localize with the session locale.
#[test]
fn history_window_bottom_line_localizes() {
    let window = history_window(&["row"], 40);
    let rows = render_viewport(
        40,
        8,
        NativeViewportSurface::Window {
            window: &window,
            position: Some((42, 200)),
        },
        None,
        Locale::ZhCn,
    );
    let bottom = without_cell_padding(&rows[7]);
    assert!(bottom.contains("第42行·共200行"), "{rows:?}");
    assert!(bottom.contains("Esc返回实时"), "{rows:?}");
    assert!(rows[0].contains("历史"), "{rows:?}");
}

// A narrow leaf degrades on grapheme boundaries: the return hint keeps
// priority while the position truncates first.
#[test]
fn history_bottom_line_hint_keeps_priority_when_narrow() {
    let window = history_window(&["row"], 18);
    let rows = render_viewport(
        18,
        6,
        NativeViewportSurface::Window {
            window: &window,
            position: Some((42, 200)),
        },
        None,
        Locale::EnUs,
    );
    assert!(
        rows[5].contains("Esc back to live"),
        "hint survives the position: {rows:?}"
    );
    assert!(
        !rows[5].contains("Row 42"),
        "the position yields first: {rows:?}"
    );
}

// `AwaitingWindow` and `Recovering` clear the leaf and paint their own
// indicator — no position without a validated window, never stale rows.
#[test]
fn history_indicators_clear_and_show_without_position() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    for surface in [
        NativeViewportSurface::AwaitingWindow,
        NativeViewportSurface::Recovering,
    ] {
        let mut terminal = infallible(Terminal::new(TestBackend::new(40, 8)));
        // Stale content under the leaf: the surface must clear it.
        infallible(terminal.draw(|frame| {
            frame.buffer_mut().set_stringn(
                0,
                0,
                "stale-live-content",
                17,
                ratatui::style::Style::new(),
            );
        }));
        infallible(terminal.draw(|frame| {
            super::viewport::render(
                frame,
                Rect::new(0, 0, 40, 8),
                surface,
                None,
                false,
                Locale::EnUs,
                &theme,
            );
        }));
        let rows = buffer_rows(&terminal, 40, 8);
        let joined = rows.concat();
        assert!(
            !joined.contains("stale-live-content"),
            "indicator surface left stale content: {rows:?}"
        );
        assert!(rows[7].contains("Esc back to live"), "{rows:?}");
        assert!(!joined.contains("Row"), "no position shown: {rows:?}");
        let badge = match surface {
            NativeViewportSurface::AwaitingWindow => "Loading history…",
            NativeViewportSurface::Recovering => "Returning to live…",
            _ => unreachable!(),
        };
        assert!(joined.contains(badge), "{rows:?}");
    }
}

// `Entering` keeps the last verified live frame beneath the loading badge.
#[test]
fn history_entering_keeps_the_live_frame() {
    let live = TerminalTextFrame {
        route: TerminalFrameRoute {
            attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
            host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
            sequence: NonZeroU64::new(1).expect("sequence"),
            generation: NonZeroU64::new(1).expect("generation"),
            revision: 1,
            base_revision: None,
        },
        geometry: TerminalPixelGeometry {
            cell_pixel_height: 16,
            cell_pixel_width: 8,
            columns: 40,
            rows: 8,
        },
        cursor: TerminalTextCursor {
            column: 0,
            row: 0,
            visible: false,
        },
        alternate_screen: false,
        interaction: TerminalInteraction::default(),
        foreground: 0,
        background: 0,
        theme_revision: 1,
        scrollback_rows: 9,
        rows: vec![TerminalTextRow {
            index: 0,
            flags: 0,
            cells: "live-tail"
                .chars()
                .map(|c| TerminalTextCell {
                    text: c.to_string().into(),
                    style: TerminalTextStyle {
                        foreground: 0,
                        background: 0,
                        underline_color: 0,
                        flags: 0,
                        underline: 0,
                    },
                    width: 0,
                })
                .collect(),
        }],
    };
    let rows = render_viewport(
        40,
        8,
        NativeViewportSurface::Entering,
        Some(&live),
        Locale::EnUs,
    );
    let joined = rows.concat();
    assert!(
        joined.contains("live-tail"),
        "live frame preserved: {rows:?}"
    );
    assert!(joined.contains("Loading history…"), "{rows:?}");
    assert!(rows[7].contains("Esc back to live"), "{rows:?}");
}
