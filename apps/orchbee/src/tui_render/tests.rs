use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend};

use crate::{
    client_view::{BackendAction, ClientKey, ClientView, SessionActionDisposition},
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
            ViewportTier::Full,
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
    infallible(terminal.draw(|frame| render_shell(frame, view, locale)));
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

fn prefix(view: &mut ClientView, key: ClientKey) {
    commit(view, ClientKey::ControlB);
    commit(view, key);
}

fn commit(view: &mut ClientView, key: ClientKey) {
    let transition = view.decide(key);
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

    prefix(&mut view, ClientKey::Plain('b'));
    let overlaid = rendered_text(70, 24, &view, Locale::EnUs);
    assert!(overlaid.contains("Projects"));
    assert!(overlaid.contains("Backend unavailable"));
}

#[test]
fn renders_visible_prefix_and_localized_shortcut_overlays() {
    let mut view = ClientView::new(120);
    commit(&mut view, ClientKey::ControlB);
    let prefix_mode = rendered_text(120, 40, &view, Locale::ZhCn);
    assert!(without_cell_padding(&prefix_mode).contains("前缀Ctrl+B"));

    commit(&mut view, ClientKey::Plain('?'));
    let help = rendered_text(120, 40, &view, Locale::ZhCn);
    let compact_help = without_cell_padding(&help);
    assert!(compact_help.contains("键盘快捷键"));
    assert!(help.contains("Ctrl+B c"));
    assert!(help.contains("Ctrl+B ,"));
    assert!(help.contains("Ctrl+B Ctrl+B"));
    assert!(compact_help.contains("终端输入仍会继续传递"));
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
    prefix(&mut view, ClientKey::Plain('?'));
    let rows = rendered_rows(80, 24, &view, Locale::EnUs);
    let status = &rows[23];
    assert!(status.contains("unavailable"), "{status:?}");
    for row in &rows[..23] {
        assert!(!row.contains("Backend unavailable ·"), "{row:?}");
    }

    let mut view = ClientView::new(60);
    prefix(&mut view, ClientKey::Plain('w'));
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
    prefix(&mut toggled, ClientKey::Plain('b'));
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
