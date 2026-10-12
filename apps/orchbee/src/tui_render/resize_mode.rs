//! Mode controls reuse the keyboard resize owner and carry the painted layout.
use crate::{
    client_view::{ClientView, PanelDirection},
    localization::Locale,
    terminal_session::NativeSessionView,
    terminal_text::display_width,
    tui_layout::DrawnResize,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget, ResizeControl},
    tui_theme::{Role, Theme},
};
use ratatui::{Frame, layout::Rect, widgets::Clear};

pub(super) fn render(
    frame: &mut Frame<'_>,
    view: &ClientView,
    native: Option<&NativeSessionView<'_>>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    if !view.resize_mode() {
        return;
    }
    let viewport = frame.area();
    let area = super::controls::mode_bar(viewport, view.sidebar_surface());
    if area.is_empty() {
        return;
    }
    hits.cover_chrome(area);
    hits.record(HitLayer::ChromeSurface, area, HitTarget::OverlaySurface);
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));
    let done = locale.resize_done_button();
    let width = u16::try_from(display_width(done)).unwrap_or(u16::MAX);
    let close = if width <= area.width {
        super::controls::button(
            frame,
            Rect::new(area.right() - width, area.y, width, 1),
            done,
            theme,
            false,
        )
    } else {
        super::controls::close(frame, area, theme)
    };
    let Some(close) = close else {
        return;
    };
    hits.record(
        HitLayer::Chrome,
        close,
        HitTarget::Resize(ResizeControl::Exit),
    );
    let mut remaining = Rect::new(area.x, area.y, close.x.saturating_sub(area.x + 1), 1);
    let target = native
        .and_then(NativeSessionView::divider_source)
        .map(|source| DrawnResize {
            source,
            viewport,
            sidebar: view.sidebar_surface(),
        });
    for (label, direction) in [
        ("[<]", PanelDirection::Left),
        ("[v]", PanelDirection::Down),
        ("[^]", PanelDirection::Up),
        ("[>]", PanelDirection::Right),
    ] {
        if let Some(rect) = super::controls::button(frame, remaining, label, theme, false) {
            if let Some(target) = target.as_ref().filter(|_| !view.resize_pending) {
                hits.record(
                    HitLayer::Chrome,
                    rect,
                    HitTarget::Resize(ResizeControl::Adjust {
                        target: Box::new(target.clone()),
                        direction,
                        fine: view.resize_pointer_fine,
                    }),
                );
            } else {
                frame
                    .buffer_mut()
                    .set_style(rect, theme.style(Role::TextDisabled));
            }
            remaining.x = rect.right() + 1;
            remaining.width = close.x.saturating_sub(remaining.x + 1);
        }
    }
    for (label, control) in [
        (
            if view.resize_pointer_fine {
                "[1%]"
            } else {
                "[5%]"
            },
            ResizeControl::Step(view.resize_pointer_fine),
        ),
        ("[?]", ResizeControl::Help),
    ] {
        if let Some(rect) = super::controls::button(frame, remaining, label, theme, false) {
            hits.record(HitLayer::Chrome, rect, HitTarget::Resize(control));
            remaining.x = rect.right() + 1;
            remaining.width = close.x.saturating_sub(remaining.x + 1);
        }
    }
}
