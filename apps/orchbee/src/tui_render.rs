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
        NativeCloseStatusToken, NativeDisplayPhase, NativeDrawReceipt, NativePanelTree,
        NativeProjectionStatus, NativeRefreshStatus, NativeSessionView, NativeTabView,
        TerminalSessionView,
    },
    tui_layout::{
        ShellAreas, SidebarArea, SidebarLayer, ViewportTier, divide_panes, overlay_area,
        pane_content, place_overlay, project_shell_areas, terminal_area,
    },
};

use crate::terminal_session::TerminalStatus;

mod contexts;
mod tab_close;
mod tab_rename;
mod tabs;
mod terminal_cells;
mod workspace_setup;

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
    let tier = areas.tier;

    let mut status_draw: Option<(NativeCloseStatusToken, Rect)> = None;
    let mut prompt = None;
    let mut rename = None;
    let mut setup = None;
    let mut overlay_refused = false;
    let sidebar_status;
    let status_text;
    let mut secondary_notice = None;

    match session.as_ref() {
        None => {
            sidebar_status = text.backend_status.to_owned();
            status_text = locale.shell_notice(None).to_owned();
            if tier != ViewportTier::TooSmall {
                render_unavailable_workbench(frame, &areas, text, text.tabs_unavailable, tier);
            }
        }
        #[cfg(test)]
        Some(TerminalSessionView::Legacy { projection, status }) => {
            sidebar_status = locale.terminal_status(*status).to_owned();
            status_text = sidebar_status.clone();
            if tier != ViewportTier::TooSmall {
                render_unavailable_workbench(frame, &areas, text, text.workbench_title, tier);
                render_legacy_terminal(frame, areas.panel, view, locale, *projection, *status);
            }
        }
        Some(TerminalSessionView::Native(native)) => {
            let phase_status = locale.native_phase_status(native);
            let close = native.close_status();
            let status = close.as_ref().map_or_else(
                || locale.native_status(native),
                |status| locale.native_close_status(status),
            );
            secondary_notice = close
                .as_ref()
                .and_then(|_| native.notice())
                .map(|notice| locale.native_notice(notice));
            sidebar_status = phase_status.to_owned();
            status_text = status;
            if tier != ViewportTier::TooSmall {
                render_native_workbench(frame, &areas, view, locale, text, native, &phase_status);
            }
        }
    }

    if tier == ViewportTier::TooSmall {
        render_too_small_notice(frame, areas.body, text);
    }

    if tier != ViewportTier::TooSmall {
        if let Some(sidebar) = areas.sidebar {
            render_sidebar(frame, sidebar, text, &sidebar_status);
        }

        let overlay = match view.overlay() {
            Some(Overlay::ShortcutHelp) => {
                Some(render_shortcut_help(frame, overlay_area(&areas), text))
            }
            Some(Overlay::WorkbenchNavigator) => Some(match session.as_ref() {
                Some(TerminalSessionView::Native(native)) => render_native_workbench_navigator(
                    frame,
                    overlay_area(&areas),
                    text,
                    locale,
                    native,
                ),
                #[cfg(test)]
                Some(TerminalSessionView::Legacy { .. }) => {
                    render_unavailable_workbench_navigator(frame, overlay_area(&areas), text)
                }
                None => render_unavailable_workbench_navigator(frame, overlay_area(&areas), text),
            }),
            Some(Overlay::BusinessSidebar) => Some(areas.sidebar.map(|sidebar| sidebar.area)),
            None => None,
        };
        if let Some(None) = overlay {
            overlay_refused = true;
        }

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
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.rename_draft()
        {
            let (area, token) = tab_rename::render(frame, locale, &draft);
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            rename = token;
        }
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.setup_draft()
        {
            let (area, token) =
                workspace_setup::render(frame, locale, &draft, native.setup_status());
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            rename = None;
            setup = token;
        }
    } else if view.overlay().is_some() {
        overlay_refused = true;
    }

    let notice = if overlay_refused {
        Some(text.overlay_refused)
    } else {
        secondary_notice.as_deref()
    };
    let visible = render_status(
        frame,
        areas.status,
        view,
        locale,
        text,
        &status_text,
        notice,
        tier,
    );
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(close) = native.close_status()
    {
        status_draw = status_draw.or(visible.map(|area| (close.token(), area)));
    }

    NativeDrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
        .with_rename(rename)
        .with_setup(setup)
}

fn render_too_small_notice(frame: &mut Frame<'_>, body: Rect, text: ShellText) {
    let row = Rect::new(
        body.x,
        body.y.saturating_add(body.height / 2),
        body.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(tabs::truncate_display(
            text.too_small_notice,
            usize::from(body.width),
        ))
        .alignment(Alignment::Center)
        .style(Style::new().fg(WARNING)),
        row,
    );
}

fn render_unavailable_workbench(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    text: ShellText,
    tab_status: &str,
    tier: ViewportTier,
) {
    if !areas.tabs.is_empty() {
        let tab_line = Line::from(vec![
            Span::styled(
                format!(" {} ", text.workbench_title),
                Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {tab_status} "), Style::new().fg(MUTED)),
            Span::styled(
                "  !  ",
                Style::new().fg(WARNING).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(tab_line), areas.tabs);
    }

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
    let content_area = terminal_area(areas.panel);
    if tier.shows_panel_frame() {
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
            areas.panel,
        );
    } else {
        frame.render_widget(
            Paragraph::new(content)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            content_area,
        );
    }
}

#[cfg(test)]
fn render_legacy_terminal(
    frame: &mut Frame<'_>,
    panel: Rect,
    view: &ClientView,
    locale: Locale,
    projection: Option<&crate::terminal_projection::TerminalProjection>,
    status: TerminalStatus,
) {
    let area = terminal_area(panel);
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
    let panel_area = areas.panel;
    let location = native_location_label(native, text.standalone);
    render_tab_strip(frame, areas.tabs, native, location.as_deref());

    let content_area = terminal_area(panel_area);
    if areas.tier.shows_panel_frame() {
        let panel_block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(BORDER))
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                format!(" {} ", text.panel_title),
                Style::new().fg(TEXT),
            ));
        frame.render_widget(panel_block, panel_area);
    }
    let terminal = if native.projection_status() == NativeProjectionStatus::FrameAvailable {
        native
            .projection()
            .and_then(crate::terminal_projection::TerminalProjection::frame)
    } else {
        None
    };
    let active_tab = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index));
    if let Some(tab) = active_tab.filter(|tab| tab.panel_count() > 1) {
        let mut dividers = Vec::new();
        render_panel_tree(
            frame,
            content_area,
            tab,
            tab.panel_tree(),
            terminal,
            client_view.overlay().is_none()
                && native.close_confirmation().is_none()
                && native.rename_draft().is_none()
                && native.setup_draft().is_none(),
            text,
            phase_status,
            locale,
            native,
            &mut dividers,
        );
        render_dividers(frame, &dividers);
    } else if let Some(terminal) = terminal {
        terminal_cells::render(
            frame,
            content_area,
            terminal,
            client_view.overlay().is_none()
                && native.close_confirmation().is_none()
                && native.rename_draft().is_none()
                && native.setup_draft().is_none(),
        );
    } else {
        let panel_status = if matches!(
            native.phase(),
            NativeDisplayPhase::Ready
                | NativeDisplayPhase::MoveProjectionUnavailable
                | NativeDisplayPhase::PanelProjectionUnavailable
                | NativeDisplayPhase::RenameProjectionUnavailable
        ) && native.projection_status()
            != NativeProjectionStatus::FrameAvailable
        {
            locale.terminal_status(TerminalStatus::Blocked)
        } else {
            phase_status
        };
        let panel_status = workspace_setup::directory_guidance(
            locale,
            native.phase(),
            native.retirement().is_some(),
            native.workspace_directory_path(),
            native.workspace_directory_unconfigured(),
        )
        .unwrap_or_else(|| panel_status.to_owned());
        frame.render_widget(
            Paragraph::new(panel_status)
                .style(Style::new().fg(MUTED))
                .wrap(Wrap { trim: true }),
            content_area,
        );
    }
}

fn render_tab_strip(
    frame: &mut Frame<'_>,
    tabs_area: Rect,
    native: &NativeSessionView<'_>,
    location: Option<&str>,
) {
    if tabs_area.is_empty() {
        return;
    }
    let location = location.map(|location| format!("· {location} "));
    let (strip, location_area) = location.as_ref().map_or((tabs_area, None), |location| {
        let width = u16::try_from(Line::raw(location.as_str()).width()).unwrap_or(u16::MAX);
        if width < tabs_area.width {
            (
                Rect::new(
                    tabs_area.x,
                    tabs_area.y,
                    tabs_area.width - width - 1,
                    tabs_area.height,
                ),
                Some(Rect::new(
                    tabs_area.x + tabs_area.width - width,
                    tabs_area.y,
                    width,
                    tabs_area.height,
                )),
            )
        } else {
            (tabs_area, None)
        }
    });
    tabs::render_compact(
        frame,
        strip,
        native.tab_count(),
        native.active_position(),
        |index| native_tab(native, index),
    );
    if let (Some(location), Some(location_area)) = (location, location_area) {
        frame.render_widget(
            Paragraph::new(location)
                .alignment(Alignment::Right)
                .style(Style::new().fg(MUTED)),
            location_area,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_panel_tree(
    frame: &mut Frame<'_>,
    area: Rect,
    tab: NativeTabView<'_>,
    tree: &NativePanelTree,
    terminal: Option<&orchbee_terminal_protocol::TerminalTextFrame>,
    input_enabled: bool,
    text: ShellText,
    phase_status: &str,
    locale: Locale,
    native: &NativeSessionView<'_>,
    dividers: &mut Vec<Rect>,
) {
    match tree {
        NativePanelTree::Panel { panel_index } => {
            render_leaf(
                frame,
                area,
                tab,
                *panel_index,
                terminal,
                input_enabled,
                text,
                phase_status,
                locale,
                native,
            );
        }
        NativePanelTree::Split {
            first,
            second,
            orientation,
            ..
        } => match divide_panes(area, *orientation) {
            Some(division) => {
                dividers.push(division.divider);
                render_panel_tree(
                    frame,
                    division.first,
                    tab,
                    first,
                    terminal,
                    input_enabled,
                    text,
                    phase_status,
                    locale,
                    native,
                    dividers,
                );
                render_panel_tree(
                    frame,
                    division.second,
                    tab,
                    second,
                    terminal,
                    input_enabled,
                    text,
                    phase_status,
                    locale,
                    native,
                    dividers,
                );
            }
            None => {
                let leaf = active_leaf(tree, tab.active_panel()).or_else(|| first_leaf(tree));
                if let Some(NativePanelTree::Panel { panel_index }) = leaf {
                    render_leaf(
                        frame,
                        area,
                        tab,
                        *panel_index,
                        terminal,
                        input_enabled,
                        text,
                        phase_status,
                        locale,
                        native,
                    );
                }
            }
        },
    }
}

fn active_leaf(tree: &NativePanelTree, active: Option<usize>) -> Option<&NativePanelTree> {
    match tree {
        NativePanelTree::Panel { panel_index } => (Some(*panel_index) == active).then_some(tree),
        NativePanelTree::Split { first, second, .. } => {
            active_leaf(first, active).or_else(|| active_leaf(second, active))
        }
    }
}

fn first_leaf(tree: &NativePanelTree) -> Option<&NativePanelTree> {
    match tree {
        NativePanelTree::Panel { .. } => Some(tree),
        NativePanelTree::Split { first, .. } => first_leaf(first),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_leaf(
    frame: &mut Frame<'_>,
    area: Rect,
    tab: NativeTabView<'_>,
    panel_index: usize,
    terminal: Option<&orchbee_terminal_protocol::TerminalTextFrame>,
    input_enabled: bool,
    text: ShellText,
    phase_status: &str,
    locale: Locale,
    native: &NativeSessionView<'_>,
) {
    let active = tab.active_panel() == Some(panel_index);
    let panel = tab.panel(panel_index);
    let title = panel.map_or_else(
        || format!(" {} {} ", text.panel_title, panel_index.saturating_add(1)),
        |panel| {
            format!(
                " {} {} · {} ",
                text.panel_title,
                panel_index.saturating_add(1),
                panel.presentation_label().as_str()
            )
        },
    );
    let title_style = if active {
        Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(MUTED)
    };
    let title_area = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    frame.render_widget(
        Paragraph::new(tabs::truncate_display(&title, usize::from(area.width))).style(title_style),
        title_area,
    );
    let content = pane_content(area);
    if active {
        if let Some(terminal) = terminal {
            terminal_cells::render(frame, content, terminal, input_enabled);
        } else {
            let status = if matches!(
                native.phase(),
                NativeDisplayPhase::Ready
                    | NativeDisplayPhase::MoveProjectionUnavailable
                    | NativeDisplayPhase::PanelProjectionUnavailable
                    | NativeDisplayPhase::RenameProjectionUnavailable
            ) {
                locale.terminal_status(TerminalStatus::Blocked)
            } else {
                phase_status
            };
            frame.render_widget(
                Paragraph::new(status)
                    .style(Style::new().fg(MUTED))
                    .wrap(Wrap { trim: true }),
                content,
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(text.inactive_pane)
                .style(Style::new().fg(MUTED))
                .wrap(Wrap { trim: true }),
            content,
        );
    }
}

fn render_dividers(frame: &mut Frame<'_>, dividers: &[Rect]) {
    let mut cells = std::collections::BTreeSet::new();
    for divider in dividers {
        for y in divider.y..divider.y.saturating_add(divider.height) {
            for x in divider.x..divider.x.saturating_add(divider.width) {
                cells.insert((x, y));
            }
        }
    }
    for &(x, y) in &cells {
        let glyph = crate::tui_layout::divider_glyph(
            cells.contains(&(x, y.saturating_sub(1))),
            cells.contains(&(x, y.saturating_add(1))),
            cells.contains(&(x.saturating_sub(1), y)),
            cells.contains(&(x.saturating_add(1), y)),
        );
        frame
            .buffer_mut()
            .set_string(x, y, glyph.to_string(), Style::new().fg(BORDER));
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

fn native_location_label(
    native: &NativeSessionView<'_>,
    standalone_label: &'static str,
) -> Option<String> {
    native
        .active_context_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.context_at(index))
        .map(|context| {
            contexts::location_label(contexts::ContextDisplay {
                position: context.position(),
                active: context.active(),
                project_label: context.project_label(),
                workspace_label: context.workspace_label(),
                standalone_label,
            })
        })
}

#[allow(clippy::too_many_arguments)]
fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    text: ShellText,
    status: &str,
    secondary_notice: Option<&str>,
    tier: ViewportTier,
) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }
    let secondary_notice = secondary_notice.filter(|_| !view.prefix_active());
    let (label, style) = if let Some(notice) = secondary_notice {
        (notice.to_owned(), Style::new().fg(WARNING))
    } else if view.prefix_active() {
        (
            text.prefix_active.to_owned(),
            Style::new()
                .fg(BACKGROUND)
                .bg(EMPHASIS)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let hint = match tier {
            ViewportTier::Full => text.prefix_idle,
            ViewportTier::Compact => text.hint_short,
            ViewportTier::Minimal | ViewportTier::TooSmall => text.exit_hint,
        };
        (hint.to_owned(), Style::new().fg(MUTED))
    };
    let right_width = u16::try_from(Line::raw(label.as_str()).width())
        .unwrap_or(u16::MAX)
        .max(24)
        .min(area.width);
    let [status_area, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).areas(area);
    let status_label = view.notice().map_or_else(
        || status.to_owned(),
        |notice| locale.shell_notice(Some(notice)).to_owned(),
    );
    frame.render_widget(
        Paragraph::new(status_label).style(Style::new().fg(MUTED)),
        status_area,
    );
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(style),
        right,
    );
    (view.notice().is_none() && tab_close::unwrapped_text_fits(status, status_area))
        .then_some(status_area)
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

fn render_shortcut_help(frame: &mut Frame<'_>, area: Rect, text: ShellText) -> Option<Rect> {
    let height = u16::try_from(text.shortcuts.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let area = place_overlay(area, 76, height)?;
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
    Some(area)
}

fn render_unavailable_workbench_navigator(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
) -> Option<Rect> {
    let area = place_overlay(area, 68, 9)?;
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
    Some(area)
}

fn render_native_workbench_navigator(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    locale: Locale,
    native: &NativeSessionView<'_>,
) -> Option<Rect> {
    let area = place_overlay(area, 88, 12)?;
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
    let [contexts_area, tabs_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Min(0)]).areas(rows);
    let context_at = |index: usize| {
        native
            .context_at(index)
            .map(|context| contexts::ContextDisplay {
                position: context.position(),
                active: context.active(),
                project_label: context.project_label(),
                workspace_label: context.workspace_label(),
                standalone_label: text.standalone,
            })
    };
    contexts::render_navigator(
        frame,
        contexts_area,
        native.context_count(),
        native.active_context_position(),
        context_at,
    );
    tabs::render_navigator(
        frame,
        tabs_area,
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
    Some(area)
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
