//! Shared geometry, content hierarchy and painting for native modals; feature owners keep effects.

use crate::{
    client_view::{Binding, KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    terminal_text::{line_display_width, truncate_display},
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer},
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Padding, Widget},
};
use std::borrow::Cow;

/// Clears and paints the complete modal surface: the neutral frame, its opaque fill, and the task
/// heading on the top-left edge. Feature content paints only inside the returned layout's slots.
pub(super) fn paint_modal_frame(frame: &mut Frame<'_>, area: Rect, title: &str, theme: &Theme) {
    frame.render_widget(
        titled_overlay(theme, Padding::horizontal(2), title, area.width),
        area,
    );
}

/// Status meaning belongs to its glyph; the complete recovery text stays readable.
pub(super) fn status_line<'a>(
    text: impl Into<Cow<'a, str>>,
    role: Role,
    theme: &Theme,
) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{} ", theme.glyphs.status_bullet),
            theme.style(role),
        ),
        Span::styled(text, theme.style(Role::TextPrimary)),
    ])
}

#[derive(Clone, Copy)]
pub(super) struct ModalAction<'a> {
    label: &'a str,
    kind: super::controls::ActionKind,
    enabled: bool,
    shortcut: Option<&'a str>,
    tone: crate::action_projection::ActionTone,
    focused: bool,
}

impl<'a> ModalAction<'a> {
    pub(super) const fn new(
        label: &'a str,
        kind: super::controls::ActionKind,
        enabled: bool,
    ) -> Self {
        Self {
            label,
            kind,
            enabled,
            shortcut: None,
            tone: crate::action_projection::ActionTone::Ordinary,
            focused: false,
        }
    }

    /// Keyboard focus rests on this button, as on a confirmation's primary when it opens.
    pub(super) const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// The same button without its key text, for a window too narrow for the keyed row.
    pub(super) const fn bare(mut self) -> Self {
        self.shortcut = None;
        self
    }

    pub(super) const fn with_shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
    pub(super) const fn with_tone(mut self, tone: crate::action_projection::ActionTone) -> Self {
        self.tone = tone;
        self
    }

    pub(super) const fn availability(self) -> InteractionAvailability {
        if self.enabled {
            InteractionAvailability::Enabled
        } else {
            InteractionAvailability::Disabled
        }
    }

    pub(super) fn spec(
        self,
        pointer: super::controls::PointerPresentation,
    ) -> super::controls::ActionSpec<'a> {
        use super::controls::{
            Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
            NavigationState, OperationState,
        };
        let spec = super::controls::ActionSpec::new(
            self.label,
            self.kind,
            ControlSurface::Overlay,
            InteractionState::new(
                if self.focused {
                    FocusState::Focused
                } else {
                    FocusState::Unfocused
                },
                pointer,
                NavigationState::None,
                CurrentState::NotCurrent,
                CheckedState::Unchecked,
                if self.enabled {
                    Availability::Enabled
                } else {
                    Availability::Disabled
                },
                OperationState::Idle,
            ),
        )
        .with_tone(self.tone);
        self.shortcut
            .map_or(spec, |shortcut| spec.with_shortcut(shortcut))
    }
}

pub(super) fn pointer_presentation(
    pointer: ProjectedPointer,
) -> super::controls::PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => super::controls::PointerPresentation::Rest,
        ProjectedPointer::Hovered => super::controls::PointerPresentation::Hovered,
        ProjectedPointer::Pressed => super::controls::PointerPresentation::Pressed,
    }
}

/// Paints and publishes one specialized modal action from the same exact
/// geometry. Its existing owner still handles the semantic release effect;
/// the shared pointer projection contributes only hover and pressed feedback.
pub(super) fn paint_modal_action(
    frame: &mut Frame<'_>,
    area: Rect,
    rect: Rect,
    action: ModalAction<'_>,
    target: HitTarget,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Rect {
    let availability = action.availability();
    let pointer = hits.projected_modal_pointer(HitLayer::Overlay, rect, &target, availability);
    let painted = super::controls::render_action(
        frame,
        area,
        action.spec(pointer_presentation(pointer)),
        theme,
    )
    .expect("admitted modal action geometry must paint");
    assert_eq!(
        painted, rect,
        "modal action paint must match its admitted extent"
    );
    hits.record_modal_control(HitLayer::Overlay, rect, target, availability);
    rect
}

/// Paints and publishes the fixed close affordance while retaining the
/// specialized modal's existing token-aware release route.
pub(super) fn paint_modal_close(
    frame: &mut Frame<'_>,
    rect: Rect,
    target: HitTarget,
    dismiss_outside: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Rect {
    let availability = InteractionAvailability::Enabled;
    let pointer = hits.projected_modal_pointer(HitLayer::Overlay, rect, &target, availability);
    let pointer = pointer_presentation(pointer);
    let painted = super::controls::render_frame_icon_action(
        frame,
        rect,
        super::controls::IconActionSpec::new(
            super::controls::IconActionGlyph::Close,
            super::controls::ControlSurface::Overlay,
            super::controls::Availability::Enabled,
            pointer,
        ),
        theme,
    )
    .expect("admitted modal close geometry must paint");
    assert_eq!(
        painted, rect,
        "modal close paint must match its admitted extent"
    );
    if dismiss_outside {
        hits.record_modal_dismissal(target.clone());
    }
    hits.record_modal_control(HitLayer::Overlay, rect, target, availability);
    rect
}

pub(super) fn modal_key_label(theme: &Theme, binding: &Binding) -> String {
    let glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    format_binding_with_glyphs(binding, LabelStyle::for_host(), glyphs)
}

pub(super) fn unwrapped_text_fits(text: &str, area: Rect) -> bool {
    let text = Text::from(text);
    !area.is_empty()
        && text.height() <= usize::from(area.height)
        && text
            .iter()
            .all(|line| line_display_width(line) <= usize::from(area.width))
}

pub(crate) fn overlay_surface(theme: &Theme, padding: Padding) -> Block<'static> {
    Block::new()
        .borders(Borders::ALL)
        .border_type(theme.corners)
        .border_style(theme.style(Role::FrameUnfocused))
        .style(theme.style(Role::SurfaceOverlay))
        .padding(padding)
}

/// An overlay surface and the task heading on its top-left edge, painted as one widget.
///
/// `Block::title` would place the heading by ratatui's whole-string `Line::width`, which merges a
/// ligature across grapheme clusters, such as Lam-Alef, into one cell while the buffer writes one
/// cell per cluster; the heading's trailing blank then overwrites its last letter. The heading is
/// written with `Buffer::set_line` instead, which advances by the cells it actually placed: the
/// rule [`display_width`] measures.
pub(crate) struct OverlayFrame {
    surface: Block<'static>,
    heading: Option<Line<'static>>,
}

impl OverlayFrame {
    pub(crate) fn new(surface: Block<'static>) -> Self {
        Self {
            surface,
            heading: None,
        }
    }

    pub(crate) fn heading(self, heading: Line<'static>) -> Self {
        Self {
            heading: Some(heading),
            ..self
        }
    }

    pub(crate) fn inner(&self, area: Rect) -> Rect {
        self.surface.inner(area)
    }
}

impl Widget for OverlayFrame {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        super::clear_crossing_wide_buffer_cells(buf, area);
        Clear.render(area, buf);
        self.surface.render(area, buf);
        if let Some(heading) = self.heading
            && !area.is_empty()
        {
            // Where a block title goes: the top edge, between the side borders.
            buf.set_line(
                area.x.saturating_add(1),
                area.y,
                &heading,
                area.width.saturating_sub(2),
            );
        }
    }
}

/// Top-edge cells an overlay heading leaves to its frame: both corners, the leading `─ ` and
/// trailing blank, at least one rule cell, and the spaced close control ` [x] ─`.
pub(super) const OVERLAY_TITLE_RESERVE: usize = 12;

/// The task heading painted once on an overlay's top-left edge as `─ Title `.
///
/// The heading truncates from the end so the frame's close control always keeps its cells.
pub(super) fn overlay_title(title: &str, width: u16, theme: &Theme) -> Line<'static> {
    overlay_heading(
        title,
        usize::from(width).saturating_sub(OVERLAY_TITLE_RESERVE),
        theme,
    )
}

/// The same top-left heading inside an owner-computed display budget, for frames whose top edge
/// carries other controls.
pub(super) fn overlay_heading(title: &str, budget: usize, theme: &Theme) -> Line<'static> {
    let title = truncate_display(
        &crate::terminal_text::sanitize_terminal_text(title),
        budget,
        theme.glyphs.ellipsis,
    );
    Line::from(vec![
        Span::styled(
            theme.corners.to_border_set().horizontal_top,
            theme.style(Role::FrameUnfocused),
        ),
        Span::raw(" "),
        Span::styled(
            title,
            theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

/// An overlay surface headed by [`overlay_title`].
pub(crate) fn titled_overlay(
    theme: &Theme,
    padding: Padding,
    title: &str,
    width: u16,
) -> OverlayFrame {
    OverlayFrame::new(overlay_surface(theme, padding)).heading(overlay_title(title, width, theme))
}

pub(super) fn overlay_block(title: &str, width: u16, theme: &Theme) -> OverlayFrame {
    titled_overlay(theme, Padding::horizontal(2), title, width)
}

mod dialog;
pub(super) use dialog::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, FieldTone, NAME_FIELD_WIDTH,
    dialog_layout, field_value_area, paint_dialog, paint_field,
};
mod name_form;
pub(super) use name_form::{NameForm, paint_name_form};

#[cfg(test)]
mod tests;
