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
        .map(|p| (p.panel_id.clone(), row.clone()))
        .collect();
    ui.reports.publish_for_test(Round {
        scope: scope.clone(),
        rows,
    });
    for theme in [
        crate::tui_theme::Theme::terminal_native(),
        crate::tui_theme::Theme::rgb_test_dark(),
        crate::tui_theme::Theme::one_light(),
        crate::tui_theme::Theme::one_dark(),
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

#[test]
fn child_selection_discards_primary_report_and_paints_its_own_exact_observation() {
    use orchbee_workbench_protocol::{PanelNode, Presentation, TerminalPresentationId};
    let mut source = super::membership_tests::snapshot(2, 1);
    let child_id = TerminalPresentationId::parse(&format!("{:032x}", 9000)).unwrap();
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations.push(Presentation {
        presentation_id: child_id.clone(),
        label: "child shell".into(),
        custom_label: None,
        binding: Some(
            serde_json::from_value(
                json!({"workerId":format!("{:032x}",9001),"terminalId":format!("{:032x}",9002)}),
            )
            .unwrap(),
        ),
    });
    let (mut ui, _mailbox, _, _) = super::move_membership_tests::ready_ui_with_snapshot(source);
    let primary_scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    let row = |title: &str, token: &str| {
        PanelReport {
        report: Some(serde_json::from_value(json!({"agent":"grok","state":"blocked","authoritySource":"custom","message":null,"session":null})).unwrap()),
        metadata: serde_json::from_value(json!({"title":title,"displayAgent":"Child Agent","stateLabels":{"blocked":"Approval"}})).unwrap(),
        tokens: serde_json::from_value(json!({"branch":token})).unwrap(),
    }
    };
    let primary_round = Round {
        scope: primary_scope.clone(),
        rows: std::collections::BTreeMap::from([(
            primary_scope.panels[0].panel_id.clone(),
            row("Primary report", "main"),
        )]),
    };
    ui.reports.publish_for_test(primary_round);
    assert!(matches!(
        ui.view()
            .panel_report(ui.view().tab(0).unwrap().panel(0).unwrap()),
        super::super::PanelReportView::Ready(_)
    ));
    ui.apply_client_action(ClientAction::NextPresentation);
    let child_scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    assert_eq!(child_scope.revision, primary_scope.revision);
    assert_eq!(
        child_scope.panels[0].panel_id,
        primary_scope.panels[0].panel_id
    );
    assert_eq!(child_scope.panels[0].presentation_id, child_id);
    assert_ne!(
        child_scope.panels[0].binding,
        primary_scope.panels[0].binding
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = paint(
            &ui,
            120,
            40,
            locale,
            &crate::tui_theme::Theme::terminal_native(),
        );
        assert!(text.contains("child shell"));
        assert!(!text.contains("Primary report") && !text.contains("branch=main"));
    }
    ui.reports.publish_for_test(Round {
        scope: child_scope.clone(),
        rows: std::collections::BTreeMap::from([(
            child_scope.panels[0].panel_id.clone(),
            row("Child report", "child"),
        )]),
    });
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = paint(
            &ui,
            120,
            40,
            locale,
            &crate::tui_theme::Theme::terminal_native(),
        );
        assert!(
            text.contains("Child report")
                && text.contains("Approval")
                && text.contains("branch=child"),
            "{text}"
        );
        assert!(!text.contains("Primary report"));
    }
}
