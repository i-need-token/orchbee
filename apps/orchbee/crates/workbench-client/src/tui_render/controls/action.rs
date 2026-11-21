//! Fixed-geometry action controls.

use crate::{
    action_projection::ActionTone,
    terminal_text::{display_width, sanitize_terminal_text},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
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
    /// The one confirming action of a dialog, form, or prompt. Standing chrome never uses it.
    Primary,
    /// A secondary or cancelling action.
    Secondary,
    /// A quiet auxiliary action within navigation chrome.
    Auxiliary,
}

impl ActionKind {
    pub(crate) const fn content_role(self) -> Role {
        match self {
            Self::Primary => Role::AccentAction,
            Self::Secondary => Role::TextPrimary,
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "pending actions are part of the shared action control contract; no current owner declares one"
        )
    )]
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
        // Secondary action text rises to primary under the pointer; hover never adds weight.
        let ordinary = match spec.kind.content_role() {
            Role::TextSecondary if state.pointer != PointerPresentation::Rest => Role::TextPrimary,
            role => role,
        };
        spec.tone.content_role(ordinary)
    } else {
        Role::TextDisabled
    };
    // The brackets alone carry keyboard focus, so a resting primary keeps them off its accent.
    let bracket_role = match (state.focus, action_role) {
        (FocusState::Focused, _) => Role::AccentFocus,
        (_, Role::AccentAction) => Role::TextPrimary,
        (_, role) => role,
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
    if theme.filled_actions() {
        let block = filled_block(
            theme,
            spec.surface,
            action_fill_role(
                spec.kind,
                spec.tone,
                enabled,
                state.pointer,
                ordinary_fill_is_surface(theme, spec.surface),
            ),
            band_hue_role(spec.kind, spec.tone, enabled),
        );
        let label_role = filled_label_role(spec.kind, spec.tone, enabled, state.pointer);
        let mut content = filled_text(theme, block, label_role);
        // Focus and press mark the label alone; the shortcut stays subordinate.
        if matches!(state.focus, FocusState::Focused) {
            content = content.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        if enabled && state.pointer == PointerPresentation::Pressed {
            content = content.add_modifier(Modifier::BOLD);
        }
        let shortcut_style = filled_text(
            theme,
            block,
            filled_shortcut_role(spec.kind, spec.tone, enabled),
        );
        let (status, status_role) = if state.operation == OperationState::Pending {
            (
                pad_to_width(theme.glyphs.status_bullet, layout.status_width),
                Role::StatusPending,
            )
        } else {
            (" ".repeat(layout.status_width), label_role)
        };
        let status_style = content_style(
            theme,
            spec.surface,
            interaction,
            Some(status_role),
            theme.style(status_role),
        );
        // The block owns its cells: whatever colors and modifiers the row beneath left there go
        // first, so a fill or band never mixes with a surface it sits on.
        let status_width = u16::try_from(layout.status_width).unwrap_or(layout.rect.width);
        frame.buffer_mut().set_style(
            Rect::new(
                layout.rect.x,
                layout.rect.y,
                layout.rect.width.saturating_sub(status_width),
                1,
            ),
            Style::reset(),
        );
        // The block is one cell of padding on each side of the label, so the action has no brackets.
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" ", block),
                Span::styled(label, content),
                Span::styled(shortcut, shortcut_style),
                Span::styled(" ", block),
                Span::styled(status, status_style),
            ])),
            layout.rect,
        );
        return Some(layout.rect);
    }
    let mut label_style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(action_role),
        theme.style(action_role),
    );
    // Bold marks press only; hover changes the fill and, for secondary actions, the text role.
    if state.pointer == PointerPresentation::Pressed && enabled {
        label_style = label_style.add_modifier(Modifier::BOLD);
    }
    let status = if state.operation == OperationState::Pending {
        pad_to_width(theme.glyphs.status_bullet, layout.status_width)
    } else {
        " ".repeat(layout.status_width)
    };
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
        theme.action_hint_role(action_role)
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
        || spec.state.navigation != NavigationState::None
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
    let status_width = spec
        .pending_label
        .map(|_| display_width(theme.glyphs.status_bullet).max(1))
        .unwrap_or(0);
    // Brackets with a blank inside each take four cells; a filled action pads with one blank each side.
    let chrome_width = if theme.filled_actions() { 2 } else { 4 };
    let width = label_width
        .checked_add(chrome_width)?
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
        label_width,
        shortcut_width,
        status_width,
    })
}

/// Whether a theme's ordinary rest fill is the very color of the surface an action sits on, where
/// the action would read as plain text.
fn ordinary_fill_is_surface(theme: &Theme, surface: ControlSurface) -> bool {
    let rest = theme.style(Role::SurfaceActionSecondary);
    !is_band(rest) && rest.bg.is_some() && rest.bg == theme.style(surface.role()).bg
}

/// The fill under a filled action, by intent and pointer state. A disabled action rests on the
/// ordinary fill, and so does a destructive one: its red label carries the meaning, and the accent
/// fill belongs to the primary action alone. Where the ordinary rest fill is the surface itself,
/// `lifted` moves every ordinary step up one, so the action stays a block; press then keeps the
/// pressed fill and its bold label.
fn action_fill_role(
    kind: ActionKind,
    tone: ActionTone,
    enabled: bool,
    pointer: PointerPresentation,
    lifted: bool,
) -> Role {
    let pointer = if enabled {
        pointer
    } else {
        PointerPresentation::Rest
    };
    let primary = enabled && kind == ActionKind::Primary && matches!(tone, ActionTone::Ordinary);
    match (primary, pointer) {
        (true, PointerPresentation::Rest) => Role::SurfaceActionPrimary,
        (true, PointerPresentation::Hovered) => Role::SurfaceActionPrimaryHover,
        (true, PointerPresentation::Pressed) => Role::SurfaceActionPrimaryPressed,
        (false, PointerPresentation::Rest) if !lifted => Role::SurfaceActionSecondary,
        (false, PointerPresentation::Rest) => Role::SurfaceActionSecondaryHover,
        (false, PointerPresentation::Hovered) if !lifted => Role::SurfaceActionSecondaryHover,
        (false, PointerPresentation::Hovered | PointerPresentation::Pressed) => {
            Role::SurfaceActionSecondaryPressed
        }
    }
}

/// The label of a filled action: inverse text on the accent fill, primary text on the ordinary
/// fill, the destructive hue on the ordinary fill, and disabled text when unavailable.
fn filled_label_role(
    kind: ActionKind,
    tone: ActionTone,
    enabled: bool,
    pointer: PointerPresentation,
) -> Role {
    match (enabled, tone, kind) {
        (false, _, _) => Role::TextDisabled,
        (true, ActionTone::Destructive, _) => Role::AccentDestructive,
        (true, ActionTone::Ordinary, ActionKind::Primary) => Role::TextInverse,
        (true, ActionTone::Ordinary, ActionKind::Secondary) => Role::TextPrimary,
        (true, ActionTone::Ordinary, ActionKind::Auxiliary) => {
            if pointer == PointerPresentation::Rest {
                Role::TextSecondary
            } else {
                Role::TextPrimary
            }
        }
    }
}

/// The block a filled action paints. A colored fill is its own background. A reverse-video band
/// is one explicit pair on every cell: the hue that names its action, or primary text, over the
/// action's base surface, with that role's modifiers. A host may apply `DIM` before reverse video
/// swaps the pair, so a band whose cells differed in `DIM` would show two backgrounds.
fn filled_block(
    theme: &Theme,
    surface: ControlSurface,
    fill_role: Role,
    hue: Option<Role>,
) -> Style {
    let fill = theme.style(fill_role);
    if !is_band(fill) {
        return fill;
    }
    let ink = theme.style(hue.unwrap_or(Role::TextPrimary));
    fill.fg(ink.fg.unwrap_or(Color::Reset))
        .bg(theme.style(surface.role()).bg.unwrap_or(Color::Reset))
        .add_modifier(ink.add_modifier)
}

/// Whether a fill is the resolver's reverse-video fallback instead of a color.
fn is_band(fill: Style) -> bool {
    fill.add_modifier.contains(Modifier::REVERSED)
}

/// The hue that names a filled action in a band: the accent for the primary action, the
/// destructive hue for a destructive one, and the disabled text role when unavailable, whose `DIM`
/// then covers the whole band. The ordinary action takes primary text.
fn band_hue_role(kind: ActionKind, tone: ActionTone, enabled: bool) -> Option<Role> {
    if !enabled {
        return Some(Role::TextDisabled);
    }
    match tone {
        ActionTone::Destructive => Some(Role::AccentDestructive),
        ActionTone::Ordinary if kind == ActionKind::Primary => Some(Role::AccentAction),
        ActionTone::Ordinary => None,
    }
}

/// The shortcut inside a filled action: inverse text on the accent, secondary text elsewhere.
fn filled_shortcut_role(kind: ActionKind, tone: ActionTone, enabled: bool) -> Role {
    match (enabled, tone, kind) {
        (false, _, _) => Role::TextDisabled,
        (true, ActionTone::Ordinary, ActionKind::Primary) => Role::TextInverse,
        (true, _, _) => Role::TextSecondary,
    }
}

/// Text on a filled block: the role's foreground and modifiers over a colored fill. A band's text
/// is the band itself, so the label, the shortcut and the padding stay one block.
fn filled_text(theme: &Theme, block: Style, role: Role) -> Style {
    if is_band(block) {
        return block;
    }
    let text = theme.style(role);
    text.fg
        .map_or(block, |foreground| block.fg(foreground))
        .add_modifier(text.add_modifier)
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
