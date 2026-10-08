//! Paints the existing merged Agent projection; never reads or acknowledges terminals.
use super::sidebar_text::machine_label;
use super::*;
use crate::machine_discovery::display_label;
use crate::tui_layout::{DrawnSidebarWidth, SidebarWidthDrag};
use crate::{client_view::Overlay, machine_picker::Row};
mod tokens;

fn agent_range(
    rows: &[Row],
    window: crate::machine_picker::SidebarWindow,
    incomplete: bool,
) -> String {
    let total = rows
        .iter()
        .filter(|row| row.sidebar_agent().is_some())
        .count();
    if total == 0 {
        return String::new();
    }
    let before = rows
        .iter()
        .take(window.top)
        .filter(|row| row.sidebar_agent().is_some())
        .count();
    let visible = rows
        .iter()
        .skip(window.top)
        .take(window.capacity)
        .filter(|row| row.sidebar_agent().is_some())
        .count();
    let suffix = if incomplete { "+" } else { "" };
    if visible == 0 {
        format!("0/{total}{suffix}")
    } else {
        format!("{}-{}/{total}{suffix}", before + 1, before + visible)
    }
}

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
        || truncate_display(label, usize::from(area.width), theme.glyphs.ellipsis),
        |binding| {
            format!(
                "{label}{}{binding}",
                " ".repeat(usize::from(area.width) - display_width(label) - display_width(binding))
            )
        },
    );
    frame.render_widget(Paragraph::new(line).style(theme.style(Role::HintKey)), area);
}

/// Fit short sections to their content, giving the longer list the remaining
/// cells. Tiny sidebars keep the existing heading-only degradation.
fn section_heights(
    available: u16,
    workspaces: u16,
    agents: u16,
    workspace_minimum: u16,
    agent_minimum: u16,
) -> (u16, u16, u16) {
    if workspaces == 0 {
        return (0, agents.min(available), 0);
    }
    if available < 6 {
        let workspace_height = u16::from(available >= 2);
        return (
            workspace_height,
            agents.min(available.saturating_sub(workspace_height)),
            0,
        );
    }
    let minimum = u32::from(workspace_minimum) + u32::from(agent_minimum);
    let gap = u16::from(u32::from(available) > minimum);
    let usable = available - gap;
    let workspace_height = workspaces.min(if minimum <= u32::from(usable) {
        (usable / 2).clamp(workspace_minimum, usable - agent_minimum)
    } else {
        (usable / 2).max(3)
    });
    let agent_height = agents.min(usable - workspace_height);
    (workspaces.min(usable - agent_height), agent_height, gap)
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
        .padding(Padding::new(
            2,
            1,
            u16::from(sidebar.layer == SidebarLayer::Overlay),
            0,
        ))
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
    let labels = [
        text.site_section.to_owned(),
        backend_status.text.to_owned(),
        String::new(),
        text.business_section.to_owned(),
        format!("  {}", text.standalone),
        format!("  {}", text.projects),
        format!("  {}", text.workspaces),
    ];
    let prefix = if view.machines.sidebar_navigation_rows().is_empty() {
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
    let rows = view.machines.sidebar_rows();
    let count = rows
        .iter()
        .filter(|row| row.sidebar_agent().is_some())
        .count();
    let incomplete = rows.is_empty()
        || rows.iter().any(|row| {
            matches!(
                row.sidebar_status(),
                Some("agents-loading" | "agents-unavailable")
            )
        });
    let context_rows = u16::from(
        count > 0
            || rows
                .iter()
                .any(|row| row.sidebar_status() == Some("agents-no-matches")),
    );
    let chrome = 2 + context_rows;
    let heights: Vec<_> = rows
        .iter()
        .map(|row| tokens::height(view, row, locale))
        .collect();
    let gap = view.machines.sidebar_settings().row_gap();
    let workspace_height = if view.machines.sidebar_navigation_rows().is_empty() {
        0
    } else {
        super::workspace_sidebar::preferred_height(view, locale, area.width, theme)
    };
    let agent_minimum = rows
        .iter()
        .zip(&heights)
        .find(|(row, _)| {
            row.sidebar_agent()
                .is_some_and(|agent| view.machines.current_agent(&row.choice, agent))
        })
        .map(|(_, height)| *height)
        .or_else(|| heights.first().copied())
        .unwrap_or(1)
        .saturating_add(chrome);
    let (workspace_height, agent_height, separator) = section_heights(
        area.height,
        workspace_height,
        super::sidebar_text::list_height(heights.iter().copied(), gap, chrome),
        super::workspace_sidebar::minimum_height(view, locale, area.width, theme),
        agent_minimum,
    );
    if workspace_height > 0 {
        super::workspace_sidebar::render(
            frame,
            Rect::new(area.x, area.y, area.width, workspace_height),
            view,
            locale,
            theme,
            layer,
            hits.as_deref_mut(),
        );
        area.y += workspace_height + separator;
    }
    area.height = agent_height;
    if area.is_empty() {
        return;
    }
    let title = if count == 0 && incomplete {
        text.agents.to_owned()
    } else {
        format!(
            "{} {count}{}",
            text.agents,
            if incomplete { "+" } else { "" }
        )
    };
    let header = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(fit(&title))
            .style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD)),
        header,
    );
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, header, HitTarget::OpenMachineAgents);
    }
    if area.height < 3 {
        return;
    }
    if context_rows > 0 {
        let context = Rect::new(area.x, area.y + 1, area.width, 1);
        frame.render_widget(
            Paragraph::new(fit(&view.machines.sidebar_view_label(locale)))
                .style(theme.style(Role::HintLabel)),
            context,
        );
        if let Some(hits) = hits.as_deref_mut() {
            hits.record(layer, context, HitTarget::OpenMachineAgents);
        }
    }
    let body = Rect::new(
        area.x,
        area.y + 1 + context_rows,
        area.width,
        area.height.saturating_sub(chrome),
    );
    let window = view
        .machines
        .sidebar_window_for_heights(&heights, body.height, gap);
    let mut y = body.y;
    for (index, row) in rows
        .iter()
        .enumerate()
        .skip(window.top)
        .take(window.capacity)
    {
        let rect = Rect::new(body.x, y, body.width, heights[index].min(body.height));
        let lines = tokens::lines(view, row, index, locale, theme);
        frame.render_widget(
            Paragraph::new(
                lines
                    .into_iter()
                    .map(|line| tokens::fit(line, usize::from(body.width), theme.glyphs.ellipsis))
                    .collect::<Vec<_>>(),
            )
            .style(theme.style(Role::TextPrimary)),
            rect,
        );
        if let Some(hits) = hits.as_deref_mut() {
            let target = if row.enabled && row.sidebar_agent().is_some() {
                HitTarget::MachineAgentRow {
                    selected: row.selection(),
                    window,
                }
            } else {
                HitTarget::MachineAgentSurface(window)
            };
            hits.record(layer, rect, target);
        }
        y = rect.bottom();
        if index + 1 < window.end() {
            let blank = Rect::new(
                body.x,
                y,
                body.width,
                gap.min(body.bottom().saturating_sub(y)),
            );
            if let Some(hits) = hits.as_deref_mut() {
                hits.record(layer, blank, HitTarget::MachineAgentSurface(window));
            }
            y = blank.bottom();
        }
    }
    let used = y.saturating_sub(body.y);
    let rest = Rect::new(
        body.x,
        body.y + used,
        body.width,
        body.height.saturating_sub(used),
    );
    if rows.is_empty() && !rest.is_empty() {
        frame.render_widget(
            Paragraph::new(fit(locale.machine_picker_text("agents-loading")))
                .style(theme.style(Role::TextDisabled)),
            rest,
        );
    }
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, rest, HitTarget::MachineAgentSurface(window));
    }
    let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    let range = agent_range(rows, window, incomplete);
    let range_width = display_width(&range).min(usize::from(footer.width)) as u16;
    let label = Rect::new(
        footer.x,
        footer.y,
        footer.width.saturating_sub(range_width + 1),
        1,
    );
    frame.render_widget(
        Paragraph::new(truncate_display(
            locale.machine_picker_text("agent-sidebar-open"),
            usize::from(label.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::HintKey)),
        label,
    );
    frame.render_widget(
        Paragraph::new(range)
            .alignment(Alignment::Right)
            .style(theme.style(Role::TextSecondary)),
        Rect::new(footer.right() - range_width, footer.y, range_width, 1),
    );
    if let Some(hits) = hits {
        hits.record(layer, footer, HitTarget::OpenMachineAgents);
    }
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
    frame.render_widget(Clear, right);
    frame.render_widget(
        Paragraph::new(hint).style(theme.style(Role::HintLabel)),
        right,
    );
    true
}

#[cfg(test)]
mod tests;
