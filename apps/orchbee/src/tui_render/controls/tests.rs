use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

use crate::tui_theme::{ColorLevel, Role, Theme};

use super::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, RowContentBand,
    RowFocusCue, RowTracks, action_rect, close_rect, paint_pointer_target, paint_row_interaction,
    render_action,
};

fn state(
    focus: FocusState,
    pointer: PointerPresentation,
    navigation: NavigationState,
    current: CurrentState,
    checked: CheckedState,
    availability: Availability,
    operation: OperationState,
) -> InteractionState {
    InteractionState::new(
        focus,
        pointer,
        navigation,
        current,
        checked,
        availability,
        operation,
    )
}

fn idle(pointer: PointerPresentation) -> InteractionState {
    state(
        FocusState::Unfocused,
        pointer,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        Availability::Enabled,
        OperationState::Idle,
    )
}

fn symbols(terminal: &Terminal<TestBackend>, y: u16, width: u16) -> String {
    (0..width)
        .map(|x| terminal.backend().buffer()[(x, y)].symbol())
        .collect()
}

#[test]
fn action_reserves_the_larger_localized_pending_label_without_moving_its_extent() {
    let theme = Theme::rgb_test_dark();
    let width = 14;
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    let mut idle_rect = None;
    terminal
        .draw(|frame| {
            idle_rect = render_action(
                frame,
                frame.area(),
                ActionSpec::new(
                    "Save",
                    ActionKind::Primary,
                    ControlSurface::Overlay,
                    idle(PointerPresentation::Rest),
                )
                .with_pending_label("正在保存"),
                &theme,
            );
        })
        .unwrap();
    assert_eq!(idle_rect, Some(Rect::new(0, 0, width, 1)));
    assert_eq!(&symbols(&terminal, 0, width)[..9], " [ Save  ");

    let mut pending_rect = None;
    terminal
        .draw(|frame| {
            pending_rect = render_action(
                frame,
                frame.area(),
                ActionSpec::new(
                    "Save",
                    ActionKind::Primary,
                    ControlSurface::Overlay,
                    state(
                        FocusState::Unfocused,
                        PointerPresentation::Rest,
                        NavigationState::None,
                        CurrentState::NotCurrent,
                        CheckedState::Unchecked,
                        Availability::Enabled,
                        OperationState::Pending,
                    ),
                )
                .with_pending_label("正在保存"),
                &theme,
            );
        })
        .unwrap();
    assert_eq!(pending_rect, idle_rect);
    assert_eq!(
        terminal.backend().buffer()[(13, 0)].symbol(),
        theme.glyphs.status_bullet
    );
}

#[test]
fn pure_action_geometry_is_stable_before_pointer_projection_and_matches_paint() {
    let theme = Theme::rgb_test_dark();
    let area = Rect::new(4, 2, 30, 1);
    let rest = idle(PointerPresentation::Rest);
    let rest_spec = ActionSpec::new("Open", ActionKind::Primary, ControlSurface::Chrome, rest)
        .with_pending_label("Opening");
    let hovered_spec = ActionSpec::new(
        "Open",
        ActionKind::Primary,
        ControlSurface::Chrome,
        rest.with_pointer(PointerPresentation::Hovered),
    )
    .with_pending_label("Opening");
    let bounds = Rect::new(0, 0, 40, 4);
    let rect = action_rect(bounds, area, rest_spec, &theme).unwrap();
    assert_eq!(action_rect(bounds, area, hovered_spec, &theme), Some(rect));

    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    let mut painted = None;
    terminal
        .draw(|frame| {
            painted = render_action(frame, area, hovered_spec, &theme);
        })
        .unwrap();
    assert_eq!(painted, Some(rect));
}

#[test]
fn action_refuses_narrow_or_clipped_geometry_without_partial_paint() {
    let theme = Theme::rgb_test_dark();
    for (width, area) in [(8, Rect::new(0, 0, 8, 1)), (9, Rect::new(1, 0, 9, 1))] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        let mut painted = Some(Rect::new(0, 0, 0, 0));
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new("unchanged!")
                        .style(Style::new().fg(Color::Green).bg(Color::Magenta)),
                    frame.area(),
                );
                painted = render_action(
                    frame,
                    area,
                    ActionSpec::new(
                        "Save",
                        ActionKind::Primary,
                        ControlSurface::Overlay,
                        idle(PointerPresentation::Rest),
                    ),
                    &theme,
                );
            })
            .unwrap();
        assert_eq!(painted, None);
        assert_eq!(
            &symbols(&terminal, 0, width)[..usize::from(width)],
            &"unchanged!"[..usize::from(width)]
        );
        assert_eq!(
            terminal.backend().buffer()[(0, 0)].style().fg,
            Some(Color::Green)
        );
        assert_eq!(
            terminal.backend().buffer()[(0, 0)].style().bg,
            Some(Color::Magenta)
        );
    }
}

#[test]
fn action_rejects_row_value_and_incomplete_pending_states_before_paint() {
    let unsupported = [
        state(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::Provisional,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        ),
        state(
            FocusState::Focused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Pending,
        ),
        state(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::Current,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        ),
        state(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Checked,
            Availability::Enabled,
            OperationState::Idle,
        ),
    ];
    let theme = Theme::rgb_test_dark();
    for interaction in unsupported {
        let mut terminal = Terminal::new(TestBackend::new(8, 1)).unwrap();
        let mut painted = Some(Rect::new(0, 0, 0, 0));
        terminal
            .draw(|frame| {
                frame.render_widget(Paragraph::new("same...."), frame.area());
                painted = render_action(
                    frame,
                    frame.area(),
                    ActionSpec::new(
                        "Go",
                        ActionKind::Primary,
                        ControlSurface::Overlay,
                        interaction,
                    ),
                    &theme,
                );
            })
            .unwrap();
        assert_eq!(painted, None, "state={interaction:?}");
        assert_eq!(symbols(&terminal, 0, 8), "same....");
    }

    let pending = state(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        Availability::Enabled,
        OperationState::Pending,
    );
    for spec in [
        ActionSpec::new(
            "",
            ActionKind::Primary,
            ControlSurface::Overlay,
            idle(PointerPresentation::Rest),
        )
        .with_pending_label("Working"),
        ActionSpec::new("Go", ActionKind::Primary, ControlSurface::Overlay, pending)
            .with_pending_label(""),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(20, 1)).unwrap();
        let mut painted = Some(Rect::new(0, 0, 0, 0));
        terminal
            .draw(|frame| {
                frame.render_widget(Paragraph::new("...................."), frame.area());
                painted = render_action(frame, frame.area(), spec, &theme);
            })
            .unwrap();
        assert_eq!(painted, None, "spec={spec:?}");
        assert_eq!(symbols(&terminal, 0, 20), "....................");
    }
}

#[test]
fn action_pointer_precedence_and_disabled_suppression_are_independent() {
    let theme = Theme::rgb_test_dark();
    let cases = [
        (idle(PointerPresentation::Hovered), Role::SurfaceHover),
        (idle(PointerPresentation::Pressed), Role::SurfacePressed),
        (
            state(
                FocusState::Focused,
                PointerPresentation::Pressed,
                NavigationState::None,
                CurrentState::NotCurrent,
                CheckedState::Unchecked,
                Availability::Disabled,
                OperationState::Idle,
            ),
            Role::SurfaceOverlay,
        ),
    ];
    for (interaction, expected) in cases {
        let mut terminal = Terminal::new(TestBackend::new(8, 1)).unwrap();
        let mut painted = None;
        terminal
            .draw(|frame| {
                painted = render_action(
                    frame,
                    frame.area(),
                    ActionSpec::new(
                        "Go",
                        ActionKind::Secondary,
                        ControlSurface::Overlay,
                        interaction,
                    ),
                    &theme,
                );
            })
            .unwrap();
        let painted = painted.expect("action fits");
        assert_eq!(painted, Rect::new(0, 0, 7, 1));
        let expected = theme.style(expected);
        for x in painted.x..painted.right() {
            let actual = terminal.backend().buffer()[(x, 0)].style();
            assert_eq!(actual.bg, expected.bg, "x={x}, state={interaction:?}");
            assert_eq!(
                actual.add_modifier.contains(Modifier::REVERSED),
                expected.add_modifier.contains(Modifier::REVERSED),
                "x={x}, state={interaction:?}"
            );
        }
    }
}

#[test]
fn pressed_emphasis_belongs_only_to_the_primary_label() {
    let theme = Theme::rgb_test_dark();
    for (kind, label_bold) in [(ActionKind::Primary, true), (ActionKind::Secondary, false)] {
        let mut terminal = Terminal::new(TestBackend::new(7, 1)).unwrap();
        let mut painted = None;
        terminal
            .draw(|frame| {
                painted = render_action(
                    frame,
                    frame.area(),
                    ActionSpec::new(
                        "Go",
                        kind,
                        ControlSurface::Overlay,
                        idle(PointerPresentation::Pressed),
                    ),
                    &theme,
                );
            })
            .unwrap();
        assert_eq!(painted, Some(Rect::new(0, 0, 7, 1)));
        let buffer = terminal.backend().buffer();
        assert_eq!(
            buffer[(3, 0)].style().add_modifier.contains(Modifier::BOLD),
            label_bold
        );
        assert!(!buffer[(1, 0)].style().add_modifier.contains(Modifier::BOLD));
        assert!(!buffer[(6, 0)].style().add_modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn reduced_color_action_paints_its_base_and_keeps_interaction_modifiers() {
    for theme in [
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_light().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ] {
        for pointer in [PointerPresentation::Hovered, PointerPresentation::Pressed] {
            let mut terminal = Terminal::new(TestBackend::new(8, 1)).unwrap();
            let mut painted = None;
            terminal
                .draw(|frame| {
                    frame.render_widget(
                        Paragraph::new("underlay").style(Style::new().bg(Color::Magenta)),
                        frame.area(),
                    );
                    painted = render_action(
                        frame,
                        frame.area(),
                        ActionSpec::new(
                            "Go",
                            ActionKind::Secondary,
                            ControlSurface::Overlay,
                            state(
                                FocusState::Focused,
                                pointer,
                                NavigationState::None,
                                CurrentState::NotCurrent,
                                CheckedState::Unchecked,
                                Availability::Enabled,
                                OperationState::Idle,
                            ),
                        ),
                        &theme,
                    );
                })
                .unwrap();
            let painted = painted.expect("action fits");
            assert_eq!(painted, Rect::new(0, 0, 7, 1));
            assert_eq!(
                terminal.backend().buffer()[(0, 0)].symbol(),
                theme.glyphs.active_marker
            );
            let interaction = theme.style(match pointer {
                PointerPresentation::Hovered => Role::SurfaceHover,
                PointerPresentation::Pressed => Role::SurfacePressed,
                PointerPresentation::Rest => unreachable!(),
            });
            for x in painted.x..painted.right() {
                let actual = terminal.backend().buffer()[(x, 0)].style();
                assert_eq!(
                    actual.bg,
                    interaction.bg.or(theme.style(Role::SurfaceOverlay).bg),
                    "x={x}, pointer={pointer:?}, theme={theme:?}"
                );
                assert!(actual.add_modifier.contains(interaction.add_modifier));
            }
            assert_eq!(
                terminal.backend().buffer()[(7, 0)].style().bg,
                Some(Color::Magenta)
            );
            assert_eq!(terminal.backend().buffer()[(7, 0)].symbol(), "y");
        }
    }
}

#[test]
fn semantic_pointer_target_keeps_exact_geometry_content_and_low_color_feedback() {
    for theme in [
        Theme::terminal_native(),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ] {
        for (pointer, interaction) in [
            (PointerPresentation::Hovered, Role::SurfaceHover),
            (PointerPresentation::Pressed, Role::SurfacePressed),
        ] {
            let row = Rect::new(0, 0, 8, 1);
            let rect = Rect::new(4, 0, 3, 1);
            let primary = Rect::new(1, 0, 2, 1);
            let tracks = RowTracks::new(RowFocusCue::Track(Rect::new(0, 0, 1, 1)), primary);
            let bands = [RowContentBand::semantic(rect, Role::TextPrimary)];
            let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
            let mut painted = None;
            let mut outside_before = Style::new();
            terminal
                .draw(|frame| {
                    frame.render_widget(
                        Paragraph::new("........").style(theme.style(Role::SurfaceOverlay)),
                        row,
                    );
                    frame.buffer_mut().set_string(
                        primary.x,
                        primary.y,
                        "Go",
                        theme.style(Role::TextPrimary),
                    );
                    frame.buffer_mut().set_string(
                        rect.x,
                        rect.y,
                        "[-]",
                        theme.style(Role::TextPrimary),
                    );
                    assert_eq!(
                        paint_row_interaction(
                            frame,
                            row,
                            tracks,
                            &bands,
                            ControlSurface::Overlay,
                            state(
                                FocusState::Unfocused,
                                PointerPresentation::Rest,
                                NavigationState::None,
                                CurrentState::Current,
                                CheckedState::Unchecked,
                                Availability::Enabled,
                                OperationState::Idle,
                            ),
                            &theme,
                        ),
                        Some(row)
                    );
                    outside_before = frame.buffer_mut()[(3, 0)].style();
                    painted = paint_pointer_target(
                        frame,
                        rect,
                        ControlSurface::Overlay,
                        pointer,
                        Role::TextPrimary,
                        Availability::Enabled,
                        &theme,
                    );
                })
                .unwrap();
            assert_eq!(painted, Some(rect));
            assert_eq!(&symbols(&terminal, 0, row.width)[4..7], "[-]");
            let expected = theme.content_on_surface(
                Role::SurfaceOverlay,
                Some(interaction),
                Some(Role::TextPrimary),
                theme.style(Role::TextPrimary),
            );
            for x in rect.x..rect.right() {
                let actual = terminal.backend().buffer()[(x, 0)].style();
                assert_eq!(actual.fg, expected.fg);
                assert_eq!(actual.bg, expected.bg);
                assert_eq!(actual.add_modifier, expected.add_modifier);
            }
            assert_eq!(terminal.backend().buffer()[(3, 0)].style(), outside_before);
            assert_eq!(
                terminal.backend().buffer()[(7, 0)].style().bg,
                theme.style(Role::SurfaceCurrent).bg
            );
        }
    }
}

#[test]
fn semantic_pointer_target_suppresses_disabled_pointer_and_rejects_clipping_atomically() {
    for theme in [
        Theme::terminal_native(),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ] {
        for (pointer, role, availability) in [
            (
                PointerPresentation::Rest,
                Role::TextPrimary,
                Availability::Enabled,
            ),
            (
                PointerPresentation::Pressed,
                Role::TextDisabled,
                Availability::Disabled,
            ),
        ] {
            let row = Rect::new(0, 0, 8, 1);
            let rect = Rect::new(4, 0, 3, 1);
            let primary = Rect::new(1, 0, 2, 1);
            let tracks = RowTracks::new(RowFocusCue::Track(Rect::new(0, 0, 1, 1)), primary);
            let bands = [RowContentBand::semantic(rect, role)];
            let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
            let mut painted = None;
            let mut before = [Style::new(); 3];
            terminal
                .draw(|frame| {
                    frame.render_widget(
                        Paragraph::new("........").style(theme.style(Role::SurfaceOverlay)),
                        row,
                    );
                    frame.buffer_mut().set_string(
                        primary.x,
                        primary.y,
                        "Go",
                        theme.style(Role::TextPrimary),
                    );
                    frame
                        .buffer_mut()
                        .set_string(rect.x, rect.y, "[+]", theme.style(role));
                    assert_eq!(
                        paint_row_interaction(
                            frame,
                            row,
                            tracks,
                            &bands,
                            ControlSurface::Overlay,
                            state(
                                FocusState::Unfocused,
                                PointerPresentation::Rest,
                                NavigationState::None,
                                CurrentState::Current,
                                CheckedState::Unchecked,
                                Availability::Enabled,
                                OperationState::Idle,
                            ),
                            &theme,
                        ),
                        Some(row)
                    );
                    for (index, x) in (rect.x..rect.right()).enumerate() {
                        before[index] = frame.buffer_mut()[(x, 0)].style();
                    }
                    painted = paint_pointer_target(
                        frame,
                        rect,
                        ControlSurface::Overlay,
                        pointer,
                        role,
                        availability,
                        &theme,
                    );
                })
                .unwrap();
            assert_eq!(painted, Some(rect));
            assert_eq!(&symbols(&terminal, 0, row.width)[4..7], "[+]");
            for (index, x) in (rect.x..rect.right()).enumerate() {
                assert_eq!(terminal.backend().buffer()[(x, 0)].style(), before[index]);
            }
        }
    }

    let theme = Theme::one_dark();
    let rect = Rect::new(1, 0, 3, 1);
    let mut terminal = Terminal::new(TestBackend::new(6, 1)).unwrap();
    let mut clipped = Some(rect);
    terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new("same!!").style(Style::new().fg(Color::Green)),
                frame.area(),
            );
            clipped = paint_pointer_target(
                frame,
                Rect::new(4, 0, 3, 1),
                ControlSurface::Overlay,
                PointerPresentation::Hovered,
                Role::TextPrimary,
                Availability::Enabled,
                &theme,
            );
        })
        .unwrap();
    assert_eq!(clipped, None);
    assert_eq!(symbols(&terminal, 0, 6), "same!!");
    assert_eq!(
        terminal.backend().buffer()[(4, 0)].style().fg,
        Some(Color::Green)
    );
}

#[test]
fn row_keeps_focus_current_checked_pending_and_explicit_content_while_disabled() {
    let theme = Theme::one_dark();
    let row = Rect::new(0, 0, 20, 1);
    let tracks = RowTracks::new(
        RowFocusCue::Track(Rect::new(0, 0, 1, 1)),
        Rect::new(3, 0, 6, 1),
    )
    .with_provisional(Rect::new(2, 0, 1, 1))
    .with_checked(Rect::new(1, 0, 1, 1), theme.glyphs.active_marker)
    .with_status(Rect::new(19, 0, 1, 1));
    let bands = [
        RowContentBand::semantic(Rect::new(10, 0, 4, 1), Role::TextSecondary),
        RowContentBand::semantic(Rect::new(14, 0, 3, 1), Role::StatusWarning),
        RowContentBand::explicit(Rect::new(17, 0, 2, 1)),
    ];
    let interaction = state(
        FocusState::Focused,
        PointerPresentation::Pressed,
        NavigationState::Provisional,
        CurrentState::Current,
        CheckedState::Checked,
        Availability::Disabled,
        OperationState::Pending,
    );
    let explicit = Style::new()
        .fg(Color::LightMagenta)
        .add_modifier(Modifier::BOLD | Modifier::DIM);
    let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
    let mut painted = None;
    terminal
        .draw(|frame| {
            frame
                .buffer_mut()
                .set_string(3, 0, "Choice", theme.style(Role::TextPrimary));
            frame
                .buffer_mut()
                .set_string(10, 0, "meta", theme.style(Role::TextSecondary));
            frame
                .buffer_mut()
                .set_string(14, 0, "war", theme.style(Role::StatusWarning));
            frame.buffer_mut().set_string(17, 0, "! ", explicit);
            painted = paint_row_interaction(
                frame,
                row,
                tracks,
                &bands,
                ControlSurface::Overlay,
                interaction,
                &theme,
            );
        })
        .unwrap();
    assert_eq!(painted, Some(row));
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(1, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(2, 0)].symbol(), theme.glyphs.navigation_marker);
    assert_eq!(buffer[(19, 0)].symbol(), theme.glyphs.status_bullet);
    assert_eq!(
        buffer[(5, 0)].style().fg,
        theme.style(Role::TextDisabled).fg
    );
    assert!(buffer[(5, 0)].style().add_modifier.contains(Modifier::BOLD));
    assert_eq!(
        buffer[(11, 0)].style().fg,
        theme.style(Role::TextDisabled).fg
    );
    assert_eq!(
        buffer[(15, 0)].style().fg,
        theme.style(Role::StatusWarning).fg
    );
    assert_eq!(buffer[(17, 0)].style().fg, Some(Color::LightMagenta));
    assert!(
        buffer[(17, 0)]
            .style()
            .add_modifier
            .contains(Modifier::BOLD | Modifier::DIM)
    );
    assert_eq!(
        buffer[(19, 0)].style().fg,
        theme.style(Role::StatusPending).fg
    );
    for x in 0..row.width {
        assert_eq!(
            buffer[(x, 0)].style().bg,
            theme.style(Role::SurfaceSelection).bg,
            "x={x}"
        );
    }
}

#[test]
fn row_state_matrix_keeps_secondary_hierarchy_and_non_color_cues() {
    let themes = [
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native(),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ];
    for theme in themes {
        let row = Rect::new(0, 0, 15, 1);
        let tracks = RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 0, 1, 1)),
            Rect::new(3, 0, 5, 1),
        )
        .with_checked(Rect::new(1, 0, 1, 1), theme.glyphs.active_marker)
        .with_status(Rect::new(14, 0, 1, 1));
        let bands = [RowContentBand::semantic(
            Rect::new(9, 0, 4, 1),
            Role::TextSecondary,
        )];
        let interaction = state(
            FocusState::Focused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::Current,
            CheckedState::Checked,
            Availability::Enabled,
            OperationState::Pending,
        );
        let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(3, 0, "Name ", theme.style(Role::TextPrimary));
                frame
                    .buffer_mut()
                    .set_string(9, 0, "meta", theme.style(Role::TextSecondary));
                assert_eq!(
                    paint_row_interaction(
                        frame,
                        row,
                        tracks,
                        &bands,
                        ControlSurface::Overlay,
                        interaction,
                        &theme,
                    ),
                    Some(row)
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let expected_secondary = theme.content_on_surface(
            Role::SurfaceOverlay,
            Some(Role::SurfaceCurrent),
            Some(Role::TextSecondary),
            theme.style(Role::TextSecondary),
        );
        assert_eq!(buffer[(0, 0)].symbol(), theme.glyphs.active_marker);
        assert_eq!(buffer[(1, 0)].symbol(), theme.glyphs.active_marker);
        assert_eq!(buffer[(14, 0)].symbol(), theme.glyphs.status_bullet);
        assert!(buffer[(4, 0)].style().add_modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(10, 0)].style().fg, expected_secondary.fg);
        assert_eq!(
            buffer[(10, 0)].style().add_modifier.contains(Modifier::DIM),
            expected_secondary.add_modifier.contains(Modifier::DIM),
            "theme={theme:?}"
        );
        assert_eq!(
            buffer[(14, 0)].style().fg,
            theme.style(Role::StatusPending).fg
        );
    }
}

#[test]
fn current_and_hover_remain_independent_on_a_row() {
    let theme = Theme::one_dark();
    let row = Rect::new(0, 0, 10, 1);
    let tracks = RowTracks::new(
        RowFocusCue::Track(Rect::new(0, 0, 1, 1)),
        Rect::new(2, 0, 4, 1),
    );
    let bands = [RowContentBand::semantic(
        Rect::new(7, 0, 3, 1),
        Role::TextSecondary,
    )];
    let interaction = state(
        FocusState::Unfocused,
        PointerPresentation::Hovered,
        NavigationState::None,
        CurrentState::Current,
        CheckedState::Unchecked,
        Availability::Enabled,
        OperationState::Idle,
    );
    let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
    terminal
        .draw(|frame| {
            frame
                .buffer_mut()
                .set_string(2, 0, "Name", theme.style(Role::TextPrimary));
            frame
                .buffer_mut()
                .set_string(7, 0, "aux", theme.style(Role::TextSecondary));
            assert_eq!(
                paint_row_interaction(
                    frame,
                    row,
                    tracks,
                    &bands,
                    ControlSurface::Overlay,
                    interaction,
                    &theme,
                ),
                Some(row)
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    for x in 0..row.width {
        assert_eq!(
            buffer[(x, 0)].style().bg,
            theme.style(Role::SurfaceHover).bg,
            "x={x}"
        );
    }
    assert!(buffer[(3, 0)].style().add_modifier.contains(Modifier::BOLD));
    assert_eq!(
        buffer[(8, 0)].style().fg,
        theme.style(Role::TextSecondary).fg
    );
    assert!(!buffer[(8, 0)].style().add_modifier.contains(Modifier::BOLD));
}

#[test]
fn row_primary_role_preserves_declared_content_and_disabled_override() {
    let theme = Theme::one_dark();
    let row = Rect::new(0, 0, 10, 1);
    let primary = Rect::new(2, 0, 6, 1);
    let render = |role: Role, availability: Availability| {
        let tracks = RowTracks::new(RowFocusCue::Track(Rect::new(0, 0, 1, 1)), primary)
            .with_primary_role(role);
        let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(primary.x, primary.y, "Save  ", theme.style(role));
                assert_eq!(
                    paint_row_interaction(
                        frame,
                        row,
                        tracks,
                        &[],
                        ControlSurface::Overlay,
                        state(
                            FocusState::Unfocused,
                            PointerPresentation::Rest,
                            NavigationState::None,
                            CurrentState::NotCurrent,
                            CheckedState::Unchecked,
                            availability,
                            OperationState::Idle,
                        ),
                        &theme,
                    ),
                    Some(row)
                );
            })
            .unwrap();
        terminal.backend().buffer()[(primary.x, primary.y)].style()
    };

    let action = render(Role::AccentAction, Availability::Enabled);
    assert_eq!(
        action,
        theme.content_on_surface(
            Role::SurfaceOverlay,
            None,
            Some(Role::AccentAction),
            theme.style(Role::AccentAction),
        )
    );
    assert!(!action.add_modifier.contains(Modifier::BOLD));

    let pending = render(Role::StatusPending, Availability::Enabled);
    assert_eq!(
        pending,
        theme.content_on_surface(
            Role::SurfaceOverlay,
            None,
            Some(Role::StatusPending),
            theme.style(Role::StatusPending),
        )
    );
    assert!(pending.add_modifier.contains(Modifier::DIM));

    let disabled = render(Role::AccentAction, Availability::Disabled);
    assert_eq!(
        disabled,
        theme.content_on_surface(
            Role::SurfaceOverlay,
            None,
            Some(Role::TextDisabled),
            theme.style(Role::TextDisabled),
        )
    );
}

#[test]
fn enclosing_frame_focus_keeps_one_shared_navigation_cue_in_low_color() {
    for theme in [
        Theme::terminal_native(),
        Theme::one_dark(),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ] {
        let render = |navigation: NavigationState, current: CurrentState, checked: CheckedState| {
            let row = Rect::new(0, 0, 10, 1);
            let primary = Rect::new(3, 0, 4, 1);
            let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
                .with_provisional(Rect::new(0, 0, 1, 1))
                .with_checked(Rect::new(1, 0, 1, 1), "x");
            let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
            terminal
                .draw(|frame| {
                    frame.buffer_mut().set_string(
                        primary.x,
                        primary.y,
                        "Name",
                        theme.style(Role::TextPrimary),
                    );
                    assert_eq!(
                        paint_row_interaction(
                            frame,
                            row,
                            tracks,
                            &[],
                            ControlSurface::Overlay,
                            state(
                                FocusState::Focused,
                                PointerPresentation::Rest,
                                navigation,
                                current,
                                checked,
                                Availability::Enabled,
                                OperationState::Idle,
                            ),
                            &theme,
                        ),
                        Some(row)
                    );
                })
                .unwrap();
            terminal
        };

        let provisional = render(
            NavigationState::Provisional,
            CurrentState::NotCurrent,
            CheckedState::Checked,
        );
        let provisional_buffer = provisional.backend().buffer();
        assert_eq!(
            provisional_buffer[(0, 0)].symbol(),
            theme.glyphs.navigation_marker
        );
        assert_eq!(provisional_buffer[(1, 0)].symbol(), "x");
        assert_eq!(
            symbols(&provisional, 0, 10)
                .matches(theme.glyphs.active_marker)
                .count(),
            0,
            "the enclosing frame is the only focus cue for {theme:?}"
        );
        assert_eq!(
            provisional_buffer[(0, 0)].style(),
            theme.content_on_surface(
                Role::SurfaceOverlay,
                Some(Role::SurfaceSelection),
                Some(Role::AccentSelection),
                theme.style(Role::AccentSelection),
            )
        );

        let current = render(
            NavigationState::None,
            CurrentState::Current,
            CheckedState::Unchecked,
        );
        let current_buffer = current.backend().buffer();
        assert_eq!(current_buffer[(0, 0)].symbol(), " ");
        assert_eq!(current_buffer[(1, 0)].symbol(), " ");
        assert!(
            current_buffer[(4, 0)]
                .style()
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }
}

#[test]
fn monochrome_rows_distinguish_rest_current_provisional_and_checked() {
    let theme = Theme::one_dark().quantize(ColorLevel::Monochrome);
    let render = |navigation: NavigationState, current: CurrentState, checked: CheckedState| {
        let row = Rect::new(0, 0, 10, 1);
        let tracks = RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 0, 1, 1)),
            Rect::new(4, 0, 3, 1),
        )
        .with_provisional(Rect::new(1, 0, 1, 1))
        .with_checked(Rect::new(2, 0, 1, 1), theme.glyphs.active_marker);
        let bands = [RowContentBand::semantic(
            Rect::new(8, 0, 2, 1),
            Role::TextSecondary,
        )];
        let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(4, 0, "Row", theme.style(Role::TextPrimary));
                frame
                    .buffer_mut()
                    .set_string(8, 0, "m ", theme.style(Role::TextSecondary));
                assert_eq!(
                    paint_row_interaction(
                        frame,
                        row,
                        tracks,
                        &bands,
                        ControlSurface::Overlay,
                        state(
                            FocusState::Unfocused,
                            PointerPresentation::Rest,
                            navigation,
                            current,
                            checked,
                            Availability::Enabled,
                            OperationState::Idle,
                        ),
                        &theme,
                    ),
                    Some(row)
                );
            })
            .unwrap();
        terminal
    };

    let rest = render(
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
    );
    let current = render(
        NavigationState::None,
        CurrentState::Current,
        CheckedState::Unchecked,
    );
    let provisional = render(
        NavigationState::Provisional,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
    );
    let checked_provisional = render(
        NavigationState::Provisional,
        CurrentState::NotCurrent,
        CheckedState::Checked,
    );

    assert!(
        !rest.backend().buffer()[(5, 0)]
            .style()
            .add_modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        current.backend().buffer()[(5, 0)]
            .style()
            .add_modifier
            .contains(Modifier::BOLD)
    );
    assert_eq!(current.backend().buffer()[(1, 0)].symbol(), " ");
    assert_eq!(
        provisional.backend().buffer()[(1, 0)].symbol(),
        theme.glyphs.navigation_marker
    );
    assert_eq!(provisional.backend().buffer()[(2, 0)].symbol(), " ");
    assert_eq!(
        checked_provisional.backend().buffer()[(1, 0)].symbol(),
        theme.glyphs.navigation_marker
    );
    assert_eq!(
        checked_provisional.backend().buffer()[(2, 0)].symbol(),
        theme.glyphs.active_marker
    );
}

#[test]
fn two_line_row_uses_complete_height_tracks_and_surface_band() {
    let theme = Theme::terminal_native();
    let row = Rect::new(0, 0, 12, 2);
    let tracks = RowTracks::new(
        RowFocusCue::Track(Rect::new(0, 0, 1, 2)),
        Rect::new(3, 0, 5, 2),
    )
    .with_checked(Rect::new(1, 0, 1, 2), theme.glyphs.active_marker)
    .with_status(Rect::new(11, 0, 1, 2));
    let bands = [RowContentBand::semantic(
        Rect::new(9, 0, 2, 2),
        Role::TextSecondary,
    )];
    let interaction = state(
        FocusState::Focused,
        PointerPresentation::Rest,
        NavigationState::None,
        CurrentState::Current,
        CheckedState::Checked,
        Availability::Enabled,
        OperationState::Pending,
    );
    let mut terminal = Terminal::new(TestBackend::new(row.width, row.height)).unwrap();
    terminal
        .draw(|frame| {
            for y in 0..2 {
                frame
                    .buffer_mut()
                    .set_string(3, y, "item ", theme.style(Role::TextPrimary));
                frame
                    .buffer_mut()
                    .set_string(9, y, "m ", theme.style(Role::TextSecondary));
            }
            assert_eq!(
                paint_row_interaction(
                    frame,
                    row,
                    tracks,
                    &bands,
                    ControlSurface::Overlay,
                    interaction,
                    &theme,
                ),
                Some(row)
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(0, 1)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(1, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(1, 1)].symbol(), " ");
    assert_eq!(buffer[(11, 0)].symbol(), theme.glyphs.status_bullet);
    assert_eq!(buffer[(11, 1)].symbol(), " ");
    for y in 0..2 {
        assert!(buffer[(4, y)].style().add_modifier.contains(Modifier::BOLD));
        for x in 0..row.width {
            assert_eq!(
                buffer[(x, y)].style().bg,
                theme.style(Role::SurfaceCurrent).bg,
                "x={x}, y={y}"
            );
        }
    }
}

fn assert_row_rejected(
    row: Rect,
    tracks: RowTracks<'_>,
    bands: &[RowContentBand],
    interaction: InteractionState,
) {
    let theme = Theme::rgb_test_dark();
    let mut terminal = Terminal::new(TestBackend::new(10, 2)).unwrap();
    let mut painted = Some(Rect::new(0, 0, 0, 0));
    terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        "unchanged!",
                        Style::new().fg(Color::Green).bg(Color::Magenta),
                    ),
                    Line::styled(
                        "unchanged!",
                        Style::new().fg(Color::Green).bg(Color::Magenta),
                    ),
                ]),
                frame.area(),
            );
            painted = paint_row_interaction(
                frame,
                row,
                tracks,
                bands,
                ControlSurface::Overlay,
                interaction,
                &theme,
            );
        })
        .unwrap();
    assert_eq!(painted, None);
    assert_eq!(symbols(&terminal, 0, 10), "unchanged!");
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].style().fg,
        Some(Color::Green)
    );
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].style().bg,
        Some(Color::Magenta)
    );
}

#[test]
fn row_rejects_clipped_overlapping_and_partial_height_geometry_before_paint() {
    assert_row_rejected(
        Rect::new(0, 1, 10, 2),
        RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 1, 1, 2)),
            Rect::new(2, 1, 6, 2),
        ),
        &[],
        idle(PointerPresentation::Rest),
    );
    assert_row_rejected(
        Rect::new(0, 0, 10, 2),
        RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 0, 2, 2)),
            Rect::new(1, 0, 6, 2),
        ),
        &[],
        idle(PointerPresentation::Rest),
    );
    assert_row_rejected(
        Rect::new(0, 0, 10, 2),
        RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 0, 1, 1)),
            Rect::new(2, 0, 6, 2),
        ),
        &[],
        idle(PointerPresentation::Rest),
    );
    assert_row_rejected(
        Rect::new(0, 0, 10, 2),
        RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 0, 1, 2)),
            Rect::new(2, 0, 6, 2),
        ),
        &[RowContentBand::semantic(
            Rect::new(7, 0, 2, 2),
            Role::TextSecondary,
        )],
        idle(PointerPresentation::Rest),
    );
    assert_row_rejected(
        Rect::new(0, 0, 10, 2),
        RowTracks::new(
            RowFocusCue::Track(Rect::new(0, 0, 1, 2)),
            Rect::new(2, 0, 6, 2),
        )
        .with_primary_role(Role::SurfaceHover),
        &[],
        idle(PointerPresentation::Rest),
    );
}

#[test]
fn row_rejects_required_semantic_cues_without_their_tracks_before_paint() {
    let row = Rect::new(0, 0, 10, 2);
    let tracks = RowTracks::new(
        RowFocusCue::Track(Rect::new(0, 0, 1, 2)),
        Rect::new(2, 0, 6, 2),
    );
    for interaction in [
        state(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::Provisional,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        ),
        state(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Checked,
            Availability::Enabled,
            OperationState::Idle,
        ),
        state(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Pending,
        ),
    ] {
        assert_row_rejected(row, tracks, &[], interaction);
    }
}

#[test]
fn close_geometry_preserves_the_right_inset_and_refuses_clipping() {
    let bounds = Rect::new(0, 0, 12, 1);
    let wide_area = Rect::new(2, 0, 8, 1);
    let narrow_area = Rect::new(0, 0, 2, 1);
    assert_eq!(close_rect(bounds, wide_area), Some(Rect::new(6, 0, 3, 1)));
    assert_eq!(close_rect(bounds, narrow_area), None);
    assert_eq!(
        close_rect(Rect::new(0, 0, u16::MAX, 1), Rect::new(u16::MAX, 0, 1, 1),),
        None
    );
    let clipped_area = Rect::new(10, 0, 3, 1);
    assert_eq!(close_rect(bounds, clipped_area), None);
}
