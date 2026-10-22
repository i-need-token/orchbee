use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};

use crate::tui_theme::{ColorLevel, Role, Theme};

use super::{IconActionGlyph, IconActionSpec, icon_action_rect, render_icon_action};
use crate::tui_render::controls::{Availability, ControlSurface, PointerPresentation};

fn symbols(terminal: &Terminal<TestBackend>, width: u16) -> String {
    (0..width)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect()
}

#[test]
fn icon_geometry_requires_the_complete_three_cell_action_before_paint() {
    let bounds = Rect::new(0, 0, 8, 1);
    assert_eq!(
        icon_action_rect(bounds, Rect::new(2, 0, 6, 1)),
        Some(Rect::new(2, 0, 3, 1))
    );
    assert_eq!(icon_action_rect(bounds, Rect::new(7, 0, 1, 1)), None);
    assert_eq!(icon_action_rect(bounds, Rect::new(7, 0, 3, 1)), None);
    assert_eq!(
        icon_action_rect(
            Rect::new(0, 0, u16::MAX, 1),
            Rect::new(u16::MAX - 1, 0, 3, 1),
        ),
        None
    );
    assert_eq!(
        icon_action_rect(Rect::new(0, 0, 3, u16::MAX), Rect::new(0, u16::MAX, 3, 1),),
        None
    );

    let theme = Theme::rgb_test_dark();
    let mut terminal = Terminal::new(TestBackend::new(8, 1)).unwrap();
    let mut painted = Some(Rect::default());
    terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new("unchang").style(Style::new().fg(Color::Green)),
                frame.area(),
            );
            painted = render_icon_action(
                frame,
                Rect::new(7, 0, 1, 1),
                IconActionSpec::new(
                    IconActionGlyph::Close,
                    ControlSurface::Chrome,
                    Availability::Enabled,
                    PointerPresentation::Rest,
                ),
                &theme,
            );
        })
        .unwrap();
    assert_eq!(painted, None);
    assert_eq!(symbols(&terminal, 8), "unchang ");
}

#[test]
fn icon_grammar_is_fixed_and_rest_uses_secondary_content() {
    let theme = Theme::one_dark();
    for (glyph, expected) in [
        (IconActionGlyph::Close, "[x]"),
        (IconActionGlyph::Previous, "[<]"),
        (IconActionGlyph::Next, "[>]"),
        (IconActionGlyph::Up, "[^]"),
        (IconActionGlyph::Down, "[v]"),
        (IconActionGlyph::Help, "[?]"),
        (IconActionGlyph::Add, "[+]"),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(3, 1)).unwrap();
        let mut painted = None;
        terminal
            .draw(|frame| {
                painted = render_icon_action(
                    frame,
                    frame.area(),
                    IconActionSpec::new(
                        glyph,
                        ControlSurface::Chrome,
                        Availability::Enabled,
                        PointerPresentation::Rest,
                    ),
                    &theme,
                );
            })
            .unwrap();
        assert_eq!(painted, Some(Rect::new(0, 0, 3, 1)));
        assert_eq!(symbols(&terminal, 3), expected);
        let expected = theme.content_on_surface(
            Role::SurfaceChrome,
            None,
            Some(Role::TextSecondary),
            theme.style(Role::TextSecondary),
        );
        for x in 0..3 {
            assert_eq!(terminal.backend().buffer()[(x, 0)].style(), expected);
        }
    }
}

#[test]
fn icon_hover_press_and_disabled_are_complete_across_color_capabilities() {
    for theme in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ] {
        // Hover strengthens the glyph alone; press adds the pressed fill under the same glyph.
        for (pointer, interaction) in [
            (PointerPresentation::Hovered, None),
            (PointerPresentation::Pressed, Some(Role::SurfacePressed)),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(3, 1)).unwrap();
            terminal
                .draw(|frame| {
                    frame.render_widget(
                        Paragraph::new("old").style(
                            Style::new()
                                .fg(Color::Green)
                                .bg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        frame.area(),
                    );
                    render_icon_action(
                        frame,
                        frame.area(),
                        IconActionSpec::new(
                            IconActionGlyph::Next,
                            ControlSurface::Overlay,
                            Availability::Enabled,
                            pointer,
                        ),
                        &theme,
                    )
                    .unwrap();
                })
                .unwrap();
            let expected = theme.content_on_surface(
                Role::SurfaceOverlay,
                interaction,
                Some(Role::TextPrimary),
                theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
            );
            assert_eq!(symbols(&terminal, 3), "[>]");
            for x in 0..3 {
                assert_eq!(terminal.backend().buffer()[(x, 0)].style(), expected);
            }
        }

        let mut terminal = Terminal::new(TestBackend::new(3, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_icon_action(
                    frame,
                    frame.area(),
                    IconActionSpec::new(
                        IconActionGlyph::Previous,
                        ControlSurface::Overlay,
                        Availability::Disabled,
                        PointerPresentation::Pressed,
                    ),
                    &theme,
                )
                .unwrap();
            })
            .unwrap();
        let expected = theme.content_on_surface(
            Role::SurfaceOverlay,
            None,
            Some(Role::TextDisabled),
            theme.style(Role::TextDisabled),
        );
        assert_eq!(symbols(&terminal, 3), "[<]");
        for x in 0..3 {
            assert_eq!(terminal.backend().buffer()[(x, 0)].style(), expected);
        }
    }
}
