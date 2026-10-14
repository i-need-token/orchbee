//! App-private controls own fixed cell geometry and semantic interaction styling.
//!
//! Feature owners retain semantic identity, availability reasons, and typed
//! [`crate::tui_pointer::HitTarget`] values. This module paints admitted state and returns the
//! complete painted rectangle; it never derives an intent from product copy or glyphs.
use crate::{
    terminal_text::display_width,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Text,
    widgets::{Paragraph, Wrap},
};

mod action;
mod hint;
mod icon;
mod row;

pub(crate) use action::{ActionKind, ActionSpec, action_rect, close_rect, render_action};
pub(crate) use hint::{KeyHint, measure_key_hints, render_key_hints};
pub(crate) use icon::{IconActionGlyph, IconActionSpec, icon_action_rect, render_icon_action};
pub(crate) use row::{RowContentBand, RowFocusCue, RowTracks, paint_row_interaction};

/// The base surface owned by a control's compositor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlSurface {
    /// Persistent application chrome.
    Chrome,
    /// An opaque overlay surface.
    Overlay,
}

impl ControlSurface {
    fn role(self) -> Role {
        match self {
            Self::Chrome => Role::SurfaceChrome,
            Self::Overlay => Role::SurfaceOverlay,
        }
    }
}

/// Whether this component's declared keyboard owner owns contextual input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FocusState {
    /// Another component owns keyboard input.
    Unfocused,
    /// This component's declared keyboard owner owns input.
    Focused,
}

/// Ephemeral pointer presentation that never changes focus or a domain value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerPresentation {
    /// No admitted pointer is over or armed on the target.
    Rest,
    /// The admitted pointer is over the target.
    Hovered,
    /// A valid gesture is armed and remains inside the target.
    Pressed,
}

/// Reversible keyboard navigation before acceptance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationState {
    /// This item is not the provisional keyboard item.
    None,
    /// This item is the provisional keyboard item.
    Provisional,
}

/// Whether this component represents the persistent current context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CurrentState {
    /// The component is not current.
    NotCurrent,
    /// The component represents the current context.
    Current,
}

/// Whether the owning value model marks this component as checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckedState {
    /// The value is not checked.
    Unchecked,
    /// The value is checked at the owner's declared commit boundary.
    Checked,
}

/// Whether the owner currently admits activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    /// Activation is admitted.
    Enabled,
    /// Activation is rejected; the feature owner retains the localized reason.
    Disabled,
}

/// Operation presentation after an admitted activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationState {
    /// No operation is in flight.
    Idle,
    /// The owner reports an in-flight operation.
    Pending,
}

/// Independent visual axes supplied by the semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InteractionState {
    focus: FocusState,
    pointer: PointerPresentation,
    navigation: NavigationState,
    current: CurrentState,
    checked: CheckedState,
    availability: Availability,
    operation: OperationState,
}

impl InteractionState {
    /// Constructs the complete state without compressing independent axes into `selected`.
    pub(crate) const fn new(
        focus: FocusState,
        pointer: PointerPresentation,
        navigation: NavigationState,
        current: CurrentState,
        checked: CheckedState,
        availability: Availability,
        operation: OperationState,
    ) -> Self {
        Self {
            focus,
            pointer,
            navigation,
            current,
            checked,
            availability,
            operation,
        }
    }

    /// Replaces only ephemeral pointer presentation after exact hit geometry is resolved.
    pub(crate) const fn with_pointer(mut self, pointer: PointerPresentation) -> Self {
        self.pointer = pointer;
        self
    }

    fn pointer_surface(self) -> Option<Role> {
        if self.availability == Availability::Disabled {
            return None;
        }
        match self.pointer {
            PointerPresentation::Rest => None,
            PointerPresentation::Hovered => Some(Role::SurfaceHover),
            PointerPresentation::Pressed => Some(Role::SurfacePressed),
        }
    }

    fn interaction_role(self) -> Option<Role> {
        if self.pointer == PointerPresentation::Pressed
            && let Some(role) = self.pointer_surface()
        {
            return Some(role);
        }
        if self.navigation == NavigationState::Provisional {
            return Some(Role::SurfaceSelection);
        }
        if let Some(role) = self.pointer_surface() {
            return Some(role);
        }
        if self.current == CurrentState::Current {
            return Some(Role::SurfaceCurrent);
        }
        None
    }
}

/// Applies pointer feedback to one already-painted semantic child target.
///
/// The owner first paints the target's rest or disabled content and any enclosing row interaction.
/// Rest and disabled calls validate geometry but leave that composition intact. Hover and press
/// keep the symbols, restore the declared semantic role, and apply the child interaction over its
/// exact extent. Success returns that same extent for typed hit publication.
pub(crate) fn paint_pointer_target(
    frame: &mut Frame<'_>,
    rect: Rect,
    surface: ControlSurface,
    pointer: PointerPresentation,
    role: Role,
    availability: Availability,
    theme: &Theme,
) -> Option<Rect> {
    if rect.is_empty() || frame.area().intersection(rect) != rect || !is_content_role(role) {
        return None;
    }
    if availability == Availability::Disabled || pointer == PointerPresentation::Rest {
        return Some(rect);
    }
    let interaction = match pointer {
        PointerPresentation::Hovered => Role::SurfaceHover,
        PointerPresentation::Pressed => Role::SurfacePressed,
        PointerPresentation::Rest => unreachable!("rest returned before composition"),
    };
    let style = content_style(
        theme,
        surface,
        Some(interaction),
        Some(role),
        theme.style(role),
    );
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            cell.set_style(Style::reset()).set_style(style);
        }
    }
    Some(rect)
}

fn content_style(
    theme: &Theme,
    surface: ControlSurface,
    interaction: Option<Role>,
    role: Option<Role>,
    content: Style,
) -> Style {
    theme.content_on_surface(surface.role(), interaction, role, content)
}

fn is_content_role(role: Role) -> bool {
    match role {
        Role::TextPrimary
        | Role::TextSecondary
        | Role::TextDisabled
        | Role::TextInverse
        | Role::AccentAction
        | Role::AccentFocus
        | Role::AccentSelection
        | Role::StatusSuccess
        | Role::StatusWarning
        | Role::StatusError
        | Role::StatusInfo
        | Role::StatusPending
        | Role::StatusWorking
        | Role::StatusUnread
        | Role::HintKey
        | Role::HintLabel => true,
        #[cfg(test)]
        Role::AccentLink => true,
        _ => false,
    }
}

fn pad_to_width(value: &str, width: usize) -> String {
    let mut value = value.to_owned();
    value.push_str(&" ".repeat(width.saturating_sub(display_width(&value))));
    value
}

/// The wrapped position of a logical text row, shared by fields and actions.
pub(super) fn text_row(text: &Text<'_>, inner: Rect, index: usize) -> Rect {
    let offset = Paragraph::new(Text::from(text.lines[..index].to_vec()))
        .wrap(Wrap { trim: false })
        .line_count(inner.width);
    let y = inner
        .y
        .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX));
    Rect::new(inner.x, y, inner.width, u16::from(y < inner.bottom()))
}

#[cfg(windows)]
pub(super) fn mode_bar(
    viewport: ratatui::layout::Rect,
    sidebar: crate::client_view::SidebarSurface,
) -> ratatui::layout::Rect {
    let areas = crate::tui_layout::project_shell_areas(viewport, sidebar);
    if areas.tabs.is_empty() {
        ratatui::layout::Rect::new(
            areas.workbench.x,
            areas.workbench.y,
            areas.workbench.width,
            areas.workbench.height.min(1),
        )
    } else {
        areas.tabs
    }
}

#[cfg(test)]
mod tests;
