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
fn provided_titles_and_creation_numbers_survive_layout_reordering() {
    let report = row();
    assert_eq!(
        title(
            Some("1"),
            "Shell",
            "Terminal",
            PanelReportView::Ready(&report),
            "·",
        ),
        "1 · branch=feature"
    );

    let mut metadata = row();
    metadata.metadata.title = Some(serde_json::from_value(json!("Build Shell")).unwrap());
    metadata.tokens = serde_json::from_value(json!({})).unwrap();
    assert_eq!(
        title(
            None,
            "Shell",
            "Terminal",
            PanelReportView::Ready(&metadata),
            "·"
        ),
        "Build Shell"
    );

    assert_eq!(
        title(None, "Shell", "Terminal", PanelReportView::Pending, "·"),
        "Shell"
    );

    for inherited_number in ["1", "5"] {
        assert_eq!(
            title(
                None,
                inherited_number,
                "Terminal",
                PanelReportView::Pending,
                "·"
            ),
            format!("Terminal {inherited_number}"),
        );
    }
}

#[test]
fn custom_and_display_titles_never_inject_terminal_controls() {
    // Metadata titles are rejected at the contract boundary; Presentation labels arrive raw.
    let report = row();
    for (custom, display) in [
        (Some("自定义标题\u{1b}[31m\n\u{202e}"), "Shell"),
        (None, "名称\u{1b}]0;x\u{7}"),
    ] {
        let text = title(
            custom,
            display,
            "Terminal",
            PanelReportView::Ready(&report),
            "·",
        );
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
