use super::super::{move_membership_tests::ready_ui, viewport_tests::install_projection};
use super::*;
use crate::terminal_session::agent_read_lane::AttentionRound;
use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_session::{NativeDrawReceipt, TerminalSessionView},
    tui_pointer::HitMapBuilder,
};
use orchbee_agent_observation_protocol::{AgentChangesCursor, AgentSessionSummary};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

fn fixture() -> (NativeUi, PaneRef) {
    let (mut ui, _, _, _) = ready_ui();
    let pane = install_projection(&mut ui, Some((9, false, 24)), 0);
    let presentation = ui.view().attention_presentation(&pane).unwrap();
    let row: AgentSessionSummary = serde_json::from_value(json!({
        "executionLocation":{"executionLocationId":"local"}, "lifecycle":"live",
        "observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":1,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,
        "presentationAttachments":[{"generation":"g1","navigable":true,"presentationId":presentation,"visibility":"visible"}],
        "providerId":"test","sessionId":"agent-1","status":{"kind":"succeeded"}
    })).unwrap();
    ui.attention.publish_for_test(&AttentionRound {
        items: vec![row],
        changes: vec![],
        prior_revision: None,
        revision: 1,
        cursor: AgentChangesCursor::parse(&format!("v1.{}.{}", "A".repeat(59), "A".repeat(43)))
            .unwrap(),
        reset: true,
    });
    (ui, pane)
}

fn draw(ui: &NativeUi, width: u16, height: u16, shell: &ClientView) -> NativeDrawReceipt {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                shell,
                Locale::EnUs,
                Some(TerminalSessionView::Native(ui.view())),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    receipt
}

#[test]
fn attention_render_receipts_only_acknowledge_visible_live_panes_after_success() {
    let (mut ui, pane) = fixture();
    let shell = ClientView::new(120);
    let count = |ui: &NativeUi| ui.view().panel_unseen_count(pane.pane().panel());
    let receipt = draw(&ui, 120, 40, &shell);
    assert_eq!(count(&ui), 1, "rendering alone never acknowledges");
    ui.observe_focus(false);
    ui.after_close_draw(receipt);
    assert_eq!(count(&ui), 1);
    ui.observe_focus(true);
    assert!(ui.attention.tick(), "focus gain requests a fresh paint");
    assert!(
        !ui.attention.tick(),
        "one focus event cannot sustain a redraw loop"
    );
    ui.after_close_draw(draw(&ui, 8, 2, &shell));
    assert_eq!(count(&ui), 1, "too small has no visible pane content");
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(count(&ui), 0);
}

#[test]
fn attention_hidden_history_overlay_and_changed_pane_reject_receipts() {
    let (mut ui, pane) = fixture();
    let shell = ClientView::new(120);
    ui.board.open();
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
    ui.board.close();
    let receipt = draw(&ui, 120, 40, &shell);
    ui.projections[0].status = NativeProjectionStatus::Empty;
    ui.after_close_draw(receipt);
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
    ui.projections[0].status = NativeProjectionStatus::FrameAvailable;
    ui.enter_history();
    assert!(ui.view().pane_viewport(pane.pane().panel()).is_some());
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
}
