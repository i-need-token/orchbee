#[cfg(not(windows))]
use crate::terminal_text::sanitize_terminal_text;
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
        AgentBoardDraw, ModalPresentation, NativeCloseStatusToken, NativeDisplayPhase,
        NativeDrawReceipt, NativeProjectionStatus, NativeRefreshStatus, NativeSessionView,
        NativeTabView, TerminalSessionView,
    },
    terminal_text::{display_width, truncate_display},
    tui_layout::{
        ShellAreas, SidebarArea, SidebarLayer, ViewportTier, overlay_area, place_overlay,
        project_shell_areas, terminal_area,
    },
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};

use crate::terminal_session::TerminalStatus;

mod agent_board;
mod contexts;
mod panel_close;
#[cfg(windows)]
mod panel_reports;
mod presentation_selector;
mod tab_close;
mod tab_rename;
mod tabs;
mod terminal_cells;
mod viewport;
mod workspace_setup;

#[cfg(test)]
pub(crate) fn render_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
) {
    render_terminal_shell(frame, view, locale, None, theme, &mut HitMapBuilder::new());
}

/// Paints the shell and records the pointer hit map in the same pass:
/// every painter reports the exact rect it drew, so a recorded region
/// can never disagree with the frame on screen. The shell publishes the
/// staged map only when the whole draw completes.
pub(crate) fn render_terminal_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    session: Option<TerminalSessionView<'_>>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
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
    let mut board_draw = None;
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
                    hits,
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
                render_native_workbench(
                    frame,
                    &areas,
                    view,
                    locale,
                    native,
                    &phase_status,
                    theme,
                    hits,
                );
            }
        }
    }

    if tier == ViewportTier::TooSmall {
        render_too_small_notice(frame, areas.body, text, theme);
    }

    if tier != ViewportTier::TooSmall {
        if let Some(sidebar) = areas.sidebar {
            let agent_line = match session.as_ref() {
                Some(TerminalSessionView::Native(native)) => native
                    .agent_board_summary()
                    .map(|summary| locale.agent_sidebar_line(summary, summary.at.elapsed())),
                _ => None,
            };
            render_sidebar(frame, sidebar, text, &sidebar_status, agent_line, theme);
        }

        let overlay = match view.overlay() {
            Some(Overlay::ShortcutHelp) => {
                // The help page is ClientView-owned; the renderer reports the
                // clamped effective page back through the draw receipt so the
                // dispatcher's counter settles on what was actually painted.
                let presented = render_shortcut_help(
                    frame,
                    overlay_area(&areas),
                    text,
                    view.help_page(),
                    theme,
                );
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
                    hits,
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
        if let Some(Some(area)) = overlay {
            // A presented chrome overlay owns the pointer: clicks on its
            // body die on the surface, and an outside click resolves as
            // the backdrop that dismisses it.
            hits.mark_overlay_active();
            hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
        }

        // The Agent board is session-owned rather than a ClientView
        // overlay: it paints above the workbench (and any chrome overlay)
        // but below armed modals, and its painted truth settles input
        // capture through the draw receipt.
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(board) = native.agent_board()
        {
            let draw = agent_board::render(
                frame,
                overlay_area(&areas),
                text,
                locale,
                board,
                theme,
                hits,
            );
            overlay_refused |= !draw.presented;
            board_draw = Some(draw);
        }

        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(confirmation) = native.close_confirmation()
        {
            let modal =
                tab_close::render(frame, locale, &confirmation, overlay_area(&areas), theme);
            status_draw = status_draw.filter(|(_, area)| !area.intersects(modal.area));
            prompt = Some(modal.prompt);
            if let Some(token) = modal.status {
                status_draw = Some((token, modal.area));
            }
        }
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(confirmation) = native.panel_close_confirmation()
        {
            let modal =
                panel_close::render(frame, locale, &confirmation, overlay_area(&areas), theme);
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
    } else {
        if view.overlay().is_some() {
            overlay_refused = true;
        }
        // A too-small viewport cannot present any armed modal, but the draw
        // still owes the state machine an outcome: the topmost armed capture
        // reports NotPresented so it settles cancelled with a notice instead
        // of surviving as an invisible key capture.
        if let Some(TerminalSessionView::Native(native)) = session.as_ref() {
            if native.close_confirmation().is_some() {
                prompt = Some(ModalPresentation::NotPresented);
            }
            if native.panel_close_confirmation().is_some() {
                prompt = None;
                panel_prompt = Some(ModalPresentation::NotPresented);
            }
            if native.rename_draft().is_some() {
                prompt = None;
                panel_prompt = None;
                rename = Some(ModalPresentation::NotPresented);
            }
            if native.setup_draft().is_some() {
                prompt = None;
                panel_prompt = None;
                rename = None;
                setup = Some(ModalPresentation::NotPresented);
            }
        }
    }

    // An open board that could not paint at all (too-small viewport)
    // still reports its verdict — input capture never settles on an
    // unpainted surface.
    if board_draw.is_none()
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && native.agent_board().is_some()
    {
        board_draw = Some(AgentBoardDraw {
            presented: false,
            viewport_rows: 0,
        });
        overlay_refused = true;
    }

    let notice = if overlay_refused {
        Some(text.overlay_refused)
    } else {
        secondary_notice.as_deref()
    };
    let zoomed = matches!(
        session.as_ref(),
        Some(TerminalSessionView::Native(native)) if native.zoom_active()
    );
    let visible = render_status(
        frame,
        areas.status,
        view,
        locale,
        text,
        &status_text,
        notice,
        zoomed,
        tier,
        theme,
    );
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(close) = native.close_status()
    {
        status_draw = status_draw.or(visible.map(|area| (close.token(), area)));
    }

    let receipt = NativeDrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
        .with_help_page(help_page)
        .with_panel_close(panel_prompt)
        .with_rename(rename)
        .with_setup(setup)
        .with_agent_board(board_draw);
    #[cfg(windows)]
    let receipt = receipt.with_attention(match session.as_ref() {
        Some(TerminalSessionView::Native(native)) if view.overlay().is_none() => {
            native.attention_draw(hits.painted_panes())
        }
        _ => None,
    });
    receipt
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
#[expect(clippy::too_many_arguments)]
fn render_legacy_terminal(
    frame: &mut Frame<'_>,
    panel: Rect,
    view: &ClientView,
    locale: Locale,
    projection: Option<&crate::terminal_projection::TerminalProjection>,
    status: TerminalStatus,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let area = terminal_area(panel, true);
    frame.render_widget(Clear, area);
    if let Some(terminal) =
        projection.and_then(crate::terminal_projection::TerminalProjection::frame)
    {
        terminal_cells::render(frame, area, terminal, view.overlay().is_none());
        // The legacy session owns one terminal and no workbench: its pane
        // records the fixed sentinel incarnation the probe resolves
        // identically.
        hits.record(
            HitLayer::Foreground,
            area,
            HitTarget::Pane {
                pane: crate::terminal_input::legacy_pane_ref(),
                content: area,
            },
        );
    } else {
        frame.render_widget(
            Paragraph::new(locale.terminal_status(status))
                .style(theme.style(Role::TextSecondary))
                .wrap(Wrap { trim: true }),
            area,
        );
    }
}

#[expect(clippy::too_many_arguments)]
fn render_native_workbench(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    client_view: &ClientView,
    locale: Locale,
    native: &NativeSessionView<'_>,
    phase_status: &str,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let text = locale.shell();
    let panel_area = areas.panel;
    let location = native_location_label(native, text.standalone);
    render_tab_strip(
        frame,
        areas.tabs,
        native,
        location.as_deref(),
        text,
        theme,
        hits,
    );

    let active_tab = native
        .active_position()
        .and_then(|p| p.checked_sub(1))
        .and_then(|index| native.tab(index));
    let framed = areas.tier.shows_panel_frame();
    let content_area = terminal_area(panel_area, framed);
    let panel_title = text.panel_title.to_owned();
    #[cfg(windows)]
    let panel_title = native
        .focus_panel_id()
        .map_or(panel_title.clone(), |panel| {
            format!(
                "{}{}",
                panel_title,
                locale.agent_unseen_badge(native.panel_unseen_count(panel))
            )
        });
    let outer_header = Rect::new(
        panel_area.x.saturating_add(1),
        panel_area.y,
        panel_area.width.saturating_sub(2),
        1.min(panel_area.height),
    );
    let selector = active_tab
        .filter(|tab| framed && (tab.panel_count() == 1 || native.zoom_active()))
        .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)))
        .and_then(|panel| {
            presentation_selector::PresentationSelector::new(outer_header, panel, native)
        });
    #[cfg(windows)]
    let outer_title = selector
        .as_ref()
        .map_or(outer_header, |s| s.title_area(outer_header));
    let frame_title = Line::from(Span::styled(
        format!(" {panel_title} "),
        theme.style(Role::TextSecondary),
    ));
    #[cfg(windows)]
    let frame_title = active_tab
        .filter(|tab| tab.panel_count() == 1 || native.zoom_active())
        .and_then(|tab| {
            let index = tab.active_panel()?;
            let panel = tab.panel(index)?;
            let title = format!(
                "{}{}",
                panel_reports::title(
                    native.selected_presentation(panel),
                    index + 1,
                    text.panel_title,
                    native.panel_report(panel),
                    theme
                ),
                locale.agent_unseen_badge(native.panel_unseen_count(panel.panel_id()))
            );
            let title = if panel.presentation_count() > 1 && selector.is_none() {
                let position = panel
                    .presentation_index(native.selected_presentation(panel).presentation_id())
                    .unwrap_or(0)
                    + 1;
                format!("{title} [{position}/{}]", panel.presentation_count())
            } else {
                title
            };
            Some(panel_reports::header(
                title,
                native.panel_report(panel),
                usize::from(outer_title.width),
                locale,
                theme,
                theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD),
            ))
        })
        .unwrap_or(frame_title);
    if framed {
        let panel_block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme.style(Role::FrameFocused))
            .border_type(theme.corners)
            .title(frame_title);
        frame.render_widget(panel_block, panel_area);
        if let Some(selector) = &selector {
            selector.paint(frame, theme, hits);
        }
    }
    // Under zoom the drawn set narrows to the focused leaf: the single-pane
    // path below paints it across the whole content area and records the
    // only pane hit region — hidden leaves stay attached but undrawn.
    if let Some(tab) =
        active_tab.filter(|tab| framed && tab.panel_count() > 1 && !native.zoom_active())
    {
        let projection = crate::tui_layout::project_panel_tree(
            content_area,
            tab.panel_tree(),
            tab.active_panel(),
            &|split, first, second| tab.split_ratio(split, first, second),
        );
        for leaf in &projection.leaves {
            let selector = render_leaf(
                frame,
                leaf,
                tab,
                client_view.overlay().is_none()
                    && native.close_confirmation().is_none()
                    && native.rename_draft().is_none()
                    && native.setup_draft().is_none(),
                text,
                phase_status,
                locale,
                native,
                theme,
                hits,
            );
            // The recorded region carries the pane's exact live
            // incarnation — a press resolves it bit for bit, so it can
            // never migrate onto a re-minted record of the same panel.
            if let Some(pane) = tab
                .panel(leaf.panel_index)
                .and_then(|panel| native.pane_ref_for_panel(panel.panel_id()))
            {
                presentation_selector::record_leaf(hits, leaf.leaf, leaf.content, pane, selector);
            }
        }
        render_dividers(frame, &projection.dividers, theme);
    } else {
        let terminal = if native.projection_status() == NativeProjectionStatus::FrameAvailable {
            native
                .projection()
                .and_then(crate::terminal_projection::TerminalProjection::frame)
        } else {
            None
        };
        let viewport = native
            .focus_panel_id()
            .and_then(|panel_id| native.pane_viewport(panel_id));
        if let Some(surface) = viewport {
            // A non-Live core owns the whole content area the same way a
            // split leaf does — validated window or mode indicator.
            viewport::render(
                frame,
                content_area,
                surface,
                terminal,
                client_view.overlay().is_none()
                    && native.close_confirmation().is_none()
                    && native.rename_draft().is_none()
                    && native.setup_draft().is_none(),
                locale,
                theme,
            );
            if let Some(pane) = active_tab
                .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)))
                .and_then(|panel| native.pane_ref_for_panel(panel.panel_id()))
            {
                hits.record(
                    HitLayer::Foreground,
                    content_area,
                    HitTarget::Pane {
                        pane,
                        content: content_area,
                    },
                );
            }
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
            // The single-pane paint covers the whole content area; the frame
            // border around it stays Client chrome and records nothing. The
            // recorded region carries the pane's exact live incarnation.
            if let Some(pane) = active_tab
                .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)))
                .and_then(|panel| native.pane_ref_for_panel(panel.panel_id()))
            {
                hits.record(
                    HitLayer::Foreground,
                    content_area,
                    HitTarget::Pane {
                        pane,
                        content: content_area,
                    },
                );
            }
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
}

fn render_tab_strip(
    frame: &mut Frame<'_>,
    tabs_area: Rect,
    native: &NativeSessionView<'_>,
    location: Option<&str>,
    text: ShellText,
    theme: &Theme,
    hits: &mut HitMapBuilder,
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
        |index| native_tab(native, index, text.pinned_tab),
        theme,
        &mut |index, x, width| {
            if let Some(tab) = native.tab(index) {
                hits.record(
                    HitLayer::Foreground,
                    Rect::new(
                        strip.x.saturating_add(u16::try_from(x).unwrap_or(u16::MAX)),
                        strip.y,
                        u16::try_from(width).unwrap_or(u16::MAX),
                        strip.height,
                    ),
                    HitTarget::Tab(tab.tab_id().clone()),
                );
            }
        },
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

/// Paints one projected leaf: its own pane's latest frame when the pane's
/// viewer has delivered one, otherwise the pane-local status. Background
/// panes keep their real terminal content — focus only changes which pane
/// accepts input and shows the focused title chrome.
#[allow(clippy::too_many_arguments)]
fn render_leaf(
    frame: &mut Frame<'_>,
    leaf: &crate::tui_layout::LeafArea,
    tab: NativeTabView<'_>,
    input_enabled: bool,
    text: ShellText,
    phase_status: &str,
    locale: Locale,
    native: &NativeSessionView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let area = leaf.leaf;
    let panel_index = leaf.panel_index;
    let active = tab.active_panel() == Some(panel_index);
    let panel = tab.panel(panel_index);
    let header_area = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    let selector = panel.and_then(|panel| {
        presentation_selector::PresentationSelector::new(header_area, panel, native)
    });
    #[cfg(windows)]
    let title = panel.map_or_else(
        || format!(" {} {} ", text.panel_title, panel_index + 1),
        |p| {
            panel_reports::title(
                native.selected_presentation(p),
                panel_index + 1,
                text.panel_title,
                native.panel_report(p),
                theme,
            )
        },
    );
    #[cfg(not(windows))]
    let title = panel.map_or_else(
        || format!(" {} {} ", text.panel_title, panel_index.saturating_add(1)),
        |panel| {
            let override_title = native.selected_presentation(panel).custom_label();
            if let Some(title) = override_title {
                return format!(
                    " {} {} {} ",
                    sanitize_terminal_text(title),
                    theme.glyphs.middle_dot,
                    panel_index.saturating_add(1)
                );
            }
            format!(
                " {} {} {} {} ",
                text.panel_title,
                panel_index.saturating_add(1),
                theme.glyphs.middle_dot,
                sanitize_terminal_text(native.selected_presentation(panel).display_label())
            )
        },
    );
    let title = panel.map_or(title.clone(), |panel| {
        if panel.presentation_count() == 1 || selector.is_some() {
            return title.clone();
        }
        let member = native.selected_presentation(panel);
        let position = panel
            .presentation_index(member.presentation_id())
            .unwrap_or(0)
            + 1;
        format!("{title}[{position}/{}] ", panel.presentation_count())
    });
    #[cfg(windows)]
    let title = panel.map_or(title.clone(), |panel| {
        format!(
            "{}{}",
            title,
            locale.agent_unseen_badge(native.panel_unseen_count(panel.panel_id()))
        )
    });
    let title_style = if active {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else {
        theme.style(Role::TextSecondary)
    };
    let title_area = selector
        .as_ref()
        .map_or(header_area, |s| s.title_area(header_area));
    #[cfg(windows)]
    let header = panel_reports::header(
        title,
        panel.map_or(crate::terminal_session::PanelReportView::Pending, |p| {
            native.panel_report(p)
        }),
        usize::from(title_area.width),
        locale,
        theme,
        title_style,
    );
    #[cfg(not(windows))]
    let header = Line::from(Span::styled(
        truncate_display(&title, usize::from(title_area.width), theme.glyphs.ellipsis),
        title_style,
    ));
    frame.render_widget(Paragraph::new(header), title_area);
    if let Some(selector) = &selector {
        selector.paint(frame, theme, hits);
    }
    let content = leaf.content;
    if !active {
        frame.render_widget(
            Block::new().style(theme.style(Role::SurfaceInactive)),
            content,
        );
    }
    let terminal = panel.map(|panel| panel.panel_id()).and_then(|panel_id| {
        (native.pane_projection_status(panel_id) == NativeProjectionStatus::FrameAvailable)
            .then(|| {
                native
                    .pane_projection(panel_id)
                    .and_then(crate::terminal_projection::TerminalProjection::frame)
            })
            .flatten()
    });
    let viewport = panel
        .map(|panel| panel.panel_id())
        .and_then(|panel_id| native.pane_viewport(panel_id));
    if let Some(surface) = viewport {
        // A non-Live core owns the leaf: its validated window or its mode
        // indicator paints instead of the live frame path.
        viewport::render(
            frame,
            content,
            surface,
            terminal,
            input_enabled && active,
            locale,
            theme,
        );
    } else if let Some(terminal) = terminal {
        terminal_cells::render(frame, content, terminal, input_enabled && active);
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
    selector.map(|s| s.area)
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

fn native_tab<'a>(
    native: &NativeSessionView<'a>,
    index: usize,
    pinned_label: &str,
) -> Option<tabs::TabDisplay<'a>> {
    let tab: NativeTabView<'a> = native.tab(index)?;
    Some(tabs::TabDisplay {
        position: tab.position(),
        active: tab.active(),
        label: tabs::label_with_metadata(
            tab.tab_label(),
            tab.pinned().then_some(pinned_label),
            tab.group_label(),
        ),
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
    zoomed: bool,
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
    // A released pointer is a persistent mode, not a transient notice: it
    // rides the status label on every draw until capture re-arms.
    let status_label = if view.pointer_released() {
        format!("{status_label} · {}", text.pointer_released)
    } else {
        status_label
    };
    // So is an active zoom: the badge marks the focused-only pane set until
    // the flag drops.
    let status_label = if zoomed {
        format!("{status_label} · {}", text.zoom_badge)
    } else {
        status_label
    };
    let fits =
        view.notice().is_none() && tab_close::unwrapped_text_fits(&status_label, status_area);
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
    fits.then_some(status_area)
}

fn render_sidebar(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    text: ShellText,
    backend_status: &str,
    agent_line: Option<String>,
    theme: &Theme,
) {
    if sidebar.layer == SidebarLayer::Overlay {
        frame.render_widget(Clear, sidebar.area);
    }

    // Inner width: right border plus the block's 2/1 horizontal padding.
    // Labels truncate on grapheme boundaries with an ellipsis marker so a
    // long status never clips mid-word or silently hides that it was cut.
    let inner_width = usize::from(sidebar.area.width.saturating_sub(4));
    let fit = |label: String| truncate_display(&label, inner_width, theme.glyphs.ellipsis);
    let items = [
        ListItem::new(Line::styled(
            fit(text.site_section.to_owned()),
            theme
                .style(Role::TextSecondary)
                .add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            fit(format!("{} {backend_status}", theme.glyphs.status_bullet)),
            theme.style(Role::StatusWarning),
        )),
        ListItem::new(""),
        ListItem::new(Line::styled(
            fit(text.business_section.to_owned()),
            theme
                .style(Role::TextSecondary)
                .add_modifier(Modifier::BOLD),
        )),
        ListItem::new(Line::styled(
            fit(format!("  {}", text.standalone)),
            theme.style(Role::TextPrimary),
        )),
        ListItem::new(Line::styled(
            fit(format!("  {}", text.projects)),
            theme.style(Role::TextSecondary),
        )),
        ListItem::new(Line::styled(
            fit(format!("  {}", text.workspaces)),
            theme.style(Role::TextSecondary),
        )),
        ListItem::new(Line::styled(
            fit(format!("  {}", text.agents)),
            theme.style(Role::TextSecondary),
        )),
        // The last *completed* read's count plus age/completeness marks —
        // absent entirely when no read ever completed (never an implied
        // zero, never live data).
        agent_line.map_or_else(
            || ListItem::new(""),
            |line| ListItem::new(Line::styled(fit(line), theme.style(Role::TextSecondary))),
        ),
        ListItem::new(""),
        ListItem::new(Line::styled(
            fit(text.sidebar_hint.to_owned()),
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

/// Groups whole shortcut rows into pages that each fit `rows_per_page`
/// rendered lines: `row_lines[i]` is the line count row `i` occupies after
/// width-aware wrapping. A row is never split across a page boundary, so no
/// wrapped tail can be drawn as a half-visible line. Returns the page ranges
/// as row indices, or `None` when a single row exceeds the page budget —
/// the caller then refuses placement instead of clipping content.
fn paginate_help_rows(row_lines: &[usize], rows_per_page: usize) -> Option<Vec<(usize, usize)>> {
    let mut pages = Vec::new();
    let mut start = 0;
    let mut used = 0;
    for (index, &lines) in row_lines.iter().enumerate() {
        if lines > rows_per_page {
            return None;
        }
        if used + lines > rows_per_page {
            pages.push((start, index));
            start = index;
            used = 0;
        }
        used += lines;
    }
    pages.push((start, row_lines.len()));
    Some(pages)
}

/// Wraps `text` to `width` display cells on grapheme boundaries, preferring
/// to break at ASCII spaces; a word wider than `width` breaks mid-word so no
/// cell is ever clipped. Returns `None` when the text cannot be presented
/// honestly — a zero budget, or a grapheme wider than `width` that no line
/// could hold without clipping. Never fabricates a width the caller lacks.
fn wrap_display(text: &str, width: usize) -> Option<Vec<String>> {
    if width == 0 {
        return text.is_empty().then(|| vec![String::new()]);
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0_usize;
    for word in text.split(' ') {
        let gap = usize::from(!line.is_empty());
        if !line.is_empty() && line_width + gap + display_width(word) > width {
            lines.push(std::mem::take(&mut line));
            line_width = 0;
        } else if gap == 1 {
            line.push(' ');
            line_width += 1;
        }
        // Hard-wrap an over-wide word across lines on grapheme boundaries.
        for grapheme in
            ratatui::text::Span::raw(word).styled_graphemes(ratatui::style::Style::default())
        {
            let cell = display_width(grapheme.symbol);
            if cell > width {
                return None;
            }
            if line_width + cell > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push_str(grapheme.symbol);
            line_width += cell;
        }
    }
    lines.push(line);
    Some(lines)
}

/// Render the shortcut-help overlay paged to fit the placed area: each
/// shortcut description wraps to the inner width on grapheme boundaries, a
/// shortcut's wrapped lines always stay on one page, and the footer
/// (dismiss key, paging hint, typing purpose, and `page/total`) is always
/// drawn — content is never silently clipped. Returns the placed rect and
/// the effective page after clamping to the page count, or `None` when rows
/// plus footer cannot fit whole shortcuts.
fn render_shortcut_help(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    page: usize,
    theme: &Theme,
) -> Option<(Rect, usize)> {
    let rows = text.shortcut_rows();
    let height = u16::try_from(rows.len())
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
    let width = usize::from(inner.width);

    // Each registry row is a (padded chord, description) pair: the chord
    // column is already a uniform width, so continuation lines align under
    // the description column by the chord's display width. A chord wider
    // than the inner width cannot be drawn without clipping — refuse.
    let row_lines = rows
        .iter()
        .map(|(chord, label)| {
            let indent = display_width(chord);
            if indent > width {
                return None;
            }
            let wrapped = wrap_display(label, width - indent)?;
            let mut lines = Vec::with_capacity(wrapped.len());
            let mut parts = wrapped.into_iter();
            lines.push(Line::from(vec![
                Span::styled(chord.clone(), theme.style(Role::HintKey)),
                Span::styled(
                    parts.next().unwrap_or_default(),
                    theme.style(Role::TextPrimary),
                ),
            ]));
            lines.extend(parts.map(|part| {
                Line::styled(
                    format!("{:indent$}{part}", ""),
                    theme.style(Role::TextPrimary),
                )
            }));
            Some(lines)
        })
        .collect::<Option<Vec<_>>>()?;
    let pages = paginate_help_rows(
        &row_lines.iter().map(Vec::len).collect::<Vec<_>>(),
        rows_per_page,
    )?;
    let total_pages = pages.len();
    let page = page.min(total_pages - 1);
    let (start, end) = pages[page];

    frame.render_widget(Clear, area);
    let block = overlay_block(text.shortcut_title, theme);
    let [rows_area, footer_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(HELP_FOOTER_ROWS)]).areas(inner);
    frame.render_widget(block, area);

    let items = row_lines[start..end]
        .iter()
        .flatten()
        .map(|line| ListItem::new(line.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), rows_area);

    // The footer is mandatory and is never truncated: the dismiss line
    // uses the widest localized form that fits whole — long first, then
    // the explicit short form. The paging line leads with `page/total`
    // and keeps the full paging hint, so the position is always visible.
    // When the dismiss line cannot fit even in short form — or the paging
    // line does not fit — placement refuses instead of drawing a keyless
    // or numberless footer.
    let dismiss = [text.help_dismiss_hint, text.help_dismiss_hint_short]
        .into_iter()
        .find(|hint| display_width(hint) <= width)?;
    let paging = format!(
        "{}/{} {} {}",
        page + 1,
        total_pages,
        theme.glyphs.middle_dot,
        text.help_paging_hint
    );
    if display_width(&paging) > width {
        return None;
    }
    let footer = vec![
        Line::styled(dismiss, theme.style(Role::HintLabel)),
        Line::styled(paging, theme.style(Role::HintLabel)),
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
    hits: &mut HitMapBuilder,
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
        &mut |index, offset| {
            if let Some(context) = native.context_at(index) {
                hits.record(
                    HitLayer::Overlay,
                    Rect::new(
                        contexts_area.x,
                        contexts_area.y.saturating_add(offset),
                        contexts_area.width,
                        1,
                    ),
                    HitTarget::Context(context.location().clone()),
                );
            }
        },
    );
    tabs::render_navigator(
        frame,
        tabs_area,
        native.tab_count(),
        native.active_position(),
        |index| native_tab(native, index, text.pinned_tab),
        theme,
        &mut |index, offset| {
            if let Some(tab) = native.tab(index) {
                hits.record(
                    HitLayer::Overlay,
                    Rect::new(
                        tabs_area.x,
                        tabs_area.y.saturating_add(offset),
                        tabs_area.width,
                        1,
                    ),
                    HitTarget::Tab(tab.tab_id().clone()),
                );
            }
        },
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
