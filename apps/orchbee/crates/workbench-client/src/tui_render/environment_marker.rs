//! The environment marker on the sidebar's Settings row: in a development or test build, the
//! environment's word and, when it fits whole, the checkout name, right-aligned two cells before
//! Help. It is the row's lowest priority: the checkout name drops first, then the whole marker,
//! before Settings or Help gives up a cell. Release paints nothing.
//!
//! The marker is not a target. With a checkout name it offers a tooltip that names the build and
//! the checkout in full, from the sidebar surface beneath it.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::sidebar_chrome::{Context, TRAILING_GAP};
use crate::{
    terminal_text::{display_width, sanitize_terminal_text},
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget, TooltipContent, TooltipText},
    tui_theme::Role,
};

/// A marker laid out on its row, ready to paint.
pub(super) struct Marker {
    pub(super) rect: Rect,
    line: Line<'static>,
    tooltip: Option<String>,
}

/// Lays out the marker on row `y` between the columns `start` and `end`, the first two cells after
/// Settings and the last two before Help; `None` in release or when the word does not fit.
pub(super) fn layout(context: &Context<'_>, y: u16, start: u16, end: u16) -> Option<Marker> {
    let marker = context.view.environment_marker()?;
    let theme = context.theme;
    let available = usize::from(end.checked_sub(start)?);
    let word = context.locale.environment_label(marker.environment);
    let mut width = display_width(word);
    if width == 0 || width > available {
        return None;
    }
    let mut spans = vec![Span::styled(word, theme.style(Role::TextSecondary))];
    let checkout = marker
        .checkout
        .map(|name| sanitize_terminal_text(name).into_owned())
        .filter(|name| !name.is_empty());
    if let Some(name) = checkout.as_deref() {
        let separator = format!(" {} ", theme.glyphs.middle_dot);
        let whole = width + display_width(&separator) + display_width(name);
        if whole <= available {
            let muted = theme.style(Role::TextMuted);
            spans.push(Span::styled(separator, muted));
            spans.push(Span::styled(name.to_owned(), muted));
            width = whole;
        }
    }
    let width = u16::try_from(width).ok()?;
    Some(Marker {
        rect: Rect::new(end - width, y, width, 1),
        line: Line::from(spans),
        tooltip: checkout.map(|name| {
            context
                .locale
                .environment_checkout(marker.environment, &name)
        }),
    })
}

/// The columns a marker may take on a row whose Settings ends at `settings_right` and whose Help
/// brackets start at `help_x`, each kept [`TRAILING_GAP`] cells away.
pub(super) const fn bounds(settings_right: u16, help_x: u16) -> (u16, u16) {
    (
        settings_right.saturating_add(TRAILING_GAP),
        help_x.saturating_sub(TRAILING_GAP),
    )
}

/// Paints `marker` over the sidebar `surface` and offers its tooltip there.
pub(super) fn paint(
    frame: &mut Frame<'_>,
    marker: Marker,
    surface: Rect,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) {
    frame.render_widget(Paragraph::new(marker.line), marker.rect);
    let (Some(hits), Some(text)) = (hits, marker.tooltip) else {
        return;
    };
    // The docked sidebar's surface lies under chrome; a presented overlay sidebar records its own.
    let layer = if context.layer == HitLayer::Overlay {
        HitLayer::OverlaySurface
    } else {
        HitLayer::ChromeSurface
    };
    hits.record_target_part_tooltip(
        layer,
        marker.rect,
        surface,
        HitTarget::OverlaySurface,
        TooltipContent::Describe {
            text: Some(TooltipText {
                text,
                middle: false,
            }),
            legend: Vec::new(),
        },
    );
}
