use super::{directory_guidance, render_content};
use crate::{localization::Locale, terminal_session::NativeSetupStage};
use ratatui::{
    Terminal,
    backend::TestBackend,
    text::{Line, Text},
};

fn draw(width: u16, height: u16, locale: Locale, lines: Vec<Line<'_>>) -> (bool, String) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut complete = false;
    terminal
        .draw(|frame| {
            complete = render_content(frame, locale, Text::from(lines.clone())).1;
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
            Line::from(locale.setup_title(NativeSetupStage::WorkspaceLabel)),
            Line::from("Project: Project 雪"),
            Line::from("Workspace 雪"),
            Line::from(locale.setup_project_retained()),
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
