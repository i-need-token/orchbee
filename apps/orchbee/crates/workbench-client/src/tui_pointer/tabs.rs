//! A local press/drag/release transaction. Painted geometry and the exact source
//! snapshot remain fixed; only release can emit one canonical configuration.
use super::hit_map::tabs::{exposed_regions, tab_regions};
use super::{
    FrameHitMap, Gesture, HitTarget, MouseAction, MouseButton, MouseEvent, PointerOutcome,
    PointerState, TabDragPreview, contains,
};
use crate::{
    action_menu::DrawnTabCreate,
    client_view::{ClientAction, ClientView, Modifiers},
    terminal_input::{SurfaceAction, SurfaceInput, TabAction, TerminalInputPort},
};
use orchbee_workbench_protocol::{
    TabConfiguration, WorkbenchTabGroupId, WorkbenchTabId, WorkbenchTabIdSet,
    WorkbenchTabInsertIndex,
};
use ratatui::layout::Rect;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabDragTab {
    pub(crate) id: WorkbenchTabId,
    pub(crate) pinned: bool,
    pub(crate) group: Option<WorkbenchTabGroupId>,
    pub(crate) selected: bool,
    pub(crate) collapsed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabDragSource {
    pub(crate) drawn: DrawnTabCreate,
    pub(crate) tabs: Vec<TabDragTab>,
}

pub(super) struct TabClick {
    source: DrawnTabCreate,
    tab: WorkbenchTabId,
    position: (u16, u16),
    at: std::time::Instant,
}

enum Phase {
    Pressed { activation_cancelled: bool },
    Dragging(Option<Drop>),
}

struct Drop {
    index: WorkbenchTabInsertIndex,
    group: Option<WorkbenchTabGroupId>,
    marker: (u16, u16),
}

pub(super) struct TabDrag {
    pub(super) source: TabDragSource,
    target: HitTarget,
    /// The pressed target's regions as painted at the press: activation and the pressed projection.
    pressed: Vec<Rect>,
    origin: (u16, u16),
    /// Whole strip tokens, which the drop resolves against and the preview tints.
    regions: Vec<(Rect, HitTarget)>,
    ids: WorkbenchTabIdSet,
    carries_group: bool,
    pinned: bool,
    phase: Phase,
}

impl TabDrag {
    pub(super) fn begin(
        source: TabDragSource,
        map: &FrameHitMap,
        target: HitTarget,
        event: MouseEvent,
    ) -> Option<Self> {
        if map.overlay_active || map.menu_source.as_ref() != Some(&source.drawn.source) {
            return None;
        }
        let id = match &target {
            HitTarget::Tab(id) | HitTarget::TabGroup { tab: id, .. } => id,
            _ => return None,
        };
        let source_tab = source.tabs.iter().find(|tab| &tab.id == id)?;
        let pinned = source_tab.pinned;
        let ids: Vec<_> = source
            .tabs
            .iter()
            .filter(|tab| match &target {
                HitTarget::TabGroup { group, .. } => tab.group.as_ref() == Some(group),
                _ if source_tab.selected => tab.selected && tab.pinned == pinned,
                _ => &tab.id == id,
            })
            .map(|tab| tab.id.clone())
            .collect();
        let carries_group = source
            .tabs
            .iter()
            .filter_map(|tab| tab.group.as_ref())
            .any(|group| {
                source
                    .tabs
                    .iter()
                    .filter(|tab| tab.group.as_ref() == Some(group))
                    .all(|tab| ids.contains(&tab.id))
            });
        let pressed = exposed_regions(map)
            .filter(|(_, painted)| **painted == target)
            .map(|(rect, _)| rect)
            .collect();
        Some(Self {
            source,
            target,
            pressed,
            origin: (event.column, event.row),
            regions: tab_regions(map),
            ids: WorkbenchTabIdSet::try_from(ids).ok()?,
            carries_group,
            pinned,
            phase: Phase::Pressed {
                activation_cancelled: false,
            },
        })
    }

    pub(super) fn painted_in(&self, map: &FrameHitMap) -> bool {
        !map.overlay_active
            && map.menu_source.as_ref() == Some(&self.source.drawn.source)
            && tab_regions(map) == self.regions
    }

    pub(super) fn projection_fingerprint(&self) -> Option<super::hit_map::chrome::Fingerprint> {
        if !matches!(
            self.phase,
            Phase::Pressed {
                activation_cancelled: false,
            }
        ) {
            return None;
        }
        let area = *self.pressed.first()?;
        super::hit_map::chrome::Fingerprint::tab(&self.source.drawn, &self.target, area)
    }

    fn over_pressed_target(&self, column: u16, row: u16) -> bool {
        self.pressed.iter().any(|area| contains(*area, column, row))
    }

    fn destination(&self, column: u16, row: u16) -> Option<Drop> {
        let first = self.regions.iter().min_by_key(|(area, _)| area.x)?.0;
        let last = self.regions.iter().max_by_key(|(area, _)| area.right())?.0;
        if row != first.y || column < first.x || column >= last.right() {
            return None;
        }
        let hit = self
            .regions
            .iter()
            .find(|(area, _)| contains(*area, column, row));
        let (index, group, marker) = match hit {
            Some((area, HitTarget::Tab(id))) => {
                let index = self.source.tabs.iter().position(|tab| &tab.id == id)?;
                let tab = &self.source.tabs[index];
                if tab.pinned != self.pinned {
                    return None;
                }
                let after = column >= area.x + area.width / 2;
                (
                    index + usize::from(after),
                    (!self.carries_group && !self.pinned && !tab.collapsed)
                        .then(|| tab.group.clone())
                        .flatten(),
                    (
                        if after {
                            area.right().saturating_sub(1)
                        } else {
                            area.x
                        },
                        row,
                    ),
                )
            }
            Some((area, HitTarget::TabGroup { group, .. })) => {
                if self.pinned {
                    return None;
                }
                let start = self
                    .source
                    .tabs
                    .iter()
                    .position(|tab| tab.group.as_ref() == Some(group))?;
                let end = start
                    + self.source.tabs[start..]
                        .iter()
                        .take_while(|tab| tab.group.as_ref() == Some(group))
                        .count();
                let after = column >= area.x + area.width / 2;
                let end_column =
                    self.regions
                        .iter()
                        .filter_map(|(rect, target)| match target {
                            HitTarget::Tab(id)
                                if self.source.tabs.iter().any(|tab| {
                                    &tab.id == id && tab.group.as_ref() == Some(group)
                                }) =>
                            {
                                Some(rect.right().saturating_sub(1))
                            }
                            _ => None,
                        })
                        .max()
                        .unwrap_or(area.right().saturating_sub(1));
                (
                    if after { end } else { start },
                    None,
                    (if after { end_column } else { area.x }, row),
                )
            }
            _ => {
                let next = self
                    .regions
                    .iter()
                    .filter(|(area, _)| area.x > column)
                    .min_by_key(|(area, _)| area.x);
                let index = next
                    .and_then(|(_, target)| match target {
                        HitTarget::Tab(id) => self.source.tabs.iter().position(|tab| &tab.id == id),
                        HitTarget::TabGroup { group, .. } => self
                            .source
                            .tabs
                            .iter()
                            .position(|tab| tab.group.as_ref() == Some(group)),
                        _ => None,
                    })
                    .or_else(|| {
                        self.regions
                            .iter()
                            .filter(|(area, _)| area.x <= column)
                            .filter_map(|(area, target)| match target {
                                HitTarget::Tab(id) => self
                                    .source
                                    .tabs
                                    .iter()
                                    .position(|tab| &tab.id == id)
                                    .map(|index| (area.x, index + 1)),
                                HitTarget::TabGroup { group, .. } => self
                                    .source
                                    .tabs
                                    .iter()
                                    .rposition(|tab| tab.group.as_ref() == Some(group))
                                    .map(|index| (area.x, index + 1)),
                                _ => None,
                            })
                            .max_by_key(|(x, _)| *x)
                            .map(|(_, index)| index)
                    })?;
                (index, None, (column, row))
            }
        };
        let partition = self.source.tabs.iter().take_while(|tab| tab.pinned).count();
        if (self.pinned && index > partition) || (!self.pinned && index < partition) {
            return None;
        }
        // Show the same collection boundary the pure authority will choose.
        let first_index = self
            .source
            .tabs
            .iter()
            .position(|tab| self.ids.as_slice().contains(&tab.id))?;
        let remaining: Vec<_> = self
            .source
            .tabs
            .iter()
            .filter(|tab| !self.ids.as_slice().contains(&tab.id))
            .collect();
        let adjusted = index
            - self.source.tabs[..index]
                .iter()
                .filter(|tab| self.ids.as_slice().contains(&tab.id))
                .count();
        let left = adjusted
            .checked_sub(1)
            .and_then(|i| remaining.get(i))
            .and_then(|tab| tab.group.as_ref());
        let right = remaining.get(adjusted).and_then(|tab| tab.group.as_ref());
        let marker = if let Some(adjacent) =
            left.filter(|id| Some(*id) == right && Some(*id) != group.as_ref())
        {
            let group_regions: Vec<_> = self
                .regions
                .iter()
                .filter(|(_, target)| match target {
                    HitTarget::Tab(id) => self
                        .source
                        .tabs
                        .iter()
                        .any(|tab| &tab.id == id && tab.group.as_ref() == Some(adjacent)),
                    HitTarget::TabGroup { group, .. } => group == adjacent,
                    _ => false,
                })
                .map(|(area, _)| area)
                .collect();
            let column = if index <= first_index {
                group_regions.iter().map(|area| area.x).min()
            } else {
                group_regions
                    .iter()
                    .map(|area| area.right().saturating_sub(1))
                    .max()
            };
            (column?, row)
        } else {
            marker
        };
        Some(Drop {
            index: WorkbenchTabInsertIndex::new(index).ok()?,
            group,
            marker,
        })
    }

    fn preview(&self) -> Option<TabDragPreview> {
        let Phase::Dragging(drop) = &self.phase else {
            return None;
        };
        Some(TabDragPreview {
            grabbed: self
                .regions
                .iter()
                .filter(|(_, target)| match target {
                    HitTarget::Tab(id) => self.ids.as_slice().contains(id),
                    HitTarget::TabGroup { group, .. } => self
                        .source
                        .tabs
                        .iter()
                        .filter(|tab| tab.group.as_ref() == Some(group))
                        .all(|tab| self.ids.as_slice().contains(&tab.id)),
                    _ => false,
                })
                .map(|(area, _)| *area)
                .collect(),
            marker: drop.as_ref().map(|drop| drop.marker),
            count: self.ids.as_slice().len(),
        })
    }
}

impl PointerState {
    pub(crate) fn tab_drag_preview(&self) -> Option<TabDragPreview> {
        match &self.gesture {
            Gesture::Tab(drag) => drag.preview(),
            _ => None,
        }
    }

    pub(crate) fn tab_drag_captured(&self) -> bool {
        matches!(self.gesture, Gesture::Tab(_))
    }

    /// Cancels an armed or dragging tab gesture. A remembered completed click survives: map
    /// invalidation after its own tab selection must not erase the first half of a double click.
    pub(crate) fn cancel_tab_drag(&mut self) -> bool {
        if matches!(self.gesture, Gesture::Tab(_)) {
            self.gesture = Gesture::TabCancelled;
            true
        } else {
            false
        }
    }

    pub(super) fn tab_event(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: std::time::Instant,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::Tab(mut drag) = std::mem::replace(&mut self.gesture, Gesture::TabCancelled)
        else {
            if event.action == MouseAction::Press {
                self.gesture = Gesture::Idle;
                let probe = port.pointer_probe();
                return self.fresh_event(view, port, &probe, now, event);
            }
            if event.action == MouseAction::Release
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                self.gesture = Gesture::Idle;
                return PointerOutcome::TabGestureEnded;
            }
            return PointerOutcome::Handled;
        };
        let alive = port.tab_drag_source().as_ref() == Some(&drag.source)
            && self.map.as_ref().is_some_and(|map| drag.painted_in(map))
            && event.mods == Modifiers::NONE;
        if !alive {
            if event.action == MouseAction::Release
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                self.gesture = Gesture::Idle;
                return PointerOutcome::TabGestureEnded;
            }
            // This report is the causal end of the active drag. Keep the
            // cancelled tombstone for its late primary release, but tell the
            // dispatcher to fence the rest of this physical packet now.
            return PointerOutcome::TabGestureEnded;
        }
        if event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None)
        {
            self.gesture = Gesture::Idle;
            match drag.phase {
                Phase::Pressed {
                    activation_cancelled,
                } => {
                    if !activation_cancelled && drag.over_pressed_target(event.column, event.row) {
                        let action = match drag.target {
                            HitTarget::Tab(id) => {
                                let position = (event.column, event.row);
                                // The first click selects the tab, which republishes the
                                // workbench revision; the pair only needs the same context.
                                let double = self.tab_click.take().is_some_and(|click| {
                                    click.source.source.session == drag.source.drawn.source.session
                                        && click.source.location == drag.source.drawn.location
                                        && click.tab == id
                                        && click.position.0.abs_diff(position.0) <= 1
                                        && click.position.1 == position.1
                                        && now.saturating_duration_since(click.at)
                                            <= std::time::Duration::from_millis(500)
                                });
                                if double {
                                    if let Some(disposition) = port.surface_action(
                                        SurfaceAction::Tabs(TabAction::RequestRename {
                                            source: &drag.source.drawn,
                                            tab: &id,
                                        }),
                                    ) {
                                        view.commit_pointer(disposition);
                                    }
                                    None
                                } else {
                                    port.surface_input(SurfaceInput::TabStripFollow {
                                        source: &drag.source.drawn,
                                        tab: &id,
                                    });
                                    self.tab_click = Some(TabClick {
                                        source: drag.source.drawn.clone(),
                                        tab: id.clone(),
                                        position,
                                        at: now,
                                    });
                                    drag.source
                                        .tabs
                                        .iter()
                                        .position(|tab| tab.id == id)
                                        .and_then(|index| u8::try_from(index + 1).ok())
                                        .map(ClientAction::SelectTab)
                                }
                            }
                            HitTarget::TabGroup { group, .. } => {
                                Some(ClientAction::ToggleTabGroup(group))
                            }
                            _ => None,
                        };
                        if let Some(disposition) =
                            action.and_then(|action| port.apply_client_action((action).into()))
                        {
                            view.commit_pointer(disposition);
                        }
                        return PointerOutcome::TabGestureEnded;
                    }
                }
                Phase::Dragging(_) => {
                    self.tab_click = None;
                    if let Some(drop) = drag.destination(event.column, event.row) {
                        port.surface_action(SurfaceAction::Tabs(TabAction::Drag {
                            source: &drag.source.drawn,
                            operation: TabConfiguration::MoveTabsByDrag {
                                tab_ids: drag.ids,
                                insert_index: drop.index,
                                group_id: drop.group,
                            },
                        }));
                        return PointerOutcome::TabGestureEnded;
                    }
                }
            }
            return PointerOutcome::TabGestureEnded;
        } else if event.action == MouseAction::Motion && event.button == MouseButton::Left {
            let crossed_drag_threshold =
                drag.origin.0.abs_diff(event.column) >= 2 || drag.origin.1 != event.row;
            let over_pressed_target = drag.over_pressed_target(event.column, event.row);
            if matches!(drag.phase, Phase::Dragging(_)) || crossed_drag_threshold {
                drag.phase = Phase::Dragging(drag.destination(event.column, event.row));
            } else if let Phase::Pressed {
                activation_cancelled,
            } = &mut drag.phase
                && !over_pressed_target
            {
                *activation_cancelled = true;
            }
            self.gesture = Gesture::Tab(drag);
        } else if event.action == MouseAction::Press {
            return PointerOutcome::TabGestureEnded;
        } else {
            self.gesture = Gesture::Tab(drag);
        }
        PointerOutcome::Handled
    }
}
