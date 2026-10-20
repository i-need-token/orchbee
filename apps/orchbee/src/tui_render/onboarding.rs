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
    tui_pointer::{
        GlobalChromeAction, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
};

pub(super) fn area(content: Rect) -> Option<Rect> {
    if content.width < 20 || content.height < 5 {
        return None;
    }
    let height = if content.width >= 32 && content.height >= 12 {
        9
    } else {
        4
    };
    let width = content.width.min(68);
    Some(Rect::new(
        content.right() - width,
        content.y + 1,
        width,
        height,
    ))
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    connected: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    frame.render_widget(Clear, area);
    let block = Block::new()
        .borders(Borders::ALL)
        .title(locale.onboarding_title())
        .style(theme.style(Role::SurfaceChrome));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let compact = area.height < 9;
    let actions: &[Action] = if compact {
        &[Action::Help, Action::Dismiss]
    } else {
        &Action::ALL
    };
    let start = if compact { 0 } else { 1 };
    if !compact {
        frame.render_widget(
            Paragraph::new(locale.onboarding_status(view.onboarding.read_failed)).style(
                theme.style(if view.onboarding.read_failed {
                    Role::StatusWarning
                } else {
                    Role::TextSecondary
                }),
            ),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
    }
    for (index, action) in actions.iter().copied().enumerate() {
        let row = Rect::new(inner.x, inner.y + start + index as u16, inner.width, 1);
        let enabled = connected || !action.needs_session();
        let hint = view
            .bindings()
            .action_labels_with_glyphs(
                action.binding(),
                super::agent_sidebar::key_label_glyphs(theme),
            )
            .into_iter()
            .next();
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
        let pointer =
            match hits.projected_pointer(HitLayer::Chrome, rect, rect, &target, interaction) {
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
            hits.record_interaction(HitLayer::Chrome, rect, rect, target, interaction);
        }
    }
    hits.record(HitLayer::ChromeSurface, area, HitTarget::OnboardingSurface);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guidance_geometry_is_bounded_and_degrades_without_terminal_resize() {
        for (width, height) in [(0, 0), (19, 5), (20, 5), (30, 10), (60, 12), (120, 40)] {
            let content = Rect::new(0, 0, width, height);
            if let Some(card) = area(content) {
                assert_eq!(content.intersection(card), card);
                assert!(card.height == 4 || card.height == 9);
            }
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    #[test]
    fn localized_cards_expose_only_painted_controls_and_do_not_capture_input() {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (width, height) in [(80, 24), (32, 12), (20, 5)] {
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                        .unwrap();
                let mut view = ClientView::new(width);
                view.onboarding.visible = true;
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            area(frame.area()).unwrap(),
                            &view,
                            locale,
                            true,
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
                assert_eq!(has(Action::Workspace), width >= 32 && height >= 12);
                assert_eq!(view.overlay(), None);
                assert!(!view.prefix_active());
            }
        }
    }
}
