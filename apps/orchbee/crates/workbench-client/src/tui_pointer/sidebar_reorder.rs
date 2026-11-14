//! One primary-button owner for sidebar clicks and drag organization.
//! A drag shows its move live in this Client's sidebar, around the entry it holds; only a matching
//! release submits that move, and every other ending restores the canonical order.
use super::{
    FrameHitMap, Gesture, HitRegionResolution, InteractionTarget, MouseAction, MouseButton,
    MouseEvent, PointerOutcome, PointerState, PrimaryContract, SidebarReorderDraw,
    SidebarReorderPaint, contains,
};
use crate::client_view::{ClientView, Modifiers};
use crate::machine_picker::{DrawnSidebarTarget, SidebarPart, SidebarSource, SidebarTarget};
use crate::machine_spaces::navigation_projection::SidebarNavigationProjection;
use crate::workbench_client::input::{
    LayoutAction, SidebarReorderSource, SurfaceAction, TerminalInputPort,
};
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::{
    NavigationConfiguration, WorkbenchProjectId, WorkbenchWorkspaceId, configure_navigation,
};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
mod destination;
use destination::{Drop, Item, Landing};
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
    /// The surface and catalog of the last frame this gesture accepted.
    paint: SidebarReorderPaint,
    target: DrawnSidebarTarget,
    fingerprint: crate::tui_hit_map::interaction::Fingerprint,
    item: Item,
    origin: (u16, u16),
    position: (u16, u16),
    dragging: bool,
    activation_cancelled: bool,
    last_scroll: Instant,
    /// The move the sidebar shows while dragging; `None` shows the canonical order.
    shown: Option<Box<Drop>>,
    /// The move the accepted frame paints, which can trail `shown` until the next frame.
    painted: Option<Box<Drop>>,
    /// Whether the pointer moved while no frame was on record, so the next accepted frame moves
    /// the destination to it.
    unresolved: bool,
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
            shown: None,
            painted: None,
            unresolved: false,
        })
    }

    /// The sidebar paint of `map` when it keeps this gesture's surface. A live move may change
    /// the list's length, and with it the free rows that form the footer, so only the footer of
    /// an accepted frame is compared.
    fn surface<'a>(&self, map: &'a FrameHitMap) -> Option<&'a SidebarReorderPaint> {
        let paint = map.sidebar_reorder.as_ref()?;
        ((!map.overlay_active || paint.layer == super::HitLayer::Overlay)
            && paint.layer == self.paint.layer
            && paint.viewport == self.paint.viewport
            && paint.body == self.paint.body)
            .then_some(paint)
    }

    /// Whether `map` is the frame this gesture last accepted: the same surface and catalog. Before
    /// a drag latches, the press is a click on its row, so only that row's own painted facts
    /// matter: a repaint after an unrelated canonical commit keeps it. A drag shows its move over
    /// its canonical baseline, so the frame must be painted from that source.
    pub(super) fn painted_in(&self, map: &FrameHitMap) -> bool {
        self.surface(map).is_some_and(|paint| {
            paint.footer == self.paint.footer && paint.source == self.paint.source
        }) && if self.dragging {
            self.baseline_painted(map)
        } else {
            map.interaction_at(self.origin.0, self.origin.1).as_ref() == Some(&self.fingerprint)
        }
    }

    fn baseline_painted(&self, map: &FrameHitMap) -> bool {
        map.menu_source
            .as_ref()
            .is_some_and(|painted| self.source.matches_menu_source(painted))
    }

    /// Whether a drag's `map` paints its baseline with either the catalog of the frame it last
    /// accepted or the move it now shows.
    fn frames(&self, map: &FrameHitMap) -> bool {
        self.dragging
            && self.baseline_painted(map)
            && self
                .surface(map)
                .is_some_and(|paint| paint.source == self.paint.source || self.shows(&paint.source))
    }

    fn shows(&self, painted: &SidebarSource) -> bool {
        let moved;
        let navigation = match &self.shown {
            None => self.source.navigation(),
            Some(drop) => match configure_navigation(self.source.navigation(), &drop.operation) {
                Ok(Some(navigation)) => {
                    moved = navigation;
                    &moved
                }
                Ok(None) | Err(_) => return false,
            },
        };
        painted.matches_navigation(
            self.target.target().machine(),
            self.source.session_id(),
            navigation,
        )
    }

    /// Takes a newly published frame and returns whether the gesture survives it, with whether
    /// the move it shows changed. A click keeps only its own row's frame. A drag takes the frame
    /// that shows its move and resolves against it from then on; a scroll moves other rows under
    /// the pointer, and a pointer that moved while no frame was on record has not been resolved,
    /// so either way the destination follows the pointer there.
    fn accept(&mut self, map: &FrameHitMap) -> (bool, bool) {
        if !self.dragging {
            return (self.painted_in(map), false);
        }
        let Some(paint) = map.sidebar_reorder.as_ref().filter(|_| self.frames(map)) else {
            return (false, false);
        };
        let scrolled = paint.window.top != self.paint.window.top;
        let unresolved = std::mem::take(&mut self.unresolved);
        if self.shows(&paint.source) {
            self.painted.clone_from(&self.shown);
        }
        self.paint = paint.clone();
        (true, (scrolled || unresolved) && self.follow(map))
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

    /// Moves the shown destination to the pointer over `map`, the frame on screen. Returns
    /// whether the shown move changed.
    fn follow(&mut self, map: &FrameHitMap) -> bool {
        if !self.dragging {
            return false;
        }
        let Some(shown) = self.resolve(map) else {
            return false;
        };
        let changed = shown.as_ref().map(|drop| &drop.operation)
            != self.shown.as_ref().map(|drop| &drop.operation);
        self.shown = shown.map(Box::new);
        changed
    }

    /// The destination under the pointer: `Some(None)` returns the dragged entry to its origin,
    /// and `None` keeps what is shown, over gaps, rows that are not entries and destinations the
    /// baseline refuses. Over the dragged entry itself the destination is where the frame paints
    /// it; since a shown move puts that entry under the pointer, a pointer that stays put keeps
    /// its destination.
    fn resolve(&self, map: &FrameHitMap) -> Option<Option<Drop>> {
        let paint = map.sidebar_reorder.as_ref()?;
        let (x, y) = self.position;
        let landing = if contains(paint.footer, x, y) {
            // The free rows end a Project drag only; a Workspace has no destination there, so the
            // shown move stays, as over a gap.
            self.source.destination(&self.item, None, false)
        } else if !contains(paint.body, x, y) {
            // Outside the list the sidebar shows the canonical order, which a release there keeps.
            return Some(None);
        } else {
            // Child controls remain their own buttons on press. During an established
            // drag their row is a destination, without invoking disclosure or Create.
            let entries = self.entries(map);
            let index = entries
                .iter()
                .position(|entry| contains(entry.rows, x, y))?;
            let entry = &entries[index];
            if entry.item == self.item {
                return Some(self.painted.as_deref().cloned());
            }
            // The side of the dragged entry as painted; one hidden in a folded Project falls back
            // to the press.
            let dragged = entries
                .iter()
                .find(|entry| entry.item == self.item)
                .map_or(self.origin.1, |entry| entry.rows.y);
            let after = after(entry.rows, y, entry.rows.y > dragged);
            match (&self.item, &entry.item) {
                (Item::Workspace(workspace), Item::Project(project)) => {
                    let Some(landing) =
                        self.onto_heading(workspace, project, &entries, index, after)
                    else {
                        return Some(self.painted.as_deref().cloned());
                    };
                    landing
                }
                _ => self
                    .source
                    .destination(&self.item, Some(&entry.item), after),
            }
        };
        match landing {
            Landing::Unavailable => None,
            Landing::Origin => Some(None),
            Landing::Moved(drop) => Some(Some(drop)),
        }
    }

    /// A Workspace dragged onto the Project heading at `entries[index]`. A heading with no other
    /// Workspace painted under it takes the Workspace in. Otherwise the heading is the Project's
    /// top edge: from above, the Workspace becomes the Project's first; from below, it lands after
    /// the Workspace painted just above the heading, or first when none is. `None` leaves the
    /// Workspace where the frame paints it, just above the heading.
    fn onto_heading(
        &self,
        workspace: &WorkbenchWorkspaceId,
        project: &WorkbenchProjectId,
        entries: &[Entry],
        index: usize,
        after: bool,
    ) -> Option<Landing> {
        let navigation = self.source.navigation();
        let others = entries
            .get(index + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|entry| match &entry.item {
                Item::Workspace(id) => {
                    id == workspace
                        || navigation
                            .workspace(id)
                            .is_some_and(|other| other.project_id.as_ref() == Some(project))
                }
                Item::Project(_) => false,
            })
            .any(|entry| entry.item != self.item);
        if !others {
            return Some(self.source.destination(
                &self.item,
                Some(&Item::Project(project.clone())),
                false,
            ));
        }
        if !after
            && let Some(above) = index.checked_sub(1).and_then(|index| entries.get(index))
            && matches!(above.item, Item::Workspace(_))
        {
            if above.item == self.item {
                return None;
            }
            return Some(self.source.destination(&self.item, Some(&above.item), true));
        }
        Some(self.source.first_in(workspace, project))
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

    /// The order the sidebar shows while this gesture drags: its baseline with the shown move.
    fn order(&self) -> Option<SidebarNavigationProjection> {
        self.dragging.then(|| SidebarNavigationProjection {
            snapshot: self.source.snapshot(),
            pending: self.shown.as_ref().map(|drop| drop.operation.clone()),
            leaving: self.source.leaving().to_vec(),
        })
    }

    /// The entry a latched drag holds.
    fn held(&self) -> Option<SidebarTarget> {
        self.dragging.then(|| self.target.target().clone())
    }

    /// The Workspace a latched drag holds; it stays current however the person ends the drag.
    fn held_workspace(&self) -> Option<SidebarTarget> {
        self.held()
            .filter(|_| matches!(self.item, Item::Workspace(_)))
    }

    fn preview(&self, map: &FrameHitMap) -> Option<SidebarReorderDraw> {
        if !self.frames(map) {
            return None;
        }
        Some(SidebarReorderDraw {
            destination: self
                .shown
                .as_ref()
                .map(|drop| (drop.placement, drop.label.clone())),
            footer: map
                .sidebar_reorder
                .as_ref()
                .map_or(self.paint.footer, |paint| paint.footer),
            workspace: matches!(self.item, Item::Workspace(_)),
        })
    }
}

/// One painted entry a drag can land on. `rows` is where the pointer selects it: a Workspace row
/// with its Agent rows, or a Project heading. `block` is every line it moves as: a Project heading
/// with the rows it groups while a Project is dragged.
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

impl PointerState {
    pub(crate) fn sidebar_reorder_captured(&self) -> bool {
        matches!(self.gesture, Gesture::SidebarReorder(_))
    }
    pub(crate) fn sidebar_reorder_preview(&self) -> Option<SidebarReorderPreview> {
        let Gesture::SidebarReorder(drag) = &self.gesture else {
            return None;
        };
        (drag.dragging && self.sidebar_reorder_framed(drag))
            .then(|| SidebarReorderPreview { drag: drag.clone() })
    }
    /// Whether the frame on record still admits `drag`. A press that has not latched a drag is a
    /// click on its painted row, so it needs that row's frame. A latched drag outlives a map
    /// dropped until the next frame, as a scroll drops it: that frame, painted from the same
    /// source, resolves the pointer and keeps or ends the drag.
    fn sidebar_reorder_framed(&self, drag: &SidebarReorder) -> bool {
        self.map
            .as_ref()
            .map_or(drag.dragging, |map| drag.painted_in(map))
    }
    /// The order a drag shows in the sidebar; `None` while no drag shows one.
    pub(crate) fn sidebar_reorder_order(&self) -> Option<SidebarNavigationProjection> {
        let Gesture::SidebarReorder(drag) = &self.gesture else {
            return None;
        };
        drag.order()
    }
    /// The entry a drag holds while it shows its order.
    pub(crate) fn sidebar_reorder_held(&self) -> Option<SidebarTarget> {
        let Gesture::SidebarReorder(drag) = &self.gesture else {
            return None;
        };
        drag.held()
    }
    /// Escape ends a drag as the person's own cancel: its held Workspace stays current.
    pub(crate) fn escape_sidebar_reorder(&mut self) -> bool {
        if let Gesture::SidebarReorder(drag) = &self.gesture {
            self.sidebar_reorder_switch = drag.held_workspace();
        }
        self.cancel_sidebar_reorder()
    }
    /// The Workspace to make current after the person ended a drag that held it.
    pub(crate) fn take_sidebar_reorder_switch(&mut self) -> Option<SidebarTarget> {
        self.sidebar_reorder_switch.take()
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

    /// A published frame either carries the gesture or ends it. A drag that ends here, or whose
    /// shown move follows a scroll here, still shows its old order, so the next frame repaints it.
    pub(super) fn publish_sidebar_reorder(&mut self, map: &FrameHitMap) {
        let Gesture::SidebarReorder(drag) = &mut self.gesture else {
            return;
        };
        drag.follow_reveal(map);
        let dragging = drag.dragging;
        let (kept, moved) = drag.accept(map);
        self.sidebar_reorder_redraw |= moved || (dragging && !kept);
        if !kept {
            self.cancel_sidebar_reorder();
        }
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
            && view.machines.sidebar_source() == drag.paint.source
            && drag.baseline_current(port.sidebar_reorder_source().as_ref())
            && self.sidebar_reorder_framed(&drag);
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
        // Only a latched drag is alive without a frame on record.
        let map = self.map.as_ref();
        if release {
            self.sidebar_reorder_switch = drag.held_workspace();
            if drag.dragging {
                // A release where the last motion left the pointer commits the move shown there;
                // a terminal that reports it elsewhere first moves the destination to it. Before
                // the next frame there is no other destination than the one on screen.
                if let Some(map) = map
                    && drag.position != (event.column, event.row)
                {
                    drag.position = (event.column, event.row);
                    drag.follow(map);
                }
                if let Some(drop) = drag.shown.take() {
                    port.surface_action(SurfaceAction::Layout(LayoutAction::SidebarReorder {
                        source: &drag.source,
                        operation: drop.operation,
                    }));
                }
                return PointerOutcome::SidebarReorderEnded;
            }
            if !drag.activation_cancelled
                && map
                    .and_then(|map| map.interaction_at(event.column, event.row))
                    .as_ref()
                    == Some(&drag.fingerprint)
            {
                return PointerOutcome::Interaction {
                    target: InteractionTarget::Sidebar(drag.target),
                    contract: PrimaryContract::ActivateOnRelease,
                };
            }
            return PointerOutcome::SidebarReorderEnded;
        }
        if event.action == MouseAction::Motion && event.button == MouseButton::Left {
            let moved = drag.position != (event.column, event.row);
            drag.position = (event.column, event.row);
            let Some(map) = map else {
                // Repeated or new, this position waits for the next frame to resolve it.
                drag.unresolved |= moved;
                self.gesture = Gesture::SidebarReorder(drag);
                return PointerOutcome::Handled;
            };
            if !drag.dragging
                && (drag.origin.0.abs_diff(event.column) >= 2 || drag.origin.1 != event.row)
            {
                drag.dragging = true;
                // The drag begins now, so it moves against the canonical source painted now; the
                // click it grew from never depended on one.
                if let Some(live) = port.sidebar_reorder_source()
                    && map
                        .menu_source
                        .as_ref()
                        .is_some_and(|painted| live.matches_menu_source(painted))
                    && live.matches_sidebar(&drag.paint.source, &view.machines.current)
                {
                    drag.source = live;
                }
            }
            if !drag.dragging
                && map.interaction_at(event.column, event.row).as_ref() != Some(&drag.fingerprint)
            {
                drag.activation_cancelled = true;
            }
            drag.follow(map);
        } else if event.action == MouseAction::Press
            && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
            // A notch scrolls from the window of a frame on record.
            && let Some(paint) = map.and_then(|map| map.sidebar_reorder.as_ref())
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
            || !self.sidebar_reorder_framed(drag)
        {
            return self.cancel_sidebar_reorder();
        }
        // The next step waits for the frame that paints the last one and starts from its window.
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
        // Each edge row, and anything past it in the list's columns, scrolls toward that edge, up
        // to the end a wheel reaches: a Project drag's end leaves End of Projects a free row.
        let up = drag.position.1 <= paint.body.y;
        let down = drag.position.1 >= paint.body.bottom().saturating_sub(1);
        if (up && paint.window.top > 0) || (down && paint.window.top < paint.window.maximum) {
            view.machines.queue_space_scroll(paint.window, up);
            if let Gesture::SidebarReorder(drag) = &mut self.gesture {
                drag.last_scroll = now;
            }
            return true;
        }
        false
    }
}
