//! Paints the existing merged Agent projection; never reads or acknowledges terminals.
use super::sidebar_text::machine_label;
use super::*;
use crate::client_view::Overlay;
use crate::machine_discovery::display_label;
use crate::tui_layout::{DrawnSidebarWidth, SidebarWidthDrag};
pub(super) mod tokens;

fn render_menu_entry(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
) {
    let label = locale.menu_button();
    let binding = view
        .bindings()
        .labels(crate::client_view::PrefixDesc::ActionMenu);
    let binding = binding.first().filter(|binding| {
        display_width(label) + 1 + display_width(binding) <= usize::from(area.width)
    });
    let line = binding.map_or_else(
        || {
            Line::styled(
                truncate_display(label, usize::from(area.width), theme.glyphs.ellipsis),
                theme.style(Role::TextPrimary),
            )
        },
        |binding| {
            Line::from(vec![
                Span::styled(label, theme.style(Role::TextPrimary)),
                Span::raw(" ".repeat(
                    usize::from(area.width) - display_width(label) - display_width(binding),
                )),
                Span::styled(binding.clone(), theme.style(Role::HintLabel)),
            ])
        },
    );
    frame.render_widget(Paragraph::new(line), area);
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    view: &ClientView,
    locale: Locale,
    backend_status: SiteStatus<'_>,
    theme: &Theme,
    mut hits: Option<&mut HitMapBuilder>,
) {
    if sidebar.layer == SidebarLayer::Overlay {
        frame.render_widget(Clear, sidebar.area);
    }
    let text = locale.shell();
    let block = Block::new()
        .borders(Borders::RIGHT)
        .border_style(theme.style(Role::FrameDivider))
        .padding(Padding::new(0, 1, 1, 0))
        .style(theme.style(Role::SurfaceChrome));
    let inner = block.inner(sidebar.area);
    frame.render_widget(block, sidebar.area);
    let layer = if view.overlay() == Some(Overlay::BusinessSidebar) {
        HitLayer::Overlay
    } else {
        HitLayer::Chrome
    };
    if let Some(target) = DrawnSidebarWidth::new(frame.area(), view.sidebar_surface())
        && target.sidebar == sidebar
        && let Some(hits) = hits.as_deref_mut()
    {
        hits.record(layer, target.edge(), HitTarget::SidebarResize(target));
    }
    if layer == HitLayer::Chrome
        && let Some(hits) = hits.as_deref_mut()
    {
        hits.record(
            HitLayer::ChromeSurface,
            sidebar.area,
            HitTarget::OverlaySurface,
        );
    }
    // Build the exact Workspace -> Agent projection once for this frame. The same
    // borrowed rows drive text, hit geometry, focus, and the unified scroll window.
    let rows = view.machines.sidebar_navigation_rows();
    let labels = [
        text.site_section.to_owned(),
        backend_status.text.to_owned(),
        String::new(),
        text.business_section.to_owned(),
        format!("  {}", text.standalone),
        format!("  {}", text.projects),
        format!("  {}", text.workspaces),
    ];
    let prefix = if rows.is_empty() {
        if inner.height >= 16 { 7 } else { 2 }
    } else {
        0
    }
    .min(inner.height);
    let fit =
        |label: &str| truncate_display(label, usize::from(inner.width), theme.glyphs.ellipsis);
    frame.render_widget(
        List::new(
            labels
                .iter()
                .take(usize::from(prefix))
                .enumerate()
                .map(|(index, label)| {
                    if index == 1 {
                        ListItem::new(backend_status.line(usize::from(inner.width), theme))
                    } else {
                        ListItem::new(Line::styled(fit(label), theme.style(Role::TextSecondary)))
                    }
                })
                .collect::<Vec<_>>(),
        ),
        Rect::new(inner.x, inner.y, inner.width, prefix),
    );
    let mut area = Rect::new(
        inner.x,
        inner.y + prefix,
        inner.width,
        inner.height.saturating_sub(prefix),
    );
    if area.is_empty() {
        return;
    }
    let menu = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    render_menu_entry(frame, menu, view, locale, theme);
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, menu, HitTarget::ActionMenuButton);
    }
    area.height = area.height.saturating_sub(1);
    super::workspace_sidebar::render(
        frame,
        area,
        super::workspace_sidebar::RenderContext {
            view,
            locale,
            theme,
            layer,
        },
        &rows,
        hits,
    );
}

/// Reversible border feedback uses the existing shell preview/graphics-cover lifecycle.
pub(crate) fn render_width_preview(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    drag: &SidebarWidthDrag,
    theme: &Theme,
    hits: &HitMapBuilder,
) -> bool {
    if !matches!(view.overlay(), None | Some(Overlay::BusinessSidebar))
        || view.prefix_active()
        || view.resize_mode()
        || view.machines.open
        || view.pointer_released()
        || DrawnSidebarWidth::new(frame.area(), view.sidebar_surface()) != Some(drag.target)
        || !hits.exposes_sidebar_width(&drag.target)
    {
        return false;
    }
    let glyph = if theme.glyphs.ellipsis == ".." {
        "|"
    } else {
        "│"
    };
    for y in drag.preview.y..drag.preview.bottom() {
        frame.buffer_mut().set_string(
            drag.preview.x,
            y,
            glyph,
            theme
                .style(Role::AccentSelection)
                .add_modifier(Modifier::BOLD),
        );
    }
    let status = project_shell_areas(frame.area(), view.sidebar_surface()).status;
    let columns = drag
        .preview
        .right()
        .saturating_sub(drag.target.sidebar.area.x);
    let hint = locale.sidebar_width_drag_hint(columns, status.width >= 100);
    let width = u16::try_from(display_width(&hint))
        .unwrap_or(u16::MAX)
        .min(status.width);
    let right = Rect::new(status.right() - width, status.y, width, status.height);
    let separator = right.x.saturating_sub(1).max(status.x);
    let mut clear_from = status.x;
    // Include only the complete glyph under the separator, never the preceding glyph.
    while clear_from < separator {
        let symbol = frame.buffer_mut()[(clear_from, status.y)].symbol();
        let next = clear_from
            .saturating_add(u16::try_from(display_width(symbol).max(1)).unwrap_or(u16::MAX));
        if next > separator {
            break;
        }
        clear_from = next;
    }
    frame.render_widget(
        Clear,
        Rect::new(
            clear_from,
            status.y,
            status.right() - clear_from,
            status.height,
        ),
    );
    frame.render_widget(
        Paragraph::new(hint).style(theme.style(Role::HintLabel)),
        right,
    );
    true
}

#[cfg(test)]
mod tests;
