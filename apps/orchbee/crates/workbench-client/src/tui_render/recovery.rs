//! Geometry feedback with a complete, owner-supplied local return hint.

use ratatui::{Frame, layout::Rect};

use crate::{
    client_view::{
        Binding, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers, format_binding_with_glyphs,
    },
    tui_theme::{GlyphMode, Role, Theme},
};

/// The informational message yields space to the complete local return path.
/// This is a keyboard hint, not an actionable pointer control or admission proof.
/// It floats as the status chip and returns the cells it covers.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    notice: &str,
    local_return: Option<&'static str>,
    theme: &Theme,
) -> Option<Rect> {
    if area.is_empty() || frame.area().intersection(area) != area {
        return None;
    }
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Esc, Modifiers::NONE),
        LabelStyle::for_host(),
        key_glyphs,
    );
    super::status_line::render_notice_chip(
        frame,
        area,
        notice,
        Role::StatusWarning,
        local_return.map(|label| (key, label)),
        true,
        theme,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{localization::Locale, terminal_text::display_width, tui_theme::GlyphSet};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn narrowing_preserves_complete_local_return_and_never_crops_a_key_hint() {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
                let mut theme = Theme::rgb_test_dark();
                theme.glyphs = glyphs;
                let label = locale.recovery_verb(crate::localization::RecoveryVerb::Close);
                let complete_hint = format!("Esc {label}");
                for width in [3, 8, 9, 16, 40, 80] {
                    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                    terminal
                        .draw(|frame| {
                            render(
                                frame,
                                frame.area(),
                                locale.presentation_notice(true),
                                Some(label),
                                &theme,
                            );
                        })
                        .unwrap();
                    let row = terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect::<String>();
                    let normalized = row.replace(' ', "");
                    // The chip pads one cell at each end around the complete hint.
                    if display_width(&complete_hint) + 2 <= usize::from(width) {
                        assert!(
                            normalized.ends_with(&complete_hint.replace(' ', "")),
                            "{locale:?} {width}: {row:?}"
                        );
                    } else {
                        assert!(!row.contains("Esc"), "{locale:?} {width}: {row:?}");
                    }
                    assert!(
                        !row.contains("Ctrl+"),
                        "recovery must remain local: {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn first_refusal_has_no_phantom_local_return_and_clipped_offer_is_untouched() {
        let theme = Theme::rgb_test_dark();
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    Locale::EnUs.presentation_notice(false),
                    None,
                    &theme,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Cannot open here: enlarge the terminal."));
        assert!(!text.contains("Esc"));
        let marker_width = display_width(theme.glyphs.status_bullet);
        let marker = (0..80)
            .find(|x| buffer[(*x, 0)].symbol() == theme.glyphs.status_bullet)
            .expect("the notice marker floats in the chip");
        assert_eq!(buffer[(marker, 0)].fg, theme.color(Role::StatusWarning));
        assert_eq!(
            buffer[(marker + u16::try_from(marker_width + 1).unwrap(), 0)].fg,
            theme.color(Role::TextPrimary)
        );

        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(0, 0, "unchanged", theme.style(Role::TextPrimary));
                let before = frame.buffer_mut().clone();
                render(
                    frame,
                    Rect::new(79, 0, 2, 1),
                    "refused",
                    Some("Close"),
                    &theme,
                );
                assert_eq!(*frame.buffer_mut(), before);
            })
            .unwrap();
    }
}
