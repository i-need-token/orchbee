use super::{directory_guidance, render_content};
use crate::{localization::Locale, terminal_session::NativeSetupStage};
use ratatui::{
    Terminal,
    backend::TestBackend,
    text::{Line, Text},
};

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
                    let token = NativeSetupToken::for_test(17);
                    let mut hits = HitMapBuilder::new();
                    hits.start_modal();
                    let long = format!("Project 界{}\x1b[31m", "猫".repeat(500));
                    let visited = Cell::new(0);
                    let mut result = None;
                    terminal
                        .draw(|frame| {
                            let content = Rect::new(0, 0, width, height - 1);
                            frame.render_widget(
                                Paragraph::new("status"),
                                Rect::new(0, height - 1, width, 1),
                            );
                            result = Some(render_projects(
                                frame,
                                locale,
                                ProjectPicker {
                                    token,
                                    stage: NativeSetupStage::ChooseWorkspaceProject,
                                    workspace: "Workspace 界",
                                    count: 202,
                                    selected,
                                },
                                content,
                                &theme,
                                &mut hits,
                                |index| {
                                    visited.set(visited.get() + 1);
                                    Some(ProjectChoice {
                                        position: index + 1,
                                        selected: index == selected,
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
                            }
                        }
                    }
                    if width == 20 {
                        assert!(matches!(presented, ModalPresentation::NotPresented));
                        assert!(choices.is_empty());
                    } else {
                        assert!(
                            matches!(presented, ModalPresentation::Presented(value) if value == token)
                        );
                        assert!(
                            choices.contains(&selected),
                            "{width}x{height} selected {selected}: {choices:?}"
                        );
                        assert!(choices.len() <= usize::from(height - 7));
                        let text = terminal
                            .backend()
                            .buffer()
                            .content()
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>();
                        assert!(text.contains(&format!("{}/202", selected + 1)));
                        assert!(
                            text.replace(' ', "")
                                .contains(&locale.setup_cancel_button().replace(' ', ""))
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
    hits.start_modal();
    let mut area = Rect::default();
    terminal
        .draw(|frame| {
            area = render_projects(
                frame,
                Locale::EnUs,
                ProjectPicker {
                    token: NativeSetupToken::for_test(1),
                    stage: NativeSetupStage::ChooseWorkspaceProject,
                    workspace: "Workspace",
                    count: 3,
                    selected: 0,
                },
                frame.area(),
                &Theme::terminal_native(),
                &mut hits,
                |index| {
                    (index != 1).then_some(ProjectChoice {
                        position: index + 1,
                        selected: index == 0,
                        label: "Project",
                    })
                },
            )
            .0;
        })
        .unwrap();
    let inner = overlay_block(&Theme::terminal_native()).inner(area);
    let map = hits.finish();
    assert!(matches!(
        map.hit_target_at(inner.x, inner.y + 3),
        Some(HitTarget::ListScroll(
            crate::tui_pointer::ListScrollTarget::Setup(_)
        ))
    ));
    assert!(matches!(
        map.hit_target_at(inner.x, inner.y + 4),
        Some(HitTarget::Setup {
            control: ModalControl::Choice(2),
            ..
        })
    ));
}

fn draw(width: u16, height: u16, locale: Locale, lines: Vec<Line<'_>>) -> (bool, String) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut complete = false;
    terminal
        .draw(|frame| {
            complete = render_content(
                frame,
                frame.area(),
                locale.setup_resize_hint(),
                Text::from(lines.clone()),
                None,
                &crate::tui_theme::Theme::rgb_test_dark(),
            )
            .1;
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    (complete, text)
}

#[test]
fn setup_requires_complete_visible_text_and_rejects_a_narrow_draw() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let lines = vec![
            Line::from(locale.setup_title(NativeSetupStage::WorkspaceProjectName)),
            Line::from("Project: Project 雪"),
            Line::from("Workspace 雪"),
            Line::from(locale.setup_actions(false)),
        ];
        let (complete, wide) = draw(120, 24, locale, lines.clone());
        assert!(complete);
        assert!(wide.replace(' ', "").contains("Workspace雪"));
        let (complete, narrow) = draw(22, 8, locale, lines);
        assert!(!complete);
        assert!(!narrow.contains("Workspace 雪"));
        assert!(!narrow.contains("Enter · submit"));
    }
}

#[test]
fn long_multibyte_directory_must_fit_in_full_before_submit_is_authorized() {
    let path = format!("/{}abc", "雪".repeat(1364));
    assert_eq!(path.len(), 4096);
    let lines = vec![
        Line::from(Locale::EnUs.setup_title(NativeSetupStage::DirectoryPath)),
        Line::from(path),
        Line::from(Locale::EnUs.setup_input_hint(true)),
        Line::from(Locale::EnUs.setup_actions(false)),
    ];
    assert!(!draw(120, 24, Locale::EnUs, lines.clone()).0);
    assert!(draw(160, 48, Locale::EnUs, lines).0);
}

#[test]
fn directory_actions_only_replace_a_live_empty_panel() {
    use crate::terminal_session::NativeDisplayPhase as Phase;
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
        assert_eq!(
            directory_guidance(locale, Phase::Empty, false, None, true).as_deref(),
            Some(locale.setup_directory_needed())
        );
        assert_eq!(
            directory_guidance(locale, Phase::Empty, false, Some("/workspace"), false),
            Some(locale.setup_directory_ready("/workspace"))
        );
    }
}

#[test]
fn location_rename_complete_draw_and_resize_fallback_keep_the_exact_target() {
    for project in [true, false] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let lines = vec![
                Line::from(locale.location_rename_title(project)),
                Line::from("界".repeat(512)),
                Line::from(locale.location_rename_hint(project)),
                Line::from(locale.location_rename_actions(project)),
            ];
            let mut narrow = Terminal::new(TestBackend::new(100, 8)).unwrap();
            let mut complete = true;
            narrow
                .draw(|frame| {
                    complete = render_content(
                        frame,
                        frame.area(),
                        locale.location_rename_resize_hint(project),
                        Text::from(lines.clone()),
                        None,
                        &crate::tui_theme::Theme::rgb_test_dark(),
                    )
                    .1;
                })
                .unwrap();
            assert!(!complete);
            let text = narrow
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            // Wide glyph continuation cells are blank in the test buffer.
            let visible = text.split_whitespace().collect::<String>();
            assert!(visible.contains(match (locale, project) {
                (Locale::EnUs, true) => "Project",
                (Locale::EnUs, false) => "Workspace",
                (Locale::ZhCn, true) => "项目",
                (Locale::ZhCn, false) => "工作区",
            }));
            assert!(!text.contains("Ctrl+U"));
            assert!(draw(120, 24, locale, lines).0);
        }
    }
}
