use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
};

use crate::{
    client_view::{ClientView, Overlay},
    localization::{Locale, ShellText},
    terminal_session::{
        NativeCloseStatusToken, NativeDisplayPhase, NativeDrawReceipt, NativeProjectionStatus,
        NativeRefreshStatus, NativeSessionView, NativeTabView, TerminalSessionView,
    },
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_layout::{
        ShellAreas, SidebarArea, SidebarLayer, ViewportTier, overlay_area, place_overlay,
        project_shell_areas, terminal_area,
    },
    tui_theme::{Role, Theme},
};

use crate::terminal_session::TerminalStatus;

mod contexts;
mod panel_close;
mod tab_close;
mod tab_rename;
mod tabs;
mod terminal_cells;
mod workspace_setup;

#[cfg(test)]
pub(crate) fn render_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
) {
    render_terminal_shell(frame, view, locale, None, theme);
}

pub(crate) fn render_terminal_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    session: Option<TerminalSessionView<'_>>,
    theme: &Theme,
) -> NativeDrawReceipt {
    let area = frame.area();
    let text = locale.shell();
    frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), area);

    let areas = project_shell_areas(area, view.sidebar_surface());
    let tier = areas.tier;

    let mut status_draw: Option<(NativeCloseStatusToken, Rect)> = None;
    let mut prompt = None;
    let mut panel_prompt = None;
    let mut rename = None;
    let mut setup = None;
    let mut overlay_refused = false;
    let mut help_page = None;
    let sidebar_status;
    let status_text;
    let mut secondary_notice = None;

    match session.as_ref() {
        None => {
            sidebar_status = text.backend_status.to_owned();
            status_text = locale.shell_notice(None).to_owned();
            if tier != ViewportTier::TooSmall {
                render_unavailable_workbench(
                    frame,
                    &areas,
                    text,
                    text.tabs_unavailable,
                    tier,
                    theme,
                );
            }
        }
        #[cfg(test)]
        Some(TerminalSessionView::Legacy { projection, status }) => {
            sidebar_status = locale.terminal_status(*status).to_owned();
            status_text = sidebar_status.clone();
            if tier != ViewportTier::TooSmall {
                render_unavailable_workbench(
                    frame,
                    &areas,
                    text,
                    text.workbench_title,
                    tier,
                    theme,
                );
                render_legacy_terminal(
                    frame,
                    areas.panel,
                    view,
                    locale,
                    *projection,
                    *status,
                    theme,
                );
            }
        }
        Some(TerminalSessionView::Native(native)) => {
            let phase_status = locale.native_phase_status(native);
            let close = native.close_status();
            let status = close.as_ref().map_or_else(
                || locale.native_status(native),
                |status| locale.native_close_status(status),
            );
            secondary_notice = native.notice().map(|notice| locale.native_notice(notice));
            sidebar_status = phase_status.to_owned();
            status_text = status;
            if tier != ViewportTier::TooSmall {
                render_native_workbench(frame, &areas, view, locale, native, &phase_status, theme);
            }
        }
    }

    if tier == ViewportTier::TooSmall {
        render_too_small_notice(frame, areas.body, text, theme);
    }

    if tier != ViewportTier::TooSmall {
        if let Some(sidebar) = areas.sidebar {
            render_sidebar(frame, sidebar, text, &sidebar_status, theme);
        }

        let overlay = match view.overlay() {
            Some(Overlay::ShortcutHelp) => {
                // The help page is ClientView-owned; the renderer reports the
                // clamped effective page back through the draw receipt so the
                // dispatcher's counter settles on what was actually painted.
                let presented =
                    render_shortcut_help(frame, overlay_area(&areas), text, view.help_page(), theme);
                help_page = presented.map(|(_, page)| page);
                Some(presented.map(|(area, _page)| area))
            }
            Some(Overlay::WorkbenchNavigator) => Some(match session.as_ref() {
                Some(TerminalSessionView::Native(native)) => render_native_workbench_navigator(
                    frame,
                    overlay_area(&areas),
                    text,
                    locale,
                    native,
                    theme,
                ),
                #[cfg(test)]
                Some(TerminalSessionView::Legacy { .. }) => {
                    render_unavailable_workbench_navigator(frame, overlay_area(&areas), text, theme)
                }
                None => {
                    render_unavailable_workbench_navigator(frame, overlay_area(&areas), text, theme)
                }
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
            let modal = tab_close::render(
                frame,
                locale,
                &confirmation,
                overlay_area(&areas),
                theme,
            );
            status_draw = status_draw.filter(|(_, area)| !area.intersects(modal.area));
            prompt = Some(modal.prompt);
            if let Some(token) = modal.status {
                status_draw = Some((token, modal.area));
            }
        }
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(confirmation) = native.panel_close_confirmation()
        {
            let modal = panel_close::render(
                frame,
                locale,
                &confirmation,
                overlay_area(&areas),
                theme,
            );
            status_draw = status_draw.filter(|(_, area)| !area.intersects(modal.area));
            prompt = None;
            panel_prompt = Some(modal.prompt);
        }
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.rename_draft()
        {
            let (area, token) =
                tab_rename::render(frame, locale, &draft, overlay_area(&areas), theme);
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            panel_prompt = None;
            rename = Some(token);
        }
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.setup_draft()
        {
            let (area, token) = workspace_setup::render(
                frame,
                locale,
                &draft,
                native.setup_status(),
                overlay_area(&areas),
                theme,
            );
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            panel_prompt = None;
            rename = None;
            setup = Some(token);
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
        theme,
    );
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(close) = native.close_status()
    {
        status_draw = status_draw.or(visible.map(|area| (close.token(), area)));
    }

    NativeDrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
        .with_help_page(help_page)
        .with_panel_close(panel_prompt)
        .with_rename(rename)
        .with_setup(setup)
}

fn render_too_small_notice(frame: &mut Frame<'_>, body: Rect, text: ShellText, theme: &Theme) {
    let row = Rect::new(
        body.x,
        body.y.saturating_add(body.height / 2),
        body.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(truncate_display(
            text.too_small_notice,
            usize::from(body.width),
            theme.glyphs.ellipsis,
        ))
        .alignment(Alignment::Center)
        .style(theme.style(Role::StatusWarning)),
        row,
    );
}

fn render_unavailable_workbench(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    text: ShellText,
    tab_status: &str,
    tier: ViewportTier,
    theme: &Theme,
) {
    if !areas.tabs.is_empty() {
        let tab_line = Line::from(vec![
            Span::styled(
                format!(" {} ", text.workbench_title),
                theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {tab_status} "), theme.style(Role::TextSecondary)),
            Span::styled("  !  ", theme.style(Role::StatusWarning)),
        ]);
        frame.render_widget(Paragraph::new(tab_line), areas.tabs);
    }

    let content = vec![
        Line::from(""),
        Line::styled(text.terminal_unavailable, theme.style(Role::StatusWarning)),
        Line::from(""),
        Line::styled(text.shell_only_detail, theme.style(Role::TextPrimary)),
        Line::styled(text.no_client_session, theme.style(Role::TextSecondary)),
    ];
    let content_area = terminal_area(areas.panel, tier.shows_panel_frame());
    if tier.shows_panel_frame() {
        frame.render_widget(
            Paragraph::new(content)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(
                    Block::new()
                        .borders(Borders::ALL)
                        .border_style(theme.style(Role::FrameFocused))
                        .border_type(theme.corners)
                        .padding(Padding::vertical(1))
                        .title(Span::styled(
                            format!(" {} ", text.panel_title),
                            theme.style(Role::TextSecondary),
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
    theme: &Theme,
) {
    let area = terminal_area(panel, true);
    frame.render_widget(Clear, area);
    if let Some(terminal) =
        projection.and_then(crate::terminal_projection::TerminalProjection::frame)
    {
        terminal_cells::render(frame, area, terminal, view.overlay().is_none());
    } else {
        frame.render_widget(
            Paragraph::new(locale.terminal_status(status))
                .style(theme.style(Role::TextSecondary))
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
    native: &NativeSessionView<'_>,
    phase_status: &str,
    theme: &Theme,
) {
    let text = locale.shell();
    let panel_area = areas.panel;
    let location = native_location_label(native, text.standalone);
    render_tab_strip(frame, areas.tabs, native, location.as_deref(), text, theme);

    let framed = areas.tier.shows_panel_frame();
    let content_area = terminal_area(panel_area, framed);
    if framed {
        let panel_block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme.style(Role::FrameFocused))
            .border_type(theme.corners)
            .title(Span::styled(
                format!(" {} ", text.panel_title),
                theme.style(Role::TextSecondary),
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
    if let Some(tab) = active_tab.filter(|tab| framed && tab.panel_count() > 1) {
        let projection = crate::tui_layout::project_panel_tree(
            content_area,
            tab.panel_tree(),
            tab.active_panel(),
        );
        for leaf in &projection.leaves {
            render_leaf(
                frame,
                leaf,
                tab,
                terminal,
                client_view.overlay().is_none()
                    && native.close_confirmation().is_none()
                    && native.rename_draft().is_none()
                    && native.setup_draft().is_none(),
                text,
                phase_status,
                locale,
                native,
                theme,
            );
        }
        render_dividers(frame, &projection.dividers, theme);
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
                .style(theme.style(Role::TextSecondary))
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
    text: ShellText,
    theme: &Theme,
) {
    if tabs_area.is_empty() {
        return;
    }
    let title = location.map_or_else(
        || format!("{} ", text.workbench_title),
        |location| {
            format!(
                "{} {} {location} ",
                text.workbench_title, theme.glyphs.middle_dot
            )
        },
    );
    let title_width = u16::try_from(Line::raw(title.as_str()).width()).unwrap_or(u16::MAX);
    let (strip, title_area) = if title_width < tabs_area.width {
        (
            Rect::new(
                tabs_area.x,
                tabs_area.y,
                tabs_area.width - title_width - 1,
                tabs_area.height,
            ),
            Some(Rect::new(
                tabs_area.x + tabs_area.width - title_width,
                tabs_area.y,
                title_width,
                tabs_area.height,
            )),
        )
    } else {
        (tabs_area, None)
    };
    tabs::render_compact(
        frame,
        strip,
        native.tab_count(),
        native.active_position(),
        |index| native_tab(native, index),
        theme,
    );
    if let Some(title_area) = title_area {
        frame.render_widget(
            Paragraph::new(title)
                .alignment(Alignment::Right)
                .style(theme.style(Role::TextSecondary)),
            title_area,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_leaf(
    frame: &mut Frame<'_>,
    leaf: &crate::tui_layout::LeafArea,
    tab: NativeTabView<'_>,
    terminal: Option<&orchbee_terminal_protocol::TerminalTextFrame>,
    input_enabled: bool,
    text: ShellText,
    phase_status: &str,
    locale: Locale,
    native: &NativeSessionView<'_>,
    theme: &Theme,
) {
    let area = leaf.leaf;
    let panel_index = leaf.panel_index;
    let active = tab.active_panel() == Some(panel_index);
    let panel = tab.panel(panel_index);
    let title = panel.map_or_else(
        || format!(" {} {} ", text.panel_title, panel_index.saturating_add(1)),
        |panel| {
            format!(
                " {} {} {} {} ",
                text.panel_title,
                panel_index.saturating_add(1),
                theme.glyphs.middle_dot,
                sanitize_terminal_text(panel.presentation_label().as_str())
            )
        },
    );
    let title_style = if active {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else {
        theme.style(Role::TextSecondary)
    };
    let title_area = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    frame.render_widget(
        Paragraph::new(truncate_display(
            &title,
            usize::from(area.width),
            theme.glyphs.ellipsis,
        ))
        .style(title_style),
        title_area,
    );
    let content = leaf.content;
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
                    .style(theme.style(Role::TextSecondary))
                    .wrap(Wrap { trim: true }),
                content,
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(text.inactive_pane)
                .style(theme.style(Role::SurfaceInactive))
                .wrap(Wrap { trim: true }),
            content,
        );
    }
}

fn render_dividers(frame: &mut Frame<'_>, dividers: &[Rect], theme: &Theme) {
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
            .set_string(x, y, glyph.to_string(), theme.style(Role::FrameDivider));
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
    theme: &Theme,
) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }
    let secondary_notice = secondary_notice.filter(|_| !view.prefix_active());
    let (label, style) = if let Some(notice) = secondary_notice {
        (notice.to_owned(), theme.style(Role::StatusWarning))
    } else if view.prefix_active() {
        (
            text.prefix_active.to_owned(),
            theme
                .style(Role::TextInverse)
                .bg(theme.color(Role::AccentFocus))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let hint = match tier {
            ViewportTier::Full => text.prefix_idle,
            ViewportTier::Compact => text.hint_short,
            ViewportTier::Minimal | ViewportTier::TooSmall => text.exit_hint,
        };
        (hint.to_owned(), theme.style(Role::HintLabel))
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
        Paragraph::new(status_label).style(theme.style(Role::TextSecondary)),
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
    theme: &Theme,
) {
    if sidebar.layer == SidebarLayer::Overlay {
        frame.render_widget(Clear, sidebar.area);
    }

    let items = [
        ListItem::new(Line::styled(
            text.site_section,
            theme
                .style(Role::TextSecondary)
                .add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            format!("{} {backend_status}", theme.glyphs.status_bullet),
            theme.style(Role::StatusWarning),
        )),
        ListItem::new(""),
        ListItem::new(Line::styled(
            text.business_section,
            theme
                .style(Role::TextSecondary)
                .add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.standalone),
            theme.style(Role::TextPrimary),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.projects),
            theme.style(Role::TextSecondary),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.workspaces),
            theme.style(Role::TextSecondary),
        )),
        ListItem::new(Line::styled(
            format!("  {}", text.agents),
            theme.style(Role::TextSecondary),
        )),
        ListItem::new(""),
        ListItem::new(Line::styled(
            text.sidebar_hint,
            theme.style(Role::HintLabel),
        )),
    ];
    frame.render_widget(
        List::new(items).block(
            Block::new()
                .borders(Borders::RIGHT)
                .border_style(theme.style(Role::FrameDivider))
                .padding(Padding::new(2, 1, 1, 0))
                .style(theme.style(Role::SurfaceChrome))
                .title(Span::styled(
                    format!(" {} ", text.app_title),
                    theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD),
                )),
        ),
        sidebar.area,
    );
}

/// Rows the help footer always occupies: the dismiss/typing line plus the
/// paging/position line. Help refuses placement when fewer than
/// `MINIMUM_HELP_ROWS_PER_PAGE` shortcut rows would remain above it.
const HELP_FOOTER_ROWS: u16 = 2;
const MINIMUM_HELP_ROWS_PER_PAGE: u16 = 2;

/// The total shortcut-help pages for `row_count` rows shown
/// `rows_per_page` at a time. Pure projection shared with the input layer:
/// the dispatcher clamps its page state with the same count the renderer
/// uses, so the two can never disagree about the last page.
pub(crate) fn help_page_count(rows_per_page: usize, row_count: usize) -> usize {
    row_count.div_ceil(rows_per_page.max(1)).max(1)
}

/// Render the shortcut-help overlay paged to fit the placed area: the footer
/// (dismiss key, paging hint, typing purpose, and `page/total`) is always
/// drawn, and only the current page's rows are shown — content is never
/// silently clipped. Returns the placed rect and the effective page after
/// clamping to the page count, or `None` when rows plus footer cannot fit.
fn render_shortcut_help(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    page: usize,
    theme: &Theme,
) -> Option<(Rect, usize)> {
    let rows = text.shortcut_rows();
    let row_count = rows.len();
    let height = u16::try_from(row_count)
        .unwrap_or(u16::MAX)
        .saturating_add(HELP_FOOTER_ROWS)
        .saturating_add(2);
    let area = place_overlay(area, 76, height)?;
    let inner = overlay_block(text.shortcut_title, theme).inner(area);
    let rows_per_page = inner.height.saturating_sub(HELP_FOOTER_ROWS);
    if rows_per_page < MINIMUM_HELP_ROWS_PER_PAGE || inner.width == 0 {
        return None;
    }
    let rows_per_page = usize::from(rows_per_page);
    let total_pages = help_page_count(rows_per_page, row_count);
    let page = page.min(total_pages - 1);
    let start = page.saturating_mul(rows_per_page);
    let end = start.saturating_add(rows_per_page).min(row_count);

    frame.render_widget(Clear, area);
    let block = overlay_block(text.shortcut_title, theme);
    let [rows_area, footer_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(HELP_FOOTER_ROWS)]).areas(inner);
    frame.render_widget(block, area);

    let items = rows[start..end]
        .iter()
        .map(|shortcut| {
            let row = shortcut.split_once("  ").map_or_else(
                || Line::styled(shortcut.clone(), theme.style(Role::TextPrimary)),
                |(keys, label)| {
                    Line::from(vec![
                        Span::styled(keys, theme.style(Role::HintKey)),
                        Span::styled(format!("  {label}"), theme.style(Role::TextPrimary)),
                    ])
                },
            );
            ListItem::new(row)
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), rows_area);

    let footer = vec![
        Line::styled(text.help_dismiss_hint, theme.style(Role::HintLabel)),
        Line::styled(
            format!(
                "{} {} {}/{}",
                text.help_paging_hint,
                theme.glyphs.middle_dot,
                page + 1,
                total_pages
            ),
            theme.style(Role::HintLabel),
        ),
    ];
    frame.render_widget(Paragraph::new(footer), footer_area);
    Some((area, page))
}

fn render_unavailable_workbench_navigator(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    theme: &Theme,
) -> Option<Rect> {
    let area = place_overlay(area, 68, 9)?;
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(text.backend_status, theme.style(Role::StatusWarning)),
            Line::from(""),
            Line::styled(text.navigator_detail, theme.style(Role::TextPrimary)),
            Line::from(""),
            Line::styled(text.overlay_input_hint, theme.style(Role::HintLabel)),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(overlay_block(text.navigator_title, theme)),
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
    theme: &Theme,
) -> Option<Rect> {
    let area = place_overlay(area, 88, 12)?;
    frame.render_widget(Clear, area);
    let block = overlay_block(text.navigator_title, theme);
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
        theme,
    );
    tabs::render_navigator(
        frame,
        tabs_area,
        native.tab_count(),
        native.active_position(),
        |index| native_tab(native, index),
        theme,
    );
    if let (Some(refresh), Some(status_area)) = (refresh, refresh_area) {
        let style = match refresh {
            NativeRefreshStatus::Refreshing => theme.style(Role::TextSecondary),
            NativeRefreshStatus::Success(_) => theme.style(Role::StatusSuccess),
            NativeRefreshStatus::Failure(_) => theme.style(Role::StatusError),
        };
        frame.render_widget(
            Paragraph::new(locale.native_refresh_status(refresh)).style(style),
            status_area,
        );
    }
    Some(area)
}

fn overlay_block(title: &'static str, theme: &Theme) -> Block<'static> {
    Block::new()
        .borders(Borders::ALL)
        .border_type(theme.corners)
        .border_style(theme.style(Role::AccentFocus))
        .style(theme.style(Role::SurfaceOverlay))
        .padding(Padding::horizontal(2))
        .title(Span::styled(
            format!(" {title} "),
            theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD),
        ))
}

#[cfg(test)]
mod tests;
