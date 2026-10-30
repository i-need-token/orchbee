//! Shared geometry, content hierarchy and painting for native modals; feature owners keep effects.

use crate::{
    client_view::{Binding, KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    terminal_text::{display_width, line_display_width, truncate_display, wrap_text},
    tui_layout::centered_overlay,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer},
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Padding, Widget},
};
use std::borrow::Cow;

/// Section headings inside a modal share the frame heading's hierarchy.
pub(super) fn title_line<'a>(text: impl Into<Cow<'a, str>>, theme: &Theme) -> Line<'a> {
    Line::from(Span::styled(
        text,
        theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
    ))
}

/// Clears and paints the complete modal surface: the neutral frame, its opaque fill, and the task
/// heading on the top-left edge. Feature content paints only inside the returned layout's slots.
pub(super) fn paint_modal_frame(frame: &mut Frame<'_>, area: Rect, title: &str, theme: &Theme) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        titled_overlay(theme, Padding::horizontal(2), title, area.width),
        area,
    );
}

/// Inner width that keeps a modal heading complete beside its frame-borne close control.
fn title_inner_width(title: &str) -> usize {
    // The outer frame adds two border and four padding cells around the inner width.
    display_width(title)
        .saturating_add(OVERLAY_TITLE_RESERVE)
        .saturating_sub(6)
}

/// Supporting context and field help remain subordinate to task content.
pub(super) fn description_line<'a>(text: impl Into<Cow<'a, str>>, theme: &Theme) -> Line<'a> {
    Line::styled(text, theme.style(Role::TextSecondary))
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

/// Complete geometry admitted before a modal mutates the frame or hit map.
///
/// The action row and close control are part of the surface contract: a
/// clipped control makes the whole surface unavailable rather than leaving a
/// partially painted, partially actionable dialog behind.
#[derive(Clone, Copy)]
pub(super) struct ModalLayout {
    pub(super) area: Rect,
    pub(super) inner: Rect,
    pub(super) close: Rect,
    pub(super) hints: Rect,
    pub(super) actions: [Rect; 2],
}

#[derive(Clone, Copy)]
pub(super) struct SingleModalLayout {
    pub(super) area: Rect,
    pub(super) inner: Rect,
    pub(super) close: Rect,
    pub(super) hints: Rect,
    pub(super) action: Rect,
}

#[derive(Clone, Copy)]
pub(super) struct ModalAction<'a> {
    label: &'a str,
    kind: super::controls::ActionKind,
    enabled: bool,
    shortcut: Option<&'a str>,
    tone: crate::action_projection::ActionTone,
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
        }
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
                FocusState::Unfocused,
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
    hits.record_modal_dismissal(target.clone());
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

pub(super) fn modal_layout(
    bounds: Rect,
    content_area: Rect,
    title: &str,
    text: &Text<'_>,
    hints: &[super::controls::KeyHint<'_>],
    actions: [ModalAction<'_>; 2],
    theme: &Theme,
) -> Option<ModalLayout> {
    let first_hint_width = if hints.is_empty() {
        0
    } else {
        super::controls::measure_overlay_hints(hints, theme)?;
        super::controls::measure_overlay_hints(&hints[..1], theme)?
    };
    let action_widths = actions.map(|action| {
        super::controls::action_rect(
            Rect::new(0, 0, u16::MAX, 1),
            Rect::new(0, 0, u16::MAX, 1),
            action.spec(super::controls::PointerPresentation::Rest),
            theme,
        )
        .map(|rect| rect.width)
    });
    let [Some(first_action_width), Some(second_action_width)] = action_widths else {
        return None;
    };
    let controls_width = usize::from(
        first_action_width
            .checked_add(2)?
            .checked_add(second_action_width)?,
    )
    .max(first_hint_width)
    .max(title_inner_width(title));
    let area = modal_area_with_minimum_width(content_area, text, controls_width);
    if area.is_empty() || bounds.intersection(area) != area {
        return None;
    }
    let inner = super::overlay_surface(theme, Padding::horizontal(2)).inner(area);
    if !wrapped_text_fits(text, inner) {
        return None;
    }
    let hint_row = super::controls::text_row(text, inner, text.lines.len().checked_sub(2)?);
    let row = super::controls::text_row(text, inner, text.lines.len().checked_sub(1)?);
    if hint_row.height != 1 || row.height != 1 || first_hint_width > usize::from(hint_row.width) {
        return None;
    }
    let first = super::controls::action_rect(
        bounds,
        row,
        actions[0].spec(super::controls::PointerPresentation::Rest),
        theme,
    )?;
    let second_x = first.right().checked_add(2)?;
    let second_area = Rect::new(
        second_x,
        row.y,
        row.right().checked_sub(second_x)?,
        row.height,
    );
    let second = super::controls::action_rect(
        bounds,
        second_area,
        actions[1].spec(super::controls::PointerPresentation::Rest),
        theme,
    )?;
    let action_rects = [first, second];
    if action_rects.iter().any(|action| {
        bounds.intersection(*action) != *action || inner.intersection(*action) != *action
    }) {
        return None;
    }
    let close = super::controls::frame_close_rect(bounds, area)?;
    Some(ModalLayout {
        area,
        inner,
        close,
        hints: hint_row,
        actions: action_rects,
    })
}

pub(super) fn single_modal_layout(
    bounds: Rect,
    content_area: Rect,
    title: &str,
    text: &Text<'_>,
    hints: &[super::controls::KeyHint<'_>],
    action: ModalAction<'_>,
    theme: &Theme,
) -> Option<SingleModalLayout> {
    let first_hint_width = if hints.is_empty() {
        0
    } else {
        super::controls::measure_overlay_hints(hints, theme)?;
        super::controls::measure_overlay_hints(&hints[..1], theme)?
    };
    let action_width = super::controls::action_rect(
        Rect::new(0, 0, u16::MAX, 1),
        Rect::new(0, 0, u16::MAX, 1),
        action.spec(super::controls::PointerPresentation::Rest),
        theme,
    )?
    .width;
    let controls_width = first_hint_width
        .max(usize::from(action_width))
        .max(title_inner_width(title));
    let area = modal_area_with_minimum_width(content_area, text, controls_width);
    if area.is_empty() || bounds.intersection(area) != area {
        return None;
    }
    let inner = super::overlay_surface(theme, Padding::horizontal(2)).inner(area);
    if !wrapped_text_fits(text, inner) {
        return None;
    }
    let hint_row = super::controls::text_row(text, inner, text.lines.len().checked_sub(2)?);
    let row = super::controls::text_row(text, inner, text.lines.len().checked_sub(1)?);
    if hint_row.height != 1 || row.height != 1 || first_hint_width > usize::from(hint_row.width) {
        return None;
    }
    let action = super::controls::action_rect(
        bounds,
        row,
        action.spec(super::controls::PointerPresentation::Rest),
        theme,
    )?;
    if bounds.intersection(action) != action || inner.intersection(action) != action {
        return None;
    }
    let close = super::controls::frame_close_rect(bounds, area)?;
    Some(SingleModalLayout {
        area,
        inner,
        close,
        hints: hint_row,
        action,
    })
}

fn modal_area_with_minimum_width(
    content_area: Rect,
    text: &Text<'_>,
    minimum_inner_width: usize,
) -> Rect {
    // Prose and optional help must not stretch a simple decision across the host.
    // Complete required controls may exceed the preferred reading width; admission
    // below still refuses them if the available width cannot contain them.
    let natural_width = text
        .iter()
        .map(line_display_width)
        .max()
        .unwrap_or(0)
        .clamp(48, 72)
        .max(minimum_inner_width);
    let outer_width = u16::try_from(natural_width.saturating_add(6)).unwrap_or(u16::MAX);
    let width = outer_width.min(content_area.width.saturating_sub(4));
    let inner_width = width.saturating_sub(6).max(1);
    let wrapped_height = wrap_text(text, inner_width).lines.len().saturating_add(2);
    let height = u16::try_from(wrapped_height)
        .unwrap_or(u16::MAX)
        .min(content_area.height.saturating_sub(2));
    centered_overlay(content_area, width, height)
}

pub(super) fn wrapped_text_fits(text: &Text<'_>, area: Rect) -> bool {
    if area.is_empty() {
        return false;
    }
    // A grapheme wider than the line is clipped when painted; the row count alone cannot detect that.
    let graphemes_fit = text.iter().all(|line| {
        line.spans.iter().all(|span| {
            span.styled_graphemes(Style::default())
                .all(|grapheme| display_width(grapheme.symbol) <= usize::from(area.width))
        })
    });
    graphemes_fit && wrap_text(text, area.width).lines.len() <= usize::from(area.height)
}

/// Places the terminal cursor inside a presented modal: `column` cells into
/// the wrapped row that `line_index` begins on. The caller attests the column
/// is already inside the inner width — a field window's cursor column is
/// always strictly below the field width it was projected for.
pub(super) fn set_modal_cursor(
    frame: &mut Frame<'_>,
    text: &Text<'_>,
    inner: Rect,
    line_index: usize,
    column: usize,
) {
    let row = wrap_text(&Text::from(text.lines[..line_index].to_vec()), inner.width)
        .lines
        .len();
    frame.set_cursor_position((
        inner.x + column as u16,
        inner.y + u16::try_from(row).unwrap_or(u16::MAX),
    ));
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
        self.surface.render(area, buf);
        let area = area.intersection(buf.area);
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

#[cfg(test)]
mod tests;
