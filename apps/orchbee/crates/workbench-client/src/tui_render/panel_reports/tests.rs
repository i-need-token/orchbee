use super::*;
use crate::workbench_client::PanelReport;
use serde_json::json;

fn row() -> PanelReport {
    PanelReport {
        report: Some(
            serde_json::from_value(
                json!({"agent":"grok","state":"blocked","authoritySource":null,"message":null}),
            )
            .unwrap(),
        ),
        metadata: serde_json::from_value(
            json!({"title":null,"displayAgent":"审核 Agent","stateLabels":{"blocked":"等待确认"}}),
        )
        .unwrap(),
        tokens: serde_json::from_value(json!({"branch":"feature"})).unwrap(),
    }
}

#[test]
fn custom_and_display_titles_never_inject_terminal_controls() {
    // Metadata titles are rejected at the contract boundary; Presentation labels arrive raw.
    let report = row();
    for display in ["自定义标题\u{1b}[31m\n\u{202e}", "名称\u{1b}]0;x\u{7}"] {
        let text = title(display, PanelReportView::Ready(&report), "·");
        assert!(text.ends_with("branch=feature"), "{text:?}");
        assert!(
            !text
                .chars()
                .any(crate::terminal_text::is_forbidden_terminal_scalar),
            "{text:?}"
        );
    }
}

#[test]
fn only_a_reported_agent_has_a_state() {
    let mut agentless = row();
    agentless.report = None;
    agentless.metadata.display_agent = None;
    for view in [
        PanelReportView::Pending,
        PanelReportView::Unavailable,
        PanelReportView::Ready(&agentless),
    ] {
        assert_eq!(agent_state(view), None);
    }
    let mut unreported = row();
    unreported.report = None;
    assert_eq!(
        agent_state(PanelReportView::Ready(&unreported)),
        Some(MetadataState::Unknown)
    );
    for (state, expected) in [
        (PanelAgentState::Working, MetadataState::Working),
        (PanelAgentState::Blocked, MetadataState::Blocked),
        (PanelAgentState::Done, MetadataState::Done),
        (PanelAgentState::Error, MetadataState::Error),
        (PanelAgentState::Idle, MetadataState::Idle),
        (PanelAgentState::Unknown, MetadataState::Unknown),
    ] {
        let mut report = row();
        report.report.as_mut().unwrap().state = state;
        assert_eq!(agent_state(PanelReportView::Ready(&report)), Some(expected));
    }
}
