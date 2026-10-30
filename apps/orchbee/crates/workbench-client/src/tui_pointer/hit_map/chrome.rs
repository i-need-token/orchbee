//! Painted shell chrome witnesses and their projected pointer state.

use orchbee_workbench_protocol::{WorkbenchTabGroupId, WorkbenchTabId};
use ratatui::layout::Rect;

use super::{HitLayer, HitTarget, ProjectedPointer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) enum TabTarget {
    Tab(WorkbenchTabId),
    Group {
        tab: WorkbenchTabId,
        group: WorkbenchTabGroupId,
    },
}

impl TabTarget {
    pub(in crate::tui_pointer) fn from_hit(target: &HitTarget) -> Option<Self> {
        Some(match target {
            HitTarget::Tab(id) => Self::Tab(id.clone()),
            HitTarget::TabGroup { tab, group } => Self::Group {
                tab: tab.clone(),
                group: group.clone(),
            },
            _ => return None,
        })
    }

    pub(in crate::tui_pointer) fn hit_target(&self) -> HitTarget {
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
pub(in crate::tui_pointer) enum Target {
    Divider(crate::tui_layout::DrawnDivider),
    Tab {
        source: crate::action_menu::DrawnTabCreate,
        target: TabTarget,
    },
}

/// Exact specialized-chrome paint identity. A different source snapshot,
/// semantic target, layer, or painted extent cannot inherit presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) struct Fingerprint {
    pub(in crate::tui_pointer) target: Target,
    pub(in crate::tui_pointer) layer: HitLayer,
    pub(in crate::tui_pointer) area: Rect,
}

impl Fingerprint {
    pub(in crate::tui_pointer) fn divider(target: &crate::tui_layout::DrawnDivider) -> Self {
        Self {
            target: Target::Divider(target.clone()),
            layer: HitLayer::Foreground,
            area: target.divider,
        }
    }

    pub(in crate::tui_pointer) fn tab(
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

    pub(in crate::tui_pointer) fn hit_target(&self) -> HitTarget {
        match &self.target {
            Target::Divider(target) => HitTarget::Divider(target.clone()),
            Target::Tab { target, .. } => target.hit_target(),
        }
    }

    pub(in crate::tui_pointer) fn matches_region(
        &self,
        layer: HitLayer,
        area: Rect,
        target: &HitTarget,
    ) -> bool {
        self.layer == layer && self.area == area && self.hit_target() == *target
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::tui_pointer) struct Projection {
    pub(in crate::tui_pointer) hovered: Option<Fingerprint>,
    pub(in crate::tui_pointer) pressed: Option<Fingerprint>,
}

impl Projection {
    pub(in crate::tui_pointer) const fn empty() -> Self {
        Self {
            hovered: None,
            pressed: None,
        }
    }

    pub(in crate::tui_pointer) fn project(&self, fingerprint: &Fingerprint) -> ProjectedPointer {
        if self.pressed.as_ref() == Some(fingerprint) {
            ProjectedPointer::Pressed
        } else if self.hovered.as_ref() == Some(fingerprint) {
            ProjectedPointer::Hovered
        } else {
            ProjectedPointer::Rest
        }
    }
}
