use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
};

use crate::{
    client_view::{ClientView, Overlay},
    localization::{Locale, ShellText},
    tui_layout::{SidebarArea, SidebarLayer, centered_overlay, project_shell_areas},
};

mod terminal_cells;

const BACKGROUND: Color = Color::Rgb(11, 14, 20);
const BORDER: Color = Color::Rgb(52, 61, 78);
const EMPHASIS: Color = Color::Rgb(122, 162, 247);
const MUTED: Color = Color::Rgb(133, 146, 171);
const SIDEBAR_BACKGROUND: Color = Color::Rgb(18, 23, 32);
const TEXT: Color = Color::Rgb(211, 218, 232);
const WARNING: Color = Color::Rgb(232, 179, 95);

#[cfg(test)]
pub(crate) fn render_shell(frame: &mut Frame<'_>, view: &ClientView, locale: Locale) {
    render_terminal_shell(frame, view, locale, None, None);
}

pub(crate) fn render_terminal_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    projection: Option<&crate::terminal_projection::TerminalProjection>,
    status: Option<crate::terminal_session::TerminalStatus>,
) {
    let area = frame.area();
    let mut text = locale.shell();
    if let Some(status) = status {
        text.backend_status = locale.terminal_status(status);
        text.tabs_unavailable = text.workbench_title;
    }
    frame.render_widget(Block::new().style(Style::new().bg(BACKGROUND)), area);

    let areas = project_shell_areas(area, view.sidebar_surface());
    render_workbench(frame, areas.tabs, areas.panel, text);
    if let Some(status) = status {
        let area = crate::tui_layout::terminal_area(areas.panel);
        frame.render_widget(Clear, area);
        if let Some(terminal) =
            projection.and_then(crate::terminal_projection::TerminalProjection::frame)
        {
            terminal_cells::render(frame, area, terminal, view.overlay().is_none());
        } else {
            frame.render_widget(
                Paragraph::new(locale.terminal_status(status))
                    .style(Style::new().fg(MUTED))
                    .wrap(Wrap { trim: true }),
                area,
            );
        }
    }
    render_status(frame, areas.status, view, locale, text, status);

    if let Some(sidebar) = areas.sidebar {
        render_sidebar(frame, sidebar, text);
    }

    match view.overlay() {
        Some(Overlay::ShortcutHelp) => render_shortcut_help(frame, area, text),
        Some(Overlay::WorkbenchNavigator) => render_workbench_navigator(frame, area, text),
        Some(Overlay::BusinessSidebar) | None => {}
    }
}

fn render_workbench(frame: &mut Frame<'_>, tabs: Rect, panel: Rect, text: ShellText) {
    let tab_line = Line::from(vec![
        Span::styled(
            format!(" {} ", text.tabs_unavailable),
            Style::new().fg(MUTED),
        ),
        Span::styled(
            "  !  ",
            Style::new().fg(WARNING).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(tab_line).block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(BORDER))
                .border_type(BorderType::Rounded)
                .title(Span::styled(
                    format!(" {} ", text.workbench_title),
                    Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD),
                )),
        ),
        tabs,
    );

    let content = vec![
        Line::from(""),
        Line::styled(
            text.terminal_unavailable,
            Style::new().fg(WARNING).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::styled(text.shell_only_detail, Style::new().fg(TEXT)),
        Line::styled(text.no_client_session, Style::new().fg(MUTED)),
    ];
    frame.render_widget(
        Paragraph::new(content)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_style(Style::new().fg(BORDER))
                    .border_type(BorderType::Rounded)
                    .padding(Padding::vertical(1))
                    .title(Span::styled(
                        format!(" {} ", text.panel_title),
                        Style::new().fg(TEXT),
                    )),
            ),
        panel,
    );
}

fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    text: ShellText,
    status: Option<crate::terminal_session::TerminalStatus>,
) {
    let [notice, prefix] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(24)]).areas(area);
    let status_block = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(BORDER));
    frame.render_widget(
        Paragraph::new(if view.notice().is_none() {
            status.map_or_else(
                || locale.shell_notice(None),
                |status| locale.terminal_status(status),
            )
        } else {
            locale.shell_notice(view.notice())
        })
        .style(Style::new().fg(MUTED))
        .block(status_block.clone()),
        notice,
    );
    let (label, style) = if view.prefix_active() {
        (
            text.prefix_active,
            Style::new()
                .fg(BACKGROUND)
                .bg(EMPHASIS)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (text.prefix_idle, Style::new().fg(MUTED))
    };
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(style)
            .block(status_block),
        prefix,
    );
}

fn render_sidebar(frame: &mut Frame<'_>, sidebar: SidebarArea, text: ShellText) {
    if sidebar.layer == SidebarLayer::Overlay {
        frame.render_widget(Clear, sidebar.area);
    }

    let items = [
        ListItem::new(Line::styled(
            text.site_section,
            Style::new().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            format!("● {}", text.backend_status),
            Style::new().fg(WARNING),
        )),
        ListItem::new(""),
        ListItem::new(Line::styled(
            text.business_section,
            Style::new().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.standalone),
            Style::new().fg(TEXT),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.projects),
            Style::new().fg(MUTED),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.workspaces),
            Style::new().fg(MUTED),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.agents),
            Style::new().fg(MUTED),
        )),
        ListItem::new(""),
        ListItem::new(Line::styled(text.sidebar_hint, Style::new().fg(MUTED))),
    ];
    frame.render_widget(
        List::new(items).block(
            Block::new()
                .borders(Borders::RIGHT)
                .border_style(Style::new().fg(BORDER))
                .padding(Padding::new(2, 1, 1, 0))
                .style(Style::new().bg(SIDEBAR_BACKGROUND))
                .title(Span::styled(
                    format!(" {} ", text.app_title),
                    Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD),
                )),
        ),
        sidebar.area,
    );
}

fn render_shortcut_help(frame: &mut Frame<'_>, viewport: Rect, text: ShellText) {
    let area = centered_overlay(viewport, 76, 16);
    frame.render_widget(Clear, area);
    let mut items = text
        .shortcuts
        .iter()
        .map(|shortcut| ListItem::new(Line::styled(*shortcut, Style::new().fg(TEXT))))
        .collect::<Vec<_>>();
    items.push(ListItem::new(""));
    items.push(ListItem::new(Line::styled(
        text.overlay_input_hint,
        Style::new().fg(MUTED),
    )));
    frame.render_widget(
        List::new(items).block(overlay_block(text.shortcut_title)),
        area,
    );
}

fn render_workbench_navigator(frame: &mut Frame<'_>, viewport: Rect, text: ShellText) {
    let area = centered_overlay(viewport, 68, 9);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(text.backend_status, Style::new().fg(WARNING)),
            Line::from(""),
            Line::styled(text.navigator_detail, Style::new().fg(TEXT)),
            Line::from(""),
            Line::styled(text.overlay_input_hint, Style::new().fg(MUTED)),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(overlay_block(text.navigator_title)),
        area,
    );
}

fn overlay_block(title: &'static str) -> Block<'static> {
    Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(EMPHASIS))
        .style(Style::new().bg(SIDEBAR_BACKGROUND))
        .padding(Padding::horizontal(2))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD),
        ))
}

#[cfg(test)]
mod tests;
