use super::*;
use orchbee_workbench_protocol::{
    ClientPopupSource, ClientPresentationSelection, ClientViewSequence, PluginInvocationContext,
    PluginPaneSelector, PluginPopupSpec, WorkbenchClientId, WorkbenchRevision,
};

#[test]
fn popup_admission_rejects_changed_source_generation_or_incarnation_before_a_lane() {
    let source = super::super::tests::membership::snapshot(2, 1);
    let (mut ui, _, _, _) =
        super::super::tests::move_membership::ready_ui_with_snapshot(source.clone());
    super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
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
        for kind in 0..3 {
            command.intent = match kind {
                0 => ClientViewIntent::EditorOpen {
                    spec: Box::new(orchbee_workbench_protocol::ScrollbackEditorSpec {
                        source: stale.source.clone(),
                    }),
                },
                1 => ClientViewIntent::PopupOpen {
                    spec: Box::new(stale.clone()),
                },
                _ => ClientViewIntent::CommandPopupOpen {
                    spec: Box::new(orchbee_workbench_protocol::ConfiguredCommandPopupSpec {
                        command_id: "cmd_tools".into(),
                        source: stale.source.clone(),
                        context: stale.context.clone(),
                        width: None,
                        height: None,
                    }),
                },
            };
            assert_eq!(
                ui.begin_popup_control(&command, Some(&source)),
                Err(Reason::StaleView)
            );
            assert!(!ui.popup.busy());
        }
    }
}

#[test]
fn popup_pending_and_closing_composition_hide_the_underlying_terminal_cursor() {
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    for popup in [None, Some(false), Some(true)] {
        for size in [
            None,
            Some(orchbee_workbench_protocol::PluginPopupSize::Percent(100)),
        ] {
            ui.popup = crate::workbench_client::independent_popup::Driver::composition_fixture(
                popup, size,
            );
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|frame| {
                    let view = ui.view();
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &crate::client_view::ClientView::new(120),
                        crate::localization::Locale::EnUs,
                        Some(view),
                        &crate::tui_theme::Theme::rgb_test_dark(),
                        &mut crate::tui_pointer::HitMapBuilder::new(),
                    );
                })
                .unwrap();
            assert_eq!(terminal.backend().cursor_visible(), popup.is_none());
        }
    }
}

#[test]
fn too_small_popup_root_frame_shows_plain_recovery_and_restores_the_surface_after_resize() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        tui_pointer::{HitMapBuilder, HitTarget},
        tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};

    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.popup =
        crate::workbench_client::independent_popup::Driver::composition_fixture(Some(false), None);

    for locale in [Locale::EnUs, Locale::ZhCn] {
        let theme = Theme::one_dark();
        let client = ClientView::new(100);
        let mut too_small = Terminal::new(TestBackend::new(100, 4)).unwrap();
        let mut too_small_hits = HitMapBuilder::new();
        too_small
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &client,
                    locale,
                    Some(ui.view()),
                    &theme,
                    &mut too_small_hits,
                );
            })
            .unwrap();
        let too_small_map = too_small_hits.finish();
        for row in 0..4 {
            for column in 0..100 {
                if let Some(target) = too_small_map.hit_target_at(column, row) {
                    assert_eq!(
                        target,
                        &HitTarget::OverlaySurface,
                        "TooSmall popup published {target:?}"
                    );
                }
            }
        }
        let too_small_text = too_small
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(
            too_small_text
                .replace(' ', "")
                .contains(&locale.plugin_popup_resize_recovery().replace(' ', "")),
            "{locale:?}: {too_small_text:?}"
        );
        assert!(!too_small_text.contains("Esc"), "{too_small_text:?}");
        assert!(!too_small_text.contains("Ctrl"), "{too_small_text:?}");

        let mut restored = Terminal::new(TestBackend::new(100, 24)).unwrap();
        let mut restored_hits = HitMapBuilder::new();
        restored
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &client,
                    locale,
                    Some(ui.view()),
                    &theme,
                    &mut restored_hits,
                );
            })
            .unwrap();
        let restored_map = restored_hits.finish();
        assert!((0..24).any(|row| {
            (0..100).any(|column| {
                matches!(
                    restored_map.hit_target_at(column, row),
                    Some(HitTarget::Popup { .. } | HitTarget::PopupClose(_))
                )
            })
        }));
        let restored_text = restored
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(
            restored_text
                .replace(' ', "")
                .contains(&locale.plugin_popup_pending(false).replace(' ', "")),
            "{locale:?}: {restored_text:?}"
        );
        assert!(
            !restored_text
                .replace(' ', "")
                .contains(&locale.plugin_popup_resize_recovery().replace(' ', "")),
            "{locale:?}: {restored_text:?}"
        );
    }
}
