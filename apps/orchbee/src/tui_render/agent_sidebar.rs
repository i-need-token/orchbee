//! Paints the existing merged Agent projection; never reads or acknowledges terminals.
use super::sidebar_text::machine_label;
use super::*;
use crate::machine_discovery::display_label;
use crate::{client_view::Overlay, machine_picker::Row};
mod tokens;

pub(super) fn render(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    view: &ClientView,
    locale: Locale,
    backend_status: &str,
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
        .padding(Padding::new(2, 1, 1, 0))
        .style(theme.style(Role::SurfaceChrome))
        .title(Span::styled(
            format!(" {} ", text.app_title),
            theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(sidebar.area);
    frame.render_widget(block, sidebar.area);
    let layer = if view.overlay() == Some(Overlay::BusinessSidebar) {
        HitLayer::Overlay
    } else {
        HitLayer::Chrome
    };
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
        format!("{} {backend_status}", theme.glyphs.status_bullet),
        String::new(),
        text.business_section.to_owned(),
        format!("  {}", text.standalone),
        format!("  {}", text.projects),
        format!("  {}", text.workspaces),
    ];
    let prefix = if inner.height >= 16 && view.machines.space_rows().is_empty() {
        7
    } else {
        2
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
                    ListItem::new(Line::styled(
                        fit(label),
                        theme.style(if index == 1 {
                            Role::StatusWarning
                        } else {
                            Role::TextSecondary
                        }),
                    ))
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
    if !view.machines.space_rows().is_empty() {
        let height = if area.height >= 6 {
            (area.height / 2).max(3)
        } else if area.height >= 2 {
            1
        } else {
            0
        };
        if height > 0 {
            super::workspace_sidebar::render(
                frame,
                Rect::new(area.x, area.y, area.width, height),
                view,
                locale,
                theme,
                layer,
                hits.as_deref_mut(),
            );
            area.y += height;
            area.height -= height;
        }
    }
    if area.is_empty() {
        return;
    }
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
    let context = Rect::new(area.x, area.y + 1, area.width, 1);
    frame.render_widget(
        Paragraph::new(fit(&view.machines.sidebar_view_label(locale)))
            .style(theme.style(Role::HintLabel)),
        context,
    );
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, context, HitTarget::OpenMachineAgents);
    }
    let body = Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(3),
    );
    let heights: Vec<_> = rows
        .iter()
        .map(|row| tokens::height(view, row, locale))
        .collect();
    let gap = view.machines.sidebar_settings().row_gap();
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
    let range = format!(
        "{}-{}/{}",
        if window.end() > window.top {
            window.top + 1
        } else {
            0
        },
        window.end(),
        window.total
    );
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

#[cfg(test)]
mod tests;
