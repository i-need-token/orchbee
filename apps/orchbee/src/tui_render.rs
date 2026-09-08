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
    terminal_session::{
        NativeDisplayPhase, NativeDrawReceipt, NativeProjectionStatus, NativeRefreshStatus,
        NativeSessionView, NativeTabView, TerminalSessionView, TerminalStatus,
    },
    tui_layout::{ShellAreas, SidebarArea, SidebarLayer, centered_overlay, project_shell_areas},
};

mod tab_close;
mod tabs;
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
    render_terminal_shell(frame, view, locale, None);
}

pub(crate) fn render_terminal_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    session: Option<TerminalSessionView<'_>>,
) -> NativeDrawReceipt {
    let area = frame.area();
    let text = locale.shell();
    frame.render_widget(Block::new().style(Style::new().bg(BACKGROUND)), area);

    let areas = project_shell_areas(area, view.sidebar_surface());
    let mut status_draw = None;
    let sidebar_status = match session.as_ref() {
        None => {
            render_unavailable_workbench(
                frame,
                areas.tabs,
                areas.panel,
                text,
                text.tabs_unavailable,
            );
            render_status(
                frame,
                areas.status,
                view,
                locale,
                text,
                locale.shell_notice(None),
                None,
            );
            text.backend_status.to_owned()
        }
        Some(TerminalSessionView::Legacy { projection, status }) => {
            render_unavailable_workbench(
                frame,
                areas.tabs,
                areas.panel,
                text,
                text.workbench_title,
            );
            render_legacy_terminal(frame, areas.panel, view, locale, *projection, *status);
            let status = locale.terminal_status(*status);
            render_status(frame, areas.status, view, locale, text, status, None);
            status.to_owned()
        }
        Some(TerminalSessionView::Native(native)) => {
            let phase_status = locale.native_phase_status(native);
            render_native_workbench(frame, &areas, view, locale, text, native, &phase_status);
            let close = native.close_status();
            let status = close.as_ref().map_or_else(
                || locale.native_status(native),
                |status| locale.native_close_status(status),
            );
            let local_notice = close
                .as_ref()
                .and_then(|_| native.notice())
                .map(|notice| locale.native_notice(notice));
            let visible = render_status(
                frame,
                areas.status,
                view,
                locale,
                text,
                &status,
                local_notice.as_deref(),
            );
            status_draw = close.and_then(|status| visible.map(|area| (status.token(), area)));
            phase_status
        }
    };

    if let Some(sidebar) = areas.sidebar {
        render_sidebar(frame, sidebar, text, &sidebar_status);
    }

    let overlay = match view.overlay() {
        Some(Overlay::ShortcutHelp) => Some(render_shortcut_help(frame, area, text)),
        Some(Overlay::WorkbenchNavigator) => Some(match session.as_ref() {
            Some(TerminalSessionView::Native(native)) => {
                render_native_workbench_navigator(frame, area, text, locale, native)
            }
            None | Some(TerminalSessionView::Legacy { .. }) => {
                render_unavailable_workbench_navigator(frame, area, text)
            }
        }),
        Some(Overlay::BusinessSidebar) => areas.sidebar.map(|sidebar| sidebar.area),
        None => None,
    };
    if let Some(overlay) = overlay {
        status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(overlay));
    }
    let mut prompt = None;
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(confirmation) = native.close_confirmation()
    {
        let modal = tab_close::render(frame, locale, &confirmation);
        status_draw = status_draw.filter(|(_, area)| !area.intersects(modal.area));
        prompt = modal.prompt;
        if let Some(token) = modal.status {
            status_draw = Some((token, modal.area));
        }
    }
    NativeDrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
}

fn render_unavailable_workbench(
    frame: &mut Frame<'_>,
    tabs: Rect,
    panel: Rect,
    text: ShellText,
    tab_status: &str,
) {
    let tab_line = Line::from(vec![
        Span::styled(format!(" {tab_status} "), Style::new().fg(MUTED)),
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

fn render_legacy_terminal(
    frame: &mut Frame<'_>,
    panel: Rect,
    view: &ClientView,
    locale: Locale,
    projection: Option<&crate::terminal_projection::TerminalProjection>,
    status: TerminalStatus,
) {
    let area = crate::tui_layout::terminal_area(panel);
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

fn render_native_workbench(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    client_view: &ClientView,
    locale: Locale,
    text: ShellText,
    native: &NativeSessionView<'_>,
    phase_status: &str,
) {
    let tabs_area = areas.tabs;
    let panel_area = areas.panel;
    let tabs_block = Block::new()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(BORDER))
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {} ", text.workbench_title),
            Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD),
        ));
    let tabs_content = tabs_block.inner(tabs_area);
    frame.render_widget(tabs_block, tabs_area);
    tabs::render_compact(
        frame,
        tabs_content,
        native.tab_count(),
        native.active_position(),
        |index| native_tab(native, index),
    );

    let panel_block = Block::new()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(BORDER))
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {} ", text.panel_title),
            Style::new().fg(TEXT),
        ));
    let panel_content = panel_block.inner(panel_area);
    frame.render_widget(panel_block, panel_area);
    let terminal = if native.projection_status() == NativeProjectionStatus::FrameAvailable {
        native
            .projection()
            .and_then(crate::terminal_projection::TerminalProjection::frame)
    } else {
        None
    };
    if let Some(terminal) = terminal {
        terminal_cells::render(
            frame,
            panel_content,
            terminal,
            client_view.overlay().is_none() && native.close_confirmation().is_none(),
        );
    } else {
        let panel_status = if matches!(
            native.phase(),
            NativeDisplayPhase::Ready | NativeDisplayPhase::MoveProjectionUnavailable
        ) && native.projection_status()
            != NativeProjectionStatus::FrameAvailable
        {
            locale.terminal_status(TerminalStatus::Blocked)
        } else {
            phase_status
        };
        frame.render_widget(
            Paragraph::new(panel_status)
                .style(Style::new().fg(MUTED))
                .wrap(Wrap { trim: true }),
            panel_content,
        );
    }
}

fn native_tab<'a>(native: &NativeSessionView<'a>, index: usize) -> Option<tabs::TabDisplay<'a>> {
    let tab: NativeTabView<'a> = native.tab(index)?;
    Some(tabs::TabDisplay {
        position: tab.position(),
        active: tab.active(),
        label: tab.tab_label(),
    })
}

fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    text: ShellText,
    status: &str,
    secondary_notice: Option<&str>,
) -> Option<Rect> {
    let secondary_notice = secondary_notice.filter(|_| !view.prefix_active());
    let secondary_width = secondary_notice.map_or(24, |notice| {
        u16::try_from(Line::raw(notice).width())
            .unwrap_or(u16::MAX)
            .max(24)
    });
    let [notice, prefix] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(secondary_width)]).areas(area);
    let status_block = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(BORDER));
    let inner = status_block.inner(notice);
    frame.render_widget(
        Paragraph::new(
            view.notice()
                .map_or(status, |notice| locale.shell_notice(Some(notice))),
        )
        .style(Style::new().fg(MUTED))
        .block(status_block.clone()),
        notice,
    );
    let (label, style) = if let Some(notice) = secondary_notice {
        (notice, Style::new().fg(WARNING))
    } else if view.prefix_active() {
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
    (view.notice().is_none() && tab_close::unwrapped_text_fits(status, inner)).then_some(inner)
}

fn render_sidebar(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    text: ShellText,
    backend_status: &str,
) {
    if sidebar.layer == SidebarLayer::Overlay {
        frame.render_widget(Clear, sidebar.area);
    }

    let items = [
        ListItem::new(Line::styled(
            text.site_section,
            Style::new().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            format!("● {backend_status}"),
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

fn render_shortcut_help(frame: &mut Frame<'_>, viewport: Rect, text: ShellText) -> Rect {
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
    area
}

fn render_unavailable_workbench_navigator(
    frame: &mut Frame<'_>,
    viewport: Rect,
    text: ShellText,
) -> Rect {
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
    area
}

fn render_native_workbench_navigator(
    frame: &mut Frame<'_>,
    viewport: Rect,
    text: ShellText,
    locale: Locale,
    native: &NativeSessionView<'_>,
) -> Rect {
    let area = centered_overlay(viewport, 68, 9);
    frame.render_widget(Clear, area);
    let block = overlay_block(text.navigator_title);
    let content = block.inner(area);
    frame.render_widget(block, area);

    let refresh = native.refresh_status();
    let (rows, refresh_area) = if refresh.is_some() {
        let [rows, status] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(content);
        (rows, Some(status))
    } else {
        (content, None)
    };
    tabs::render_navigator(
        frame,
        rows,
        native.tab_count(),
        native.active_position(),
        |index| native_tab(native, index),
    );
    if let (Some(refresh), Some(status_area)) = (refresh, refresh_area) {
        let style = match refresh {
            NativeRefreshStatus::Refreshing | NativeRefreshStatus::Success(_) => {
                Style::new().fg(MUTED)
            }
            NativeRefreshStatus::Failure(_) => Style::new().fg(WARNING),
        };
        frame.render_widget(
            Paragraph::new(locale.native_refresh_status(refresh)).style(style),
            status_area,
        );
    }
    area
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
