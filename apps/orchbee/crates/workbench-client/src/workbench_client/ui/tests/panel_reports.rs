use super::super::*;
use crate::{
    client_view::ClientView,
    localization::Locale,
    tui_hit_map::HitMapBuilder,
    workbench_client::panel_reports::{PanelReport, Round, Scope},
};
use orchbee_workbench_protocol::WorkbenchRevision;
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

#[test]
fn panel_names_share_custom_guarded_metadata_automatic_and_position_precedence() {
    use orchbee_workbench_protocol::{PanelCustomLabel, PanelNode, PresentationLaunchProgram};
    for custom in [None, Some("Named Panel")] {
        for metadata in [None, Some("Guarded title")] {
            for program in [false, true] {
                let mut source = super::super::tests::publication_hint::split_snapshot(1);
                let PanelNode::Split {
                    children,
                    orientation,
                    ..
                } = &mut source.terminal_contexts[0].tabs[0].root
                else {
                    panic!()
                };
                *orientation = orchbee_workbench_protocol::PanelOrientation::Vertical;
                let PanelNode::Panel {
                    primary_presentation,
                    ..
                } = &mut children[0].node
                else {
                    panic!()
                };
                primary_presentation.custom_label =
                    custom.map(|label| PanelCustomLabel::parse(label).unwrap());
                primary_presentation.launch_program =
                    program.then(|| PresentationLaunchProgram::parse("pwsh.exe").unwrap());
                let (mut ui, _, _, _) =
                    super::super::tests::move_membership::ready_ui_with_snapshot(source);
                let scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
                let row = PanelReport {
                    report: None,
                    metadata: serde_json::from_value(
                        json!({"title":metadata,"displayAgent":null,"stateLabels":{}}),
                    )
                    .unwrap(),
                    tokens: Default::default(),
                };
                ui.reports.publish_for_test(Round {
                    rows: scope
                        .panels
                        .iter()
                        .map(|panel| (panel.panel_id.clone(), row.clone()))
                        .collect(),
                    scope,
                });
                let expected = |locale: Locale, include_custom: bool| {
                    include_custom
                        .then_some(custom)
                        .flatten()
                        .or(metadata)
                        .or(program.then_some("PowerShell"))
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("{} 1", locale.shell().panel_title))
                };
                for locale in [Locale::EnUs, Locale::ZhCn] {
                    let view = ui.view();
                    let tab = view.tab(0).unwrap();
                    assert_eq!(
                        view.panel_label(&tab, tab.panel(0).unwrap(), locale),
                        expected(locale, true)
                    );
                    assert!(
                        paint(
                            &ui,
                            120,
                            40,
                            locale,
                            &crate::tui_theme::Theme::terminal_native()
                        )
                        .contains(&expected(locale, true))
                    );
                }
                let pane = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
                assert!(ui.open_action_menu(
                    crate::action_menu::MenuTarget::Pane(pane),
                    None,
                    false
                ));
                for locale in [Locale::EnUs, Locale::ZhCn] {
                    assert!(
                        paint(
                            &ui,
                            120,
                            40,
                            locale,
                            &crate::tui_theme::Theme::terminal_native()
                        )
                        .contains(&expected(locale, true))
                    );
                }
                ui.cancel_action_menu();
                ui.request_panel_close_confirmation();
                for locale in [Locale::EnUs, Locale::ZhCn] {
                    assert_eq!(
                        ui.view()
                            .panel_close_confirmation()
                            .unwrap()
                            .panel_label(locale),
                        expected(locale, true)
                    );
                }
                ui.state.as_mut().unwrap().cancel_panel_close_confirmation();
                ui.request_panel_rename_draft().unwrap();
                for locale in [Locale::EnUs, Locale::ZhCn] {
                    assert_eq!(
                        ui.view().rename_draft().unwrap().target_label(locale),
                        expected(locale, false)
                    );
                }
            }
        }
    }
}

#[test]
fn completion_state_cells_use_server_facts_even_without_a_client_working_observation() {
    use orchbee_workbench_protocol::{AgentCompletion, MetadataState, PanelAgentState};
    let source = super::super::tests::publication_hint::split_snapshot(1);
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let shell = ClientView::new(120);
    for (state, completion, expected) in [
        (
            PanelAgentState::Working,
            AgentCompletion::None,
            MetadataState::Working,
        ),
        (
            PanelAgentState::Idle,
            AgentCompletion::AwaitingInput,
            MetadataState::Done,
        ),
        (
            PanelAgentState::Idle,
            AgentCompletion::Acknowledged,
            MetadataState::Idle,
        ),
    ] {
        let scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
        let row = PanelReport {
            report: Some(orchbee_workbench_protocol::PanelAgentView {
                agent: Some("test-agent".into()),
                state,
                completion,
                ..Default::default()
            }),
            metadata: serde_json::from_value(
                json!({"title":"completion-title","displayAgent":null,"stateLabels":{}}),
            )
            .unwrap(),
            tokens: Default::default(),
        };
        ui.reports.publish_for_test(Round {
            rows: scope
                .panels
                .iter()
                .map(|p| (p.panel_id.clone(), row.clone()))
                .collect(),
            scope,
        });
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &shell,
                    Locale::EnUs,
                    Some(ui.view()),
                    &theme,
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let view = ui.view();
        let tab = view.tab(0).unwrap();
        let panel_title = view.panel_label(&tab, tab.panel(0).unwrap(), Locale::EnUs);
        let (y, x) = (0..40)
            .find_map(|y| {
                let text: String = (0..120).map(|x| buffer[(x, y)].symbol()).collect();
                let prefix = &text[..text.find(panel_title.as_ref())?];
                let title_x = crate::terminal_text::display_width(prefix);
                let glyph = theme.agent_glyph(expected);
                Some((
                    y,
                    u16::try_from(title_x - crate::terminal_text::display_width(glyph) - 1)
                        .unwrap(),
                ))
            })
            .expect("a visible state cell accompanies the Panel's title");
        assert_eq!(
            Some(buffer[(x, y)].fg),
            theme
                .agent_style(expected, shell.machines.working_frame())
                .fg,
            "the state cell must use the semantic color; glyph identity alone cannot distinguish Working and Done"
        );
    }
}

#[test]
fn single_terminal_chrome_keeps_settings_in_the_sidebar_and_matches_the_live_geometry() {
    use crate::client_view::{ClientActionDisposition, KeyCode, KeyEvent, Modifiers, PrefixInput};
    use crate::machine_picker::Choice;
    use crate::tui_layout::{ViewportTier, project_shell_areas};
    use crate::tui_theme::{Role, Theme};
    use ratatui::layout::Rect;

    let mut source = super::super::tests::membership::snapshot(1, 1);
    source.terminal_contexts[0].tabs[0].label = "orchbee".into();
    let orchbee_workbench_protocol::PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    primary_presentation.label = "orchbee".into();
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    let scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    let row = PanelReport {
        report: None,
        metadata: serde_json::from_value(
            json!({"title":null,"displayAgent":null,"stateLabels":{}}),
        )
        .unwrap(),
        tokens: serde_json::from_value(json!({})).unwrap(),
    };
    ui.reports.publish_for_test(Round {
        rows: scope
            .panels
            .iter()
            .map(|p| (p.panel_id.clone(), row.clone()))
            .collect(),
        scope,
    });

    for (theme_name, theme) in [
        ("native", Theme::terminal_native()),
        ("dark", Theme::orchbee_dark()),
        ("light", Theme::orchbee_light()),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (width, height, hide) in [
                (120, 40, false),
                (120, 40, true),
                (80, 24, false),
                (60, 12, false),
                (50, 10, false),
                (15, 4, false),
            ] {
                let mut shell = ClientView::new(width);
                if hide {
                    for key in [
                        KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL),
                        KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE),
                    ] {
                        let transition = shell.decide(PrefixInput::Key(key));
                        shell.commit(transition, ClientActionDisposition::KeepNavigator);
                    }
                }
                let areas =
                    project_shell_areas(Rect::new(0, 0, width, height), shell.sidebar_surface());
                shell.machines.available = true;
                shell
                    .machines
                    .observe_agent_sidebar(areas.sidebar.is_some());
                let target = crate::workbench_projection::ClientContextTarget {
                    session_id: ui.state.as_ref().unwrap().cache().session_id().clone(),
                    location: ui.view().context_at(0).unwrap().location().clone(),
                };
                shell
                    .machines
                    .observe_local(vec![crate::machine_discovery::Context {
                        target: target.clone(),
                        project_id: None,
                        project: None,
                        workspace: None,
                    }]);
                shell
                    .machines
                    .observe_focus(Choice::Local, Some(target), None);
                shell.machines.observe_agents(vec![(
                    Choice::Local,
                    crate::machine_agents::Observation::Ready(
                        crate::machine_agents::test_support::checkpoint(Vec::new()),
                    ),
                )]);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::default();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &shell,
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let text: String = (0..height)
                    .flat_map(|y| {
                        let mut row = String::new();
                        let mut x = 0;
                        while x < width {
                            let symbol = buffer[(x, y)].symbol();
                            row.push_str(symbol);
                            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1))
                                .unwrap();
                        }
                        row.chars().collect::<Vec<_>>()
                    })
                    .collect();
                assert!(
                    ["[Menu]", "[菜单]"]
                        .iter()
                        .all(|label| !text.contains(label)),
                    "{theme_name} {width}x{height} {hide}: {text}"
                );
                let area = crate::tui_layout::arranged_pane_area(&areas, &ui.view());
                if areas.tier == ViewportTier::TooSmall {
                    assert!(area.is_none());
                    continue;
                }
                let hits = hits.finish();
                let area = area.unwrap();
                // A single pane is bare in every tier: only its scrollbar column is chrome.
                assert_eq!(area.x, areas.panel.x);
                assert_eq!(area.y, areas.panel.y);
                assert_eq!(area.width + 1, areas.panel.width);
                assert_eq!(area.height, areas.panel.height);
                // The pane reaches the last row; the status floats over it.
                assert_eq!(area.bottom(), height);
                if areas.tier.shows_panel_frame() {
                    let row: String = (areas.tabs.x..areas.tabs.right())
                        .map(|x| buffer[(x, areas.tabs.y)].symbol())
                        .collect();
                    assert!(row.contains("orchbee"), "tab label: {row}");
                    assert!(
                        !text.contains(&format!("{} 1", locale.shell().panel_title)),
                        "{text}"
                    );
                    assert!(!text.contains("[Menu]"), "{text}");
                    // The current Tab's first label cell, after its padding.
                    let tab_x = (areas.tabs.x..areas.tabs.right())
                        .find(|x| {
                            matches!(
                                hits.hit_target_at(*x, areas.tabs.y),
                                Some(crate::tui_hit_map::HitTarget::Tab(_))
                            ) && buffer[(*x, areas.tabs.y)].symbol() != " "
                        })
                        .unwrap();
                    let current_tab = &buffer[(tab_x, areas.tabs.y)];
                    let current_content =
                        theme.current_context_style(theme.style(Role::TextPrimary));
                    assert_eq!(current_tab.bg, theme.color(Role::SurfaceCurrent));
                    assert_eq!(current_tab.style().fg, current_content.fg);
                    for y in area.y..area.bottom() {
                        assert!(!matches!(buffer[(area.x, y)].symbol(), "│" | "└" | "╰"));
                        assert!(!matches!(
                            buffer[(area.right() - 1, y)].symbol(),
                            "│" | "┘" | "╯"
                        ));
                    }
                }
                assert_eq!(
                    (0..height).any(|y| (0..width).any(|x| matches!(
                        hits.hit_target_at(x, y),
                        Some(crate::tui_hit_map::HitTarget::SettingsButton)
                    ))),
                    areas.sidebar.is_some()
                );
            }
        }
    }
}

fn paint(
    ui: &WorkbenchClientUi,
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
                Some(ui.view()),
                theme,
                &mut HitMapBuilder::default(),
            );
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width))
        .map(|row| {
            let mut text = String::new();
            let mut column = 0;
            while let Some(cell) = row.get(column) {
                text.push_str(cell.symbol());
                column += crate::terminal_text::display_width(cell.symbol()).max(1);
            }
            text
        })
        .collect()
}
#[test]
fn split_panel_titles_paint_reports_and_reject_stale_workbench_scope() {
    let mut source = super::super::tests::publication_hint::split_snapshot(1);
    let orchbee_workbench_protocol::PanelNode::Split { orientation, .. } =
        &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    *orientation = orchbee_workbench_protocol::PanelOrientation::Vertical;
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    let scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    let row=PanelReport { report:Some(serde_json::from_value(json!({"agent":"grok","state":"blocked","authoritySource":null,"message":null})).unwrap()),
        metadata:serde_json::from_value(json!({"title":"Approval title","displayAgent":"Agent","stateLabels":{"blocked":"Review needed"}})).unwrap(),tokens:serde_json::from_value(json!({"branch":"test"})).unwrap() };
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
        crate::tui_theme::Theme::orchbee_light(),
        crate::tui_theme::Theme::orchbee_dark(),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (width, height) in [(120, 40), (80, 24), (60, 12), (59, 11), (19, 4)] {
                let text = paint(&ui, width, height, locale, &theme);
                // The state glyph leads the title; state words stay in the sidebar.
                assert!(!text.contains("Review needed"), "{width}x{height}: {text}");
                if width == 120 {
                    let glyph =
                        theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Blocked);
                    assert!(text.contains(glyph), "{text}");
                    assert!(text.contains("branch=test"), "{text}");
                    assert!(text.contains("Approval title"), "{text}");
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
    assert!(!text.contains("branch=test"));
}

#[test]
fn a_bare_single_panel_leaves_reports_to_split_and_zoomed_titles() {
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui();
    let scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    let row = PanelReport {
        report: Some(serde_json::from_value(json!({"agent":"grok","state":"blocked","authoritySource":null,"message":null})).unwrap()),
        metadata: serde_json::from_value(json!({"title":"Approval title","displayAgent":"Review Agent","stateLabels":{"blocked":"Review needed"}})).unwrap(),
        tokens: serde_json::from_value(json!({"branch":"test"})).unwrap(),
    };
    ui.reports.publish_for_test(Round {
        rows: scope
            .panels
            .iter()
            .map(|panel| (panel.panel_id.clone(), row.clone()))
            .collect(),
        scope,
    });
    let native = ui.view();
    assert!(
        matches!(native.panel_report(native.tab(0).unwrap().panel(0).unwrap()),
        crate::workbench_client::PanelReportView::Ready(observed)
        if observed.metadata.title.as_ref().unwrap().as_str() == "Approval title")
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            crate::tui_theme::Theme::terminal_native(),
            crate::tui_theme::Theme::orchbee_light(),
        ] {
            for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (19, 4)] {
                let text = paint(&ui, width, height, locale, &theme);
                for label in [
                    "Approval title",
                    "Review Agent",
                    "Review needed",
                    "branch=test",
                ] {
                    assert!(!text.contains(label), "{width}x{height}: {text}");
                }
            }
        }
    }
}

#[test]
fn child_selection_discards_primary_report_and_paints_its_own_exact_observation() {
    use orchbee_workbench_protocol::{
        PanelNode, Presentation, PresentationLaunchProgram, TerminalPresentationId,
    };
    let mut source = super::super::tests::publication_hint::split_snapshot(1);
    let child_id = TerminalPresentationId::parse(&format!("{:032x}", 9000)).unwrap();
    let PanelNode::Split {
        children,
        orientation,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    *orientation = orchbee_workbench_protocol::PanelOrientation::Vertical;
    let PanelNode::Panel {
        child_presentations,
        primary_presentation,
        ..
    } = &mut children[0].node
    else {
        unreachable!()
    };
    primary_presentation.launch_program =
        Some(PresentationLaunchProgram::parse("pwsh.exe").unwrap());
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
        launch_failure: None,
        agent_not_resumed: None,
        launch_program: None,
    });
    let (mut ui, _mailbox, _, _) =
        super::super::tests::move_membership::ready_ui_with_snapshot(source);
    let primary_scope = Scope::capture(ui.state.as_ref().unwrap()).unwrap();
    let row = |title: &str, token: &str| {
        PanelReport {
        report: Some(serde_json::from_value(json!({"agent":"grok","state":"blocked","authoritySource":"custom","message":null})).unwrap()),
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
        super::super::super::PanelReportView::Ready(_)
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
        let view = ui.view();
        let tab = view.tab(0).unwrap();
        assert_eq!(
            view.panel_label(&tab, tab.panel(0).unwrap(), locale),
            "PowerShell"
        );
        assert!(!text.contains("Primary report") && !text.contains("branch=main"));
    }
    ui.reports.publish_for_test(Round {
        scope: child_scope.clone(),
        rows: std::collections::BTreeMap::from([(
            child_scope.panels[0].panel_id.clone(),
            row("Child report", "child"),
        )]),
    });
    let theme = crate::tui_theme::Theme::terminal_native();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = paint(&ui, 120, 40, locale, &theme);
        assert!(text.contains("branch=child"), "{text}");
        assert!(!text.contains("Primary report"));
    }
}
