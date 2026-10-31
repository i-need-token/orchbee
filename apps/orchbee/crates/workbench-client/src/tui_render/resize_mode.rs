//! Mode controls reuse the keyboard resize owner and carry the painted layout.
use crate::{
    client_view::{ClientView, PanelDirection},
    localization::Locale,
    terminal_text::display_width,
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer,
        ResizeControl,
    },
    tui_layout::DrawnResize,
    tui_theme::{Role, Theme},
    workbench_client::WorkbenchClientView,
};
use ratatui::{Frame, layout::Rect, widgets::Clear};

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, NavigationState, OperationState,
    PointerPresentation, action_rect, icon_action_rect, paint_pointer_target, render_action,
    render_icon_action,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompactControl {
    Adjust {
        direction: PanelDirection,
        fine: bool,
    },
    Step {
        fine: bool,
    },
    Help,
}

impl CompactControl {
    const fn icon(self) -> Option<IconActionGlyph> {
        match self {
            Self::Adjust {
                direction: PanelDirection::Left,
                ..
            } => Some(IconActionGlyph::Previous),
            Self::Adjust {
                direction: PanelDirection::Down,
                ..
            } => Some(IconActionGlyph::Down),
            Self::Adjust {
                direction: PanelDirection::Up,
                ..
            } => Some(IconActionGlyph::Up),
            Self::Adjust {
                direction: PanelDirection::Right,
                ..
            } => Some(IconActionGlyph::Next),
            Self::Step { .. } => None,
            Self::Help => Some(IconActionGlyph::Help),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompactGeometry {
    control: CompactControl,
    rect: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExitGeometry {
    Action(Rect),
    Close(Rect),
}

impl ExitGeometry {
    const fn rect(self) -> Rect {
        match self {
            Self::Action(rect) | Self::Close(rect) => rect,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResizeGeometry {
    controls: Vec<CompactGeometry>,
    exit: ExitGeometry,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    view: &ClientView,
    native: Option<&WorkbenchClientView<'_>>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    if !view.resize_mode() {
        return;
    }
    let viewport = frame.area();
    let area = super::controls::mode_bar(viewport, view.sidebar_surface());
    let Some(geometry) = resolve_geometry(
        viewport,
        area,
        locale.resize_done_label(),
        view.resize_pointer_fine,
        theme,
    ) else {
        return;
    };
    let resize_target = native
        .and_then(WorkbenchClientView::divider_source)
        .map(|source| DrawnResize {
            source,
            viewport,
            sidebar: view.sidebar_surface(),
        });

    hits.cover_chrome(area);
    hits.record(HitLayer::ChromeSurface, area, HitTarget::OverlaySurface);
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));

    for control in &geometry.controls {
        paint_compact_control(
            frame,
            *control,
            resize_target.as_ref(),
            view.resize_pending,
            theme,
            hits,
        );
    }
    paint_exit(
        frame,
        geometry.exit,
        locale.resize_done_label(),
        theme,
        hits,
    );
}

fn resolve_geometry(
    bounds: Rect,
    area: Rect,
    done_label: &str,
    fine: bool,
    theme: &Theme,
) -> Option<ResizeGeometry> {
    if area.is_empty() || bounds.intersection(area) != area {
        return None;
    }
    let exit_state = action_state(PointerPresentation::Rest, true);
    let exit_spec = ActionSpec::new(
        done_label,
        ActionKind::Secondary,
        ControlSurface::Chrome,
        exit_state,
    );
    let exit = action_rect(bounds, area, exit_spec, theme)
        .and_then(|seed| {
            let rect = Rect::new(area.right().checked_sub(seed.width)?, area.y, seed.width, 1);
            (action_rect(bounds, rect, exit_spec, theme) == Some(rect))
                .then_some(ExitGeometry::Action(rect))
        })
        .or_else(|| super::controls::close_rect(bounds, area).map(ExitGeometry::Close))?;

    let optional_right = exit.rect().x.saturating_sub(1);
    let mut cursor = area.x;
    let mut controls = Vec::new();
    for control in [
        CompactControl::Adjust {
            direction: PanelDirection::Left,
            fine,
        },
        CompactControl::Adjust {
            direction: PanelDirection::Down,
            fine,
        },
        CompactControl::Adjust {
            direction: PanelDirection::Up,
            fine,
        },
        CompactControl::Adjust {
            direction: PanelDirection::Right,
            fine,
        },
        CompactControl::Step { fine },
        CompactControl::Help,
    ] {
        let Some(remaining) = optional_right.checked_sub(cursor) else {
            break;
        };
        let offered = Rect::new(cursor, area.y, remaining, 1);
        let rect = match control {
            CompactControl::Step { fine } => step_rect(bounds, offered, fine),
            _ => icon_action_rect(bounds, offered),
        };
        let Some(rect) = rect.filter(|rect| area.intersection(*rect) == *rect) else {
            break;
        };
        controls.push(CompactGeometry { control, rect });
        cursor = rect.right().saturating_add(1);
    }
    Some(ResizeGeometry { controls, exit })
}

fn step_rect(bounds: Rect, area: Rect, fine: bool) -> Option<Rect> {
    let width = u16::try_from(display_width(step_label(fine))).ok()?;
    if area.height == 0 || width == 0 || width > area.width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    (area.intersection(rect) == rect && bounds.intersection(rect) == rect).then_some(rect)
}

const fn step_label(fine: bool) -> &'static str {
    if fine { "[1%]" } else { "[5%]" }
}

fn paint_compact_control(
    frame: &mut Frame<'_>,
    geometry: CompactGeometry,
    resize_target: Option<&DrawnResize>,
    pending: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let control = match geometry.control {
        CompactControl::Adjust { direction, fine } => {
            resize_target.map(|target| ResizeControl::Adjust {
                target: Box::new(target.clone()),
                direction,
                fine,
            })
        }
        CompactControl::Step { fine } => Some(ResizeControl::Step(fine)),
        CompactControl::Help => Some(ResizeControl::Help),
    };
    let enabled = !matches!(geometry.control, CompactControl::Adjust { .. }) || !pending;
    let available = control.is_some() && enabled;
    let availability = pointer_availability(available);
    let target = control.map(HitTarget::Resize);
    let pointer = target.as_ref().map_or(ProjectedPointer::Rest, |target| {
        hits.projected_modal_pointer(HitLayer::Chrome, geometry.rect, target, availability)
    });
    let painted = if let Some(glyph) = geometry.control.icon() {
        render_icon_action(
            frame,
            geometry.rect,
            IconActionSpec::new(
                glyph,
                ControlSurface::Chrome,
                component_availability(available),
                pointer_presentation(pointer),
            ),
            theme,
        )
    } else if let CompactControl::Step { fine } = geometry.control {
        render_step_value(
            frame,
            geometry.rect,
            fine,
            pointer_presentation(pointer),
            available,
            theme,
        )
    } else {
        None
    };
    if painted != Some(geometry.rect) {
        return;
    }
    if let Some(target) = target {
        hits.record_modal_control(HitLayer::Chrome, geometry.rect, target, availability);
    }
}

fn render_step_value(
    frame: &mut Frame<'_>,
    rect: Rect,
    fine: bool,
    pointer: PointerPresentation,
    available: bool,
    theme: &Theme,
) -> Option<Rect> {
    if step_rect(frame.area(), rect, fine) != Some(rect) {
        return None;
    }
    let role = if available {
        Role::TextPrimary
    } else {
        Role::TextDisabled
    };
    let style = theme.content_on_surface(Role::SurfaceChrome, None, Some(role), theme.style(role));
    frame.render_widget(
        ratatui::widgets::Paragraph::new(step_label(fine)).style(style),
        rect,
    );
    paint_pointer_target(
        frame,
        rect,
        ControlSurface::Chrome,
        pointer,
        role,
        component_availability(available),
        theme,
    )
}

fn paint_exit(
    frame: &mut Frame<'_>,
    geometry: ExitGeometry,
    done_label: &str,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let rect = geometry.rect();
    let target = HitTarget::Resize(ResizeControl::Exit);
    let availability = InteractionAvailability::Enabled;
    let pointer = hits.projected_modal_pointer(HitLayer::Chrome, rect, &target, availability);
    let painted = match geometry {
        ExitGeometry::Action(_) => render_action(
            frame,
            rect,
            ActionSpec::new(
                done_label,
                ActionKind::Secondary,
                ControlSurface::Chrome,
                action_state(pointer_presentation(pointer), true),
            ),
            theme,
        ),
        ExitGeometry::Close(_) => render_icon_action(
            frame,
            rect,
            IconActionSpec::new(
                IconActionGlyph::Close,
                ControlSurface::Chrome,
                Availability::Enabled,
                pointer_presentation(pointer),
            ),
            theme,
        ),
    };
    if painted == Some(rect) {
        hits.record_modal_control(HitLayer::Chrome, rect, target, availability);
    }
}

const fn action_state(pointer: PointerPresentation, enabled: bool) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        pointer,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        component_availability(enabled),
        OperationState::Idle,
    )
}

const fn component_availability(enabled: bool) -> Availability {
    if enabled {
        Availability::Enabled
    } else {
        Availability::Disabled
    }
}

const fn pointer_availability(enabled: bool) -> InteractionAvailability {
    if enabled {
        InteractionAvailability::Enabled
    } else {
        InteractionAvailability::Disabled
    }
}

const fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

#[cfg(test)]
mod tests;
