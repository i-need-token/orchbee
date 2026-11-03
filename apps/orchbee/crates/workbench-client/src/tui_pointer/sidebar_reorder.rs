//! One primary-button owner for sidebar clicks and drag organization.
//! Preview never mutates navigation; only a matching release can submit a move.
use super::{
    FrameHitMap, Gesture, HitRegionResolution, InteractionTarget, MouseAction, MouseButton,
    MouseEvent, PointerOutcome, PointerState, PrimaryContract, SidebarReorderDraw,
    SidebarReorderPaint, contains,
};
use crate::client_view::{ClientView, Modifiers};
use crate::machine_picker::{DrawnSidebarTarget, SidebarPart, SidebarTarget};
use crate::tui_hit_map::sidebar_reorder::Placement;
use crate::workbench_client::input::{
    LayoutAction, SidebarReorderSource, SurfaceAction, TerminalInputPort,
};
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::NavigationConfiguration;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
mod destination;
use destination::{Drop, Item};
#[cfg(test)]
mod tests;

/// Immutable gesture data; geometry resolves against the candidate frame being painted.
pub struct SidebarReorderPreview {
    drag: SidebarReorder,
}
impl super::HitMapBuilder {
    pub fn resolve_sidebar_reorder_preview(
        &self,
        preview: &SidebarReorderPreview,
    ) -> Option<SidebarReorderDraw> {
        preview.resolve(&self.map)
    }
}

impl SidebarReorderPreview {
    pub(super) fn resolve(&self, map: &FrameHitMap) -> Option<SidebarReorderDraw> {
        self.drag.preview(map)
    }
}

#[derive(Clone)]
pub(super) struct SidebarReorder {
    pub(super) source: SidebarReorderSource,
    paint: SidebarReorderPaint,
    target: DrawnSidebarTarget,
    fingerprint: crate::tui_hit_map::interaction::Fingerprint,
    item: Item,
    origin: (u16, u16),
    position: (u16, u16),
    dragging: bool,
    activation_cancelled: bool,
    last_scroll: Instant,
}

fn row_at(map: &FrameHitMap, column: u16, row: u16) -> Option<(&DrawnSidebarTarget, Rect)> {
    let HitRegionResolution::Region(region) = map.hit_region(column, row) else {
        return None;
    };
    let interaction = region.interaction.as_ref()?;
    let InteractionTarget::Sidebar(target) = interaction.target() else {
        return None;
    };
    (target.part() == SidebarPart::Primary).then_some((target, region.rect))
}

impl SidebarReorder {
    fn begin(
        source: SidebarReorderSource,
        map: &FrameHitMap,
        view: &ClientView,
        event: MouseEvent,
        now: Instant,
    ) -> Option<Self> {
        let paint = map.sidebar_reorder.as_ref()?;
        if !map
            .menu_source
            .as_ref()
            .is_some_and(|painted| source.matches_menu_source(painted))
        {
            return None;
        }
        if (map.overlay_active && paint.layer != super::HitLayer::Overlay)
            || paint.source != view.machines.sidebar_source()
        {
            return None;
        }
        if !source.matches_sidebar(&paint.source, &view.machines.current) {
            return None;
        }
        let (target, _) = row_at(map, event.column, event.row)?;
        if target.target().machine() != &view.machines.current
            || target.target().session_id() != source.session_id()
        {
            return None;
        }
        // Reuse the Workspace row capability for reordering.
        if !paint.source.has_part(target.target(), SidebarPart::Menu) {
            return None;
        }
        let item = Item::from_target(target.target())?;
        Some(Self {
            source,
            paint: paint.clone(),
            target: target.clone(),
            fingerprint: map.interaction_at(event.column, event.row)?,
            item,
            origin: (event.column, event.row),
            position: (event.column, event.row),
            dragging: false,
            activation_cancelled: false,
            last_scroll: now,
        })
    }

    /// Whether `map` still paints this gesture's surface and catalog. Before a drag latches, the
    /// press is a click on its row, so only that row's own painted facts matter: a repaint after
    /// an unrelated canonical commit keeps it. A drag previews against its canonical baseline,
    /// so the frame must be painted from that source.
    pub(super) fn painted_in(&self, map: &FrameHitMap) -> bool {
        let Some(paint) = &map.sidebar_reorder else {
            return false;
        };
        let surface = (!map.overlay_active || paint.layer == super::HitLayer::Overlay)
            && paint.layer == self.paint.layer
            && paint.viewport == self.paint.viewport
            && paint.body == self.paint.body
            && paint.footer == self.paint.footer
            && paint.source == self.paint.source;
        surface
            && if self.dragging {
                map.menu_source
                    .as_ref()
                    .is_some_and(|painted| self.source.matches_menu_source(painted))
            } else {
                map.interaction_at(self.origin.0, self.origin.1).as_ref() == Some(&self.fingerprint)
            }
    }

    /// Before a drag, takes the exposure `map` recorded for the pressed row when that frame
    /// painted the press and then revealed the row's pointer-only controls, as a generic arm
    /// does: a press read together with the motion onto the row precedes the frame that reveals
    /// its close control.
    pub(super) fn follow_reveal(&mut self, map: &FrameHitMap) {
        if self.dragging || !map.revealed_for_pointer(&self.fingerprint) {
            return;
        }
        if let Some(current) = map
            .interaction_at(self.origin.0, self.origin.1)
            .filter(|current| current.same_interaction(&self.fingerprint))
        {
            self.fingerprint = current;
        }
    }

    /// Whether the live canonical source still admits this gesture. A click owns no canonical
    /// operation, so background refreshes and unrelated commits never end it. A drag's move is
    /// computed from its baseline: a changed source ends it, while a refresh still in flight
    /// (no live source yet) changes nothing it knows, and its drop rechecks the source anyway.
    fn baseline_current(&self, live: Option<&SidebarReorderSource>) -> bool {
        !self.dragging || live.is_none_or(|live| *live == self.source)
    }

    fn destination(&self, map: &FrameHitMap) -> Option<Drop> {
        let paint = map.sidebar_reorder.as_ref()?;
        let (x, y) = self.position;
        if contains(paint.footer, x, y) {
            return self
                .source
                .destination(&self.item, None, false, paint.footer);
        }
        if !contains(paint.body, x, y) {
            return None;
        }
        // Child controls remain their own buttons on press. During an established
        // drag their row is a destination, without invoking disclosure or Create.
        let entry = self
            .entries(map)
            .into_iter()
            .find(|entry| contains(entry.rows, x, y))?;
        let after = after(entry.rows, y, entry.rows.y > self.origin.1);
        self.source
            .destination(&self.item, Some(&entry.item), after, entry.block)
    }

    /// The painted entries of this gesture's machine and Session in display order.
    fn entries(&self, map: &FrameHitMap) -> Vec<Entry> {
        entries(
            map,
            self.target.target().machine(),
            self.source.session_id(),
            matches!(self.item, Item::Project(_)),
        )
    }

    fn preview(&self, map: &FrameHitMap) -> Option<SidebarReorderDraw> {
        if !self.dragging || !self.painted_in(map) {
            return None;
        }
        Some(SidebarReorderDraw {
            grabbed: map.regions.iter().find_map(|region| {
                let InteractionTarget::Sidebar(target) = region.interaction.as_ref()?.target()
                else {
                    return None;
                };
                (target.part() == SidebarPart::Primary && target.target() == self.target.target())
                    .then_some(region.rect)
            }),
            destination: self.destination(map).map(|drop| {
                (
                    insertion_edge(drop.area, drop.placement, self.paint.body),
                    drop.placement,
                    drop.label,
                )
            }),
            footer: self.paint.footer,
            workspace: matches!(self.item, Item::Workspace(_)),
        })
    }
}

/// One painted entry a drag can land on. `rows` is where the pointer selects it: a Workspace row
/// with its Agent rows, or a Project heading. `block` is every line it moves as: a Project heading
/// with the rows it groups.
struct Entry {
    item: Item,
    rows: Rect,
    block: Rect,
}

fn entries(
    map: &FrameHitMap,
    machine: &crate::machine_picker::Choice,
    session: &orchbee_workbench_protocol::SessionId,
    projects: bool,
) -> Vec<Entry> {
    let mut rows = map
        .regions
        .iter()
        .filter_map(|region| {
            let InteractionTarget::Sidebar(drawn) = region.interaction.as_ref()?.target() else {
                return None;
            };
            (drawn.part() == SidebarPart::Primary
                && drawn.target().machine() == machine
                && drawn.target().session_id() == session)
                .then_some((region.rect, drawn.target()))
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|(rect, _)| rect.y);
    let mut entries: Vec<Entry> = Vec::new();
    #[derive(PartialEq)]
    enum Open {
        Workspace,
        Project,
    }
    // Which kind of entry the following Agent or grouped rows still belong to.
    let mut open = None;
    for (rect, target) in rows {
        match (target, Item::from_target(target)) {
            (SidebarTarget::Project { .. }, Some(item)) => {
                entries.push(Entry {
                    item,
                    rows: rect,
                    block: rect,
                });
                open = Some(Open::Project);
            }
            (SidebarTarget::Context { .. }, Some(item)) if !projects => {
                entries.push(Entry {
                    item,
                    rows: rect,
                    block: rect,
                });
                open = Some(Open::Workspace);
            }
            (SidebarTarget::Agent { .. }, _) if open == Some(Open::Workspace) => {
                if let Some(entry) = entries.last_mut() {
                    entry.rows = entry.rows.union(rect);
                    entry.block = entry.block.union(rect);
                }
            }
            (SidebarTarget::Context { .. } | SidebarTarget::Agent { .. }, _)
                if projects && open == Some(Open::Project) =>
            {
                if let Some(entry) = entries.last_mut() {
                    entry.block = entry.block.union(rect);
                }
            }
            _ => open = None,
        }
    }
    entries
}

/// Whether a pointer on `rows` places the dragged entry after it: from the lower half of a taller
/// entry, and from the far side of a one-line entry, the side away from the dragged one.
fn after(rows: Rect, y: u16, below_dragged: bool) -> bool {
    let half = if below_dragged {
        rows.height / 2
    } else {
        rows.height.div_ceil(2)
    };
    y >= rows.y.saturating_add(half)
}

/// The one line whose underline lies on the gap a drop fills: the entry's last line for After, the
/// line above it for Before. Into and the footer keep their whole area.
fn insertion_edge(area: Rect, placement: Placement, body: Rect) -> Rect {
    match placement {
        Placement::After => Rect {
            y: area.bottom().saturating_sub(1),
            height: area.height.min(1),
            ..area
        },
        Placement::Before if area.y > body.y => Rect {
            y: area.y - 1,
            height: 1,
            ..area
        },
        Placement::Before => Rect {
            height: area.height.min(1),
            ..area
        },
        Placement::Into | Placement::Ungrouped | Placement::End => area,
    }
}

impl PointerState {
    pub(crate) fn sidebar_reorder_captured(&self) -> bool {
        matches!(self.gesture, Gesture::SidebarReorder(_))
    }
    pub(crate) fn sidebar_reorder_preview(&self) -> Option<SidebarReorderPreview> {
        let Gesture::SidebarReorder(drag) = &self.gesture else {
            return None;
        };
        (drag.dragging && drag.painted_in(self.map.as_ref()?))
            .then(|| SidebarReorderPreview { drag: drag.clone() })
    }

    pub(crate) fn cancel_sidebar_reorder(&mut self) -> bool {
        if !self.sidebar_reorder_captured() {
            return false;
        }
        self.gesture = Gesture::Idle;
        self.cancelled_chrome_release
            .get_or_insert(super::CancelledChromeRelease::SidebarReorder);
        true
    }
    pub(super) fn sidebar_reorder_projection(
        &self,
        projection: super::PointerProjection,
    ) -> super::PointerProjection {
        let Gesture::SidebarReorder(drag) = &self.gesture else {
            return projection;
        };
        projection.with_specialized_owner(
            (!drag.dragging && !drag.activation_cancelled).then(|| drag.fingerprint.clone()),
        )
    }
    pub(super) fn begin_sidebar_reorder(
        &mut self,
        view: &ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: Instant,
        event: MouseEvent,
    ) -> bool {
        if !self.can_admit_interaction()
            || event.action != MouseAction::Press
            || event.button != MouseButton::Left
            || event.mods != Modifiers::NONE
            || view.resize_mode()
            || view.prefix_active()
        {
            return false;
        }
        let Some(map) = self.map.as_ref() else {
            return false;
        };
        if row_at(map, event.column, event.row).is_none() {
            return false;
        }
        let Some(drag) = port
            .sidebar_reorder_source()
            .and_then(|source| SidebarReorder::begin(source, map, view, event, now))
        else {
            return false;
        };
        self.click = None;
        self.wheel.clear();
        self.gesture = Gesture::SidebarReorder(drag);
        true
    }
    pub(super) fn sidebar_reorder_event(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::SidebarReorder(mut drag) = std::mem::replace(&mut self.gesture, Gesture::Idle)
        else {
            return PointerOutcome::Ignored;
        };
        let release = event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None);
        let alive = event.mods == Modifiers::NONE
            && !(event.action == MouseAction::Motion && event.button != MouseButton::Left)
            && !view.resize_mode()
            && view.machines.sidebar_source() == drag.paint.source
            && drag.baseline_current(port.sidebar_reorder_source().as_ref())
            && self.map.as_ref().is_some_and(|map| drag.painted_in(map));
        if !alive
            || (event.action == MouseAction::Press
                && event.button != MouseButton::WheelUp
                && event.button != MouseButton::WheelDown)
        {
            if !release {
                self.cancelled_chrome_release
                    .get_or_insert(super::CancelledChromeRelease::SidebarReorder);
            }
            return PointerOutcome::SidebarReorderEnded;
        }
        if release {
            drag.position = (event.column, event.row);
            let map = self.map.as_ref().expect("alive drag has an admitted frame");
            if drag.dragging {
                if let Some(drop) = drag.destination(map) {
                    port.surface_action(SurfaceAction::Layout(LayoutAction::SidebarReorder {
                        source: &drag.source,
                        operation: drop.operation,
                    }));
                }
                return PointerOutcome::SidebarReorderEnded;
            }
            if !drag.activation_cancelled
                && map.interaction_at(event.column, event.row).as_ref() == Some(&drag.fingerprint)
            {
                return PointerOutcome::Interaction {
                    target: InteractionTarget::Sidebar(drag.target),
                    contract: PrimaryContract::ActivateOnRelease,
                };
            }
            return PointerOutcome::SidebarReorderEnded;
        }
        if event.action == MouseAction::Motion && event.button == MouseButton::Left {
            drag.position = (event.column, event.row);
            if !drag.dragging
                && (drag.origin.0.abs_diff(event.column) >= 2 || drag.origin.1 != event.row)
            {
                drag.dragging = true;
                // The drag begins now, so it previews and moves against the canonical source
                // painted now; the click it grew from never depended on one.
                if let Some(live) = port.sidebar_reorder_source()
                    && self.map.as_ref().is_some_and(|map| {
                        map.menu_source
                            .as_ref()
                            .is_some_and(|painted| live.matches_menu_source(painted))
                    })
                    && live.matches_sidebar(&drag.paint.source, &view.machines.current)
                {
                    drag.source = live;
                }
            }
            if !drag.dragging
                && self
                    .map
                    .as_ref()
                    .and_then(|map| map.interaction_at(event.column, event.row))
                    .as_ref()
                    != Some(&drag.fingerprint)
            {
                drag.activation_cancelled = true;
            }
        } else if event.action == MouseAction::Press
            && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
            && let Some(paint) = self
                .map
                .as_ref()
                .and_then(|map| map.sidebar_reorder.as_ref())
            && contains(paint.body, event.column, event.row)
        {
            drag.position = (event.column, event.row);
            view.machines
                .queue_space_scroll(paint.window, event.button == MouseButton::WheelUp);
        }
        self.gesture = Gesture::SidebarReorder(drag);
        PointerOutcome::Handled
    }

    pub(super) fn tick_sidebar_reorder(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: Instant,
    ) -> bool {
        let Gesture::SidebarReorder(drag) = &self.gesture else {
            return false;
        };
        if !drag.baseline_current(port.sidebar_reorder_source().as_ref())
            || view.machines.sidebar_source() != drag.paint.source
            || view.resize_mode()
            || !self.map.as_ref().is_some_and(|map| drag.painted_in(map))
        {
            return self.cancel_sidebar_reorder();
        }
        let Some(paint) = self
            .map
            .as_ref()
            .and_then(|map| map.sidebar_reorder.as_ref())
        else {
            return false;
        };
        if !drag.dragging
            || now.saturating_duration_since(drag.last_scroll) < Duration::from_millis(120)
            || drag.position.0 < paint.body.x
            || drag.position.0 >= paint.body.right()
        {
            return false;
        }
        let up = drag.position.1 <= paint.body.y;
        let down = drag.position.1 >= paint.body.bottom().saturating_sub(1)
            && drag.position.1 < paint.footer.y;
        if (up && paint.window.top > 0) || (down && paint.window.end() < paint.window.total) {
            view.machines.queue_space_scroll(paint.window, up);
            if let Gesture::SidebarReorder(drag) = &mut self.gesture {
                drag.last_scroll = now;
            }
            return true;
        }
        false
    }
}
