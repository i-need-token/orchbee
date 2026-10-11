//! Paints the existing merged Agent projection; never reads or acknowledges terminals.
use super::sidebar_text::machine_label;
use super::{
    Block, Borders, Clear, ClientView, Frame, HitLayer, HitMapBuilder, HitTarget, Line, List,
    ListItem, Locale, Modifier, Padding, Paragraph, Rect, Role, SidebarArea, SidebarLayer,
    SiteStatus, Span, Theme, display_width, truncate_display,
};
use crate::client_view::Overlay;
use crate::machine_discovery::display_label;
use crate::tui_layout::{DrawnSidebarWidth, SidebarWidthDrag};
use ratatui::style::Style;
pub(super) mod tokens;

fn render_actions(
    frame: &mut Frame<'_>,
    area: Rect,
    locale: Locale,
    theme: &Theme,
    layer: HitLayer,
    mut hits: Option<&mut HitMapBuilder>,
) {
    use crate::machine_picker::SidebarCommand;
    for (offset, (icon, command, target)) in [
        (
            "+",
            SidebarCommand::NewWorkspace,
            HitTarget::NewWorkspaceButton,
        ),
        ("/", SidebarCommand::Search, HitTarget::SearchButton),
    ]
    .into_iter()
    .enumerate()
    {
        if offset >= usize::from(area.height) {
            break;
        }
        let row = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        frame.render_widget(
            Paragraph::new(truncate_display(
                &format!("{icon} {}", locale.sidebar_navigation_action(command)),
                usize::from(row.width),
                theme.glyphs.ellipsis,
            ))
            .style(theme.style(Role::TextPrimary)),
            row,
        );
        if let Some(hits) = hits.as_deref_mut() {
            hits.record(layer, row, target);
        }
    }
}

fn render_settings_entry(frame: &mut Frame<'_>, area: Rect, locale: Locale, theme: &Theme) {
    let line = Line::styled(
        truncate_display(
            locale.settings_title(),
            usize::from(area.width),
            theme.glyphs.ellipsis,
        ),
        theme.style(Role::TextPrimary),
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
    let mut inner = block.inner(sidebar.area);
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
    render_actions(frame, inner, locale, theme, layer, hits.as_deref_mut());
    let action_rows = inner
        .height
        .min(if frame.area().height >= 24 { 3 } else { 2 });
    inner.y += action_rows;
    inner.height -= action_rows;
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
    let settings = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    render_settings_entry(frame, settings, locale, theme);
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, settings, HitTarget::SettingsButton);
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
            Style::new().fg(theme.color(Role::AccentFocus)),
        );
    }
    true
}

#[cfg(test)]
mod tests;
