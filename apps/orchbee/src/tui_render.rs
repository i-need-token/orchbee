#[cfg(not(windows))]
use crate::terminal_text::sanitize_terminal_text;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
};
#[cfg(windows)]
mod action_menu;
#[cfg(all(windows, test))]
pub(crate) fn render_action_menu_for_test(
    frame: &mut Frame<'_>,
    content: Rect,
    menu: crate::action_menu::MenuView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<crate::action_menu::MenuDraw> {
    action_menu::render(frame, content, menu, locale, theme, hits)
}
#[cfg(windows)]
mod resize_mode;
mod settings;

use crate::{
    client_view::{ClientView, Notice, Overlay},
    localization::{Locale, ShellText},
    terminal_session::{
        AgentBoardDraw, ModalPresentation, NativeCloseStatusToken, NativeDisplayController,
        NativeDisplayPhase, NativeDrawReceipt, NativeProjectionStatus, NativeRefreshStatus,
        NativeSessionView, NativeTabView, TerminalSessionView,
    },
    terminal_text::{display_width, truncate_display},
    tui_layout::{
        ShellAreas, SidebarArea, SidebarLayer, ViewportTier, native_terminal_area, overlay_area,
        place_overlay, project_shell_areas, terminal_area,
    },
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};

use crate::terminal_session::TerminalStatus;

mod agent_board;
#[cfg(test)]
pub(crate) fn render_agent_board_for_test(
    frame: &mut Frame<'_>,
    locale: Locale,
    board: &crate::terminal_session::NativeAgentBoardView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> AgentBoardDraw {
    agent_board::render(
        frame,
        frame.area(),
        locale.shell(),
        locale,
        board,
        theme,
        hits,
    )
}
#[cfg(windows)]
mod agent_sidebar;
mod contexts;
pub(crate) mod controls;
#[cfg(windows)]
mod copy_mode;
#[cfg(windows)]
mod divider_resize;
#[cfg(windows)]
mod navigator;
#[cfg(windows)]
mod pane_graphics;
mod panel_close;
#[cfg(windows)]
mod panel_reports;
mod presentation_selector;
mod selection;
#[cfg(windows)]
pub(crate) use agent_sidebar::render_width_preview;
#[cfg(windows)]
pub(crate) use divider_resize::render_divider_preview;
#[cfg(windows)]
pub(crate) use tabs::render_tab_drag_preview;
mod scrollbar;
#[cfg(windows)]
mod sidebar_text;
mod tab_close;
mod tab_rename;
mod tabs;
mod terminal_cells;
mod viewport;
mod workspace_close;
mod workspace_setup;
#[cfg(windows)]
mod workspace_sidebar;
#[cfg(windows)]
mod worktree_launch;

#[derive(Clone, Copy)]
struct SiteStatus<'a> {
    text: &'a str,
    role: Role,
}

impl SiteStatus<'_> {
    fn line(self, width: usize, theme: &Theme) -> Line<'static> {
        let marker = format!("{} ", theme.glyphs.status_bullet);
        let remaining = width.saturating_sub(display_width(&marker));
        Line::from(vec![
            Span::styled(
                truncate_display(&marker, width, theme.glyphs.ellipsis),
                theme.style(self.role),
            ),
            Span::styled(
                truncate_display(self.text, remaining, theme.glyphs.ellipsis),
                theme.style(Role::TextPrimary),
            ),
        ])
    }
}

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
    #[cfg(windows)]
    hits.record_action_menu_source(match session.as_ref() {
        Some(TerminalSessionView::Native(native)) => native.action_menu_source(),
        _ => None,
    });
    let bound_text = locale.shell().bind(view);
    let text = bound_text.text();
    frame.render_widget(Block::new().style(theme.style(Role::SurfaceCanvas)), area);

    let areas = project_shell_areas(area, view.sidebar_surface());
    frame
        .buffer_mut()
        .set_style(areas.tabs, theme.style(Role::SurfaceChrome));
    #[cfg(windows)]
    let mut areas = areas;
    #[cfg(windows)]
    let restore_button =
        if view.machines.available && areas.sidebar.is_none() && !areas.tabs.is_empty() {
            let width = u16::try_from(display_width(locale.sidebar_restore_button()))
                .unwrap_or(u16::MAX)
                .min(areas.tabs.width);
            let button = Rect::new(areas.tabs.x, areas.tabs.y, width, areas.tabs.height);
            areas.tabs.x = areas.tabs.x.saturating_add(width);
            areas.tabs.width = areas.tabs.width.saturating_sub(width);
            Some(button)
        } else {
            None
        };
    #[cfg(windows)]
    let menu_label = format!("[{}]", locale.menu_title());
    #[cfg(windows)]
    let menu_button = {
        let width = u16::try_from(display_width(&menu_label)).unwrap_or(u16::MAX);
        if areas.tabs.width >= width + 2 && areas.tabs.height > 0 {
            let rect = Rect::new(areas.tabs.right() - width, areas.tabs.y, width, 1);
            areas.tabs.width -= width + 1;
            Some(rect)
        } else if areas.tabs.is_empty()
            && areas.workbench.width >= width + 2
            && areas.workbench.height > 1
        {
            Some(Rect::new(
                areas.workbench.right() - width - 1,
                areas.workbench.y,
                width,
                1,
            ))
        } else {
            None
        }
    };
    let tier = areas.tier;
    #[cfg(windows)]
    let graphics_uncovered = matches!(session.as_ref(), Some(TerminalSessionView::Native(native))
        if !status_overlay_visible(view, session.as_ref()) && native.popup().is_none() && view.overlay().is_none()
            && native.close_confirmation().is_none() && native.panel_close_confirmation().is_none()
            && native.workspace_close_confirmation().is_none()
            && native.rename_draft().is_none() && native.setup_draft().is_none() && native.worktree().is_none()
            && native.agent_board().is_none() && native.action_menu().is_none() && native.settings().is_none()
            && areas.sidebar.is_none_or(|sidebar| sidebar.layer != SidebarLayer::Overlay));
    #[cfg(windows)]
    if !graphics_uncovered && let Some(TerminalSessionView::Native(native)) = session.as_ref() {
        native.cover_graphics();
    }

    let mut status_draw: Option<(NativeCloseStatusToken, Rect)> = None;
    let mut prompt = None;
    let mut panel_prompt = None;
    let mut workspace_prompt = None;
    let mut rename = None;
    let mut setup = None;
    #[cfg(windows)]
    let mut worktree = None;
    let mut overlay_refused = false;
    let mut help_page = None;
    let mut board_draw = None;
    #[cfg(windows)]
    let mut graphics_draws = Vec::new();
    let sidebar_status;
    let mut sidebar_status_role = Role::StatusWarning;
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
            let status = native.workspace_close_status().map_or_else(
                || {
                    close.as_ref().map_or_else(
                        || locale.native_status(native),
                        |status| locale.native_close_status(status),
                    )
                },
                |status| locale.workspace_close_status(status),
            );
            secondary_notice = native
                .notice()
                .map(|notice| locale.native_notice(notice))
                .or_else(|| {
                    native
                        .completion_reminder()
                        .map(|reminder| locale.command_completion_reminder(reminder))
                });
            sidebar_status = phase_status.to_owned();
            if native.phase() == NativeDisplayPhase::Ready {
                sidebar_status_role = Role::StatusSuccess;
            }
            status_text = status;
            if secondary_notice.is_none()
                && view.notice().is_none()
                && !close.as_ref().is_some_and(|status| status.resumable())
            {
                secondary_notice = native
                    .close_set_status()
                    .map(|set| locale.close_set_status(set));
            }
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
                    #[cfg(windows)]
                    graphics_uncovered,
                    #[cfg(windows)]
                    &mut graphics_draws,
                );
            }
        }
    }

    #[cfg(windows)]
    if let Some(button) = restore_button {
        frame.render_widget(
            Paragraph::new(locale.sidebar_restore_button()).style(theme.style(Role::HintKey)),
            button,
        );
        hits.record(
            HitLayer::Foreground,
            button,
            HitTarget::SidebarRestoreButton,
        );
    }

    #[cfg(windows)]
    if let Some(rect) =
        menu_button.and_then(|rect| controls::button(frame, rect, &menu_label, theme, false))
    {
        hits.record(HitLayer::Chrome, rect, HitTarget::MenuButton);
    }

    #[cfg(windows)]
    resize_mode::render(
        frame,
        view,
        match session.as_ref() {
            Some(TerminalSessionView::Native(native)) => Some(native),
            _ => None,
        },
        locale,
        theme,
        hits,
    );
    #[cfg(windows)]
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(copy) = native.copy_mode()
    {
        copy_mode::render_controls(frame, view, copy, locale, theme, hits);
    }

    let site_status = SiteStatus {
        text: &sidebar_status,
        role: sidebar_status_role,
    };
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
            #[cfg(windows)]
            if view.machines.available {
                let captured = session.as_ref().is_some_and(|session| match session {
                    TerminalSessionView::Native(native) => {
                        native.agent_board().is_some()
                            || native.close_confirmation().is_some()
                            || native.panel_close_confirmation().is_some()
                            || native.workspace_close_confirmation().is_some()
                            || native.rename_draft().is_some()
                            || native.setup_draft().is_some()
                            || native.worktree().is_some()
                            || native.settings().is_some()
                    }
                    #[cfg(test)]
                    TerminalSessionView::Legacy { .. } => false,
                });
                agent_sidebar::render(
                    frame,
                    sidebar,
                    view,
                    locale,
                    site_status,
                    theme,
                    (!captured).then_some(&mut *hits),
                );
            } else {
                render_sidebar(frame, sidebar, text, site_status, agent_line, theme);
            }
            #[cfg(not(windows))]
            render_sidebar(frame, sidebar, text, site_status, agent_line, theme);
        }

        let overlay = match view.overlay() {
            Some(Overlay::ShortcutHelp) => {
                // The help page is ClientView-owned; the renderer reports the
                // clamped effective page back through the draw receipt so the
                // dispatcher's counter settles on what was actually painted.
                let presented = render_shortcut_help_with_controls(
                    frame,
                    overlay_area(&areas),
                    text,
                    view,
                    view.help_page(),
                    theme,
                    hits,
                );
                help_page = presented.map(|(_, page)| page);
                Some(presented.map(|(area, _page)| area))
            }
            #[cfg(windows)]
            Some(Overlay::WorkbenchNavigator) => Some(navigator::render(
                frame,
                overlay_area(&areas),
                text,
                locale,
                view,
                match session.as_ref() {
                    Some(TerminalSessionView::Native(native)) => Some(native),
                    _ => None,
                },
                theme,
                hits,
            )),
            #[cfg(not(windows))]
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
            && let Some(confirmation) = native.workspace_close_confirmation()
        {
            let (area, token) = workspace_close::render(
                frame,
                locale,
                confirmation,
                overlay_area(&areas),
                theme,
                hits,
            );
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            workspace_prompt = Some(token);
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
                hits,
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
                hits,
            );
            status_draw = status_draw.filter(|(_, area)| !area.intersects(modal.area));
            prompt = None;
            panel_prompt = Some(modal.prompt);
        }
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.rename_draft()
        {
            let (area, token) =
                tab_rename::render(frame, locale, &draft, overlay_area(&areas), theme, hits);
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
                hits,
            );
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            panel_prompt = None;
            rename = None;
            setup = Some(token);
        }
        #[cfg(windows)]
        if let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.worktree()
        {
            let (area, token) =
                worktree_launch::render(frame, locale, draft, overlay_area(&areas), theme, hits);
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            panel_prompt = None;
            rename = None;
            setup = None;
            worktree = Some(token);
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
            if native.workspace_close_confirmation().is_some() {
                workspace_prompt = Some(ModalPresentation::NotPresented);
            }
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
            #[cfg(windows)]
            if native.worktree().is_some() {
                worktree = Some(ModalPresentation::NotPresented);
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
    let read_only = matches!(
        session.as_ref(),
        Some(TerminalSessionView::Native(native))
            if native.controller() == Some(NativeDisplayController::ReadOnly)
    );
    let status_visible = status_overlay_visible(view, session.as_ref())
        || notice.is_some()
        || tier == ViewportTier::TooSmall;
    if status_visible {
        frame.render_widget(Clear, areas.status);
        frame
            .buffer_mut()
            .set_style(areas.status, theme.style(Role::SurfaceChrome));
        // Covered cells never forward a click to the terminal beneath the hint.
        hits.record(HitLayer::Status, areas.status, HitTarget::OverlaySurface);
    }
    let status_area = if status_visible {
        areas.status
    } else {
        Rect::default()
    };
    let visible = render_status(
        frame,
        status_area,
        view,
        locale,
        text,
        &status_text,
        notice,
        zoomed,
        read_only,
        matches!(session.as_ref(), Some(TerminalSessionView::Native(native))
            if native.workspace_close_status().is_none()
                && native.close_status().is_some_and(|status| status.resumable())),
        tier,
        theme,
    );
    #[cfg(windows)]
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(copy) = native.copy_mode()
    {
        copy_mode::render_status(frame, areas.status, copy, locale, notice, theme);
    }
    if view.overlay() == Some(Overlay::ShortcutHelp) && help_page.is_none() {
        // Refused help keeps its page for a later resize, but must never leave
        // an invisible pointer owner without a visible way to dismiss it.
        frame.render_widget(Clear, areas.status);
        frame
            .buffer_mut()
            .set_style(areas.status, theme.style(Role::SurfaceChrome));
        if let Some(close) = controls::close(frame, areas.status, theme) {
            let notice_area = Rect::new(
                areas.status.x,
                areas.status.y,
                close.x.saturating_sub(areas.status.x + 1),
                areas.status.height,
            );
            frame.render_widget(
                Paragraph::new(text.overlay_refused).style(theme.style(Role::TextSecondary)),
                notice_area,
            );
            hits.mark_overlay_active();
            hits.record(
                HitLayer::Overlay,
                close,
                HitTarget::Help {
                    page: view.help_page(),
                    action: crate::tui_pointer::HelpControl::Close,
                },
            );
        }
    }
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(close) = native.close_status()
    {
        status_draw = status_draw.or(visible.map(|area| (close.token(), area)));
    }

    let receipt = NativeDrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
        .with_help_page(help_page)
        .with_panel_close(panel_prompt)
        .with_workspace_close(workspace_prompt)
        .with_rename(rename)
        .with_setup(setup)
        .with_agent_board(board_draw);
    #[cfg(windows)]
    let receipt = receipt.with_worktree(worktree);
    let receipt = receipt.with_attention(match session.as_ref() {
        Some(TerminalSessionView::Native(native))
            if view.overlay().is_none() && {
                #[cfg(windows)]
                {
                    native.popup().is_none()
                        && areas
                            .sidebar
                            .is_none_or(|sidebar| sidebar.layer != SidebarLayer::Overlay)
                }
                #[cfg(not(windows))]
                {
                    true
                }
            } =>
        {
            native.attention_draw(hits.painted_panes())
        }
        _ => None,
    });
    #[cfg(windows)]
    if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(popup) = native.popup()
    {
        render_popup(
            frame,
            native_terminal_area(
                areas.panel,
                areas.tier.shows_panel_frame(),
                native
                    .active_position()
                    .and_then(|p| p.checked_sub(1))
                    .and_then(|i| native.tab(i))
                    .is_some_and(|tab| tab.panel_count() > 1 && !native.zoom_active()),
            ),
            &popup,
            locale,
            theme,
            hits,
        );
    }
    #[cfg(windows)]
    let receipt = if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(menu) = native.action_menu()
    {
        let transferring = menu.transferring;
        let unavailable = menu.unavailable[menu.selected];
        let menu_area = if menu.anchor.is_some() {
            Rect::new(
                area.x,
                area.y,
                area.width,
                areas.status.y.saturating_sub(area.y),
            )
        } else {
            overlay_area(&areas)
        };
        let draw = action_menu::render(frame, menu_area, menu, locale, theme, hits);
        frame.render_widget(Clear, areas.status);
        frame
            .buffer_mut()
            .set_style(areas.status, theme.style(Role::SurfaceChrome));
        frame.render_widget(
            Paragraph::new(unavailable.map_or_else(
                || {
                    locale
                        .menu_status_hint(transferring, usize::from(areas.status.width))
                        .to_owned()
                },
                |reason| locale.menu_disabled_hint(reason, usize::from(areas.status.width)),
            ))
            .style(theme.style(Role::StatusPending)),
            areas.status,
        );
        receipt.with_menu(Some(draw))
    } else {
        receipt
    };
    let receipt = if let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(settings) = native.settings()
    {
        let draw = settings::render(
            frame,
            overlay_area(&areas),
            settings,
            native.settings_pending(),
            locale,
            theme,
            hits,
        );
        frame.render_widget(Clear, areas.status);
        frame
            .buffer_mut()
            .set_style(areas.status, theme.style(Role::SurfaceChrome));
        frame.render_widget(
            Paragraph::new(native.settings_notice().map_or_else(
                || locale.settings_hint(settings, native.settings_pending()),
                |notice| locale.settings_notice(notice).to_owned(),
            ))
            .style(theme.style(Role::StatusInfo)),
            areas.status,
        );
        receipt.with_settings(Some(draw))
    } else {
        receipt
    };
    #[cfg(windows)]
    let receipt = receipt.with_position(if graphics_uncovered && tier != ViewportTier::TooSmall {
        match session.as_ref() {
            Some(TerminalSessionView::Native(native)) => native.position_draw(hits.painted_panes()),
            _ => None,
        }
    } else {
        None
    });
    #[cfg(windows)]
    let receipt = receipt.with_graphics(if graphics_uncovered {
        graphics_draws
    } else {
        Vec::new()
    });
    receipt
}

#[cfg(windows)]
fn render_popup(
    frame: &mut Frame<'_>,
    area: Rect,
    popup: &crate::terminal_session::PopupView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let Some((outer, inner)) =
        crate::terminal_session::popup_geometry(popup.width, popup.height, area)
    else {
        return;
    };
    frame.render_widget(Clear, outer);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(truncate_display(
                popup.title,
                usize::from(outer.width.saturating_sub(5)),
                theme.glyphs.ellipsis,
            ))
            .border_style(theme.style(Role::TextSecondary)),
        outer,
    );
    let close = Rect::new(outer.right().saturating_sub(4), outer.y, 3, 1);
    frame.render_widget(
        Paragraph::new("[×]").style(theme.style(Role::TextPrimary)),
        close,
    );
    if let Some(terminal) = popup
        .projection
        .and_then(crate::terminal_projection::TerminalProjection::frame)
    {
        terminal_cells::render(frame, inner, terminal, !popup.closing, theme);
    } else {
        frame.render_widget(
            Paragraph::new(locale.plugin_popup_pending(popup.closing))
                .style(theme.style(Role::TextSecondary)),
            inner,
        );
    }
    hits.mark_overlay_active();
    hits.record(
        HitLayer::Overlay,
        inner,
        HitTarget::Popup {
            id: popup.id.clone(),
            content: inner,
        },
    );
    hits.record(
        HitLayer::Overlay,
        close,
        HitTarget::PopupClose(popup.id.clone()),
    );
    hits.record(HitLayer::OverlaySurface, outer, HitTarget::OverlaySurface);
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
        terminal_cells::render(frame, area, terminal, view.overlay().is_none(), theme);
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
    #[cfg(windows)] graphics_uncovered: bool,
    #[cfg(windows)] graphics_draws: &mut Vec<crate::terminal_session::PaneGraphicsDraw>,
) {
    let bound_text = locale.shell().bind(client_view);
    let text = bound_text.text();
    let pane_focused = native.close_confirmation().is_none()
        && native.panel_close_confirmation().is_none()
        && native.agent_board().is_none()
        && native.workspace_close_confirmation().is_none()
        && native.rename_draft().is_none()
        && native.setup_draft().is_none();
    #[cfg(windows)]
    let pane_focused = pane_focused
        && !client_view.navigator_active()
        && !client_view.machines.sidebar_navigation_active()
        && !client_view.machines.open
        && native.worktree().is_none()
        && native.popup().is_none()
        && native.action_menu().is_none()
        && native.settings().is_none();
    let input_enabled =
        pane_focused && client_view.overlay().is_none() && !client_view.prefix_active();
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
    let split_visible =
        framed && active_tab.is_some_and(|tab| tab.panel_count() > 1 && !native.zoom_active());
    let content_area = native_terminal_area(panel_area, framed, split_visible);
    let panel_title = text.panel_title.to_owned();
    let outer_header = Rect::new(
        panel_area.x,
        panel_area.y,
        panel_area.width,
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
        if pane_focused {
            theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
        } else {
            theme.style(Role::TextSecondary)
        },
    ));
    #[cfg(windows)]
    let frame_title = active_tab
        .filter(|tab| tab.panel_count() == 1 || native.zoom_active())
        .and_then(|tab| {
            let index = tab.active_panel()?;
            let panel = tab.panel(index)?;
            let presentation = native.selected_presentation(panel);
            let report = native.panel_report(panel);
            let repeats_tab = presentation.custom_label().is_none()
                && presentation.display_label() == tab.tab_label().as_ref()
                && !matches!(report, crate::terminal_session::PanelReportView::Ready(row)
                    if row.metadata.title.is_some());
            let title = if repeats_tab {
                format!(" {} {} ", text.panel_title, index + 1)
            } else {
                panel_reports::title(presentation, index + 1, text.panel_title, report, theme)
            };
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
                if pane_focused {
                    theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
                } else {
                    theme.style(Role::TextSecondary)
                },
                native.panel_unseen_count(panel.panel_id()) > 0,
            ))
        })
        .unwrap_or(frame_title);
    if framed {
        if split_visible {
            frame.render_widget(
                Block::new()
                    .borders(Borders::ALL)
                    .style(theme.style(Role::SurfaceCanvas))
                    .border_style(theme.style(Role::FrameUnfocused))
                    .border_type(theme.corners),
                panel_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(frame_title).style(theme.style(Role::SurfaceChrome)),
                outer_header,
            );
        }
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
                input_enabled,
                pane_focused,
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
            if let Some(panel) = tab.panel(leaf.panel_index) {
                #[cfg(windows)]
                if graphics_uncovered {
                    render_panel_graphics(frame, leaf.content, panel, native, graphics_draws);
                }
                #[cfg(windows)]
                let link_source = native.painted_link_source(panel.panel_id());
                let target = native
                    .pane_ref_for_panel(panel.panel_id())
                    .map(|pane| HitTarget::Pane {
                        pane,
                        content: leaf.content,
                    })
                    .or_else(|| {
                        native
                            .unbound_pane(panel)
                            .map(|selected| HitTarget::Unbound {
                                selected,
                                presentation: None,
                            })
                    });
                if let Some(target) = target {
                    presentation_selector::record_leaf(hits, leaf.leaf, target, selector);
                }
                scrollbar::render(
                    frame,
                    leaf.leaf,
                    leaf.content,
                    panel.panel_id(),
                    pane_focused,
                    native,
                    theme,
                    hits,
                );
                #[cfg(windows)]
                if let Some(source) = link_source {
                    hits.record_link_source(leaf.content, source);
                }
            }
        }
        render_dividers(frame, &projection.dividers, panel_area, theme);
        if pane_focused
            && let Some(leaf) = projection
                .leaves
                .iter()
                .find(|leaf| Some(leaf.panel_index) == tab.active_panel())
        {
            emphasize_pane_boundary(frame, leaf.leaf, panel_area, theme);
        }
        #[cfg(windows)]
        if input_enabled && let Some(source) = native.divider_source() {
            for split in &projection.splits {
                if let Some(target) =
                    crate::tui_layout::DrawnDivider::new(source.clone(), frame.area(), split)
                {
                    hits.record(
                        HitLayer::Foreground,
                        target.divider,
                        HitTarget::Divider(target),
                    );
                }
            }
        }
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
            viewport::render_selected(
                frame,
                content_area,
                surface,
                terminal,
                input_enabled,
                locale,
                theme,
                native
                    .focus_panel_id()
                    .and_then(|panel| native.pane_selection(panel)),
                #[cfg(windows)]
                native.copy_mode().is_some(),
            );
            #[cfg(windows)]
            if input_enabled && let Some(copy) = native.copy_mode() {
                copy_mode::paint_surface(frame, content_area, copy, theme, hits);
            }
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
                #[cfg(windows)]
                if let Some(source) = native
                    .focus_panel_id()
                    .and_then(|panel| native.painted_link_source(panel))
                {
                    hits.record_link_source(content_area, source);
                }
            }
        } else if let Some(terminal) = terminal {
            let show_cursor = input_enabled;
            #[cfg(windows)]
            let show_cursor = show_cursor && native.copy_mode().is_none();
            let tail = native
                .focus_panel_id()
                .and_then(|panel| native.pane_tail_window(panel));
            if let Some((window, rows_before)) = tail {
                terminal_cells::render_tail(
                    frame,
                    content_area,
                    terminal,
                    window,
                    rows_before,
                    show_cursor,
                    theme,
                );
            } else {
                terminal_cells::render(frame, content_area, terminal, show_cursor, theme);
            }
            selection::paint(
                frame,
                content_area,
                tail.map_or(terminal.scrollback_rows, |(window, _)| {
                    window.route.start_row
                }),
                native
                    .focus_panel_id()
                    .and_then(|panel| native.pane_selection(panel)),
                theme.style(Role::SurfaceSelection),
            );
            #[cfg(windows)]
            if input_enabled && let Some(copy) = native.copy_mode() {
                copy_mode::paint_surface(frame, content_area, copy, theme, hits);
            }
            // The title band stays Client chrome. The terminal owns every
            // content cell, including its edges; the hit region carries the
            // pane's exact live incarnation.
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
                #[cfg(windows)]
                if let Some(source) = native
                    .focus_panel_id()
                    .and_then(|panel| native.painted_link_source(panel))
                {
                    hits.record_link_source(content_area, source);
                }
            }
        } else if let Some(retained) = native.focus_panel_id().and_then(|panel| {
            native.retained_pane_frame(panel, content_area.width, content_area.height)
        }) {
            terminal_cells::render(frame, content_area, retained, false, theme);
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
            let panel_status = native
                .workspace_close_status()
                .filter(|_| native.retirement().is_some())
                .map_or(panel_status, |status| locale.workspace_close_status(status));
            frame.render_widget(
                Paragraph::new(panel_status)
                    .style(theme.style(Role::TextSecondary))
                    .wrap(Wrap { trim: true }),
                content_area,
            );
        }
        #[cfg(windows)]
        if let Some(panel) =
            active_tab.and_then(|tab| tab.active_panel().and_then(|i| tab.panel(i)))
            && graphics_uncovered
        {
            render_panel_graphics(frame, content_area, panel, native, graphics_draws);
        }
        if let Some(panel) =
            active_tab.and_then(|tab| tab.active_panel().and_then(|i| tab.panel(i)))
        {
            scrollbar::render(
                frame,
                panel_area,
                content_area,
                panel.panel_id(),
                pane_focused,
                native,
                theme,
                hits,
            );
        }
    }
}

#[cfg(windows)]
fn render_panel_graphics(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &crate::terminal_session::NativePanel,
    native: &NativeSessionView<'_>,
    draws: &mut Vec<crate::terminal_session::PaneGraphicsDraw>,
) {
    if area.is_empty()
        || native
            .retained_pane_frame(panel.panel_id(), area.width, area.height)
            .is_some()
    {
        return;
    }
    let physical = native.physical_graphics(panel, area);
    if physical.is_none()
        && let Some(canvas) = native.panel_graphics(panel)
    {
        pane_graphics::render(frame, area, canvas, native.graphics_cell_pixels().as_ref());
    }
    if let Some(draw) = native.graphics_draw(panel) {
        draws.push(match physical {
            Some(choice) => draw.physical(choice.revision),
            None => draw,
        });
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
    let title = location.map_or_else(String::new, |location| {
        format!(
            "{} ",
            truncate_display(
                location,
                usize::from(tabs_area.width / 2).saturating_sub(1),
                theme.glyphs.ellipsis
            )
        )
    });
    let title_width = u16::try_from(Line::raw(title.as_str()).width()).unwrap_or(u16::MAX);
    let (strip, title_area) = if title_width > 0 && title_width < tabs_area.width {
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
    #[cfg(windows)]
    let create_target = native.tab_create_target().filter(|_| strip.width >= 7);
    #[cfg(windows)]
    let create_width = if create_target.is_some() { 4 } else { 0 };
    #[cfg(not(windows))]
    let create_width = 0;
    let tab_area = Rect::new(strip.x, strip.y, strip.width - create_width, strip.height);
    #[cfg(windows)]
    let mut used = 0;
    #[cfg(not(windows))]
    tabs::render_compact(
        frame,
        tab_area,
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
    #[cfg(windows)]
    {
        let selected = native.selected_tab_ids();
        let displayed = (0..native.tab_count())
            .filter_map(|index| {
                let tab = native.tab(index)?;
                Some(tabs::groups::GroupTabDisplay {
                    selected: selected.len() > 1 && selected.contains(tab.tab_id()),
                    tab: tabs::TabDisplay {
                        position: tab.position(),
                        active: tab.active(),
                        label: tabs::label_with_metadata(
                            tab.tab_label(),
                            tab.pinned().then_some(text.pinned_tab),
                            None,
                        ),
                    },
                    group: tab.group(),
                    collapsed: tab
                        .group()
                        .is_some_and(|group| native.group_collapsed(&group.group_id)),
                })
            })
            .collect::<Vec<_>>();
        tabs::groups::render(
            frame,
            tab_area,
            &displayed,
            theme,
            text.unnamed_tab_group,
            &mut |index, group_header, x, width| {
                used = x.saturating_add(width);
                if let Some(tab) = native.tab(index) {
                    let target = if group_header {
                        let Some(group) = tab.group() else {
                            return;
                        };
                        HitTarget::TabGroup {
                            tab: tab.tab_id().clone(),
                            group: group.group_id.clone(),
                        }
                    } else {
                        HitTarget::Tab(tab.tab_id().clone())
                    };
                    hits.record(
                        HitLayer::Foreground,
                        Rect::new(
                            strip.x.saturating_add(u16::try_from(x).unwrap_or(u16::MAX)),
                            strip.y,
                            u16::try_from(width).unwrap_or(u16::MAX),
                            strip.height,
                        ),
                        target,
                    );
                }
            },
        );
    }
    #[cfg(windows)]
    if let Some(target) = create_target {
        let offset = u16::try_from(used)
            .unwrap_or(tab_area.width)
            .saturating_add(u16::from(used > 0))
            .min(strip.width - 3);
        let create = Rect::new(strip.x + offset, strip.y, 3, 1);
        frame.render_widget(
            Paragraph::new(" + ").style(theme.style(Role::HintLabel)),
            create,
        );
        hits.record(HitLayer::Foreground, create, HitTarget::TabCreate(target));
    }
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
    pane_focused: bool,
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
    frame
        .buffer_mut()
        .set_style(header_area, theme.style(Role::SurfaceChrome));
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
    let title_style = if pane_focused && active {
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
        panel.is_some_and(|panel| native.panel_unseen_count(panel.panel_id()) > 0),
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
        #[cfg(windows)]
        let copy_active = active && native.copy_mode().is_some();
        viewport::render_selected(
            frame,
            content,
            surface,
            terminal,
            input_enabled && active,
            locale,
            theme,
            panel.and_then(|panel| native.pane_selection(panel.panel_id())),
            #[cfg(windows)]
            copy_active,
        );
    } else if let Some(terminal) = terminal {
        let show_cursor = input_enabled && active;
        #[cfg(windows)]
        let show_cursor = show_cursor && native.copy_mode().is_none();
        let tail = panel.and_then(|panel| native.pane_tail_window(panel.panel_id()));
        if let Some((window, rows_before)) = tail {
            terminal_cells::render_tail(
                frame,
                content,
                terminal,
                window,
                rows_before,
                show_cursor,
                theme,
            );
        } else {
            terminal_cells::render(frame, content, terminal, show_cursor, theme);
        }
        selection::paint(
            frame,
            content,
            tail.map_or(terminal.scrollback_rows, |(window, _)| {
                window.route.start_row
            }),
            panel.and_then(|panel| native.pane_selection(panel.panel_id())),
            theme.style(Role::SurfaceSelection),
        );
    } else if let Some(retained) = panel.and_then(|panel| {
        native.retained_pane_frame(panel.panel_id(), content.width, content.height)
    }) {
        terminal_cells::render(frame, content, retained, false, theme);
    } else {
        let status = if panel.is_some_and(|p| native.panel_is_unbound(p.panel_id())) {
            locale.native_unbound_terminal()
        } else if matches!(
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
    #[cfg(windows)]
    if active
        && input_enabled
        && let Some(copy) = native.copy_mode()
    {
        copy_mode::paint_surface(frame, content, copy, theme, hits);
    }
    selector.map(|s| s.area)
}

fn render_dividers(frame: &mut Frame<'_>, dividers: &[Rect], panel: Rect, theme: &Theme) {
    let mut cells = std::collections::BTreeSet::new();
    for divider in dividers {
        for y in divider.y..divider.y.saturating_add(divider.height) {
            for x in divider.x..divider.x.saturating_add(divider.width) {
                cells.insert((x, y));
            }
        }
    }
    if cells.is_empty() || panel.is_empty() {
        return;
    }
    let mut boundary = std::collections::BTreeSet::new();
    for x in panel.x..panel.right() {
        boundary.insert((x, panel.y));
        boundary.insert((x, panel.bottom() - 1));
    }
    for y in panel.y..panel.bottom() {
        boundary.insert((panel.x, y));
        boundary.insert((panel.right() - 1, y));
    }
    let neighbors = |x: u16, y: u16| {
        [
            (x, y.saturating_sub(1)),
            (x, y.saturating_add(1)),
            (x.saturating_sub(1), y),
            (x.saturating_add(1), y),
        ]
    };
    for &(x, y) in cells.iter().chain(
        boundary
            .iter()
            .filter(|&&(x, y)| neighbors(x, y).iter().any(|point| cells.contains(point))),
    ) {
        let [up, down, left, right] = neighbors(x, y)
            .map(|point| point != (x, y) && (cells.contains(&point) || boundary.contains(&point)));
        let glyph = crate::tui_layout::divider_glyph(up, down, left, right);
        frame.buffer_mut().set_string(
            x,
            y,
            glyph.to_string(),
            theme.style(if boundary.contains(&(x, y)) {
                Role::FrameUnfocused
            } else {
                Role::FrameDivider
            }),
        );
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

fn status_overlay_visible(view: &ClientView, session: Option<&TerminalSessionView<'_>>) -> bool {
    let visible = view.prefix_active()
        || view.overlay().is_some()
        || view.notice().is_some()
        || view.pointer_released();
    #[cfg(windows)]
    let visible = visible
        || view.navigator_active()
        || view.machines.sidebar_navigation_active()
        || view.machines.open
        || view.resize_mode()
        || view.resize_pending;
    visible
        || session.is_some_and(|session| match session {
            TerminalSessionView::Native(native) => {
                let visible = native.phase() != NativeDisplayPhase::Ready
                    || native.notice().is_some()
                    || native.takeover_status().is_some()
                    || native.close_status().is_some()
                    || native.workspace_close_status().is_some()
                    || native.close_set_status().is_some()
                    || native.close_confirmation().is_some()
                    || native.panel_close_confirmation().is_some()
                    || native.workspace_close_confirmation().is_some()
                    || native.rename_draft().is_some()
                    || native.setup_draft().is_some()
                    || native.agent_board().is_some();
                #[cfg(windows)]
                let visible = visible
                    || native.copy_mode().is_some()
                    || native.settings().is_some()
                    || native.action_menu().is_some()
                    || native.worktree().is_some();
                visible
            }
            #[cfg(test)]
            TerminalSessionView::Legacy { .. } => false,
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
    read_only: bool,
    recovery_status: bool,
    tier: ViewportTier,
    theme: &Theme,
) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }
    // When the status surface is requested, include control loss and its takeover key.
    let notice = view
        .notice()
        .filter(|notice| !(read_only && *notice == Notice::TerminalInputUnavailable));
    let primary = notice.map_or(status, |notice| locale.shell_notice(Some(notice)));
    // Native status can already carry this notice and a delimited recovery fact.
    // Compare the primary actually painted, including any Client notice override.
    let secondary_notice = secondary_notice.filter(|notice| {
        !view.prefix_active()
            && !primary
                .strip_prefix(*notice)
                .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(" · "))
    });
    let (label, style) = if let Some(notice) = secondary_notice {
        (
            locale.shell().bound_hint(view, notice),
            theme.style(Role::StatusWarning),
        )
    } else if view.prefix_active() {
        let full = format!("{} · {}", text.prefix_active, text.prefix_cancel_hint);
        let compact = format!("{} · Esc", text.prefix_active);
        let label = [full, compact]
            .into_iter()
            .find(|hint| display_width(hint) <= usize::from(area.width))
            .unwrap_or_else(|| text.prefix_active.to_owned());
        (
            label,
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
    #[cfg(windows)]
    let (label, style) = if view.navigator_active() {
        (
            locale.navigator_hint(false),
            theme.style(Role::StatusPending),
        )
    } else if view.machines.sidebar_navigation_active() {
        let full = locale.sidebar_navigation_hint(true);
        (
            if tier == ViewportTier::Full && display_width(&full) <= usize::from(area.width) {
                full
            } else {
                locale.sidebar_navigation_hint(false)
            },
            theme
                .style(Role::TextInverse)
                .bg(theme.color(Role::AccentFocus))
                .add_modifier(Modifier::BOLD),
        )
    } else if view.resize_mode() {
        (
            locale
                .resize_hint(tier == ViewportTier::Full, view.resize_pending)
                .to_owned(),
            theme
                .style(Role::TextInverse)
                .bg(theme.color(Role::AccentFocus))
                .add_modifier(Modifier::BOLD),
        )
    } else if view.resize_pending {
        (
            locale.resize_pending_hint().to_owned(),
            theme.style(Role::StatusPending),
        )
    } else {
        (label, style)
    };
    let mut right_width = u16::try_from(Line::raw(label.as_str()).width())
        .unwrap_or(u16::MAX)
        .max(24)
        .min(area.width);
    // A recoverable Close must be fully presented before its confirmation may open.
    // Drop only ordinary hints when they would hide that status; input owners and newer
    // notices retain priority. The exact draw receipt below still rejects clipped facts.
    let ordinary_hints = !view.prefix_active() && secondary_notice.is_none();
    #[cfg(windows)]
    let ordinary_hints = ordinary_hints
        && !view.navigator_active()
        && !view.machines.sidebar_navigation_active()
        && !view.resize_mode()
        && !view.resize_pending;
    if recovery_status
        && ordinary_hints
        && display_width(&locale.shell().bound_hint(view, primary))
            > usize::from(area.width.saturating_sub(right_width))
    {
        right_width = 0;
    }
    let [status_area, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).areas(area);
    #[cfg(windows)]
    let status_text_area = if view.machines.sidebar_navigation_active() {
        Rect {
            width: status_area.width.saturating_sub(1),
            ..status_area
        }
    } else {
        status_area
    };
    #[cfg(not(windows))]
    let status_text_area = status_area;
    let status_label = locale.shell().bound_hint(view, primary);
    // A released pointer is a persistent mode, not a transient notice: it
    // rides the status label on every draw until capture re-arms.
    let status_label = if view.pointer_released() {
        format!("{status_label} · {}", text.pointer_released)
    } else {
        status_label
    };
    // An active zoom also describes the focused-only pane set when context is shown.
    let status_label = if zoomed {
        format!("{status_label} · {}", text.zoom_badge)
    } else {
        status_label
    };
    let fits =
        view.notice().is_none() && tab_close::unwrapped_text_fits(&status_label, status_text_area);
    let status_label = if status_text_area.width < status_area.width {
        truncate_display(
            &status_label,
            usize::from(status_text_area.width),
            theme.glyphs.ellipsis,
        )
    } else {
        status_label
    };
    frame.render_widget(
        Paragraph::new(status_label).style(theme.style(Role::TextSecondary)),
        status_area,
    );
    if status_text_area.width < status_area.width {
        // Keep the separator inside the existing status and receipt rectangle.
        frame.buffer_mut().set_string(
            status_text_area.right(),
            status_area.y,
            " ",
            theme.style(Role::TextSecondary),
        );
    }
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
    backend_status: SiteStatus<'_>,
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
        ListItem::new(backend_status.line(inner_width, theme)),
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
                    theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
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
#[cfg(test)]
fn render_shortcut_help(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    view: &ClientView,
    page: usize,
    theme: &Theme,
) -> Option<(Rect, usize)> {
    render_shortcut_help_with_controls(
        frame,
        area,
        text,
        view,
        page,
        theme,
        &mut HitMapBuilder::new(),
    )
}

fn render_shortcut_help_with_controls(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    view: &ClientView,
    page: usize,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<(Rect, usize)> {
    let rows = text.configured_shortcut_rows(view);
    let height = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(HELP_FOOTER_ROWS)
        .saturating_add(2);
    let area = place_overlay(area, 76, height)?;
    let inner = overlay_block(text.shortcut_title, theme, false).inner(area);
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
    let block = overlay_block(text.shortcut_title, theme, false);
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
    if let Some(rect) = controls::close(frame, area, theme) {
        hits.record(
            HitLayer::Overlay,
            rect,
            HitTarget::Help {
                page,
                action: crate::tui_pointer::HelpControl::Close,
            },
        );
    }
    // Page controls live on the title frame; keyboard instructions retain their rows.
    let controls_area = Rect::new(area.right().saturating_sub(14), area.y, 9, 1);
    if area.width >= 40
        && let Some(rects) = controls::pair(frame, controls_area, ["[<]", "[>]"], theme)
    {
        for (rect, action, enabled) in [
            (
                rects[0],
                crate::tui_pointer::HelpControl::Previous,
                page > 0,
            ),
            (
                rects[1],
                crate::tui_pointer::HelpControl::Next,
                page + 1 < total_pages,
            ),
        ] {
            if enabled {
                hits.record(HitLayer::Overlay, rect, HitTarget::Help { page, action });
            } else {
                frame
                    .buffer_mut()
                    .set_style(rect, theme.style(Role::TextDisabled));
            }
        }
    }
    hits.record(
        HitLayer::OverlaySurface,
        rows_area,
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Help {
            page,
            last: total_pages - 1,
        }),
    );
    Some((area, page))
}

#[cfg(not(windows))]
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
        .block(overlay_block(text.navigator_title, theme, false)),
        area,
    );
    Some(area)
}

#[cfg(not(windows))]
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
    let block = overlay_block(text.navigator_title, theme, false);
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
        None,
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
        None,
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

/// Style only the already-drawn shared and outer boundaries around one leaf.
/// The canonical content rectangle and its pointer/wire geometry stay intact.
fn emphasize_pane_boundary(frame: &mut Frame<'_>, leaf: Rect, panel: Rect, theme: &Theme) {
    let outline = Rect::new(
        leaf.x.saturating_sub(1),
        leaf.y.saturating_sub(1),
        leaf.width.saturating_add(2),
        leaf.height.saturating_add(2),
    )
    .intersection(panel);
    if outline.is_empty() {
        return;
    }
    let style = theme.style(Role::FrameFocused);
    for edge in [
        Rect::new(outline.x, outline.y, outline.width, 1),
        Rect::new(outline.x, outline.bottom() - 1, outline.width, 1),
        Rect::new(outline.x, outline.y, 1, outline.height),
        Rect::new(outline.right() - 1, outline.y, 1, outline.height),
    ] {
        frame.buffer_mut().set_style(edge, style);
    }
}

/// Every overlay shares one opaque theme surface and one frame treatment.
fn overlay_surface(theme: &Theme, focused: bool, padding: Padding) -> Block<'static> {
    Block::new()
        .borders(Borders::ALL)
        .border_type(theme.corners)
        .border_style(theme.style(if focused {
            Role::FrameFocused
        } else {
            Role::FrameUnfocused
        }))
        .style(theme.style(Role::SurfaceOverlay))
        .padding(padding)
        .title_style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD))
}

fn overlay_block(title: &'static str, theme: &Theme, focused: bool) -> Block<'static> {
    overlay_surface(theme, focused, Padding::horizontal(2)).title(Span::styled(
        format!(" {title} "),
        theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
    ))
}

#[cfg(test)]
mod tests;
