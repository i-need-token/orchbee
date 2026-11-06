//! Painted shell chrome witnesses and their projected pointer state.

use orchbee_workbench_protocol::{WorkbenchTabGroupId, WorkbenchTabId};
use ratatui::layout::Rect;

use super::{HitLayer, HitTarget, ProjectedPointer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TabTarget {
    Tab(WorkbenchTabId),
    Group {
        tab: WorkbenchTabId,
        group: WorkbenchTabGroupId,
    },
}

impl TabTarget {
    pub(crate) fn from_hit(target: &HitTarget) -> Option<Self> {
        Some(match target {
            HitTarget::Tab(id) => Self::Tab(id.clone()),
            HitTarget::TabGroup { tab, group } => Self::Group {
                tab: tab.clone(),
                group: group.clone(),
            },
            _ => return None,
        })
    }

    pub(crate) fn hit_target(&self) -> HitTarget {
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
pub(crate) enum Target {
    Divider(crate::tui_layout::DrawnDivider),
    Tab {
        source: crate::action_menu::DrawnTabCreate,
        target: TabTarget,
    },
}

/// Exact specialized-chrome paint identity. A different source snapshot,
/// semantic target, layer, or painted extent cannot inherit presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fingerprint {
    pub(crate) target: Target,
    pub(crate) layer: HitLayer,
    pub(crate) area: Rect,
}

impl Fingerprint {
    pub(crate) fn divider(target: &crate::tui_layout::DrawnDivider) -> Self {
        Self {
            target: Target::Divider(target.clone()),
            layer: HitLayer::Foreground,
            area: target.divider,
        }
    }

    pub(crate) fn tab_on(
        layer: HitLayer,
        source: &crate::action_menu::DrawnTabCreate,
        target: &HitTarget,
        area: Rect,
    ) -> Option<Self> {
        (!area.is_empty()).then_some(Self {
            target: Target::Tab {
                source: source.clone(),
                target: TabTarget::from_hit(target)?,
            },
            layer,
            area,
        })
    }

    pub(crate) fn hit_target(&self) -> HitTarget {
        match &self.target {
            Target::Divider(target) => HitTarget::Divider(target.clone()),
            Target::Tab { target, .. } => target.hit_target(),
        }
    }

    pub(crate) fn matches_region(&self, layer: HitLayer, area: Rect, target: &HitTarget) -> bool {
        self.layer == layer && self.area == area && self.hit_target() == *target
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Projection {
    pub(crate) hovered: Option<Fingerprint>,
    pub(crate) pressed: Option<Fingerprint>,
}

impl Projection {
    pub(crate) const fn empty() -> Self {
        Self {
            hovered: None,
            pressed: None,
        }
    }

    pub(crate) fn project(&self, fingerprint: &Fingerprint) -> ProjectedPointer {
        if self.pressed.as_ref() == Some(fingerprint) {
            ProjectedPointer::Pressed
        } else if self.hovered.as_ref() == Some(fingerprint) {
            ProjectedPointer::Hovered
        } else {
            ProjectedPointer::Rest
        }
    }
}
