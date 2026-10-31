//! Composes the Client's terminal frame and interaction receipts from admitted feature views,
//! viewport geometry, and visual roles. It paints and reports what was drawn without owning
//! feature transitions, input admission, or backend effects.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Position, Rect},
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph},
};
mod action_menu;
#[cfg(test)]
pub(crate) fn render_action_menu_for_test(
    frame: &mut Frame<'_>,
    content: Rect,
    menu: crate::action_menu::MenuView<'_>,
    bindings: &crate::client_view::Bindings,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<crate::action_menu::MenuDraw> {
    action_menu::render(frame, content, menu, &[], bindings, locale, theme, hits)
}
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

use crate::agent_board::AgentBoardDraw;
use crate::client_view::{ClientView, Overlay};
use crate::localization::{
    ClientProjectionStatus, DisplayController, DisplayPhase, Locale, RecoveryVerb, RefreshStatus,
    ShellText,
};
use crate::terminal_text::{display_width, truncate_display, wrap_text};
use crate::tui_composition::{
    CaptureClaims, CaptureOwner, ClientCaptureClaims, WorkbenchCaptureOwner,
    client_chrome_overlay_allowed, project_capture,
};
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
    ProjectedPointer,
};
use crate::tui_layout::{
    PaneFrame, ShellAreas, SidebarArea, SidebarLayer, ViewportTier, native_terminal_area,
    overlay_area, place_overlay, project_shell_areas, status_chip_area, terminal_area,
};
use crate::tui_theme::{Role, Theme};
use crate::workbench_client::{ClientTabView, DrawReceipt, ModalPresentation, WorkbenchClientView};
use crate::workbench_state::CloseStatusToken;

use crate::tui_hit_map::GlobalChromeAction;

use crate::localization::TerminalStatus;

mod agent_board;
#[cfg(test)]
pub(crate) fn render_agent_board_for_test(
    frame: &mut Frame<'_>,
    locale: Locale,
    board: &crate::agent_board::AgentBoardView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> AgentBoardDraw {
    agent_board::render(
        frame,
        frame.area(),
        locale.shell(),
        locale,
        board,
        (theme, 0),
        hits,
    )
}
mod agent_sidebar;
mod contexts;
pub(crate) mod controls;
mod copy_mode;
mod divider_resize;
mod help;
mod machine_picker;
mod modal;
pub(crate) use modal::{OverlayFrame, overlay_surface, titled_overlay};
use modal::{overlay_block, overlay_heading, overlay_title};
mod navigator;
mod onboarding;
mod pane_graphics;
mod pane_titles;
mod panel_close;
mod panel_reports;
mod presentation_selector;
mod recovery;
mod selection;
pub use agent_sidebar::render_width_preview;
pub use divider_resize::render_divider_preview;
pub use tabs::render_tab_drag_preview;
mod sidebar_reorder;
pub use sidebar_reorder::render_sidebar_reorder_preview;
mod scrollbar;
mod sidebar_text;
mod status_line;
mod tab_close;
mod tab_rename;
mod tabs;
mod terminal_cells;
mod tooltip;
// The owning application's backend tests paint captured cells through the real native writer.
#[cfg(any(test, feature = "test-support"))]
pub use terminal_cells::render as render_terminal_cells;
mod terminal_failure;
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod test_capture;
mod viewport;
mod workspace_close;
mod workspace_creation;
mod workspace_setup;
mod workspace_sidebar;
mod worktree_launch;

#[cfg(test)]
pub(crate) fn render_tab_rename_for_test(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &crate::workbench_client::RenameDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
) -> (
    Rect,
    ModalPresentation<crate::workbench_state::rename_draft::RenameDraftToken>,
) {
    tab_rename::render(
        frame,
        locale,
        draft,
        content_area,
        theme,
        &mut HitMapBuilder::new(),
    )
}

#[derive(Clone, Copy)]
struct SessionStatus<'a> {
    text: &'a str,
    role: Role,
}

impl SessionStatus<'_> {
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
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn render_terminal_shell(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    workbench_client: Option<WorkbenchClientView<'_>>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> DrawReceipt {
    render_terminal_shell_with_resize(
        frame,
        view,
        locale,
        workbench_client,
        theme,
        hits,
        &crate::tui_layout::ResizeProjection::Canonical,
    )
}

pub fn render_terminal_shell_with_resize<'a>(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    mut workbench_client: Option<WorkbenchClientView<'a>>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    resize: &'a crate::tui_layout::ResizeProjection,
) -> DrawReceipt {
    if let Some(native) = workbench_client.as_mut() {
        native.set_resize_projection(resize);
    }
    let area = frame.area();
    let client_claims = ClientCaptureClaims::default();
    let client_claims = client_claims
        .with_machine(view.machines.open)
        .with_resize(view.resize_mode());
    let native_claims = match workbench_client.as_ref() {
        Some(native) => native.capture_claims(),
        _ => CaptureClaims::default(),
    };
    let capture_owner = project_capture(client_claims, native_claims);
    let native_top = |owner| capture_owner == Some(CaptureOwner::Native(owner));
    let shell_text = locale.shell();
    let bound_text = shell_text.bind(&view.bindings().shell_bindings(shell_text.binding_unbound));
    let text = bound_text.text();
    frame.render_widget(Block::new().style(theme.style(Role::SurfaceCanvas)), area);

    let sidebar_surface = resize.sidebar(view.sidebar_surface());
    let areas = project_shell_areas(area, sidebar_surface);
    // The Tab row includes the hidden sidebar's restore control.
    let tab_row = Role::SurfaceStrip;

    frame
        .buffer_mut()
        .set_style(areas.tabs, theme.style(tab_row));
    let mut areas = areas;
    let chrome_action_state = controls::InteractionState::new(
        controls::FocusState::Unfocused,
        controls::PointerPresentation::Rest,
        controls::NavigationState::None,
        controls::CurrentState::NotCurrent,
        controls::CheckedState::Unchecked,
        controls::Availability::Enabled,
        controls::OperationState::Idle,
    );
    let restore_label = locale.sidebar_restore_label();
    let restore_button =
        if view.machines.available && areas.sidebar.is_none() && !areas.tabs.is_empty() {
            let action = controls::ActionSpec::new(
                restore_label,
                controls::ActionKind::Secondary,
                controls::ControlSurface::Strip,
                chrome_action_state,
            );
            controls::action_rect(area, areas.tabs, action, theme).inspect(|button| {
                areas.tabs.x = button.right();
                areas.tabs.width = areas.tabs.width.saturating_sub(button.width);
            })
        } else {
            None
        };

    let tier = areas.tier;
    if tier != ViewportTier::TooSmall {
        hits.record_action_menu_source(match workbench_client.as_ref() {
            Some(native) => native.action_menu_source(),
            _ => None,
        });
    }
    let position_uncovered = workbench_client.is_some()
        && tier != ViewportTier::TooSmall
        && capture_owner.is_none()
        && !view.prefix_active()
        && view.overlay().is_none()
        && !view.navigator_active()
        && !view.machines.sidebar_navigation_active()
        && !view.resize_pending
        && areas
            .sidebar
            .is_none_or(|sidebar| sidebar.layer != SidebarLayer::Overlay);
    let graphics_uncovered = position_uncovered
        && !status_overlay_visible(view, workbench_client.as_ref(), capture_owner);
    if !graphics_uncovered && let Some(native) = workbench_client.as_ref() {
        native.cover_graphics();
    }

    let mut status_draw: Option<(CloseStatusToken, Rect)> = None;
    let mut prompt = None;
    let mut panel_prompt = None;
    let mut workspace_prompt = None;
    let mut rename = None;
    let mut setup = None;
    let mut worktree = None;
    let mut overlay_refused = false;
    let mut help_presented = false;
    let mut board_draw = None;
    let mut graphics_draws = Vec::new();
    // The pane cursor is placed after the floating status, so a chip that covers its cell hides it.
    let mut pane_cursor = None;
    let sidebar_status;
    let mut sidebar_status_role = Role::StatusWarning;
    let status_text;
    let mut status_persistent = None;
    let mut status_ordinary = false;
    let mut secondary_notice = None;

    match workbench_client.as_ref() {
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
        Some(native) => {
            let phase_status = native.phase_status_text(locale);
            let close = native.close_status();
            let status = match (native.workspace_close_status(), close.as_ref()) {
                (Some(status), _) => locale.workspace_close_status(status),
                (None, Some(status)) => {
                    let label = status.target_label(locale);
                    locale.native_close_status(status.text(&label))
                }
                (None, None) => {
                    let selected = native.status(locale, status_chip_area(&areas).width);
                    status_persistent = selected.persistent;
                    status_ordinary = selected.ordinary;
                    selected.text
                }
            };
            secondary_notice = native
                .notice()
                .map(|notice| locale.native_notice_with_context(notice, native.notice_context()))
                .or_else(|| {
                    native
                        .completion_reminder()
                        .map(|reminder| locale.command_completion_reminder(reminder))
                });
            sidebar_status = phase_status.to_owned();
            if native.phase() == DisplayPhase::Ready
                && native
                    .focus_panel_id()
                    .and_then(|panel| native.terminal_end(panel))
                    .is_none()
            {
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
                    None,
                    theme,
                    hits,
                    graphics_uncovered,
                    &mut graphics_draws,
                    &mut pane_cursor,
                );
            }
        }
    }

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
                controls::ControlSurface::Strip,
                chrome_action_state.with_pointer(pointer),
            ),
            theme,
        );
        debug_assert_eq!(painted, Some(button));
        if painted == Some(button) {
            hits.record_interaction(HitLayer::Foreground, button, button, target, spec);
        }
    }

    if tier != ViewportTier::TooSmall && capture_owner == Some(CaptureOwner::Resize) {
        resize_mode::render(
            frame,
            view,
            match workbench_client.as_ref() {
                Some(native) => Some(native),
                _ => None,
            },
            locale,
            theme,
            hits,
        );
    }
    if tier != ViewportTier::TooSmall
        && capture_owner == Some(CaptureOwner::Copy)
        && let Some(native) = workbench_client.as_ref()
        && let Some(copy) = native.copy_mode()
    {
        copy_mode::render_controls(frame, view, copy, locale, theme, hits);
    }

    let session_status = SessionStatus {
        text: &sidebar_status,
        role: sidebar_status_role,
    };
    if tier == ViewportTier::TooSmall {
        render_too_small_notice(frame, areas.body, text, theme);
    }

    if tier != ViewportTier::TooSmall {
        if let Some(sidebar) = areas.sidebar {
            let agent_line = match workbench_client.as_ref() {
                Some(native) => native
                    .agent_board_summary()
                    .map(|summary| locale.agent_sidebar_line(summary, summary.at.elapsed())),
                _ => None,
            };
            if view.machines.available {
                let captured = capture_owner.is_some();
                // An open row menu keeps its target row as it was when the menu opened.
                let hold = if native_top(WorkbenchCaptureOwner::ActionMenu) {
                    workbench_client
                        .as_ref()
                        .and_then(|native| sidebar_menu_hold(view, native))
                } else {
                    None
                };
                let mut covered_sidebar = HitMapBuilder::new();
                agent_sidebar::render(
                    frame,
                    sidebar,
                    agent_sidebar::RenderContext {
                        view,
                        sidebar_surface,
                        locale,
                        creation: match workbench_client.as_ref() {
                            Some(native) => native.workspace_creation_preview(),
                            _ => None,
                        },
                        backend_status: session_status,
                        theme,
                        input: if hold.as_ref().is_some_and(|(_, keyboard)| *keyboard)
                            && view.machines.sidebar_navigation_active()
                        {
                            // A keyboard-opened row menu leaves the sidebar its focus.
                            agent_sidebar::SidebarInput::Navigation
                        } else if captured
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
                        held: hold
                            .as_ref()
                            .filter(|(_, keyboard)| !*keyboard)
                            .map(|(target, _)| target),
                    },
                    Some(if captured {
                        &mut covered_sidebar
                    } else {
                        &mut *hits
                    }),
                );
                if captured {
                    hits.record_covered_sidebar_geometry(covered_sidebar);
                }
            } else {
                render_sidebar(frame, sidebar, text, session_status, agent_line, theme);
            }
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
                        help::RenderContext::new(
                            text,
                            locale,
                            view,
                            workbench_client.is_some(),
                            candidate,
                        ),
                        theme,
                        hits,
                    )
                });
                help_presented = presented.is_some();
                Some(presented)
            }
            Some(Overlay::WorkbenchNavigator) => Some(navigator::render(
                frame,
                overlay_area(&areas),
                text,
                locale,
                view,
                match workbench_client.as_ref() {
                    Some(native) => Some(native),
                    _ => None,
                },
                theme,
                hits,
            )),

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

        // The Agent board is Client-owned rather than a ClientView
        // overlay: it paints above the workbench (and any chrome overlay)
        // but below armed modals, and its painted truth settles input
        // capture through the draw receipt.
        if native_top(WorkbenchCaptureOwner::AgentBoard)
            && let Some(native) = workbench_client.as_ref()
            && let Some(board) = native.agent_board()
        {
            let working_frame = view.machines.working_frame();

            let draw = agent_board::render(
                frame,
                overlay_area(&areas),
                text,
                locale,
                board,
                (theme, working_frame),
                hits,
            );
            board_draw = Some(draw);
        }

        if native_top(WorkbenchCaptureOwner::WorkspaceClose)
            && let Some(native) = workbench_client.as_ref()
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
        if native_top(WorkbenchCaptureOwner::TabClose)
            && let Some(native) = workbench_client.as_ref()
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
        if native_top(WorkbenchCaptureOwner::PanelClose)
            && let Some(native) = workbench_client.as_ref()
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
        if native_top(WorkbenchCaptureOwner::Rename)
            && let Some(native) = workbench_client.as_ref()
            && let Some(draft) = native.rename_draft()
        {
            let inline_area = hits.tab_area(draft.tab_id()).map(|area| {
                Rect::new(
                    area.x,
                    area.y,
                    area.width
                        .max(24)
                        .min(frame.area().right().saturating_sub(area.x)),
                    1,
                )
            });
            let (area, token) = if let Some(area) = inline_area.filter(|area| area.width >= 8) {
                tab_rename::render_inline(frame, locale, &draft, area, theme, hits)
            } else {
                tab_rename::render(frame, locale, &draft, overlay_area(&areas), theme, hits)
            };

            status_draw = status_draw.filter(|(_, status_area)| !status_area.intersects(area));
            prompt = None;
            panel_prompt = None;
            rename = Some(token);
        }
        if native_top(WorkbenchCaptureOwner::Setup)
            && let Some(native) = workbench_client.as_ref()
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
        if native_top(WorkbenchCaptureOwner::Worktree)
            && let Some(native) = workbench_client.as_ref()
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
        if workbench_client.as_ref().is_some() {
            if native_top(WorkbenchCaptureOwner::WorkspaceClose) {
                workspace_prompt = Some(ModalPresentation::NotPresented);
            }
            if native_top(WorkbenchCaptureOwner::TabClose) {
                prompt = Some(ModalPresentation::NotPresented);
            }
            if native_top(WorkbenchCaptureOwner::PanelClose) {
                panel_prompt = Some(ModalPresentation::NotPresented);
            }
            if native_top(WorkbenchCaptureOwner::Rename) {
                rename = Some(ModalPresentation::NotPresented);
            }
            if native_top(WorkbenchCaptureOwner::Setup) {
                setup = Some(ModalPresentation::NotPresented);
            }
            if native_top(WorkbenchCaptureOwner::Worktree) {
                worktree = Some(ModalPresentation::NotPresented);
            }
        }
    }

    // An open board that could not paint at all (too-small viewport)
    // still reports its verdict — input capture never settles on an
    // unpainted surface.
    if native_top(WorkbenchCaptureOwner::AgentBoard)
        && board_draw.is_none()
        && let Some(native) = workbench_client.as_ref()
        && let Some(board) = native.agent_board()
    {
        board_draw = Some(AgentBoardDraw::NotPresented(board.episode()));
    }

    // Only the actual top owner's evaluated geometry can explain this frame.
    // Covered lower owners receive no receipt and publish no recovery chrome.
    enum RecoveryStatus {
        LocalReturn {
            phase: crate::tui_presentation::PresentationPhase,
            verb: RecoveryVerb,
        },
        Plain(&'static str),
    }
    let native_recovery = if let (Some(CaptureOwner::Native(owner)), Some(native)) =
        (capture_owner, workbench_client.as_ref())
    {
        match owner {
            WorkbenchCaptureOwner::WorkspaceClose
                if matches!(
                    workspace_prompt.as_ref(),
                    Some(ModalPresentation::NotPresented)
                ) =>
            {
                native
                    .workspace_close_confirmation()
                    .map(|view| (view.presentation(), RecoveryVerb::Cancel))
            }
            WorkbenchCaptureOwner::TabClose
                if matches!(prompt.as_ref(), Some(ModalPresentation::NotPresented)) =>
            {
                native
                    .close_confirmation()
                    .map(|view| (view.presentation(), RecoveryVerb::Cancel))
            }
            WorkbenchCaptureOwner::PanelClose
                if matches!(panel_prompt.as_ref(), Some(ModalPresentation::NotPresented)) =>
            {
                native
                    .panel_close_confirmation()
                    .map(|view| (view.presentation(), RecoveryVerb::Cancel))
            }
            WorkbenchCaptureOwner::Rename
                if matches!(rename.as_ref(), Some(ModalPresentation::NotPresented)) =>
            {
                native
                    .rename_draft()
                    .map(|view| (view.presentation(), RecoveryVerb::Cancel))
            }
            WorkbenchCaptureOwner::Setup
                if matches!(setup.as_ref(), Some(ModalPresentation::NotPresented)) =>
            {
                native
                    .setup_draft()
                    .map(|view| (view.presentation(), RecoveryVerb::Cancel))
            }
            WorkbenchCaptureOwner::Worktree
                if matches!(worktree.as_ref(), Some(ModalPresentation::NotPresented)) =>
            {
                native.worktree().map(|view| {
                    (
                        view.presentation(),
                        match view.stage() {
                            crate::workbench_state::worktree_launch::WorktreeStage::Waiting => {
                                RecoveryVerb::Hide
                            }
                            crate::workbench_state::worktree_launch::WorktreeStage::Result => {
                                RecoveryVerb::Close
                            }
                            _ => RecoveryVerb::Cancel,
                        },
                    )
                })
            }
            WorkbenchCaptureOwner::AgentBoard
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
        native_recovery.map(|(phase, verb)| RecoveryStatus::LocalReturn { phase, verb });
    let native_recovery = if tier == ViewportTier::TooSmall
        && capture_owner == Some(CaptureOwner::Native(WorkbenchCaptureOwner::Popup))
        && matches!(workbench_client.as_ref(), Some(native) if native.popup().is_some())
    {
        Some(RecoveryStatus::Plain(locale.plugin_popup_resize_recovery()))
    } else {
        native_recovery
    };

    let notice = if overlay_refused {
        Some(text.overlay_refused)
    } else {
        secondary_notice.as_deref()
    };
    let zoomed = matches!(
        workbench_client.as_ref(),
        Some(native) if native.zoom_active()
    );
    let read_only = matches!(
        workbench_client.as_ref(),
        Some(native)
            if native.controller() == Some(DisplayController::ReadOnly)
    );
    // The status floats over the last row and covers only the cells it paints; nothing shows at
    // rest once the launch hint has taught the prefix.
    let status_facts = status_overlay_visible(view, workbench_client.as_ref(), capture_owner)
        || notice.is_some()
        || zoomed
        || tier == ViewportTier::TooSmall;
    // The launch hint alone floats without the fact: a connecting first frame still shows no
    // connection-only status.
    let status_visible = status_facts || view.launch_hint_pending();
    let chip_area = status_chip_area(&areas);
    let copy_status = capture_owner == Some(CaptureOwner::Copy)
        && workbench_client
            .as_ref()
            .is_some_and(|native| native.copy_mode().is_some());
    let help_refusal = tier != ViewportTier::TooSmall
        && client_chrome_overlay_allowed(capture_owner)
        && view.overlay() == Some(Overlay::ShortcutHelp)
        && !help_presented
        && view.help_draw_candidate().is_some();
    let menu_status = native_top(WorkbenchCaptureOwner::ActionMenu)
        && workbench_client
            .as_ref()
            .is_some_and(|native| native.action_menu().is_some());
    let settings_status = native_top(WorkbenchCaptureOwner::Settings)
        && workbench_client
            .as_ref()
            .is_some_and(|native| native.settings().is_some());
    // One owner paints the status cells in a frame: a local owner's feedback replaces the
    // ordinary status instead of stacking a second chip beside it.
    let status_replaced =
        native_recovery.is_some() || copy_status || help_refusal || menu_status || settings_status;
    let painted = if status_visible && !status_replaced {
        render_status(
            frame,
            chip_area,
            view,
            capture_owner,
            locale,
            status_line::StatusFacts {
                primary: &status_text,
                primary_ordinary: status_ordinary || !status_facts,
                persistent: status_persistent,
                secondary_notice: notice,
                zoomed,
                read_only,
                recovery_status: matches!(workbench_client.as_ref(), Some(native)
                    if native.workspace_close_status().is_none()
                        && native.close_status().is_some_and(|status| status.resumable())),
            },
            tier,
            theme,
            workbench_client.as_ref(),
        )
    } else {
        status_line::StatusPaint::default()
    };
    // Covered cells never forward a click to the terminal beneath the status.
    let mut status_surfaces = Vec::new();
    if let Some(surface) = painted.surface {
        hits.record(HitLayer::Status, surface, HitTarget::OverlaySurface);
        status_surfaces.push(surface);
    }
    let visible = painted.primary;
    if let Some(recovery) = native_recovery {
        let surface = match recovery {
            RecoveryStatus::LocalReturn { phase, verb } => {
                let retained = phase != crate::tui_presentation::PresentationPhase::Pending;
                recovery::render(
                    frame,
                    chip_area,
                    locale.presentation_notice(retained),
                    retained.then_some(locale.recovery_verb(verb)),
                    theme,
                )
            }
            RecoveryStatus::Plain(notice) => {
                recovery::render(frame, chip_area, notice, None, theme)
            }
        };
        if let Some(surface) = surface {
            hits.record(HitLayer::Status, surface, HitTarget::OverlaySurface);
            status_surfaces.push(surface);
        }
    }
    // A pointer tooltip explains persistent chrome only while nothing else owns the screen. Its
    // box hides what lies beneath: a press there reaches nothing.
    if tier != ViewportTier::TooSmall
        && capture_owner.is_none()
        && view
            .overlay()
            .is_none_or(|overlay| overlay == Overlay::BusinessSidebar)
        && !view.prefix_active()
        && let Some(tooltip) = hits.open_tooltip()
        && let Some(surface) = tooltip::paint(frame, tooltip, &status_surfaces, theme)
    {
        hits.record(HitLayer::Status, surface, HitTarget::OverlaySurface);
        status_surfaces.push(surface);
    }
    if let Some(cursor) =
        pane_cursor.filter(|cursor| !status_surfaces.iter().any(|chip| chip.contains(*cursor)))
    {
        frame.set_cursor_position(cursor);
    }
    if copy_status
        && let Some(copy) = workbench_client
            .as_ref()
            .and_then(|native| native.copy_mode())
    {
        // Copy mode's search editor and actions keep the whole last row while the mode is open.
        copy_mode::render_status(
            frame,
            areas.status,
            copy,
            locale,
            notice,
            theme,
            view.capabilities()
                .supports(crate::tui_capabilities::Capability::Clipboard),
        );
        hits.record(HitLayer::Status, areas.status, HitTarget::OverlaySurface);
    }
    if help_refusal && let Some(candidate) = view.help_draw_candidate() {
        // The requested page remains owner state for a later resize. Only a
        // complete fixed-width close publishes refused-surface authority.
        let _ = help::render_refused(frame, chip_area, text, candidate, theme, hits);
    }
    if capture_owner.is_none()
        && let Some(native) = workbench_client.as_ref()
        && let Some(close) = native.close_status()
    {
        status_draw = status_draw.or(visible.map(|area| (close.token(), area)));
    }

    let receipt = DrawReceipt::new(prompt, status_draw.map(|(token, _)| token))
        .with_panel_close(panel_prompt)
        .with_workspace_close(workspace_prompt)
        .with_rename(rename)
        .with_setup(setup)
        .with_agent_board(board_draw);
    let receipt = receipt.with_worktree(worktree);
    let receipt = receipt.with_attention(match workbench_client.as_ref() {
        Some(native)
            if capture_owner.is_none() && view.overlay().is_none() && {
                {
                    areas
                        .sidebar
                        .is_none_or(|sidebar| sidebar.layer != SidebarLayer::Overlay)
                }
            } =>
        {
            native.attention_draw(hits.painted_panes())
        }
        _ => None,
    });
    if tier != ViewportTier::TooSmall
        && native_top(WorkbenchCaptureOwner::Popup)
        && let Some(native) = workbench_client.as_ref()
        && let Some(popup) = native.popup()
    {
        render_popup(
            frame,
            native_terminal_area(
                areas.panel,
                PaneFrame::of(
                    areas.tier.shows_panel_frame(),
                    native
                        .active_position()
                        .and_then(|p| p.checked_sub(1))
                        .and_then(|i| native.tab(i))
                        .map_or(0, |tab| tab.panel_count()),
                    native.zoom_active(),
                ),
            ),
            &popup,
            locale,
            theme,
            hits,
        );
    }
    let receipt = if native_top(WorkbenchCaptureOwner::ActionMenu)
        && let Some(native) = workbench_client.as_ref()
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
        // An anchored menu takes the whole viewport, as Settings does; the status chip that
        // replaces the ordinary status paints after it.
        let menu_area = if native
            .action_menu_root()
            .is_some_and(|(_, anchor)| anchor.is_some())
        {
            area
        } else {
            overlay_area(&areas)
        };
        let draw = if tier == ViewportTier::TooSmall {
            ModalPresentation::NotPresented
        } else {
            let tab_title = |source: &crate::action_menu::MenuTarget| match source {
                crate::action_menu::MenuTarget::Tab(id)
                | crate::action_menu::MenuTarget::TabGroupChoices(id) => {
                    native.tab_by_id(id).map(|tab| tab.display_label(locale))
                }
                _ => None,
            };
            let title = tab_title(menu.source);
            let mut menu = menu;
            if let Some(title) = title.as_deref() {
                menu.title = title;
            }
            // A cascade parent shows the same Tab name it showed while it was open.
            let mut parents = native.action_menu_cascade();
            let parent_titles: Vec<_> = parents
                .iter()
                .map(|parent| tab_title(parent.source))
                .collect();
            for (parent, title) in parents.iter_mut().zip(&parent_titles) {
                if let Some(title) = title.as_deref() {
                    parent.title = title;
                }
            }
            action_menu::render(
                frame,
                menu_area,
                menu,
                &parents,
                view.bindings(),
                locale,
                theme,
                hits,
            )
        };
        if matches!(draw, ModalPresentation::NotPresented)
            && let Some(surface) = recovery::render(
                frame,
                chip_area,
                locale.presentation_notice(recovery),
                recovery.then_some(local_return),
                theme,
            )
        {
            hits.record(HitLayer::Status, surface, HitTarget::OverlaySurface);
        }
        receipt.with_menu(Some(draw))
    } else {
        receipt
    };
    let receipt = if native_top(WorkbenchCaptureOwner::Settings)
        && let Some(native) = workbench_client.as_ref()
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
                area,
                settings,
                native.settings_pending(),
                locale,
                theme,
                hits,
            )
        };
        let notice = if matches!(draw, ModalPresentation::NotPresented) {
            // Classify this frame from the owner's pre-settlement incarnation.
            // A new attempt is refused; an admitted task can resume later.
            Some(
                if native.settings_presentation()
                    == Some(crate::tui_presentation::PresentationPhase::Pending)
                {
                    crate::localization::SettingsNotice::PresentationUnavailable
                } else {
                    crate::localization::SettingsNotice::TooSmall
                },
            )
        } else {
            native.settings_notice()
        };
        // Settings owns its complete local return and shortcut hint inside the
        // surface. The status chip carries only a distinct notice or recovery.
        let line = notice.map(|notice| {
            let role = match notice {
                crate::localization::SettingsNotice::Saved => Role::StatusSuccess,
                crate::localization::SettingsNotice::Failed => Role::StatusError,
                crate::localization::SettingsNotice::SourceChanged
                | crate::localization::SettingsNotice::Unknown
                | crate::localization::SettingsNotice::PresentationUnavailable
                | crate::localization::SettingsNotice::TooSmall => Role::StatusWarning,
                crate::localization::SettingsNotice::Busy
                | crate::localization::SettingsNotice::Background => Role::StatusPending,
                crate::localization::SettingsNotice::NoChange => Role::StatusInfo,
            };
            (locale.settings_notice(notice), role)
        });
        let surface = if matches!(draw, ModalPresentation::NotPresented) {
            recovery::render(
                frame,
                chip_area,
                locale.presentation_notice(retained),
                local_return.filter(|_| retained),
                theme,
            )
        } else {
            line.and_then(|(text, role)| {
                status_line::render_notice_chip(frame, chip_area, text, role, None, false, theme)
            })
        };
        if let Some(surface) = surface {
            hits.record(HitLayer::Status, surface, HitTarget::OverlaySurface);
        }
        receipt.with_settings(Some(draw))
    } else {
        receipt
    };
    let receipt = receipt.with_position(if position_uncovered {
        match workbench_client.as_ref() {
            Some(native) => native.position_draw(hits.painted_panes()),
            _ => None,
        }
    } else {
        None
    });
    receipt.with_graphics(if graphics_uncovered {
        graphics_draws
    } else {
        Vec::new()
    })
}

fn render_popup(
    frame: &mut Frame<'_>,
    area: Rect,
    popup: &crate::independent_popup::PopupView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let Some((outer, inner)) =
        crate::workbench_client::popup_geometry(popup.width, popup.height, area)
    else {
        return;
    };
    if frame.area().intersection(outer) != outer || frame.area().intersection(inner) != inner {
        return;
    }
    let Some(close) = controls::frame_close_rect(frame.area(), outer) else {
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
        titled_overlay(theme, Padding::ZERO, popup.title, outer.width),
        outer,
    );
    let painted_close = controls::render_frame_icon_action(
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
    hits.record_overlay_dismissal(&target);
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
        Line::styled(text.no_client_connection, theme.style(Role::TextSecondary)),
    ];
    let content_area = terminal_area(panel, tier.shows_panel_frame());
    if tier.shows_panel_frame() {
        frame.render_widget(
            Paragraph::new(wrap_text(
                &Text::from(content),
                panel.width.saturating_sub(2),
            ))
            .alignment(Alignment::Center)
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
            Paragraph::new(wrap_text(&Text::from(content), content_area.width))
                .alignment(Alignment::Center),
            content_area,
        );
    }
}

#[expect(clippy::too_many_arguments)]
fn render_native_workbench(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    client_view: &ClientView,
    locale: Locale,
    native: &WorkbenchClientView<'_>,
    phase_status: &str,
    directory_guidance_admitted: bool,
    global_action: Option<Rect>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    graphics_uncovered: bool,
    graphics_draws: &mut Vec<crate::workbench_client::PaneGraphicsDraw>,
    pane_cursor: &mut Option<Position>,
) {
    if let Some(preview) = native
        .workspace_creation_preview()
        .filter(|preview| preview.replaces_content())
    {
        workspace_creation::render(frame, areas, preview, locale, theme);
        return;
    }
    let shell_text = locale.shell();
    let bound_text = shell_text.bind(
        &client_view
            .bindings()
            .shell_bindings(shell_text.binding_unbound),
    );
    let text = bound_text.text();
    let history_key = client_view
        .bindings()
        .action_labels(crate::client_view::PrefixAction::HelpPagePrevious)
        .into_iter()
        .next();
    let pane_focused = !native.capture_claims().exclusive_any();
    let pane_focused = pane_focused
        && !client_view.navigator_active()
        && !client_view.machines.sidebar_navigation_active()
        && !client_view.machines.open;
    let input_enabled = pane_focused
        && client_view.overlay().is_none()
        && !client_view.prefix_active()
        && native
            .focus_panel_id()
            .and_then(|panel| native.terminal_end(panel))
            .is_none();
    // An overlay sidebar paints over the panes' leading columns after them, so it covers the pane
    // cursor just as it covers pane graphics and attention.
    let cursor_visible = input_enabled
        && areas
            .sidebar
            .is_none_or(|sidebar| sidebar.layer != SidebarLayer::Overlay);
    let panel_area = areas.panel;
    let active_tab = native
        .active_position()
        .and_then(|p| p.checked_sub(1))
        .and_then(|index| native.tab(index));
    let pane_frame = PaneFrame::of(
        areas.tier.shows_panel_frame(),
        active_tab.map_or(0, |tab| tab.panel_count()),
        native.zoom_active(),
    );
    let mut tabs_area = areas.tabs;
    if areas.tier == ViewportTier::Compact
        && let Some(sidebar) = areas
            .sidebar
            .filter(|sidebar| sidebar.layer == SidebarLayer::Overlay)
    {
        let start = tabs_area.x.max(sidebar.area.right());
        tabs_area = Rect::new(
            start,
            tabs_area.y,
            tabs_area.right().saturating_sub(start),
            tabs_area.height,
        );
    }
    // A bare pane has no title line, so its member selector, or the inert position while the
    // selector is unavailable, ends the Tab strip.
    let strip_panel = active_tab
        .filter(|_| pane_frame == PaneFrame::Bare)
        .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)));
    let strip_selector = strip_panel.and_then(|panel| {
        presentation_selector::PresentationSelector::new(tabs_area, panel, native)
    });
    let strip_count = strip_panel
        .filter(|_| strip_selector.is_none())
        .and_then(|panel| {
            presentation_selector::PresentationSelector::pending_count(tabs_area, panel, native)
        });
    if let Some(end) = strip_selector
        .as_ref()
        .map(|selector| selector.area.x)
        .or(strip_count.as_ref().map(|(area, _)| area.x))
    {
        tabs_area.width = end.saturating_sub(tabs_area.x);
    }
    render_tab_strip(frame, tabs_area, native, client_view, locale, theme, hits);
    if let Some(selector) = &strip_selector {
        frame
            .buffer_mut()
            .set_style(selector.area, theme.style(Role::SurfaceStrip));
        selector.paint(frame, STRIP_CONTROL_SURFACE, theme, hits);
    }
    if let Some((area, count)) = strip_count {
        frame
            .buffer_mut()
            .set_style(area, theme.style(Role::SurfaceStrip));
        frame.render_widget(
            Paragraph::new(count).style(theme.style(Role::TextSecondary)),
            area,
        );
    }
    if native.no_current_workspace()
        && native.phase() == DisplayPhase::Empty
        && native.retirement().is_none()
    {
        return;
    }

    let content_area = native_terminal_area(panel_area, pane_frame);
    if pane_frame != PaneFrame::Bare && active_tab.is_some() {
        frame.render_widget(
            Block::new()
                .borders(Borders::ALL)
                .style(theme.style(Role::SurfaceCanvas))
                .border_style(theme.style(Role::FrameUnfocused))
                .border_type(theme.corners),
            panel_area,
        );
    }
    // Split and zoomed views share the leaf chrome. Zoom narrows the drawn set only.
    if let Some(tab) = active_tab.filter(|_| pane_frame != PaneFrame::Bare) {
        let projection = if pane_frame == PaneFrame::Split {
            crate::tui_layout::project_panel_tree(
                content_area,
                tab.panel_tree(),
                tab.active_panel(),
                &|split, first, second| tab.split_ratio(split, first, second),
            )
        } else {
            crate::tui_layout::PanelTreeProjection {
                leaves: vec![crate::tui_layout::LeafArea {
                    panel_index: tab.active_panel().unwrap_or(0),
                    leaf: terminal_area(panel_area, true),
                    content: content_area,
                }],
                ..Default::default()
            }
        };
        for leaf in &projection.leaves {
            render_leaf(
                frame,
                leaf,
                tab,
                input_enabled,
                cursor_visible,
                phase_status,
                locale,
                history_key.as_deref(),
                native,
                client_view,
                theme,
                hits,
                pane_cursor,
            );
            // The recorded region carries the pane's exact live
            // incarnation — a press resolves it bit for bit, so it can
            // never migrate onto a re-minted record of the same panel.
            if let Some(panel) = tab.panel(leaf.panel_index) {
                if native
                    .retained_pane_frame(panel.panel_id(), leaf.content.width, leaf.content.height)
                    .is_some()
                {
                    // A retained image has no live terminal input or scrollbar authority.
                    continue;
                }
                if graphics_uncovered {
                    render_panel_graphics(frame, leaf.content, panel, native, graphics_draws);
                }
                let link_source = native.painted_link_source(panel.panel_id());
                let target = native
                    .pane_ref_for_panel(panel.panel_id())
                    .map(|pane| HitTarget::Pane {
                        pane,
                        content: native
                            .pane_live_view(panel.panel_id())
                            .map_or(leaf.content, |crop| crop.content(leaf.content)),
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
                    let body = match &target {
                        HitTarget::Pane { content, .. } => *content,
                        _ => leaf.leaf,
                    };
                    hits.record(HitLayer::Foreground, body, target);
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
                if let Some(source) = link_source {
                    let content = native
                        .pane_live_view(panel.panel_id())
                        .map_or(leaf.content, |crop| crop.content(leaf.content));
                    hits.record_link_source(content, source);
                }
            }
        }
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
        render_dividers(
            frame,
            &projection.dividers,
            panel_area,
            theme,
            &drawn_dividers,
            hits,
        );

        if pane_focused
            && let Some(leaf) = projection
                .leaves
                .iter()
                .find(|leaf| Some(leaf.panel_index) == tab.active_panel())
        {
            emphasize_pane_boundary(frame, leaf.leaf, panel_area, theme);
        }
        for leaf in &projection.leaves {
            pane_titles::paint(
                frame,
                leaf,
                &pane_titles::PaneTitle {
                    tab,
                    focused: pane_focused && tab.active_panel() == Some(leaf.panel_index),
                    zoomed_panes: (pane_frame == PaneFrame::Zoomed).then_some(tab.panel_count()),
                    working_frame: client_view.machines.working_frame(),
                    text,
                    locale,
                },
                native,
                theme,
                hits,
            );
        }
        // Titles and their selectors on a divider line take pointer precedence over the divider.
        for target in drawn_dividers {
            hits.record_divider(target);
        }
    } else {
        let terminal = if native.projection_status() == ClientProjectionStatus::FrameAvailable {
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
            *pane_cursor = viewport::render_selected(
                frame,
                content_area,
                surface,
                terminal,
                cursor_visible,
                locale,
                theme,
                native
                    .focus_panel_id()
                    .and_then(|panel| native.pane_selection(panel)),
                native.copy_mode().is_some(),
            );
            if input_enabled && let Some(copy) = native.copy_mode() {
                copy_mode::paint_surface(frame, content_area, copy, native, theme, hits);
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
                if let Some(source) = native
                    .focus_panel_id()
                    .and_then(|panel| native.painted_link_source(panel))
                {
                    hits.record_link_source(content_area, source);
                }
            }
        } else if let Some(terminal) = terminal {
            let show_cursor = cursor_visible;
            let show_cursor = show_cursor && native.copy_mode().is_none();
            let tail = native
                .focus_panel_id()
                .and_then(|panel| native.pane_tail_window(panel));
            let crop = crate::terminal_live_view::LiveView::new(
                terminal,
                content_area.width,
                content_area.height,
            );
            let painted = if tail.is_some() {
                content_area
            } else {
                crop.content(content_area)
            };
            *pane_cursor = if let Some((window, rows_before)) = tail {
                terminal_cells::render_tail(
                    frame,
                    content_area,
                    terminal,
                    window,
                    rows_before,
                    show_cursor,
                    theme,
                )
            } else {
                viewport::render_live(
                    frame,
                    content_area,
                    terminal,
                    crop,
                    show_cursor,
                    history_key.as_deref(),
                    global_action,
                    locale,
                    theme,
                )
            };
            selection::paint_cropped(
                frame,
                painted,
                tail.map_or(
                    terminal.scrollback_rows.saturating_add(u64::from(crop.row)),
                    |(window, _)| window.route.start_row,
                ),
                if tail.is_some() { 0 } else { crop.column },
                native
                    .focus_panel_id()
                    .and_then(|panel| native.pane_selection(panel)),
                theme.style(Role::SurfaceSelection),
            );
            if input_enabled && let Some(copy) = native.copy_mode() {
                copy_mode::paint_surface(frame, content_area, copy, native, theme, hits);
            }
            // Only the tab strip stays above the terminal. Every content cell
            // carries the pane's exact live incarnation, including the edges.
            if let Some(pane) = active_tab
                .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)))
                .and_then(|panel| native.pane_ref_for_panel(panel.panel_id()))
            {
                hits.record(
                    HitLayer::Foreground,
                    content_area,
                    HitTarget::Pane {
                        pane,
                        content: painted,
                    },
                );
                if let Some(source) = native
                    .focus_panel_id()
                    .and_then(|panel| native.painted_link_source(panel))
                {
                    hits.record_link_source(painted, source);
                }
            }
        } else if let Some(failure) = active_tab
            .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)))
            .and_then(|panel| native.launch_failure(panel))
        {
            terminal_failure::render(frame, content_area, failure, client_view, locale, theme);
        } else if let Some(retained) = native.focus_panel_id().and_then(|panel| {
            native.retained_pane_frame(panel, content_area.width, content_area.height)
        }) {
            terminal_cells::render(frame, content_area, retained, false, theme);
        } else {
            let panel_status = if matches!(
                native.phase(),
                DisplayPhase::Ready
                    | DisplayPhase::MoveProjectionUnavailable
                    | DisplayPhase::PanelProjectionUnavailable
                    | DisplayPhase::RenameProjectionUnavailable
            ) && native.projection_status()
                != ClientProjectionStatus::FrameAvailable
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
                let content_area = crate::tui_layout::guidance_area(content_area, global_action);
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
                    Paragraph::new(wrap_text(&Text::from(panel_status), content_area.width))
                        .style(theme.style(Role::TextSecondary)),
                    content_area,
                );
            }
        }
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

fn render_panel_graphics(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &crate::workbench_projection::ClientPanel,
    native: &WorkbenchClientView<'_>,
    draws: &mut Vec<crate::workbench_client::PaneGraphicsDraw>,
) {
    if area.is_empty()
        || native
            .retained_pane_frame(panel.panel_id(), area.width, area.height)
            .is_some()
    {
        return;
    }
    let crop = native
        .pane_live_view(panel.panel_id())
        .filter(|crop| crop.clipped);
    // Host graphics placements have no local camera transform. The raster
    // fallback can share the exact text crop without covering its indicator.
    let physical = crop
        .is_none()
        .then(|| native.physical_graphics(panel, area))
        .flatten();
    if physical.is_none()
        && let Some(canvas) = native.panel_graphics(panel)
    {
        if let Some(crop) = crop
            && let Some(source) = native
                .pane_projection(panel.panel_id())
                .and_then(|projection| projection.frame())
        {
            pane_graphics::render_cropped(
                frame,
                area,
                canvas,
                native.graphics_cell_pixels().as_ref(),
                crop,
                (source.geometry.columns, source.geometry.rows),
            );
        } else {
            pane_graphics::render(frame, area, canvas, native.graphics_cell_pixels().as_ref());
        }
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
    native: &WorkbenchClientView<'_>,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let machines = &view.machines;
    let text = locale.shell();
    if tabs_area.is_empty() {
        return;
    }
    let strip = tabs_area;

    let tab_source = native.tab_create_target();
    let create_target = tab_source.clone().filter(|_| strip.width >= 7);
    let create_width = if create_target.is_some() { 4 } else { 0 };

    let tab_area = Rect::new(strip.x, strip.y, strip.width - create_width, strip.height);
    frame
        .buffer_mut()
        .set_style(strip, theme.style(Role::SurfaceStrip));

    let create_offset = {
        let selected = native.selected_tab_ids();
        let displayed = (0..native.tab_count())
            .filter_map(|index| {
                let tab = native.tab(index)?;
                Some(tabs::groups::GroupTabDisplay {
                    selected: selected.len() > 1 && selected.contains(tab.tab_id()),
                    tab: tabs::TabDisplay {
                        position: tab.position(),
                        active: tab.active(),
                        label: tab.display_label(locale),
                    },
                    group: tab.group(),
                    collapsed: tab
                        .group()
                        .is_some_and(|group| native.group_collapsed(&group.group_id)),
                    pinned: tab.pinned(),
                    agent: machines.tab_agent(tab.tab_id()),
                })
            })
            .collect::<Vec<_>>();
        let tab_target = |index: usize, part: tabs::groups::StripPart| {
            let tab = native.tab(index)?;
            match part {
                tabs::groups::StripPart::Header => Some(HitTarget::TabGroup {
                    tab: tab.tab_id().clone(),
                    group: tab.group()?.group_id.clone(),
                }),
                tabs::groups::StripPart::Tab | tabs::groups::StripPart::Close => {
                    Some(HitTarget::Tab(tab.tab_id().clone()))
                }
                tabs::groups::StripPart::Page { .. } => None,
            }
        };
        // Close and the paging edge markers are release-activated controls, not Tab gestures.
        let control_target = |index: usize, part: tabs::groups::StripPart| {
            let source = tab_source.clone()?;
            let tab = native.tab(index)?.tab_id().clone();
            match part {
                tabs::groups::StripPart::Close => {
                    Some(InteractionTarget::TabCloseRequest { source, tab })
                }
                tabs::groups::StripPart::Page { header } => Some(InteractionTarget::TabStripPage {
                    source,
                    start: crate::workbench_state::TabStripStart { tab, header },
                }),
                tabs::groups::StripPart::Tab | tabs::groups::StripPart::Header => None,
            }
        };
        let is_control = |part: tabs::groups::StripPart| {
            matches!(
                part,
                tabs::groups::StripPart::Close | tabs::groups::StripPart::Page { .. }
            )
        };
        let control_spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
        let start = native.tab_strip_start().and_then(|start| {
            Some(tabs::groups::StripStart {
                index: (0..native.tab_count()).position(|index| {
                    native
                        .tab(index)
                        .is_some_and(|tab| *tab.tab_id() == start.tab)
                })?,
                header: start.header,
            })
        });
        tabs::groups::render(
            frame,
            tab_area,
            &displayed,
            theme,
            text.unnamed_tab_group,
            machines.working_frame(),
            start,
            tabs::groups::GroupInteractions::new(
                hits,
                &mut |hits, index, part, rect| {
                    if is_control(part) {
                        return control_target(index, part).map_or(
                            ProjectedPointer::Rest,
                            |target| {
                                hits.projected_pointer(
                                    HitLayer::Foreground,
                                    rect,
                                    rect,
                                    &target,
                                    control_spec,
                                )
                            },
                        );
                    }
                    let (Some(source), Some(target)) =
                        (tab_source.as_ref(), tab_target(index, part))
                    else {
                        return ProjectedPointer::Rest;
                    };
                    hits.projected_tab_pointer(source, &target, rect)
                },
                &mut |hits, index, part, rect, cue| {
                    let active = native.tab(index).is_some_and(|tab| tab.active());
                    let content = tabs::strip_tooltip(view, active, part, cue, locale, theme);
                    if is_control(part) {
                        if let Some(target) = control_target(index, part) {
                            if let Some(content) = content {
                                hits.record_interaction_tooltip(
                                    HitLayer::Foreground,
                                    rect,
                                    &target,
                                    content,
                                );
                            }
                            hits.record_interaction(
                                HitLayer::Foreground,
                                rect,
                                rect,
                                target,
                                control_spec,
                            );
                        }
                    } else if let (Some(source), Some(target)) =
                        (tab_source.as_ref(), tab_target(index, part))
                    {
                        if let Some(content) = content {
                            hits.record_tooltip(
                                HitLayer::Foreground,
                                rect,
                                target.clone(),
                                content,
                            );
                        }
                        hits.record_tab_target(source, rect, target);
                    }
                },
            ),
        )
    };
    if let Some(target) = create_target {
        let offset = create_offset.min(strip.width - 3);
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
                    controls::ControlSurface::Strip,
                    controls::Availability::Enabled,
                    pointer,
                ),
                theme,
            ) {
                hits.record_interaction_tooltip(
                    HitLayer::Foreground,
                    rect,
                    &interaction,
                    tabs::create_tooltip(view, locale, theme),
                );
                hits.record_interaction(HitLayer::Foreground, rect, painted, interaction, spec);
            }
        }
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
    tab: ClientTabView<'_>,
    input_enabled: bool,
    cursor_visible: bool,
    phase_status: &str,
    locale: Locale,
    history_key: Option<&str>,
    native: &WorkbenchClientView<'_>,
    client_view: &ClientView,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    pane_cursor: &mut Option<Position>,
) {
    // Only the Windows copy-mode surface consumes input ownership and pointer targets here.

    let panel_index = leaf.panel_index;
    let active = tab.active_panel() == Some(panel_index);
    let panel = tab.panel(panel_index);
    let content = leaf.content;
    if !active {
        frame.render_widget(
            Block::new().style(theme.style(Role::SurfaceInactive)),
            content,
        );
    }
    let terminal = panel.map(|panel| panel.panel_id()).and_then(|panel_id| {
        (native.pane_projection_status(panel_id) == ClientProjectionStatus::FrameAvailable)
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
        let copy_active = active && native.copy_mode().is_some();
        let cursor = viewport::render_selected(
            frame,
            content,
            surface,
            terminal,
            cursor_visible && active,
            locale,
            theme,
            panel.and_then(|panel| native.pane_selection(panel.panel_id())),
            copy_active,
        );
        // Only the active leaf yields a cursor.
        *pane_cursor = pane_cursor.or(cursor);
    } else if let Some(terminal) = terminal {
        let show_cursor = cursor_visible && active;
        let show_cursor = show_cursor && native.copy_mode().is_none();
        let tail = panel.and_then(|panel| native.pane_tail_window(panel.panel_id()));
        let crop =
            crate::terminal_live_view::LiveView::new(terminal, content.width, content.height);
        let painted = if tail.is_some() {
            content
        } else {
            crop.content(content)
        };
        let cursor = if let Some((window, rows_before)) = tail {
            terminal_cells::render_tail(
                frame,
                content,
                terminal,
                window,
                rows_before,
                show_cursor,
                theme,
            )
        } else {
            viewport::render_live(
                frame,
                content,
                terminal,
                crop,
                show_cursor,
                history_key,
                None,
                locale,
                theme,
            )
        };
        *pane_cursor = pane_cursor.or(cursor);
        selection::paint_cropped(
            frame,
            painted,
            tail.map_or(
                terminal.scrollback_rows.saturating_add(u64::from(crop.row)),
                |(window, _)| window.route.start_row,
            ),
            if tail.is_some() { 0 } else { crop.column },
            panel.and_then(|panel| native.pane_selection(panel.panel_id())),
            theme.style(Role::SurfaceSelection),
        );
    } else if let Some(failure) = panel.and_then(|panel| native.launch_failure(panel)) {
        terminal_failure::render(frame, content, failure, client_view, locale, theme);
    } else if let Some(retained) = panel.and_then(|panel| {
        native.retained_pane_frame(panel.panel_id(), content.width, content.height)
    }) {
        terminal_cells::render(frame, content, retained, false, theme);
    } else {
        let status = if panel.is_some_and(|p| native.panel_is_unbound(p.panel_id())) {
            locale.native_unbound_terminal()
        } else if matches!(
            native.phase(),
            DisplayPhase::Ready
                | DisplayPhase::MoveProjectionUnavailable
                | DisplayPhase::PanelProjectionUnavailable
                | DisplayPhase::RenameProjectionUnavailable
        ) {
            locale.terminal_status(TerminalStatus::Blocked)
        } else {
            phase_status
        };
        frame.render_widget(
            Paragraph::new(wrap_text(&Text::from(status), content.width))
                .style(theme.style(Role::TextSecondary)),
            content,
        );
    }
    if active
        && input_enabled
        && let Some(copy) = native.copy_mode()
    {
        copy_mode::paint_surface(frame, content, copy, native, theme, hits);
    }
}

/// Paints shared dividers; the caller records their targets after the titles drawn over them.
fn render_dividers(
    frame: &mut Frame<'_>,
    dividers: &[Rect],
    panel: Rect,
    theme: &Theme,
    targets: &[crate::tui_layout::DrawnDivider],
    hits: &HitMapBuilder,
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
            #[cfg(any(test, feature = "test-support"))]
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
        // A hovered or dragged divider takes the focus color, leaving the canvas untouched.
        let style = if pointer == ProjectedPointer::Rest {
            theme.style(if boundary_cell {
                Role::FrameUnfocused
            } else {
                Role::FrameDivider
            })
        } else {
            theme.style(Role::FrameFocused).add_modifier(Modifier::BOLD)
        };

        frame
            .buffer_mut()
            .set_string(x, y, glyph.to_string(), style);
    }
}

/// The Tab strip's surface under its controls.
const STRIP_CONTROL_SURFACE: controls::ControlSurface = controls::ControlSurface::Strip;

fn native_tab<'a>(
    native: &WorkbenchClientView<'a>,
    index: usize,
    locale: Locale,
    pinned_label: &str,
) -> Option<tabs::TabDisplay<'a>> {
    let tab: ClientTabView<'a> = native.tab(index)?;
    Some(tabs::TabDisplay {
        position: tab.position(),
        active: tab.active(),
        label: tabs::label_with_metadata(
            tab.display_label(locale),
            tab.pinned().then_some(pinned_label),
            tab.group_label(),
        ),
    })
}

fn status_overlay_visible(
    view: &ClientView,
    workbench_client: Option<&WorkbenchClientView<'_>>,
    capture_owner: Option<CaptureOwner>,
) -> bool {
    let visible = view.prefix_active()
        || view.overlay().is_some()
        || view.notice().is_some()
        || view.pointer_released()
        || capture_owner.is_some();
    let visible = visible
        || view.navigator_active()
        || view.machines.sidebar_navigation_active()
        || view.resize_pending;
    visible
        || workbench_client.is_some_and(|native| {
            // The first live frame is already visible while its draw acknowledgement
            // settles. Do not flash a connection-only footer for that handoff.
            let connecting_frame = matches!(
                native.phase(),
                DisplayPhase::Starting | DisplayPhase::Switching
            ) && native.projection_status()
                == ClientProjectionStatus::FrameAvailable;
            // A pane closing after a normal Shell exit raises no automatic connection,
            // view-only or exit cue while Server removes it.
            let automatic = !native.normal_exit_closing()
                && ((native.phase() != DisplayPhase::Ready && !connecting_frame)
                    || native
                        .focus_panel_id()
                        .and_then(|panel| native.terminal_end(panel))
                        .is_some()
                    || native.controller() == Some(DisplayController::ReadOnly));
            let visible = automatic
                || native.notice().is_some()
                || native.takeover_status().is_some()
                || native.setup_status().is_some()
                || native.close_status().is_some()
                || native.workspace_close_status().is_some()
                || native.close_set_status().is_some();
            visible || native.duplicate_progress().is_some()
        })
}

#[allow(clippy::too_many_arguments)]
fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    locale: Locale,
    facts: status_line::StatusFacts<'_>,
    tier: ViewportTier,
    theme: &Theme,
    workbench_client: Option<&WorkbenchClientView<'_>>,
) -> status_line::StatusPaint {
    status_line::render(
        frame,
        area,
        view,
        capture_owner,
        locale,
        facts,
        tier,
        theme,
        workbench_client,
    )
}

fn render_sidebar(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    text: ShellText,
    backend_status: SessionStatus<'_>,
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
            fit(text.session_section.to_owned()),
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
    let style = theme.style(Role::FrameFocused).add_modifier(Modifier::BOLD);
    for edge in [
        Rect::new(outline.x, outline.y, outline.width, 1),
        Rect::new(outline.x, outline.bottom() - 1, outline.width, 1),
        Rect::new(outline.x, outline.y, 1, outline.height),
        Rect::new(outline.right() - 1, outline.y, 1, outline.height),
    ] {
        frame.buffer_mut().set_style(edge, style);
    }
}

/// Every overlay shares one opaque theme surface and one neutral single-line frame.
///
/// An overlay already owns input exclusively and occludes what it covers; its heading, caret, or
/// provisional marker identifies that ownership, so the border never repeats a focus accent.
/// The sidebar row that owns an open row menu, and whether the keyboard opened it.
fn sidebar_menu_hold(
    view: &ClientView,
    native: &crate::workbench_client::WorkbenchClientView<'_>,
) -> Option<(crate::machine_picker::SidebarTarget, bool)> {
    use crate::{action_menu::MenuTarget, machine_picker::SidebarTarget};
    let (target, anchor) = native.action_menu_root()?;
    let anchor = anchor?;
    let session_id = native.action_menu_source()?.session;
    let machine = view.machines.current.clone();
    let target = match target {
        MenuTarget::Project(project_id) => SidebarTarget::Project {
            machine,
            session_id,
            project_id: project_id.clone(),
        },
        MenuTarget::Workspace(location) => SidebarTarget::Context {
            machine,
            target: crate::workbench_projection::ClientContextTarget {
                session_id,
                location: location.clone(),
            },
        },
        _ => return None,
    };
    Some((
        target,
        matches!(anchor, crate::action_menu::MenuAnchor::Row { .. }),
    ))
}

/// A surviving wide lead outside a floating box would make the terminal diff skip its border.
/// Removes only the glyph remnants that cross the box edge, retaining their style.
fn clear_crossing_wide_cells(frame: &mut Frame<'_>, area: Rect) {
    let buffer = frame.buffer_mut();
    for y in area.y..area.bottom() {
        let mut x = buffer.area.x;
        while x < area.right() {
            let cell = &buffer[(x, y)];
            let width = u16::try_from(display_width(cell.symbol()).max(1)).unwrap_or(u16::MAX);
            let next = x.saturating_add(width).min(buffer.area.right());
            if (x < area.x && next > area.x) || next > area.right() {
                let style = cell.style();
                for column in x..next {
                    if column < area.x || column >= area.right() {
                        let cell = &mut buffer[(column, y)];
                        cell.reset();
                        cell.set_style(style);
                    }
                }
            }
            x = next;
        }
    }
}

#[cfg(test)]
mod tests;
