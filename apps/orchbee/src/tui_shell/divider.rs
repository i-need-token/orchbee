//! Frame composition covers images only for an actually painted resize preview.
use super::{Chrome, ClientView, HitMapBuilder, TerminalSurface, render_terminal_shell};
use crate::{
    terminal_session::NativeDrawReceipt,
    tui_layout::{DividerDrag, SidebarWidthDrag},
};
use ratatui::{Frame, widgets::Clear};

#[derive(Clone, Copy)]
pub(super) enum Preview<'a> {
    Divider(&'a DividerDrag),
    Sidebar(&'a SidebarWidthDrag),
}

/// A lost source gets one clean canonical projection in this same frame, never a second flush.
pub(super) fn render(
    frame: &mut Frame<'_>,
    view: &ClientView,
    chrome: Chrome<'_>,
    surface: &(impl TerminalSurface + ?Sized),
    graphics: Option<&crate::tui_graphics::Surface>,
    preview: Option<Preview<'_>>,
    hits: &mut HitMapBuilder,
) -> (NativeDrawReceipt, bool) {
    let mut candidate = preview;
    loop {
        if let Some(graphics) = graphics {
            graphics.begin_frame(
                candidate.is_some()
                    || view.machines.open
                    || (!view.notifications.empty()
                        && crate::notifications::available(view, surface.view())),
            );
        }
        let mut session = surface.view();
        if let Some(crate::terminal_session::TerminalSessionView::Native(native)) = session.as_mut()
        {
            native.set_graphics_surface(graphics.cloned());
        }
        let receipt =
            render_terminal_shell(frame, view, chrome.locale, session, chrome.theme, hits);
        let Some(preview) = candidate.take() else {
            return (receipt, false);
        };
        let painted = match preview {
            Preview::Divider(drag) => crate::tui_render::render_divider_preview(
                frame,
                view,
                chrome.locale,
                drag,
                chrome.theme,
                hits,
            ),
            Preview::Sidebar(drag) => {
                crate::tui_render::render_width_preview(frame, view, drag, chrome.theme, hits)
            }
        };
        if painted {
            return (receipt.with_graphics(Vec::new()).with_position(None), true);
        }
        // Covering during the rejected candidate pass suppressed image planning. Reset both
        // plans and cells before repainting with no preview; the new map then cancels the gesture.
        hits.reset_candidate();
        frame.render_widget(Clear, frame.area());
    }
}
