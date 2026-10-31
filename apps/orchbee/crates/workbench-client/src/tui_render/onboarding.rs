//! Non-capturing guidance; only painted controls own pointer gestures.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, action_rect,
    render_action,
};
use super::{
    Block, Borders, Clear, ClientView, Frame, HitLayer, HitMapBuilder, Locale, Paragraph, Rect,
    Role, Theme,
};
use crate::{
    onboarding::Action,
    terminal_text::display_width,
    tui_hit_map::{
        GlobalChromeAction, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
};

/// Reserves guidance inside the sidebar while retaining at least four tree rows.
pub(super) fn area(sidebar: Rect) -> Option<Rect> {
    if sidebar.width < 18 || sidebar.height < 8 {
        return None;
    }
    let height = if sidebar.width >= 24 && sidebar.height >= 15 {
        7
    } else {
        4
    };
    Some(Rect::new(
        sidebar.x,
        sidebar.bottom() - height,
        sidebar.width,
        height,
    ))
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    layer: HitLayer,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    frame.render_widget(Clear, area);
    // The heading and status start on the label track that follows the actions' focus track, and
    // one blank cell keeps the heading clear of its rule.
    let track = u16::try_from(display_width(theme.glyphs.active_marker).max(1)).unwrap_or(1);
    let block = Block::new()
        .borders(Borders::TOP)
        .title(format!(
            "{}{} ",
            " ".repeat(usize::from(track)),
            locale.onboarding_title()
        ))
        .style(theme.style(Role::SurfaceChrome));
    let inner = block.inner(area);
    let text_area = Rect::new(
        inner.x.saturating_add(track).min(inner.right()),
        inner.y,
        inner.width.saturating_sub(track),
        1,
    );
    frame.render_widget(block, area);
    let compact = area.height < 7;
    let actions: &[Action] = if compact {
        &[Action::Help, Action::Dismiss]
    } else {
        &Action::ALL
    };
    let start = 1;
    let status = crate::terminal_text::truncate_display(
        locale.onboarding_status(view.onboarding.read_failed),
        usize::from(text_area.width),
        theme.glyphs.ellipsis,
    );
    frame.render_widget(
        Paragraph::new(status).style(theme.style(if view.onboarding.read_failed {
            Role::StatusWarning
        } else {
            Role::TextSecondary
        })),
        text_area,
    );
    for (index, action) in actions.iter().copied().enumerate() {
        let row = Rect::new(inner.x, inner.y + start + index as u16, inner.width, 1);
        let enabled = view.machines.available || !action.needs_session();
        let hint = (action == Action::Help)
            .then(|| {
                view.bindings()
                    .action_labels_with_glyphs(action.binding(), theme.glyphs.key_labels())
                    .into_iter()
                    .next()
            })
            .flatten();
        let label = locale.onboarding_label(action);
        let state = InteractionState::new(
            FocusState::Unfocused,
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            if enabled {
                Availability::Enabled
            } else {
                Availability::Disabled
            },
            OperationState::Idle,
        );
        let text = hint
            .map(|hint| format!("{label}  {hint}"))
            .filter(|text| {
                action_rect(
                    frame.area(),
                    row,
                    ActionSpec::new(text, ActionKind::Secondary, ControlSurface::Chrome, state),
                    theme,
                )
                .is_some()
            })
            .unwrap_or_else(|| label.to_owned());
        let spec = ActionSpec::new(&text, ActionKind::Secondary, ControlSurface::Chrome, state);
        let Some(rect) = action_rect(frame.area(), row, spec, theme) else {
            continue;
        };
        let target = InteractionTarget::GlobalChrome(GlobalChromeAction::Onboarding(action));
        let interaction = if enabled {
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
        } else {
            InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
        };
        let pointer = match hits.projected_pointer(layer, rect, rect, &target, interaction) {
            ProjectedPointer::Rest => PointerPresentation::Rest,
            ProjectedPointer::Hovered => PointerPresentation::Hovered,
            ProjectedPointer::Pressed => PointerPresentation::Pressed,
        };
        if render_action(
            frame,
            row,
            ActionSpec::new(
                &text,
                ActionKind::Secondary,
                ControlSurface::Chrome,
                state.with_pointer(pointer),
            ),
            theme,
        ) == Some(rect)
        {
            hits.record_interaction(layer, rect, rect, target, interaction);
        }
    }
    hits.record(HitLayer::ChromeSurface, area, HitTarget::OnboardingSurface);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guidance_geometry_is_bounded_and_degrades_without_terminal_resize() {
        for (width, height) in [(0, 0), (17, 8), (18, 8), (24, 14), (24, 15), (40, 40)] {
            let content = Rect::new(0, 0, width, height);
            if let Some(card) = area(content) {
                assert_eq!(content.intersection(card), card);
                assert!(card.y >= content.y + 4);
                assert!(card.height == 4 || card.height == 7);
            }
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    #[test]
    fn sidebar_guidance_leaves_every_terminal_cell_untouched() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        let mut view = ClientView::new(80);
        view.machines.available = true;
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                for y in 0..24 {
                    for x in 24..80 {
                        frame.buffer_mut()[(x, y)].set_symbol("T");
                    }
                }
                render(
                    frame,
                    area(Rect::new(0, 0, 24, 24)).unwrap(),
                    &view,
                    Locale::EnUs,
                    HitLayer::Chrome,
                    &Theme::terminal_native(),
                    &mut hits,
                );
            })
            .unwrap();
        let map = hits.finish();
        for y in 0..24 {
            for x in 24..80 {
                assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), "T");
                assert!(map.hit_target_at(x, y).is_none());
            }
        }
    }

    #[test]
    fn localized_cards_expose_only_painted_controls_and_do_not_capture_input() {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (width, height) in [(32, 24), (24, 15), (20, 8)] {
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                        .unwrap();
                let mut view = ClientView::new(width);
                view.onboarding.visible = true;
                view.machines.available = true;
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            area(frame.area()).unwrap(),
                            &view,
                            locale,
                            HitLayer::Chrome,
                            &Theme::terminal_native(),
                            &mut hits,
                        )
                    })
                    .unwrap();
                let map = hits.finish();
                let has = |action| {
                    (0..height).any(|y| {
                        (0..width).any(|x| {
                            map.hit_target_at(x, y) == Some(&HitTarget::Onboarding(action))
                        })
                    })
                };
                assert!(has(Action::Dismiss));
                assert!(has(Action::Help));
                let full = width >= 24 && height >= 15;
                for action in [Action::Workspace, Action::Agent, Action::Menu] {
                    assert_eq!(has(action), full, "{locale:?} {width}x{height} {action:?}");
                }
                assert_eq!(view.overlay(), None);
                assert!(!view.prefix_active());
            }
        }
    }

    #[test]
    fn minimum_full_card_paints_every_label_and_the_complete_status() {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for read_failed in [false, true] {
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 15)).unwrap();
                let mut view = ClientView::new(24);
                view.onboarding.visible = true;
                view.machines.available = true;
                view.onboarding.read_failed = read_failed;
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            area(frame.area()).unwrap(),
                            &view,
                            locale,
                            HitLayer::Chrome,
                            &Theme::terminal_native(),
                            &mut hits,
                        )
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let rows = (0..15)
                    .map(|y| {
                        (0..24)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>()
                            .replace(' ', "")
                    })
                    .collect::<Vec<_>>();
                let mut expected = vec![locale.onboarding_status(read_failed)];
                expected.extend(
                    Action::ALL
                        .iter()
                        .map(|action| locale.onboarding_label(*action)),
                );
                for text in expected {
                    let compact = text.replace(' ', "");
                    assert!(
                        rows.iter().any(|row| row.contains(&compact)),
                        "{locale:?}: {text:?} must paint completely\n{rows:#?}"
                    );
                }
            }
        }
    }
}
