//! Fixed-track row interaction composition.

use crate::{
    terminal_text::{display_width, sanitize_terminal_text},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
};

use super::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, content_style, is_content_role, pad_to_width,
};

/// A disjoint row content band whose semantic role must survive interaction fills.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowContentBand {
    rect: Rect,
    role: Option<Role>,
}

impl RowContentBand {
    /// Marks content painted from one semantic theme role.
    pub(crate) const fn semantic(rect: Rect, role: Role) -> Self {
        Self {
            rect,
            role: Some(role),
        }
    }

    /// Marks explicitly styled content whose foreground must not be reclassified.
    pub(crate) const fn explicit(rect: Rect) -> Self {
        Self { rect, role: None }
    }
}

/// Where the row's keyboard owner presents its one focus cue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RowFocusCue {
    /// The owning framed component already presents keyboard focus.
    EnclosingFrame,
    /// This unframed row presents keyboard focus in a dedicated track.
    #[cfg(test)]
    Track(Rect),
}

impl RowFocusCue {
    const fn track(self) -> Option<Rect> {
        match self {
            Self::EnclosingFrame => None,
            #[cfg(test)]
            Self::Track(rect) => Some(rect),
        }
    }
}

/// Fixed row tracks whose symbols remain owned by the row and its domain content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowTracks<'a> {
    focus: RowFocusCue,
    primary: Rect,
    primary_role: Role,
    provisional: Option<Rect>,
    checked: Option<(Rect, &'a str)>,
    status: Option<Rect>,
    plain_current: bool,
}

impl<'a> RowTracks<'a> {
    /// Defines an explicit focus projection and the required primary track.
    pub(crate) const fn new(focus: RowFocusCue, primary: Rect) -> Self {
        Self {
            focus,
            primary,
            primary_role: Role::TextPrimary,
            provisional: None,
            checked: None,
            status: None,
            plain_current: false,
        }
    }

    /// Declares the semantic role already used to paint the primary track.
    ///
    /// Non-content roles are refused with the rest of row geometry before any frame mutation.
    pub(crate) const fn with_primary_role(mut self, role: Role) -> Self {
        self.primary_role = role;
        self
    }

    /// Reserves the shared provisional-navigation marker track.
    pub(crate) const fn with_provisional(mut self, provisional: Rect) -> Self {
        self.provisional = Some(provisional);
        self
    }

    /// Identifies the domain-owned checked-glyph track.
    pub(crate) const fn with_checked(mut self, checked: Rect, glyph: &'a str) -> Self {
        self.checked = Some((checked, glyph));
        self
    }

    /// Reserves the operation-status track for idle and pending rows.
    pub(crate) const fn with_status(mut self, status: Rect) -> Self {
        self.status = Some(status);
        self
    }

    /// Marks current identity by the current surface alone, without bold on the primary track.
    pub(crate) const fn with_plain_current(mut self) -> Self {
        self.plain_current = true;
        self
    }
}

/// Applies interaction state to one fully owned row and returns the same complete row extent.
///
/// The caller paints domain content first and identifies disjoint semantic or explicit bands. This
/// pass owns the focus, provisional, checked, and status tracks, then composes one row-wide
/// interaction surface without reconstructing the remaining symbols.
pub(crate) fn paint_row_interaction(
    frame: &mut Frame<'_>,
    row: Rect,
    tracks: RowTracks<'_>,
    bands: &[RowContentBand],
    surface: ControlSurface,
    state: InteractionState,
    theme: &Theme,
) -> Option<Rect> {
    let focus_width = display_width(theme.glyphs.active_marker).max(1);
    let status_width = display_width(theme.glyphs.status_bullet).max(1);
    let focus_track = tracks.focus.track();
    let checked_rect = tracks.checked.map(|(rect, _)| rect);
    let track_rects = [
        focus_track,
        Some(tracks.primary),
        tracks.provisional,
        checked_rect,
        tracks.status,
    ];
    if row.is_empty()
        || frame.area().intersection(row) != row
        || focus_track
            .is_some_and(|rect| rect.width < u16::try_from(focus_width).unwrap_or(u16::MAX))
        || track_rects.iter().flatten().any(|track| {
            track.is_empty()
                || row.intersection(*track) != *track
                || track.y != row.y
                || track.height != row.height
        })
        || track_rects.iter().enumerate().any(|(index, track)| {
            track.is_some_and(|track| {
                track_rects[index + 1..]
                    .iter()
                    .flatten()
                    .any(|other| !track.intersection(*other).is_empty())
            })
        })
        || bands.iter().any(|band| {
            band.rect.is_empty()
                || row.intersection(band.rect) != band.rect
                || band.role.is_some_and(|role| !is_content_role(role))
                || track_rects
                    .iter()
                    .flatten()
                    .any(|track| !track.intersection(band.rect).is_empty())
        })
        || !is_content_role(tracks.primary_role)
        || bands.iter().enumerate().any(|(index, band)| {
            bands[index + 1..]
                .iter()
                .any(|other| !band.rect.intersection(other.rect).is_empty())
        })
        || (state.navigation == NavigationState::Provisional && tracks.provisional.is_none())
        || (state.checked == CheckedState::Checked && tracks.checked.is_none())
        || tracks
            .provisional
            .is_some_and(|rect| !valid_marker_track(rect, theme.glyphs.navigation_marker))
        || tracks
            .checked
            .is_some_and(|(checked, glyph)| !valid_marker_track(checked, glyph))
        || tracks
            .status
            .is_some_and(|status| status.width < u16::try_from(status_width).unwrap_or(u16::MAX))
        || (state.operation == OperationState::Pending && tracks.status.is_none())
    {
        return None;
    }

    let interaction = state.interaction_role();
    apply_content_band(frame, row, surface, interaction, None, None, theme);
    let primary_role = if state.availability == Availability::Disabled {
        Role::TextDisabled
    } else {
        tracks.primary_role
    };
    apply_content_band(
        frame,
        tracks.primary,
        surface,
        interaction,
        Some(primary_role),
        (primary_role == Role::TextDisabled).then(|| theme.style(primary_role)),
        theme,
    );
    for band in bands {
        let role = disabled_content_role(band.role, state.availability);
        let replacement =
            (role == Some(Role::TextDisabled)).then(|| theme.style(Role::TextDisabled));
        apply_content_band(
            frame,
            band.rect,
            surface,
            interaction,
            role,
            replacement,
            theme,
        );
    }
    if state.current == CurrentState::Current && !tracks.plain_current {
        frame
            .buffer_mut()
            .set_style(tracks.primary, Style::new().add_modifier(Modifier::BOLD));
    }

    if let Some(focus) = focus_track {
        for offset in 0..focus.height {
            let marker = if state.focus == FocusState::Focused {
                pad_to_width(theme.glyphs.active_marker, usize::from(focus.width))
            } else {
                " ".repeat(usize::from(focus.width))
            };
            let role = if state.focus == FocusState::Focused {
                Role::AccentFocus
            } else if state.availability == Availability::Disabled {
                Role::TextDisabled
            } else {
                Role::TextSecondary
            };
            frame.buffer_mut().set_string(
                focus.x,
                focus.y + offset,
                marker,
                content_style(theme, surface, interaction, Some(role), theme.style(role)),
            );
        }
    }
    if let Some(provisional) = tracks.provisional {
        paint_marker_track(
            frame,
            provisional,
            theme.glyphs.navigation_marker,
            state.navigation == NavigationState::Provisional,
            surface,
            state,
            theme,
        );
    }
    if let Some((checked, glyph)) = tracks.checked {
        paint_marker_track(
            frame,
            checked,
            glyph,
            state.checked == CheckedState::Checked,
            surface,
            state,
            theme,
        );
    }
    if let Some(status) = tracks.status {
        for offset in 0..status.height {
            let marker = if offset == 0 && state.operation == OperationState::Pending {
                pad_to_width(theme.glyphs.status_bullet, usize::from(status.width))
            } else {
                " ".repeat(usize::from(status.width))
            };
            let role = if state.operation == OperationState::Pending {
                Role::StatusPending
            } else if state.availability == Availability::Disabled {
                Role::TextDisabled
            } else {
                Role::TextSecondary
            };
            frame.buffer_mut().set_string(
                status.x,
                status.y + offset,
                marker,
                content_style(theme, surface, interaction, Some(role), theme.style(role)),
            );
        }
    }
    Some(row)
}

fn valid_marker_track(rect: Rect, glyph: &str) -> bool {
    let width = display_width(&sanitize_terminal_text(glyph));
    width > 0 && width <= usize::from(rect.width)
}

fn paint_marker_track(
    frame: &mut Frame<'_>,
    rect: Rect,
    glyph: &str,
    active: bool,
    surface: ControlSurface,
    state: InteractionState,
    theme: &Theme,
) {
    let glyph = sanitize_terminal_text(glyph);
    let interaction = state.interaction_role();
    for offset in 0..rect.height {
        let marker = if offset == 0 && active {
            pad_to_width(&glyph, usize::from(rect.width))
        } else {
            " ".repeat(usize::from(rect.width))
        };
        let role = if active {
            Role::AccentSelection
        } else if state.availability == Availability::Disabled {
            Role::TextDisabled
        } else {
            Role::TextSecondary
        };
        frame.buffer_mut().set_string(
            rect.x,
            rect.y + offset,
            marker,
            content_style(theme, surface, interaction, Some(role), theme.style(role)),
        );
    }
}

fn apply_content_band(
    frame: &mut Frame<'_>,
    rect: Rect,
    surface: ControlSurface,
    interaction: Option<Role>,
    role: Option<Role>,
    replacement: Option<Style>,
    theme: &Theme,
) {
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            let content = replacement.unwrap_or_else(|| cell.style());
            cell.set_style(content_style(theme, surface, interaction, role, content));
        }
    }
}

fn disabled_content_role(role: Option<Role>, availability: Availability) -> Option<Role> {
    if availability != Availability::Disabled {
        return role;
    }
    match role {
        Some(
            Role::TextPrimary
            | Role::TextSecondary
            | Role::TextInverse
            | Role::AccentAction
            | Role::AccentDestructive
            | Role::HintKey
            | Role::HintLabel,
        ) => Some(Role::TextDisabled),
        _ => role,
    }
}
