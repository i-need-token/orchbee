//! The placeholder of a Panel a restore held because its saved directory was unavailable: its
//! title, the saved directory, and the Retry, Start in home folder and Close buttons. Buttons that
//! no longer fit beside the previous one move to the next row; a button that fits nowhere is not
//! painted and so cannot be clicked.

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, action_rect,
    render_action,
};
use crate::{
    localization::{HeldRestoreAction, Locale},
    terminal_text::{sanitize_terminal_text, wrap_text},
    tui_hit_map::{
        HeldRestoreTarget, HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget,
        PrimaryContract, ProjectedPointer,
    },
    tui_theme::{Role, Theme},
    workbench_client::HeldRestoreView,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Text},
    widgets::Paragraph,
};

const ACTIONS: [HeldRestoreAction; 3] = [
    HeldRestoreAction::Retry,
    HeldRestoreAction::StartInHome,
    HeldRestoreAction::Close,
];

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    held: &HeldRestoreView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let mut lines = vec![Line::styled(
        locale.held_restore_title(),
        theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
    )];
    lines.extend(
        wrap_text(
            &Text::from(sanitize_terminal_text(held.directory.as_str())),
            area.width,
        )
        .lines
        .into_iter()
        .map(|line| line.style(theme.style(Role::TextSecondary))),
    );
    let text_rows = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    frame.render_widget(Paragraph::new(lines), area);
    let mut row = area.y.saturating_add(text_rows).saturating_add(1);
    let mut column = area.x;
    for action in ACTIONS {
        let target = InteractionTarget::HeldRestore(HeldRestoreTarget {
            panel: held.panel.clone(),
            presentation: held.presentation.clone(),
            action,
        });
        let spec = |pointer| {
            ActionSpec::new(
                locale.held_restore_action(action),
                if action == HeldRestoreAction::Retry {
                    ActionKind::Primary
                } else {
                    ActionKind::Secondary
                },
                ControlSurface::Canvas,
                InteractionState::new(
                    if held.focused == Some(action) {
                        FocusState::Focused
                    } else {
                        FocusState::Unfocused
                    },
                    pointer,
                    NavigationState::None,
                    CurrentState::NotCurrent,
                    CheckedState::Unchecked,
                    if held.pending {
                        Availability::Disabled
                    } else {
                        Availability::Enabled
                    },
                    OperationState::Idle,
                ),
            )
        };
        let interaction = if held.pending {
            InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
        } else {
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
        };
        let slot = |column: u16, row: u16| {
            Rect::new(
                column,
                row,
                area.right().saturating_sub(column),
                u16::from(row < area.bottom()),
            )
        };
        let mut rect = action_rect(
            area,
            slot(column, row),
            spec(PointerPresentation::Rest),
            theme,
        );
        if rect.is_none() && column > area.x {
            // Wrap to the next row, one blank row below.
            row = row.saturating_add(2);
            column = area.x;
            rect = action_rect(
                area,
                slot(column, row),
                spec(PointerPresentation::Rest),
                theme,
            );
        }
        let Some(rect) = rect else {
            continue;
        };
        let pointer =
            match hits.projected_pointer(HitLayer::Foreground, rect, rect, &target, interaction) {
                ProjectedPointer::Rest => PointerPresentation::Rest,
                ProjectedPointer::Hovered => PointerPresentation::Hovered,
                ProjectedPointer::Pressed => PointerPresentation::Pressed,
            };
        let Some(painted) = render_action(frame, slot(column, row), spec(pointer), theme) else {
            continue;
        };
        hits.record_interaction(HitLayer::Foreground, painted, painted, target, interaction);
        column = painted.right().saturating_add(1);
    }
}
