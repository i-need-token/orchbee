//! Paint-only pointer projection for specialized shell chrome.
//!
//! Divider and tab presses keep their existing drag state machines. This
//! module projects only exact hover/pressed presentation from their painted
//! semantic witness; it never emits an action or starts a gesture.

use orchbee_workbench_protocol::{WorkbenchTabGroupId, WorkbenchTabId};
use ratatui::layout::Rect;

use super::{
    FrameHitMap, HitLayer, HitTarget, MotionObservability, MouseAction, MouseEvent,
    ProjectedPointer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum TabTarget {
    Tab(WorkbenchTabId),
    Group {
        tab: WorkbenchTabId,
        group: WorkbenchTabGroupId,
    },
}

impl TabTarget {
    fn from_hit(target: &HitTarget) -> Option<Self> {
        Some(match target {
            HitTarget::Tab(id) => Self::Tab(id.clone()),
            HitTarget::TabGroup { tab, group } => Self::Group {
                tab: tab.clone(),
                group: group.clone(),
            },
            _ => return None,
        })
    }

    fn hit_target(&self) -> HitTarget {
        match self {
            Self::Tab(id) => HitTarget::Tab(id.clone()),
            Self::Group { tab, group } => HitTarget::TabGroup {
                tab: tab.clone(),
                group: group.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Target {
    Divider(crate::tui_layout::DrawnDivider),
    Tab {
        source: crate::action_menu::DrawnTabCreate,
        target: TabTarget,
    },
}

/// Exact specialized-chrome paint identity. A different source snapshot,
/// semantic target, layer, or painted extent cannot inherit presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Fingerprint {
    target: Target,
    layer: HitLayer,
    area: Rect,
}

impl Fingerprint {
    pub(super) fn divider(target: &crate::tui_layout::DrawnDivider) -> Self {
        Self {
            target: Target::Divider(target.clone()),
            layer: HitLayer::Foreground,
            area: target.divider,
        }
    }

    pub(super) fn tab(
        source: &crate::action_menu::DrawnTabCreate,
        target: &HitTarget,
        area: Rect,
    ) -> Option<Self> {
        (!area.is_empty()).then_some(Self {
            target: Target::Tab {
                source: source.clone(),
                target: TabTarget::from_hit(target)?,
            },
            layer: HitLayer::Foreground,
            area,
        })
    }

    pub(super) fn hit_target(&self) -> HitTarget {
        match &self.target {
            Target::Divider(target) => HitTarget::Divider(target.clone()),
            Target::Tab { target, .. } => target.hit_target(),
        }
    }

    pub(super) fn matches_region(&self, layer: HitLayer, area: Rect, target: &HitTarget) -> bool {
        self.layer == layer && self.area == area && self.hit_target() == *target
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Projection {
    hovered: Option<Fingerprint>,
    pressed: Option<Fingerprint>,
}

impl Projection {
    pub(super) const fn empty() -> Self {
        Self {
            hovered: None,
            pressed: None,
        }
    }

    pub(super) fn project(&self, fingerprint: &Fingerprint) -> ProjectedPointer {
        if self.pressed.as_ref() == Some(fingerprint) {
            ProjectedPointer::Pressed
        } else if self.hovered.as_ref() == Some(fingerprint) {
            ProjectedPointer::Hovered
        } else {
            ProjectedPointer::Rest
        }
    }
}

#[derive(Default)]
pub(super) struct ProjectionState {
    position: Option<(u16, u16)>,
    hovered: Option<Fingerprint>,
    redraw_after_publish: bool,
}

impl ProjectionState {
    fn replace_hover(&mut self, next: Option<Fingerprint>) -> bool {
        if self.hovered == next {
            return false;
        }
        self.hovered = next;
        true
    }

    pub(super) fn observe_motion(
        &mut self,
        map: Option<&FrameHitMap>,
        observability: MotionObservability,
        event: MouseEvent,
        suppress: bool,
    ) -> bool {
        if observability != MotionObservability::AnyMotion
            || event.mods.shift
            || (event.action != MouseAction::Motion && self.position.is_none())
        {
            return false;
        }
        self.position = Some((event.column, event.row));
        let next = (!suppress)
            .then(|| map.and_then(|map| map.chrome_fingerprint_at(event.column, event.row)))
            .flatten();
        self.replace_hover(next)
    }

    pub(super) fn snapshot(
        &self,
        pressed: Option<Fingerprint>,
        suppress_hover: bool,
    ) -> Projection {
        Projection {
            hovered: if suppress_hover {
                None
            } else {
                self.hovered.clone()
            },
            pressed,
        }
    }

    pub(super) fn publish(
        &mut self,
        map: &FrameHitMap,
        pressed: Option<Fingerprint>,
        suppress_hover: bool,
    ) -> bool {
        if pressed.is_none() {
            let next = (!suppress_hover)
                .then(|| {
                    self.position
                        .and_then(|(column, row)| map.chrome_fingerprint_at(column, row))
                })
                .flatten();
            self.replace_hover(next);
        }
        self.redraw_after_publish = map.chrome_projection != self.snapshot(pressed, suppress_hover);
        self.redraw_after_publish
    }

    pub(super) fn invalidate_map(&mut self) {
        self.hovered = None;
        self.redraw_after_publish = false;
    }

    pub(super) fn clear_transport(&mut self) {
        self.position = None;
        self.hovered = None;
        self.redraw_after_publish = false;
    }

    pub(super) fn take_redraw_after_publish(&mut self) -> bool {
        std::mem::take(&mut self.redraw_after_publish)
    }
}
