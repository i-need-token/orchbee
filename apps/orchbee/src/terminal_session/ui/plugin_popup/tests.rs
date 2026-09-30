use super::*;
use orchbee_workbench_protocol::{
    ClientPopupSource, ClientPresentationSelection, ClientViewSequence, PluginInvocationContext,
    PluginPaneSelector, PluginPopupSpec, WorkbenchClientId, WorkbenchRevision,
};

#[test]
fn popup_admission_rejects_changed_source_generation_or_incarnation_before_a_lane() {
    let source = super::super::membership_tests::snapshot(2, 1);
    let (mut ui, _, _, _) =
        super::super::move_membership_tests::ready_ui_with_snapshot(source.clone());
    super::super::viewport_tests::install_projection(&mut ui, Some((9, false, 24)), 0);
    let (_, observation) = ui.control_observation(false).unwrap();
    let selected = observation.selection.unwrap();
    let panel_id = selected.panel_id.unwrap();
    let mut command = ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&"a".repeat(32)).unwrap(),
        client_id: WorkbenchClientId::parse(&"b".repeat(32)).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: source.revision,
        intent: ClientViewIntent::Presentation {
            panel_id: panel_id.clone(),
        },
    };
    let member = ui
        .read_client_presentation(&command, &source)
        .unwrap()
        .presentation
        .unwrap();
    let spec = PluginPopupSpec {
        selector: PluginPaneSelector::new("tools".into(), "dashboard".into()).unwrap(),
        source: ClientPopupSource {
            terminal_context_id: selected.terminal_context_id,
            tab_id: selected.tab_id.clone().unwrap(),
            panel_id: panel_id.clone(),
            member,
        },
        context: Box::new(PluginInvocationContext {
            tab_id: Some(selected.tab_id.unwrap().to_string()),
            focused_pane_id: Some(panel_id.to_string()),
            ..Default::default()
        }),
        cwd: None,
        environment: Default::default(),
        width: None,
        height: None,
    };
    for incarnation_change in [false, true] {
        let mut stale = spec.clone();
        let ClientPresentationSelection::Bound {
            attachment_generation,
            incarnation,
            ..
        } = &mut stale.source.member
        else {
            panic!()
        };
        if incarnation_change {
            *incarnation += 1;
        } else {
            *attachment_generation = WorkbenchRevision::new(attachment_generation.value() + 1);
        }
        command.intent = ClientViewIntent::PopupOpen {
            spec: Box::new(stale),
        };
        assert_eq!(
            ui.begin_popup_control(&command, Some(&source)),
            Err(Reason::StaleView)
        );
        assert!(!ui.popup.busy());
    }
}

#[test]
fn popup_pending_and_closing_composition_hide_the_underlying_terminal_cursor() {
    let (mut ui, _, _, _) = super::super::move_membership_tests::ready_ui();
    super::super::viewport_tests::install_projection(&mut ui, Some((9, false, 24)), 0);
    let id = WorkbenchRequestId::parse(&"a".repeat(32)).unwrap();
    for popup in [None, Some(false), Some(true)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                let mut view = ui.view();
                view.popup = popup.map(|closing| crate::terminal_session::PopupView {
                    id: &id,
                    title: "Dashboard",
                    projection: None,
                    width: None,
                    height: None,
                    closing,
                });
                crate::tui_render::render_terminal_shell(
                    frame,
                    &crate::client_view::ClientView::new(120),
                    crate::localization::Locale::EnUs,
                    Some(crate::terminal_session::TerminalSessionView::Native(view)),
                    &crate::tui_theme::Theme::rgb_test_dark(),
                    &mut crate::tui_pointer::HitMapBuilder::new(),
                );
            })
            .unwrap();
        assert_eq!(terminal.backend().cursor_visible(), popup.is_none());
    }
}
