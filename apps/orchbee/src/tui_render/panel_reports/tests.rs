use super::*;
use crate::terminal_session::PanelReport;
use serde_json::json;

fn row() -> PanelReport {
    PanelReport {
        report: Some(serde_json::from_value(json!({"agent":"grok","state":"blocked","authoritySource":null,"message":null,"session":null})).unwrap()),
        metadata: serde_json::from_value(json!({"title":null,"displayAgent":"审核 Agent","stateLabels":{"blocked":"等待确认"}})).unwrap(),
        tokens: serde_json::from_value(json!({"branch":"feature"})).unwrap(),
    }
}
#[test]
fn wide_metadata_status_and_tokens_fit_without_injecting_terminal_controls() {
    let theme = Theme::rgb_test_dark();
    for width in 0..=96 {
        let line = header(
            " 自定义标题\x1b[31m\n\u{202e} ".into(),
            PanelReportView::Ready(&row()),
            width,
            Locale::ZhCn,
            &theme,
            theme.style(Role::TextPrimary),
            false,
        );
        let text = line.to_string();
        assert!(display_width(&text) <= width);
        assert!(
            !text
                .chars()
                .any(crate::terminal_text::is_forbidden_terminal_scalar)
        );
        if width >= 32 {
            assert!(text.contains("等待确认"), "{width}: {text}");
        }
        if width == 96 {
            assert!(text.contains("branch=feature"));
            assert!(text.contains("审核 Agent"));
        }
    }
}
#[test]
fn missing_agent_loading_and_unavailable_never_invent_idle() {
    let theme = Theme::rgb_test_dark();
    let mut empty = row();
    empty.report = None;
    empty.metadata.display_agent = None;
    for (view, expected) in [
        (PanelReportView::Pending, "loading status"),
        (PanelReportView::Unavailable, "status unavailable"),
        (PanelReportView::Ready(&empty), "branch=feature"),
    ] {
        let line = header(
            " Panel ".into(),
            view,
            80,
            Locale::EnUs,
            &theme,
            theme.style(Role::TextPrimary),
            false,
        );
        assert!(line.to_string().contains(expected));
        assert!(!line.to_string().contains("idle"));
    }
}

#[test]
fn each_state_keeps_its_left_symbol_when_the_right_unread_mark_clears() {
    for theme in [
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native(),
    ] {
        for glyphs in [
            crate::tui_theme::GlyphSet::UTF8,
            crate::tui_theme::GlyphSet::ASCII,
        ] {
            let theme = theme.with_glyphs(glyphs);
            for (state, label) in [
                (PanelAgentState::Working, MetadataState::Working),
                (PanelAgentState::Blocked, MetadataState::Blocked),
                (PanelAgentState::Done, MetadataState::Done),
                (PanelAgentState::Error, MetadataState::Error),
                (PanelAgentState::Idle, MetadataState::Idle),
                (PanelAgentState::Unknown, MetadataState::Unknown),
            ] {
                let mut row = row();
                row.report.as_mut().unwrap().state = state;
                row.metadata.state_labels.clear();
                for unread in [false, true] {
                    for width in 0..=96 {
                        let line = header(
                            "Title".into(),
                            PanelReportView::Ready(&row),
                            width,
                            Locale::EnUs,
                            &theme,
                            theme.style(Role::SurfaceSelection),
                            unread,
                        );
                        assert!(display_width(&line.to_string()) <= width);
                        if width >= 4 {
                            assert!(line.to_string().starts_with(theme.agent_glyph(label)));
                            assert_eq!(line.spans[0].style, theme.style(Theme::agent_role(label)));
                            assert_eq!(line.to_string().ends_with(glyphs.status_bullet), unread);
                            assert_eq!(
                                line.spans.last().unwrap().style,
                                theme.style(Role::StatusUnread)
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn status_uses_its_role_and_all_separators_follow_the_theme() {
    for theme in [
        Theme::terminal_native(),
        Theme::rgb_test_dark(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
        for glyphs in [
            crate::tui_theme::GlyphSet::UTF8,
            crate::tui_theme::GlyphSet::ASCII,
        ] {
            let theme = theme.with_glyphs(glyphs);
            let mut row = row();
            row.metadata.display_agent = None;
            row.metadata.state_labels.clear();
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let line = header(
                    "Title".into(),
                    PanelReportView::Ready(&row),
                    80,
                    locale,
                    &theme,
                    theme.style(Role::TextPrimary),
                    false,
                );
                assert_eq!(
                    line.spans[3].content,
                    locale.panel_report_state(PanelAgentState::Blocked)
                );
                assert_eq!(line.spans[3].style, theme.style(Role::StatusWarning));
                assert_eq!(line.spans[4].style, theme.style(Role::TextSecondary));
                assert_eq!(
                    line.spans[4].content,
                    format!(" {} grok", glyphs.middle_dot)
                );
                assert!(
                    line.spans[1]
                        .content
                        .contains(&format!(" {} branch=feature", glyphs.middle_dot))
                );
            }
        }
    }
}
