use super::move_membership_tests::ready_ui;
use super::*;
use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_session::{
        TerminalSessionView,
        panel_reports::{PanelReport, Round, Scope},
    },
    tui_pointer::HitMapBuilder,
};
use orchbee_workbench_protocol::WorkbenchRevision;
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
fn paint(
    ui: &NativeUi,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &crate::tui_theme::Theme,
) -> String {
    let shell = ClientView::new(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &shell,
                locale,
                Some(TerminalSessionView::Native(ui.view())),
                theme,
                &mut HitMapBuilder::default(),
            );
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn actual_single_panel_header_paints_reports_and_rejects_stale_workbench_scope() {
    let (mut ui, _, _, _) = ready_ui();
    let scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    let row=PanelReport { report:Some(serde_json::from_value(json!({"agent":"grok","state":"blocked","authoritySource":null,"message":null,"session":null})).unwrap()),
        metadata:serde_json::from_value(json!({"title":"Approval title","displayAgent":"Review Agent","stateLabels":{"blocked":"Review needed"}})).unwrap(),tokens:serde_json::from_value(json!({"branch":"test"})).unwrap() };
    let rows = scope
        .panels
        .iter()
        .map(|(p, _)| (p.clone(), row.clone()))
        .collect();
    ui.reports.publish_for_test(Round {
        scope: scope.clone(),
        rows,
    });
    for theme in [
        crate::tui_theme::Theme::terminal_native(),
        crate::tui_theme::Theme::rgb_test_dark(),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (width, height, framed) in [
                (120, 40, true),
                (80, 24, true),
                (60, 12, true),
                (59, 11, false),
                (19, 4, false),
            ] {
                let text = paint(&ui, width, height, locale, &theme);
                for label in ["Approval title", "Review needed", "Review Agent"] {
                    assert_eq!(text.contains(label), framed, "{width}x{height}: {text}");
                }
                if width == 120 {
                    assert!(text.contains("branch=test"));
                }
            }
        }
    }
    let mut stale = scope;
    stale.revision = WorkbenchRevision::new(stale.revision.value() + 1);
    ui.reports.publish_for_test(Round {
        scope: stale,
        rows: Default::default(),
    });
    let text = paint(
        &ui,
        120,
        40,
        Locale::EnUs,
        &crate::tui_theme::Theme::terminal_native(),
    );
    assert!(!text.contains("Approval title"));
    assert!(!text.contains("Review needed"));
    assert!(text.contains("loading status"));
}
