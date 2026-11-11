use super::directory_guidance;
use crate::localization::Locale;
use ratatui::{Terminal, backend::TestBackend, text::Line};

#[test]
fn project_picker_windows_large_lists_and_records_only_painted_choices() {
    use super::*;
    use crate::tui_theme::Theme;
    use std::cell::Cell;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            for (width, height) in [(80, 24), (120, 40), (60, 12), (50, 18), (20, 5)] {
                for selected in [0, 100, 200, 201] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let token = SetupToken::for_test(17);
                    let mut hits = HitMapBuilder::new();
                    let long = format!("Project 界{}\x1b[31m", "猫".repeat(500));
                    let visited = Cell::new(0);
                    let mut result = None;
                    terminal
                        .draw(|frame| {
                            frame.render_widget(
                                Paragraph::new("status"),
                                Rect::new(0, height - 1, width, 1),
                            );
                            result = Some(render_projects(
                                frame,
                                locale,
                                &ProjectPicker {
                                    token,
                                    stage: SetupStage::ChooseWorkspaceProject,
                                    workspace: Some("Workspace 界"),
                                    count: 202,
                                    selected,
                                },
                                &theme,
                                &mut hits,
                                |index| {
                                    visited.set(visited.get() + 1);
                                    Some(ProjectChoice {
                                        position: index + 1,
                                        label: match index {
                                            200 => locale.setup_ungrouped(),
                                            201 => locale.setup_new_project(),
                                            _ => &long,
                                        },
                                    })
                                },
                            ));
                        })
                        .unwrap();
                    let (area, presented) = result.unwrap();
                    assert!(area.bottom() < height);
                    let map = hits.finish();
                    let mut choices = std::collections::BTreeSet::new();
                    let mut selected_row = None;
                    for y in 0..height {
                        for x in 0..width {
                            if let Some(HitTarget::Setup {
                                token: painted,
                                control: ModalControl::Choice(index),
                            }) = map.hit_target_at(x, y)
                            {
                                assert_eq!(*painted, token);
                                assert!(*index < 202);
                                assert!(
                                    !terminal.backend().buffer()[(x, y)]
                                        .symbol()
                                        .contains('\x1b')
                                );
                                choices.insert(*index);
                                if *index == selected {
                                    selected_row = Some(y);
                                }
                            }
                        }
                    }
                    if width == 20 {
                        assert!(matches!(presented, ModalPresentation::NotPresented));
                        assert!(area.is_empty());
                        assert!(choices.is_empty());
                    } else {
                        assert!(
                            matches!(presented, ModalPresentation::Presented(value) if value == token)
                        );
                        assert!(
                            choices.contains(&selected),
                            "{width}x{height} selected {selected}: {choices:?}"
                        );
                        let inner = area.inner(ratatui::layout::Margin::new(3, 1));
                        assert_eq!(
                            terminal.backend().buffer()[(inner.x, selected_row.unwrap())].symbol(),
                            theme.glyphs.navigation_marker,
                        );
                        assert!(choices.len() <= usize::from(height - 7));
                        let text = terminal
                            .backend()
                            .buffer()
                            .content()
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>();
                        // Wide-glyph continuation cells read as spaces.
                        assert!(
                            text.replace(' ', "")
                                .contains(&locale.setup_close_hint().replace(' ', ""))
                        );
                    }
                    assert!(
                        visited.get() <= usize::from(height),
                        "must not traverse hidden Projects"
                    );
                    assert_eq!(terminal.backend().buffer()[(0, height - 1)].symbol(), "s");
                }
            }
        }
    }
}

#[test]
fn missing_project_rows_never_acquire_a_click_target() {
    use super::*;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut area = Rect::default();
    terminal
        .draw(|frame| {
            area = render_projects(
                frame,
                Locale::EnUs,
                &ProjectPicker {
                    token: SetupToken::for_test(1),
                    stage: SetupStage::ChooseWorkspaceProject,
                    workspace: Some("Workspace"),
                    count: 3,
                    selected: 0,
                },
                &Theme::terminal_native(),
                &mut hits,
                |index| {
                    (index != 1).then_some(ProjectChoice {
                        position: index + 1,
                        label: "Project",
                    })
                },
            )
            .0;
        })
        .unwrap();
    let map = hits.finish();
    let first = (area.y..area.bottom())
        .find(|y| {
            matches!(
                map.hit_target_at(area.x + 3, *y),
                Some(HitTarget::Setup {
                    control: ModalControl::Choice(0),
                    ..
                })
            )
        })
        .unwrap();
    assert!(matches!(
        map.hit_target_at(area.x + 3, first + 1),
        Some(HitTarget::ListScroll(
            crate::tui_hit_map::ListScrollTarget::Setup(_)
        ))
    ));
    assert!(matches!(
        map.hit_target_at(area.x + 3, first + 2),
        Some(HitTarget::Setup {
            control: ModalControl::Choice(2),
            ..
        })
    ));
}

#[test]
fn refused_project_picker_preserves_the_underlay_and_prior_hit_map() {
    use super::*;
    let width = 20;
    let height = 5;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record(
        HitLayer::Overlay,
        Rect::new(0, 0, 1, 1),
        HitTarget::OverlaySurface,
    );
    let mut result = None;
    terminal
        .draw(|frame| {
            let frame_area = frame.area();
            let underlay = Text::from(
                (0..height)
                    .map(|_| Line::from("x".repeat(usize::from(width))))
                    .collect::<Vec<_>>(),
            );
            frame.render_widget(Paragraph::new(underlay), frame_area);
            result = Some(render_projects(
                frame,
                Locale::EnUs,
                &ProjectPicker {
                    token: SetupToken::for_test(1),
                    stage: SetupStage::ChooseWorkspaceProject,
                    workspace: Some("Workspace"),
                    count: 2,
                    selected: 0,
                },
                &Theme::terminal_native(),
                &mut hits,
                |index| {
                    Some(ProjectChoice {
                        position: index + 1,
                        label: "Project",
                    })
                },
            ));
        })
        .unwrap();
    let (area, presentation) = result.unwrap();
    assert!(area.is_empty());
    assert!(matches!(presentation, ModalPresentation::NotPresented));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.symbol() == "x")
    );
    let map = hits.finish();
    assert!(matches!(
        map.hit_target_at(0, 0),
        Some(HitTarget::OverlaySurface)
    ));
    for y in 0..height {
        for x in 0..width {
            if (x, y) != (0, 0) {
                assert!(map.hit_target_at(x, y).is_none());
            }
        }
    }
}

#[test]
fn directory_actions_only_replace_a_live_empty_panel() {
    use crate::client_view::{ClientAction, PrefixAction};
    use crate::localization::DisplayPhase as Phase;
    use crate::localization::ShortcutLabel;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for phase in [
            Phase::Starting,
            Phase::Ready,
            Phase::Creating,
            Phase::CreateCommittedNeedDetach,
            Phase::ProjectionUnavailable,
            Phase::MoveProjectionUnavailable,
            Phase::RenameProjectionUnavailable,
            Phase::Switching,
            Phase::SelectedWithoutAttachment,
            Phase::RetryableAttachUnavailable,
            Phase::Retired,
        ] {
            assert!(directory_guidance(locale, phase, false, Some("/workspace"), false).is_none());
            assert!(directory_guidance(locale, phase, false, None, true).is_none());
        }
        assert!(
            directory_guidance(locale, Phase::Empty, true, Some("/workspace"), false).is_none()
        );
        assert!(directory_guidance(locale, Phase::Empty, true, None, true).is_none());
        let needed = directory_guidance(locale, Phase::Empty, false, None, true).unwrap();
        assert_eq!(needed.fact, locale.setup_directory_needed());
        assert_eq!(
            needed.action,
            PrefixAction::Client(ClientAction::RequestWorkspaceDirectory)
        );
        assert_eq!(needed.label, ShortcutLabel::WorkspaceDirectory);
        let ready =
            directory_guidance(locale, Phase::Empty, false, Some("/workspace"), false).unwrap();
        assert_eq!(ready.fact, locale.setup_directory_ready("/workspace"));
        assert_eq!(ready.action, PrefixAction::Client(ClientAction::CreateTab));
        assert_eq!(ready.label, ShortcutLabel::NewTab);
    }
}

#[test]
fn directory_guidance_uses_the_effective_action_binding_only_without_a_capture_owner() {
    use super::*;
    use crate::client_view::ClientView;
    use crate::localization::DisplayPhase;
    use crate::tui_theme::Theme;
    use orchbee_local_runtime::{KeyChord, KeySettings};

    let mut client = ClientView::new(100);
    client.configure_keys(&KeySettings {
        commands: Vec::new(),
        prefix: KeyChord {
            key: "a".into(),
            ctrl: true,
            alt: false,
            shift: false,
            super_key: false,
        },
        overrides: Vec::new(),
    });
    let theme = Theme::terminal_native();
    let draw = |guidance: &DirectoryGuidance, admitted| {
        let mut terminal = Terminal::new(TestBackend::new(100, 3)).unwrap();
        terminal
            .draw(|frame| {
                render_directory_guidance(
                    frame,
                    frame.area(),
                    guidance,
                    client.bindings(),
                    Locale::EnUs,
                    admitted,
                    &theme,
                );
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    };

    let ready = directory_guidance(
        Locale::EnUs,
        DisplayPhase::Empty,
        false,
        Some("C:/workspace"),
        false,
    )
    .unwrap();
    let visible = draw(&ready, true);
    assert!(visible.contains("Directory: C:/workspace"));
    assert!(
        visible.contains(&crate::terminal_events::keys::tests::host_label(
            "Ctrl+a c New Tab"
        ))
    );
    // Shown only to a connected Client, the hint never claims a connection is still required.
    assert!(!visible.contains("requires connection"));
    assert!(!visible.contains(&crate::terminal_events::keys::tests::host_label("Ctrl+B c")));

    let needed = directory_guidance(Locale::EnUs, DisplayPhase::Empty, false, None, true).unwrap();
    let visible = draw(&needed, true);
    assert!(visible.contains("No directory configured."));
    assert!(
        visible.contains(&crate::terminal_events::keys::tests::host_label(
            "Ctrl+a e Set Workspace directory"
        ))
    );
    assert!(!visible.contains(&crate::terminal_events::keys::tests::host_label("Ctrl+B e")));

    let covered = draw(&ready, false);
    assert!(covered.contains("Directory: C:/workspace"));
    assert!(!covered.contains(&crate::terminal_events::keys::tests::host_label("Ctrl+a c")));
}
