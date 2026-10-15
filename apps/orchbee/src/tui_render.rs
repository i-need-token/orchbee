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
    bindings: &crate::client_view::Bindings,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<crate::action_menu::MenuDraw> {
    action_menu::render(frame, content, menu, bindings, locale, theme, hits)
}
#[cfg(windows)]
mod resize_mode;
mod settings;

#[cfg(test)]
pub(crate) fn render_settings_for_test(
    frame: &mut Frame<'_>,
    content: Rect,
    view: crate::native_settings::View<'_>,
    pending: bool,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<crate::native_settings::SettingsDraw> {
    settings::render(frame, content, view, pending, locale, theme, hits)
}

use crate::{
    client_view::{ClientView, Overlay},
    localization::{Locale, RecoveryVerb, ShellText},
    terminal_session::{
        AgentBoardDraw, ModalPresentation, NativeCloseStatusToken, NativeDisplayController,
        NativeDisplayPhase, NativeDrawReceipt, NativeProjectionStatus, NativeRefreshStatus,
        NativeSessionView, NativeTabView, TerminalSessionView,
    },
    terminal_text::{display_width, truncate_display},
    tui_composition::{
        CaptureOwner, ClientCaptureClaims, NativeCaptureClaims, NativeCaptureOwner,
        client_chrome_overlay_allowed, project_capture,
    },
    tui_layout::{
        ShellAreas, SidebarArea, SidebarLayer, ViewportTier, native_terminal_area, overlay_area,
        place_overlay, project_shell_areas, terminal_area,
    },
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{Role, Theme},
};

#[cfg(windows)]
use crate::tui_pointer::GlobalChromeAction;

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
mod help;
mod modal;
#[cfg(windows)]
mod navigator;
#[cfg(windows)]
mod pane_graphics;
mod panel_close;
#[cfg(windows)]
mod panel_reports;
mod presentation_selector;
mod recovery;
mod selection;
#[cfg(windows)]
pub(crate) use agent_sidebar::render_width_preview;
#[cfg(windows)]
pub(crate) use divider_resize::render_divider_preview;
#[cfg(windows)]
pub(crate) use tabs::render_tab_drag_preview;
#[cfg(windows)]
mod sidebar_reorder;
#[cfg(windows)]
pub(crate) use sidebar_reorder::render_sidebar_reorder_preview;
mod scrollbar;
#[cfg(windows)]
mod sidebar_text;
mod status_line;
mod tab_close;
mod tab_rename;
mod tabs;
mod terminal_cells;
#[cfg(test)]
pub(crate) mod test_capture;
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
    let client_claims = ClientCaptureClaims::default();
    #[cfg(windows)]
    let client_claims = client_claims
        .with_machine(view.machines.open)
        .with_resize(view.resize_mode());
    let native_claims = match session.as_ref() {
        Some(TerminalSessionView::Native(native)) => native.capture_claims(),
        _ => NativeCaptureClaims::default(),
    };
    let capture_owner = project_capture(client_claims, native_claims);
    let native_top = |owner| capture_owner == Some(CaptureOwner::Native(owner));
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
    let chrome_action_state = controls::InteractionState::new(
        controls::FocusState::Unfocused,
        controls::PointerPresentation::Rest,
        controls::NavigationState::None,
        controls::CurrentState::NotCurrent,
        controls::CheckedState::Unchecked,
        controls::Availability::Enabled,
        controls::OperationState::Idle,
    );
    #[cfg(windows)]
    let restore_label = locale.sidebar_restore_label();
    #[cfg(windows)]
    let restore_button =
        if view.machines.available && areas.sidebar.is_none() && !areas.tabs.is_empty() {
            let action = controls::ActionSpec::new(
                restore_label,
                controls::ActionKind::Secondary,
                controls::ControlSurface::Chrome,
                chrome_action_state,
            );
            controls::action_rect(area, areas.tabs, action, theme).inspect(|button| {
                areas.tabs.x = button.right();
                areas.tabs.width = areas.tabs.width.saturating_sub(button.width);
            })
        } else {
            None
        };
    #[cfg(windows)]
    let menu_label = locale.menu_title();
    #[cfg(windows)]
    let menu_button = if areas.tier == ViewportTier::TooSmall {
        None
    } else {
        let action = controls::ActionSpec::new(
            menu_label,
            controls::ActionKind::Secondary,
            controls::ControlSurface::Chrome,
            chrome_action_state,
        );
        let measurement = Rect::new(0, 0, u16::MAX, 1);
        controls::action_rect(measurement, measurement, action, theme).and_then(|measured| {
            let width = measured.width;
            if areas.tabs.width >= width.saturating_add(2) && areas.tabs.height > 0 {
                let offered = Rect::new(areas.tabs.right() - width, areas.tabs.y, width, 1);
                let button = controls::action_rect(area, offered, action, theme)?;
                areas.tabs.width = areas.tabs.width.saturating_sub(width.saturating_add(1));
                Some(button)
            } else if areas.tabs.is_empty()
                && areas.workbench.width >= width.saturating_add(2)
                && areas.workbench.height > 1
            {
                let offered = Rect::new(
                    areas.workbench.right() - width - 1,
                    areas.workbench.y,
                    width,
                    1,
                );
                controls::action_rect(area, offered, action, theme)
            } else {
                None
            }
        })
    };
    let tier = areas.tier;
    #[cfg(windows)]
    if tier != ViewportTier::TooSmall {
        hits.record_action_menu_source(match session.as_ref() {
            Some(TerminalSessionView::Native(native)) => native.action_menu_source(),
            _ => None,
        });
    }
    #[cfg(windows)]
    let graphics_uncovered = matches!(session.as_ref(), Some(TerminalSessionView::Native(_))
        if tier != ViewportTier::TooSmall
            && capture_owner.is_none() && !status_overlay_visible(view, session.as_ref(), capture_owner) && view.overlay().is_none()
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
    let mut help_presented = false;
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
                    capture_owner.is_none(),
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
        let target = InteractionTarget::GlobalChrome(GlobalChromeAction::RestoreSidebar);
        let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
        let pointer =
            match hits.projected_pointer(HitLayer::Foreground, button, button, &target, spec) {
                ProjectedPointer::Rest => controls::PointerPresentation::Rest,
                ProjectedPointer::Hovered => controls::PointerPresentation::Hovered,
                ProjectedPointer::Pressed => controls::PointerPresentation::Pressed,
            };
        let painted = controls::render_action(
            frame,
            button,
            controls::ActionSpec::new(
                restore_label,
                controls::ActionKind::Secondary,
                controls::ControlSurface::Chrome,
                chrome_action_state.with_pointer(pointer),
            ),
            theme,
        );
        debug_assert_eq!(painted, Some(button));
        if painted == Some(button) {
            hits.record_interaction(HitLayer::Foreground, button, button, target, spec);
        }
    }

    #[cfg(windows)]
    if let Some(button) = menu_button {
        let target = InteractionTarget::GlobalChrome(GlobalChromeAction::OpenMenu);
        let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
        let pointer = match hits.projected_pointer(HitLayer::Chrome, button, button, &target, spec)
        {
            ProjectedPointer::Rest => controls::PointerPresentation::Rest,
            ProjectedPointer::Hovered => controls::PointerPresentation::Hovered,
            ProjectedPointer::Pressed => controls::PointerPresentation::Pressed,
        };
        let painted = controls::render_action(
            frame,
            button,
            controls::ActionSpec::new(
                menu_label,
                controls::ActionKind::Secondary,
                controls::ControlSurface::Chrome,
                chrome_action_state.with_pointer(pointer),
            ),
            theme,
        );
        debug_assert_eq!(painted, Some(button));
        if painted == Some(button) {
            hits.record_interaction(HitLayer::Chrome, button, button, target, spec);
        }
    }

    #[cfg(windows)]
    if tier != ViewportTier::TooSmall && capture_owner == Some(CaptureOwner::Resize) {
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
    }
    #[cfg(windows)]
    if tier != ViewportTier::TooSmall
        && capture_owner == Some(CaptureOwner::Copy)
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
                let captured = capture_owner.is_some();
                agent_sidebar::render(
                    frame,
                    sidebar,
                    agent_sidebar::RenderContext {
                        view,
                        locale,
                        backend_status: site_status,
                        theme,
                        input: if captured
                            || view
                                .overlay()
                                .is_some_and(|overlay| overlay != Overlay::BusinessSidebar)
                        {
                            agent_sidebar::SidebarInput::Suppressed
                        } else if view.machines.sidebar_navigation_active() {
                            agent_sidebar::SidebarInput::Navigation
                        } else {
                            agent_sidebar::SidebarInput::Terminal
                        },
                    },
                    (!captured).then_some(&mut *hits),
                );
            } else {
                render_sidebar(frame, sidebar, text, site_status, agent_line, theme);
            }
            #[cfg(not(windows))]
            render_sidebar(frame, sidebar, text, site_status, agent_line, theme);
        }

        let overlay = match view
            .overlay()
            .filter(|_| client_chrome_overlay_allowed(capture_owner))
        {
            Some(Overlay::ShortcutHelp) => {
                let presented = view.help_draw_candidate().and_then(|candidate| {
                    help::render(
                        frame,
                        overlay_area(&areas),
                        help::RenderContext::new(text, locale, view, candidate),
                        theme,
                        hits,
                    )
                });
                help_presented = presented.is_some();
                Some(presented)
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
        if native_top(NativeCaptureOwner::AgentBoard)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
            board_draw = Some(draw);
        }

        if native_top(NativeCaptureOwner::WorkspaceClose)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
        if native_top(NativeCaptureOwner::TabClose)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
        if native_top(NativeCaptureOwner::PanelClose)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
        if native_top(NativeCaptureOwner::Rename)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
            && let Some(draft) = native.rename_draft()
        {
            let (area, token) =
                tab_rename::render(frame, locale, &draft, overlay_area(&areas), theme, hits);
            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            panel_prompt = None;
            rename = Some(token);
        }
        if native_top(NativeCaptureOwner::Setup)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
        if native_top(NativeCaptureOwner::Worktree)
            && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
        // Geometry is a draw fact, not consent or cancellation. The owner
        // refuses an incarnation that has never painted, or suspends a
        // previously presented task while revoking its current draw proof.
        if matches!(session.as_ref(), Some(TerminalSessionView::Native(_))) {
            if native_top(NativeCaptureOwner::WorkspaceClose) {
                workspace_prompt = Some(ModalPresentation::NotPresented);
            }
            if native_top(NativeCaptureOwner::TabClose) {
                prompt = Some(ModalPresentation::NotPresented);
            }
            if native_top(NativeCaptureOwner::PanelClose) {
                panel_prompt = Some(ModalPresentation::NotPresented);
            }
            if native_top(NativeCaptureOwner::Rename) {
                rename = Some(ModalPresentation::NotPresented);
            }
            if native_top(NativeCaptureOwner::Setup) {
                setup = Some(ModalPresentation::NotPresented);
            }
            #[cfg(windows)]
            if native_top(NativeCaptureOwner::Worktree) {
                worktree = Some(ModalPresentation::NotPresented);
            }
        }
    }

    // An open board that could not paint at all (too-small viewport)
    // still reports its verdict — input capture never settles on an
    // unpainted surface.
    if native_top(NativeCaptureOwner::AgentBoard)
        && board_draw.is_none()
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(board) = native.agent_board()
    {
        board_draw = Some(AgentBoardDraw::NotPresented(board.episode()));
    }

    // Only the actual top owner's evaluated geometry can explain this frame.
    // Covered lower owners receive no receipt and publish no recovery chrome.
    enum NativeRecoveryStatus {
        LocalReturn {
            phase: crate::tui_presentation::PresentationPhase,
            verb: RecoveryVerb,
        },
        #[cfg(windows)]
        Plain(&'static str),
    }
    let native_recovery =
        if let (Some(CaptureOwner::Native(owner)), Some(TerminalSessionView::Native(native))) =
            (capture_owner, session.as_ref())
        {
            match owner {
                NativeCaptureOwner::WorkspaceClose
                    if matches!(
                        workspace_prompt.as_ref(),
                        Some(ModalPresentation::NotPresented)
                    ) =>
                {
                    native
                        .workspace_close_confirmation()
                        .map(|view| (view.presentation(), RecoveryVerb::Cancel))
                }
                NativeCaptureOwner::TabClose
                    if matches!(prompt.as_ref(), Some(ModalPresentation::NotPresented)) =>
                {
                    native
                        .close_confirmation()
                        .map(|view| (view.presentation(), RecoveryVerb::Cancel))
                }
                NativeCaptureOwner::PanelClose
                    if matches!(panel_prompt.as_ref(), Some(ModalPresentation::NotPresented)) =>
                {
                    native
                        .panel_close_confirmation()
                        .map(|view| (view.presentation(), RecoveryVerb::Cancel))
                }
                NativeCaptureOwner::Rename
                    if matches!(rename.as_ref(), Some(ModalPresentation::NotPresented)) =>
                {
                    native
                        .rename_draft()
                        .map(|view| (view.presentation(), RecoveryVerb::Cancel))
                }
                NativeCaptureOwner::Setup
                    if matches!(setup.as_ref(), Some(ModalPresentation::NotPresented)) =>
                {
                    native
                        .setup_draft()
                        .map(|view| (view.presentation(), RecoveryVerb::Cancel))
                }
                #[cfg(windows)]
                NativeCaptureOwner::Worktree
                    if matches!(worktree.as_ref(), Some(ModalPresentation::NotPresented)) =>
                {
                    native.worktree().map(|view| {
                        (
                            view.presentation(),
                            match view.stage() {
                                crate::terminal_session::WorktreeStage::Waiting => {
                                    RecoveryVerb::Hide
                                }
                                crate::terminal_session::WorktreeStage::Result => {
                                    RecoveryVerb::Close
                                }
                                _ => RecoveryVerb::Cancel,
                            },
                        )
                    })
                }
                NativeCaptureOwner::AgentBoard
                    if board_draw.as_ref().is_some_and(|draw| !draw.is_presented()) =>
                {
                    native
                        .agent_board()
                        .map(|view| (view.presentation(), RecoveryVerb::Close))
                }
                _ => None,
            }
        } else {
            None
        };
    let native_recovery =
        native_recovery.map(|(phase, verb)| NativeRecoveryStatus::LocalReturn { phase, verb });
    #[cfg(windows)]
    let native_recovery = if tier == ViewportTier::TooSmall
        && capture_owner == Some(CaptureOwner::Native(NativeCaptureOwner::Popup))
        && matches!(session.as_ref(), Some(TerminalSessionView::Native(native)) if native.popup().is_some())
    {
        Some(NativeRecoveryStatus::Plain(
            locale.plugin_popup_resize_recovery(),
        ))
    } else {
        native_recovery
    };

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
    let status_visible = status_overlay_visible(view, session.as_ref(), capture_owner)
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
        capture_owner,
        locale,
        &status_text,
        notice,
        zoomed,
        read_only,
        matches!(session.as_ref(), Some(TerminalSessionView::Native(native))
            if native.workspace_close_status().is_none()
                && native.close_status().is_some_and(|status| status.resumable())),
        tier,
        theme,
        session.as_ref(),
    );
    if let Some(recovery) = native_recovery {
        match recovery {
            NativeRecoveryStatus::LocalReturn { phase, verb } => {
                let retained = phase != crate::tui_presentation::PresentationPhase::Pending;
                recovery::render(
                    frame,
                    areas.status,
                    locale.presentation_notice(retained),
                    retained.then_some(locale.recovery_verb(verb)),
                    theme,
                );
            }
            #[cfg(windows)]
            NativeRecoveryStatus::Plain(notice) => {
                recovery::render(frame, areas.status, notice, None, theme);
            }
        }
    }
    #[cfg(windows)]
    if capture_owner == Some(CaptureOwner::Copy)
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(copy) = native.copy_mode()
    {
        copy_mode::render_status(frame, areas.status, copy, locale, notice, theme);
    }
    if tier != ViewportTier::TooSmall
        && client_chrome_overlay_allowed(capture_owner)
        && view.overlay() == Some(Overlay::ShortcutHelp)
        && !help_presented
        && let Some(candidate) = view.help_draw_candidate()
    {
        // The requested page remains owner state for a later resize. Only a
        // complete fixed-width close publishes refused-surface authority.
        let _ = help::render_refused(frame, areas.status, text, candidate, theme, hits);
    }
    if capture_owner.is_none()
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(close) = native.close_status()
    {
        status_draw = status_draw.or(visible.map(|area| (close.token(), area)));
    }

    let receipt = NativeDrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
        .with_panel_close(panel_prompt)
        .with_workspace_close(workspace_prompt)
        .with_rename(rename)
        .with_setup(setup)
        .with_agent_board(board_draw);
    #[cfg(windows)]
    let receipt = receipt.with_worktree(worktree);
    let receipt = receipt.with_attention(match session.as_ref() {
        Some(TerminalSessionView::Native(native))
            if capture_owner.is_none() && view.overlay().is_none() && {
                #[cfg(windows)]
                {
                    areas
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
    if tier != ViewportTier::TooSmall
        && native_top(NativeCaptureOwner::Popup)
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
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
    let receipt = if native_top(NativeCaptureOwner::ActionMenu)
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(menu) = native.action_menu()
    {
        let recovery = native.action_menu_input_context().is_some_and(|context| {
            context.phase() != crate::tui_presentation::PresentationPhase::Pending
        });
        let local_return = if menu.has_parent {
            locale.menu_back_button()
        } else {
            locale.menu_close_button()
        };
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
        let draw = if tier == ViewportTier::TooSmall {
            ModalPresentation::NotPresented
        } else {
            action_menu::render(frame, menu_area, menu, view.bindings(), locale, theme, hits)
        };
        frame.render_widget(Clear, areas.status);
        frame
            .buffer_mut()
            .set_style(areas.status, theme.style(Role::SurfaceChrome));
        if matches!(draw, ModalPresentation::NotPresented) {
            recovery::render(
                frame,
                areas.status,
                locale.presentation_notice(recovery),
                recovery.then_some(local_return),
                theme,
            );
        }
        receipt.with_menu(Some(draw))
    } else {
        receipt
    };
    let receipt = if native_top(NativeCaptureOwner::Settings)
        && let Some(TerminalSessionView::Native(native)) = session.as_ref()
        && let Some(settings) = native.settings()
    {
        let local_return = crate::native_settings::footer_bindings(settings)
            .first()
            .map(|hint| locale.settings_hint_label(hint.label()));
        let retained = native.settings_presentation()
            != Some(crate::tui_presentation::PresentationPhase::Pending);
        let draw = if tier == ViewportTier::TooSmall {
            ModalPresentation::NotPresented
        } else {
            settings::render(
                frame,
                Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1)),
                settings,
                native.settings_pending(),
                locale,
                theme,
                hits,
            )
        };
        frame.render_widget(Clear, areas.status);
        frame
            .buffer_mut()
            .set_style(areas.status, theme.style(Role::SurfaceChrome));
        let notice = if matches!(draw, ModalPresentation::NotPresented) {
            // Classify this frame from the owner's pre-settlement incarnation.
            // A new attempt is refused; an admitted task can resume later.
            Some(
                if native.settings_presentation()
                    == Some(crate::tui_presentation::PresentationPhase::Pending)
                {
                    crate::terminal_session::SettingsNotice::PresentationUnavailable
                } else {
                    crate::terminal_session::SettingsNotice::TooSmall
                },
            )
        } else {
            native.settings_notice()
        };
        // Settings owns its complete local return and shortcut hint inside the
        // surface. This global row carries only a distinct notice or recovery.
        let line = notice.map_or_else(Line::default, |notice| {
            let role = match notice {
                crate::terminal_session::SettingsNotice::Saved => Role::StatusSuccess,
                crate::terminal_session::SettingsNotice::Failed => Role::StatusError,
                crate::terminal_session::SettingsNotice::SourceChanged
                | crate::terminal_session::SettingsNotice::Unknown
                | crate::terminal_session::SettingsNotice::PresentationUnavailable
                | crate::terminal_session::SettingsNotice::TooSmall => Role::StatusWarning,
                crate::terminal_session::SettingsNotice::Busy
                | crate::terminal_session::SettingsNotice::Background => Role::StatusPending,
                crate::terminal_session::SettingsNotice::NoChange => Role::StatusInfo,
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", theme.glyphs.status_bullet),
                    theme.style(role),
                ),
                Span::styled(
                    locale.settings_notice(notice),
                    theme.style(Role::TextPrimary),
                ),
            ])
        });
        if matches!(draw, ModalPresentation::NotPresented) {
            recovery::render(
                frame,
                areas.status,
                locale.presentation_notice(retained),
                local_return.filter(|_| retained),
                theme,
            );
        } else {
            frame.render_widget(Paragraph::new(line), areas.status);
        }
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
    if frame.area().intersection(outer) != outer || frame.area().intersection(inner) != inner {
        return;
    }
    let Some(close) = controls::close_rect(frame.area(), outer) else {
        return;
    };
    hits.mark_overlay_active();
    let target = InteractionTarget::PopupClose(popup.close_target());
    let interaction = if popup.closing {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    };
    let pointer =
        match hits.projected_pointer(HitLayer::Overlay, close, close, &target, interaction) {
            ProjectedPointer::Rest => controls::PointerPresentation::Rest,
            ProjectedPointer::Hovered => controls::PointerPresentation::Hovered,
            ProjectedPointer::Pressed => controls::PointerPresentation::Pressed,
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
    let painted_close = controls::render_icon_action(
        frame,
        close,
        controls::IconActionSpec::new(
            controls::IconActionGlyph::Close,
            controls::ControlSurface::Overlay,
            if popup.closing {
                controls::Availability::Disabled
            } else {
                controls::Availability::Enabled
            },
            pointer,
        ),
        theme,
    )
    .expect("preflighted popup close remains paintable");
    debug_assert_eq!(painted_close, close);
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
    hits.record_interaction(
        HitLayer::Overlay,
        painted_close,
        painted_close,
        target,
        interaction,
    );
    hits.record(
        HitLayer::Overlay,
        inner,
        HitTarget::Popup {
            id: popup.id.clone(),
            content: inner,
        },
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
    let marker = theme.glyphs.status_bullet;
    let marker_width = display_width(marker).saturating_add(1);
    let line = if marker_width < usize::from(body.width) {
        Line::from(vec![
            Span::styled(marker, theme.style(Role::StatusWarning)),
            Span::styled(" ", theme.style(Role::TextPrimary)),
            Span::styled(
                truncate_display(
                    text.too_small_notice,
                    usize::from(body.width) - marker_width,
                    theme.glyphs.ellipsis,
                ),
                theme.style(Role::TextPrimary),
            ),
        ])
    } else {
        Line::styled(
            truncate_display(
                text.too_small_notice,
                usize::from(body.width),
                theme.glyphs.ellipsis,
            ),
            theme.style(Role::TextPrimary),
        )
    };
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), row);
}

fn render_unavailable_workbench(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    text: ShellText,
    tab_status: &str,
    tier: ViewportTier,
    theme: &Theme,
) {
    // Authored guidance stays readable beside an overlay sidebar. This changes only
    // its paint area; terminal geometry continues to use the canonical shell areas.
    let visible_area = |area: Rect| match areas.sidebar {
        Some(sidebar) if sidebar.layer == SidebarLayer::Overlay => {
            let x = area.x.max(sidebar.area.right().min(area.right()));
            Rect::new(x, area.y, area.right().saturating_sub(x), area.height)
        }
        _ => area,
    };
    let tabs = visible_area(areas.tabs);
    let panel = visible_area(areas.panel);
    if !tabs.is_empty() {
        let tab_line = Line::from(vec![
            Span::styled(
                format!(" {} ", text.workbench_title),
                theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {tab_status} "), theme.style(Role::TextSecondary)),
            Span::styled("  !  ", theme.style(Role::StatusWarning)),
        ]);
        frame.render_widget(Paragraph::new(tab_line), tabs);
    }

    let content = vec![
        Line::from(""),
        Line::styled(text.terminal_unavailable, theme.style(Role::StatusWarning)),
        Line::from(""),
        Line::styled(text.shell_only_detail, theme.style(Role::TextPrimary)),
        Line::styled(text.no_client_session, theme.style(Role::TextSecondary)),
    ];
    let content_area = terminal_area(panel, tier.shows_panel_frame());
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
            panel,
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
    directory_guidance_admitted: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    #[cfg(windows)] graphics_uncovered: bool,
    #[cfg(windows)] graphics_draws: &mut Vec<crate::terminal_session::PaneGraphicsDraw>,
) {
    let bound_text = locale.shell().bind(client_view);
    let text = bound_text.text();
    let pane_focused = !native.capture_claims().exclusive_any();
    #[cfg(windows)]
    let pane_focused = pane_focused
        && !client_view.navigator_active()
        && !client_view.machines.sidebar_navigation_active()
        && !client_view.machines.open;
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
        #[cfg(windows)]
        let drawn_dividers = if input_enabled {
            native.divider_source().map_or_else(Vec::new, |source| {
                projection
                    .splits
                    .iter()
                    .filter_map(|split| {
                        crate::tui_layout::DrawnDivider::new(source.clone(), frame.area(), split)
                    })
                    .collect()
            })
        } else {
            Vec::new()
        };
        if pane_focused
            && let Some(leaf) = projection
                .leaves
                .iter()
                .find(|leaf| Some(leaf.panel_index) == tab.active_panel())
        {
            emphasize_pane_boundary(frame, leaf.leaf, panel_area, theme);
        }
        #[cfg(windows)]
        render_dividers(
            frame,
            &projection.dividers,
            panel_area,
            theme,
            &drawn_dividers,
            hits,
        );
        #[cfg(not(windows))]
        render_dividers(frame, &projection.dividers, panel_area, theme);
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
            let directory_guidance = workspace_setup::directory_guidance(
                locale,
                native.phase(),
                native.retirement().is_some(),
                native.workspace_directory_path(),
                native.workspace_directory_unconfigured(),
            );
            if let Some(guidance) = directory_guidance {
                workspace_setup::render_directory_guidance(
                    frame,
                    content_area,
                    &guidance,
                    client_view.bindings(),
                    locale,
                    directory_guidance_admitted,
                    theme,
                );
            } else {
                let panel_status = native
                    .workspace_close_status()
                    .filter(|_| native.retirement().is_some())
                    .map_or_else(
                        || panel_status.to_owned(),
                        |status| locale.workspace_close_status(status),
                    );
                frame.render_widget(
                    Paragraph::new(panel_status)
                        .style(theme.style(Role::TextSecondary))
                        .wrap(Wrap { trim: true }),
                    content_area,
                );
            }
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
    let location = location.filter(|location| {
        !native
            .active_position()
            .and_then(|position| position.checked_sub(1))
            .and_then(|index| native.tab(index))
            .is_some_and(|tab| {
                sanitize_terminal_text(tab.tab_label().as_ref()).as_ref() == *location
            })
    });
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
    let tab_source = native.tab_create_target();
    #[cfg(windows)]
    let create_target = tab_source.clone().filter(|_| strip.width >= 7);
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
        tabs::CompactInteractions::new(
            hits,
            &mut |hits, index, rect| {
                let Some(tab) = native.tab(index) else {
                    return ProjectedPointer::Rest;
                };
                let target = InteractionTarget::Tab(tab.tab_id().clone());
                hits.projected_pointer(
                    HitLayer::Foreground,
                    rect,
                    rect,
                    &target,
                    InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
                )
            },
            &mut |hits, index, rect| {
                if let Some(tab) = native.tab(index) {
                    hits.record_interaction(
                        HitLayer::Foreground,
                        rect,
                        rect,
                        InteractionTarget::Tab(tab.tab_id().clone()),
                        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
                    );
                }
            },
        ),
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
        let tab_target = |index: usize, group_header: bool| {
            let tab = native.tab(index)?;
            Some(if group_header {
                let group = tab.group()?;
                HitTarget::TabGroup {
                    tab: tab.tab_id().clone(),
                    group: group.group_id.clone(),
                }
            } else {
                HitTarget::Tab(tab.tab_id().clone())
            })
        };
        tabs::groups::render(
            frame,
            tab_area,
            &displayed,
            theme,
            text.unnamed_tab_group,
            tabs::groups::GroupInteractions::new(
                hits,
                &mut |hits, index, group_header, rect| {
                    let Some(source) = tab_source.as_ref() else {
                        return ProjectedPointer::Rest;
                    };
                    let Some(target) = tab_target(index, group_header) else {
                        return ProjectedPointer::Rest;
                    };
                    hits.projected_tab_pointer(source, &target, rect)
                },
                &mut |hits, index, group_header, rect| {
                    used = usize::from(rect.right().saturating_sub(strip.x));
                    if let (Some(source), Some(target)) =
                        (tab_source.as_ref(), tab_target(index, group_header))
                    {
                        hits.record_tab_target(source, rect, target);
                    }
                },
            ),
        );
    }
    #[cfg(windows)]
    if let Some(target) = create_target {
        let offset = u16::try_from(used)
            .unwrap_or(tab_area.width)
            .saturating_add(u16::from(used > 0))
            .min(strip.width - 3);
        let offered = Rect::new(strip.x + offset, strip.y, 3, 1);
        if let Some(rect) = controls::icon_action_rect(frame.area(), offered) {
            let interaction = InteractionTarget::TabCreate(target);
            let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
            let pointer = tabs::pointer_presentation(hits.projected_pointer(
                HitLayer::Foreground,
                rect,
                rect,
                &interaction,
                spec,
            ));
            if let Some(painted) = controls::render_icon_action(
                frame,
                offered,
                controls::IconActionSpec::new(
                    controls::IconActionGlyph::Add,
                    controls::ControlSurface::Chrome,
                    controls::Availability::Enabled,
                    pointer,
                ),
                theme,
            ) {
                hits.record_interaction(HitLayer::Foreground, rect, painted, interaction, spec);
            }
        }
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

fn render_dividers(
    frame: &mut Frame<'_>,
    dividers: &[Rect],
    panel: Rect,
    theme: &Theme,
    #[cfg(windows)] targets: &[crate::tui_layout::DrawnDivider],
    #[cfg(windows)] hits: &mut HitMapBuilder,
) {
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
        let glyph = match theme.glyphs.mode() {
            crate::tui_theme::GlyphMode::Unicode => {
                crate::tui_layout::divider_glyph(up, down, left, right)
            }
            #[cfg(test)]
            crate::tui_theme::GlyphMode::Simplified => {
                if (up || down) && (left || right) {
                    '+'
                } else if up || down {
                    '|'
                } else {
                    '-'
                }
            }
        };
        let boundary_cell = boundary.contains(&(x, y));
        #[cfg(windows)]
        let pointer = targets
            .iter()
            .filter(|target| target.divider.contains((x, y).into()))
            .map(|target| hits.projected_divider_pointer(target))
            .max_by_key(|state| match state {
                ProjectedPointer::Rest => 0,
                ProjectedPointer::Hovered => 1,
                ProjectedPointer::Pressed => 2,
            })
            .unwrap_or(ProjectedPointer::Rest);
        #[cfg(windows)]
        let style = if pointer == ProjectedPointer::Rest {
            theme.style(if boundary_cell {
                Role::FrameUnfocused
            } else {
                Role::FrameDivider
            })
        } else {
            theme.content_on_surface(
                Role::SurfaceCanvas,
                Some(match pointer {
                    ProjectedPointer::Hovered => Role::SurfaceHover,
                    ProjectedPointer::Pressed => Role::SurfacePressed,
                    ProjectedPointer::Rest => unreachable!("rest handled before composition"),
                }),
                Some(Role::FrameDivider),
                theme.style(Role::FrameDivider),
            )
        };
        #[cfg(not(windows))]
        let style = theme.style(if boundary_cell {
            Role::FrameUnfocused
        } else {
            Role::FrameDivider
        });
        frame
            .buffer_mut()
            .set_string(x, y, glyph.to_string(), style);
    }
    #[cfg(windows)]
    for target in targets {
        hits.record_divider(target.clone());
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

fn status_overlay_visible(
    view: &ClientView,
    session: Option<&TerminalSessionView<'_>>,
    capture_owner: Option<CaptureOwner>,
) -> bool {
    let visible = view.prefix_active()
        || view.overlay().is_some()
        || view.notice().is_some()
        || view.pointer_released()
        || capture_owner.is_some();
    #[cfg(windows)]
    let visible = visible
        || view.navigator_active()
        || view.machines.sidebar_navigation_active()
        || view.resize_pending;
    visible
        || session.is_some_and(|session| match session {
            TerminalSessionView::Native(native) => {
                let visible = native.phase() != NativeDisplayPhase::Ready
                    || native.controller() == Some(NativeDisplayController::ReadOnly)
                    || native.notice().is_some()
                    || native.takeover_status().is_some()
                    || native.setup_status().is_some()
                    || native.close_status().is_some()
                    || native.workspace_close_status().is_some()
                    || native.close_set_status().is_some();
                #[cfg(windows)]
                let visible = visible || native.duplicate_progress().is_some();
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
    capture_owner: Option<CaptureOwner>,
    locale: Locale,
    status: &str,
    secondary_notice: Option<&str>,
    zoomed: bool,
    read_only: bool,
    recovery_status: bool,
    tier: ViewportTier,
    theme: &Theme,
    session: Option<&TerminalSessionView<'_>>,
) -> Option<Rect> {
    status_line::render(
        frame,
        area,
        view,
        capture_owner,
        locale,
        status,
        secondary_notice,
        zoomed,
        read_only,
        recovery_status,
        tier,
        theme,
        session,
    )
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
        hits,
        &mut |hits, index, rect| {
            let Some(context) = native.context_at(index) else {
                return ProjectedPointer::Rest;
            };
            let target = InteractionTarget::Context(context.location().clone());
            hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
            )
        },
        &mut |hits, index, rect| {
            if let Some(context) = native.context_at(index) {
                hits.record_interaction(
                    HitLayer::Overlay,
                    rect,
                    rect,
                    InteractionTarget::Context(context.location().clone()),
                    InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
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
        hits,
        &mut |hits, index, rect| {
            let Some(tab) = native.tab(index) else {
                return ProjectedPointer::Rest;
            };
            let target = InteractionTarget::Tab(tab.tab_id().clone());
            hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
            )
        },
        &mut |hits, index, rect| {
            if let Some(tab) = native.tab(index) {
                hits.record_interaction(
                    HitLayer::Overlay,
                    rect,
                    rect,
                    InteractionTarget::Tab(tab.tab_id().clone()),
                    InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
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
