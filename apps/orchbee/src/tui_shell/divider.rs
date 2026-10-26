//! One admitted local resize projection feeds both frame composition and viewport observations.
use super::{Chrome, ClientView, HitMapBuilder, TerminalSurface};
use crate::{
    tui_layout::{DividerDrag, DrawnSidebarWidth, ResizeProjection, SidebarWidthDrag},
    workbench_client::NativeDrawReceipt,
};
use ratatui::{Frame, widgets::Clear};

#[derive(Clone, Copy)]
pub(super) enum Preview<'a> {
    Divider(&'a DividerDrag),
    Sidebar(&'a SidebarWidthDrag),
}

pub(super) struct Presentation<'a> {
    pub(super) projection: &'a ResizeProjection,
    pub(super) preview: Option<Preview<'a>>,
}

pub(super) fn project(
    view: &ClientView,
    surface: &mut (impl TerminalSurface + ?Sized),
    viewport: ratatui::layout::Rect,
    proposed: &ResizeProjection,
    transport_allowed: bool,
) -> ResizeProjection {
    let native_claims = match surface.view() {
        Some(native) => native.capture_claims(),
        _ => crate::tui_composition::NativeCaptureClaims::default(),
    };
    let allowed = transport_allowed
        && !view.prefix_active()
        && crate::tui_composition::project_capture(
            super::client_capture_claims(view),
            native_claims,
        )
        .is_none();
    if !allowed {
        let _retired = surface.pending_resize_projection(viewport, view.sidebar_surface(), false);
        return ResizeProjection::Canonical;
    }
    match proposed {
        ResizeProjection::Divider { source, .. }
            if view.overlay().is_none()
                && source.viewport == viewport
                && surface
                    .divider_projection_current(source, (viewport, view.sidebar_surface())) =>
        {
            proposed.clone()
        }
        ResizeProjection::Sidebar { source, .. }
            if view.machines.available
                && matches!(
                    view.overlay(),
                    None | Some(crate::client_view::Overlay::BusinessSidebar)
                )
                && DrawnSidebarWidth::new(viewport, view.sidebar_surface()) == Some(*source) =>
        {
            proposed.clone()
        }
        ResizeProjection::Canonical => surface.pending_resize_projection(
            viewport,
            view.sidebar_surface(),
            view.overlay().is_none(),
        ),
        _ => ResizeProjection::Canonical,
    }
}

/// A lost source gets one clean canonical projection in this same frame, never a second flush.
pub(super) fn render(
    frame: &mut Frame<'_>,
    view: &ClientView,
    chrome: Chrome<'_>,
    surface: &(impl TerminalSurface + ?Sized),
    graphics: Option<&crate::tui_graphics::Surface>,
    presentation: Presentation<'_>,
    hits: &mut HitMapBuilder,
) -> (NativeDrawReceipt, bool) {
    let mut candidate = presentation.projection;
    loop {
        if let Some(graphics) = graphics {
            graphics.begin_frame(
                *candidate != ResizeProjection::Canonical
                    || view.machines.open
                    || (!view.notifications.empty()
                        && crate::notifications::available(view, surface.view())),
            );
        }
        let mut session = surface.view();
        if let Some(native) = session.as_mut() {
            native.set_graphics_surface(graphics.cloned());
        }
        let receipt = crate::tui_render::render_terminal_shell_with_resize(
            frame,
            view,
            chrome.locale,
            session,
            chrome.theme,
            hits,
            candidate,
        );
        if *candidate == ResizeProjection::Canonical {
            return (receipt, false);
        }
        let exposed = candidate
            .divider()
            .is_some_and(|target| hits.exposes_divider(&target))
            || candidate
                .sidebar_target()
                .is_some_and(|target| hits.exposes_sidebar_width(&target));
        let painted = exposed
            && match presentation.preview {
                Some(Preview::Divider(drag)) => crate::tui_render::render_divider_preview(
                    frame,
                    view,
                    chrome.locale,
                    drag,
                    chrome.theme,
                    hits,
                ),
                Some(Preview::Sidebar(drag)) => {
                    crate::tui_render::render_width_preview(frame, view, drag, chrome.theme, hits)
                }
                None => true,
            };
        if painted {
            return (receipt.with_graphics(Vec::new()).with_position(None), true);
        }
        // Covering during the rejected candidate pass suppressed image planning. Reset both
        // plans and cells before repainting with no preview; the new map then cancels the gesture.
        hits.reset_candidate();
        frame.render_widget(Clear, frame.area());
        candidate = &ResizeProjection::Canonical;
    }
}
