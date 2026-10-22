//! Fixed-geometry action controls.

use crate::{
    action_projection::ActionTone,
    terminal_text::{display_width, sanitize_terminal_text},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, PointerPresentation, content_style, pad_to_width,
};

/// Reading hierarchy for a standalone action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionKind {
    /// The primary action in a decision context.
    Primary,
    /// A secondary or cancelling action.
    Secondary,
    /// A quiet auxiliary action within navigation chrome.
    #[cfg(any(windows, test))]
    Auxiliary,
}

impl ActionKind {
    pub(crate) const fn content_role(self) -> Role {
        match self {
            Self::Primary => Role::AccentAction,
            Self::Secondary => Role::TextPrimary,
            #[cfg(any(windows, test))]
            Self::Auxiliary => Role::TextSecondary,
        }
    }
}

/// Owner-supplied action copy and visual state; semantic intent remains with the caller.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ActionSpec<'a> {
    label: &'a str,
    pending_label: Option<&'a str>,
    shortcut: Option<&'a str>,
    kind: ActionKind,
    tone: ActionTone,
    surface: ControlSurface,
    state: InteractionState,
}

impl<'a> ActionSpec<'a> {
    /// Defines a canonical `[ Label ]` action from raw localized copy.
    pub(crate) const fn new(
        label: &'a str,
        kind: ActionKind,
        surface: ControlSurface,
        state: InteractionState,
    ) -> Self {
        Self {
            label,
            pending_label: None,
            shortcut: None,
            kind,
            tone: ActionTone::Ordinary,
            surface,
            state,
        }
    }

    /// Declares pending capability and reserves its status slot plus the larger localized label.
    pub(crate) const fn with_pending_label(mut self, label: &'a str) -> Self {
        self.pending_label = Some(label);
        self
    }

    /// Keeps the owner's formatted binding inside the action's fixed extent.
    pub(crate) const fn with_shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub(crate) const fn with_tone(mut self, tone: ActionTone) -> Self {
        self.tone = tone;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActionLayout {
    rect: Rect,
    focus_width: usize,
    label_width: usize,
    shortcut_width: usize,
    status_width: usize,
}

/// Resolves the exact fixed action extent without painting or consulting pointer state.
///
/// A renderer uses this rectangle to project prior successfully presented pointer state, replaces
/// only [`InteractionState`]'s pointer axis, then calls [`render_action`].
pub(crate) fn action_rect(
    bounds: Rect,
    area: Rect,
    spec: ActionSpec<'_>,
    theme: &Theme,
) -> Option<Rect> {
    resolve_action_layout(bounds, area, spec, theme).map(|layout| layout.rect)
}

/// Paints one canonical action and returns its complete visible and clickable extent.
///
/// Geometry is validated before the frame is changed. The caller may record the returned rectangle
/// against its exact typed target only after this function succeeds.
pub(crate) fn render_action(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: ActionSpec<'_>,
    theme: &Theme,
) -> Option<Rect> {
    let layout = resolve_action_layout(frame.area(), area, spec, theme)?;
    let idle = sanitize_terminal_text(spec.label);
    let pending = sanitize_terminal_text(spec.pending_label.unwrap_or(spec.label));
    let state = spec.state;
    let interaction = state.interaction_role();
    let enabled = state.availability == Availability::Enabled;
    let action_role = if enabled {
        spec.tone.content_role(spec.kind.content_role())
    } else {
        Role::TextDisabled
    };
    let bracket_role = if state.focus == FocusState::Focused {
        Role::AccentFocus
    } else {
        action_role
    };
    let focus = if state.focus == FocusState::Focused {
        pad_to_width(theme.glyphs.active_marker, layout.focus_width)
    } else {
        " ".repeat(layout.focus_width)
    };
    let label = if state.operation == OperationState::Pending {
        pending.as_ref()
    } else {
        idle.as_ref()
    };
    let label = pad_to_width(label, layout.label_width);
    let shortcut = spec.shortcut.map_or_else(String::new, |shortcut| {
        format!("  {}", sanitize_terminal_text(shortcut))
    });
    let shortcut = pad_to_width(&shortcut, layout.shortcut_width);
    let mut label_style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(action_role),
        theme.style(action_role),
    );
    // Hover and press strengthen the label as well as the fill, so pointer feedback survives
    // themes whose hover surface is close to the base surface.
    if state.pointer != PointerPresentation::Rest && enabled {
        label_style = label_style.add_modifier(Modifier::BOLD);
    }
    let status = if state.operation == OperationState::Pending {
        pad_to_width(theme.glyphs.status_bullet, layout.status_width)
    } else {
        " ".repeat(layout.status_width)
    };
    let focus_role = if state.focus == FocusState::Focused {
        Role::AccentFocus
    } else {
        action_role
    };
    let focus_style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(focus_role),
        theme.style(focus_role),
    );
    let bracket_style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(bracket_role),
        theme.style(bracket_role),
    );
    let status_role = if state.operation == OperationState::Pending {
        Role::StatusPending
    } else {
        action_role
    };
    let status_style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(status_role),
        theme.style(status_role),
    );
    let shortcut_role = if enabled {
        Role::TextSecondary
    } else {
        Role::TextDisabled
    };
    let shortcut_style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(shortcut_role),
        theme.style(shortcut_role),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(focus, focus_style),
            Span::styled("[ ", bracket_style),
            Span::styled(label, label_style),
            Span::styled(shortcut, shortcut_style),
            Span::styled(" ]", bracket_style),
            Span::styled(status, status_style),
        ])),
        layout.rect,
    );
    Some(layout.rect)
}

fn resolve_action_layout(
    bounds: Rect,
    area: Rect,
    spec: ActionSpec<'_>,
    theme: &Theme,
) -> Option<ActionLayout> {
    // A plain action has no value boundary. Checked projection belongs to a boolean,
    // choice, or row-value component with its own stable glyph track.
    if area.height == 0
        || spec.state.checked == CheckedState::Checked
        || spec.state.current == CurrentState::Current
        || spec.state.navigation == NavigationState::Provisional
        || (spec.state.operation == OperationState::Pending && spec.pending_label.is_none())
    {
        return None;
    }
    let idle_width = display_width(&sanitize_terminal_text(spec.label));
    let pending_width = spec
        .pending_label
        .map(|label| display_width(&sanitize_terminal_text(label)));
    if idle_width == 0 || pending_width == Some(0) {
        return None;
    }
    let label_width = idle_width.max(pending_width.unwrap_or(idle_width));
    let shortcut_width = match spec.shortcut {
        Some(shortcut) => {
            let width = display_width(&sanitize_terminal_text(shortcut));
            if width == 0 {
                return None;
            }
            width.checked_add(2)?
        }
        None => 0,
    };
    let focus_width = display_width(theme.glyphs.active_marker).max(1);
    let status_width = spec
        .pending_label
        .map(|_| display_width(theme.glyphs.status_bullet).max(1))
        .unwrap_or(0);
    let width = focus_width
        .checked_add(4)?
        .checked_add(label_width)?
        .checked_add(shortcut_width)?
        .checked_add(status_width)?;
    let width = u16::try_from(width).ok()?;
    if width > area.width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    if bounds.intersection(rect) != rect {
        return None;
    }
    Some(ActionLayout {
        rect,
        focus_width,
        label_width,
        shortcut_width,
        status_width,
    })
}

/// Resolves the complete close-target extent before pointer projection or paint.
pub(crate) fn close_rect(bounds: Rect, area: Rect) -> Option<Rect> {
    if area.height == 0 || area.width < 3 {
        return None;
    }
    let inset = u16::from(area.width >= 5);
    let rect = Rect::new(
        area.right().checked_sub(3_u16.checked_add(inset)?)?,
        area.y,
        3,
        1,
    );
    super::icon::icon_action_rect(bounds, rect).filter(|rect| area.intersection(*rect) == *rect)
}
