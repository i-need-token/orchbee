//! Shared geometry and painting for recurring native modal chrome.

use crate::{
    client_view::{Binding, KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    tui_layout::centered_overlay,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer},
    tui_theme::{GlyphMode, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Padding, Paragraph, Wrap},
};

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
    let painted = super::controls::render_icon_action(
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
        #[cfg(test)]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    format_binding_with_glyphs(binding, LabelStyle::for_host(), glyphs)
}

pub(super) fn modal_layout(
    bounds: Rect,
    content_area: Rect,
    text: &Text<'_>,
    hints: &[super::controls::KeyHint<'_>],
    actions: [ModalAction<'_>; 2],
    theme: &Theme,
) -> Option<ModalLayout> {
    let (hint_width, first_hint_width) = if hints.is_empty() {
        (0, 0)
    } else {
        (
            super::controls::measure_key_hints(hints, theme)?,
            super::controls::measure_key_hints(&hints[..1], theme)?,
        )
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
    .max(hint_width);
    let area = modal_area_with_minimum_width(content_area, text, controls_width);
    if area.is_empty() || bounds.intersection(area) != area {
        return None;
    }
    let inner = super::overlay_surface(theme, true, Padding::horizontal(2)).inner(area);
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
    let close = super::controls::close_rect(bounds, area)?;
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
    text: &Text<'_>,
    hints: &[super::controls::KeyHint<'_>],
    action: ModalAction<'_>,
    theme: &Theme,
) -> Option<SingleModalLayout> {
    let hint_width = super::controls::measure_key_hints(hints, theme)?;
    let first_hint_width = super::controls::measure_key_hints(hints.get(..1)?, theme)?;
    let action_width = super::controls::action_rect(
        Rect::new(0, 0, u16::MAX, 1),
        Rect::new(0, 0, u16::MAX, 1),
        action.spec(super::controls::PointerPresentation::Rest),
        theme,
    )?
    .width;
    let controls_width = hint_width.max(usize::from(action_width));
    let area = modal_area_with_minimum_width(content_area, text, controls_width);
    if area.is_empty() || bounds.intersection(area) != area {
        return None;
    }
    let inner = super::overlay_surface(theme, true, Padding::horizontal(2)).inner(area);
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
    let close = super::controls::close_rect(bounds, area)?;
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
    let natural_width = text.width().max(minimum_inner_width);
    let outer_width = u16::try_from(natural_width.saturating_add(6)).unwrap_or(u16::MAX);
    let width = outer_width.min(content_area.width.saturating_sub(4));
    let inner_width = width.saturating_sub(6).max(1);
    let wrapped_height = Paragraph::new(text.clone())
        .wrap(Wrap { trim: false })
        .line_count(inner_width)
        .saturating_add(2);
    let height = u16::try_from(wrapped_height)
        .unwrap_or(u16::MAX)
        .min(content_area.height.saturating_sub(2));
    centered_overlay(content_area, width, height)
}

pub(super) fn wrapped_text_fits(text: &Text<'_>, area: Rect) -> bool {
    if area.is_empty() {
        return false;
    }
    // The wrapper drops any grapheme wider than the line; line_count alone cannot detect that.
    let graphemes_fit = text.iter().all(|line| {
        line.spans.iter().all(|span| {
            span.styled_graphemes(Style::default())
                .all(|grapheme| Line::raw(grapheme.symbol).width() <= usize::from(area.width))
        })
    });
    graphemes_fit
        && Paragraph::new(text.clone())
            .wrap(Wrap { trim: false })
            .line_count(area.width)
            <= usize::from(area.height)
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
    let row = Paragraph::new(Text::from(text.lines[..line_index].to_vec()))
        .wrap(Wrap { trim: false })
        .line_count(inner.width);
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
            .all(|line| line.width() <= usize::from(area.width))
}

#[cfg(test)]
mod tests;
