//! Reversible divider geometry pinned to one successfully painted canonical source.
#[cfg(any(windows, test))]
use super::{SplitArea, divide_panes};
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, WorkbenchPanelSlotId, WorkbenchSplitId,
};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchPanelId, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
};
#[cfg(any(windows, test))]
use ratatui::layout::Rect;

/// Selected canonical identity; drawing it grants no terminal input authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DividerSource {
    pub(crate) site: SiteId,
    pub(crate) context: WorkbenchTerminalContextId,
    pub(crate) tab: WorkbenchTabId,
    pub(crate) focus: WorkbenchPanelId,
    pub(crate) revision: WorkbenchRevision,
}

/// A resize toolbar refers to one focused layout and its painted viewport.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(windows)]
pub(crate) struct DrawnResize {
    pub(crate) source: DividerSource,
    pub(crate) viewport: Rect,
    pub(crate) sidebar: crate::client_view::SidebarSurface,
}

/// Exact painted split and viewport, revalidated before every gesture step.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(any(windows, test))]
pub(crate) struct DrawnDivider {
    pub(crate) source: DividerSource,
    pub(crate) viewport: Rect,
    pub(crate) split: WorkbenchSplitId,
    pub(crate) first: WorkbenchPanelSlotId,
    pub(crate) second: WorkbenchPanelSlotId,
    pub(crate) orientation: PanelOrientation,
    pub(crate) ratio: ClientSplitRatio,
    pub(crate) area: Rect,
    pub(crate) divider: Rect,
}

#[cfg(any(windows, test))]
impl DrawnDivider {
    /// Captures only geometry that the ordinary pane projector actually exposes.
    pub(crate) fn new(
        source: DividerSource,
        viewport: Rect,
        split: &SplitArea<'_>,
    ) -> Option<Self> {
        let area = split.area?;
        let divider = split.divider?;
        if divide_panes(area, split.orientation, split.ratio)?.divider != divider {
            return None;
        }
        Some(Self {
            source,
            viewport,
            split: split.split_id.clone(),
            first: split.first_slot_id.clone(),
            second: split.second_slot_id.clone(),
            orientation: split.orientation,
            ratio: split.ratio,
            area,
            divider,
        })
    }
}

/// Local preview; the original source and ratio remain immutable until it is dropped.
#[derive(Clone, Debug)]
#[cfg(any(windows, test))]
pub(crate) struct DividerDrag {
    pub(crate) target: DrawnDivider,
    /// Last successfully painted geometry of this same local draft, never a new origin.
    pub(crate) painted: DrawnDivider,
    pressed: (u16, u16),
    pub(crate) ratio: ClientSplitRatio,
    pub(crate) preview: Rect,
}

#[cfg(any(windows, test))]
impl DividerDrag {
    /// Begins only inside the exact painted divider.
    pub(crate) fn begin(target: DrawnDivider, column: u16, row: u16) -> Option<Self> {
        if !target.divider.contains((column, row).into()) {
            return None;
        }
        Some(Self {
            ratio: target.ratio,
            preview: target.divider,
            painted: target.clone(),
            target,
            pressed: (column, row),
        })
    }

    /// Replaces the projection from absolute displacement, never incremental motion.
    pub(crate) fn update(&mut self, column: u16, row: u16) {
        let (position, pressed, share) = match self.target.orientation {
            PanelOrientation::Horizontal => (column, self.pressed.0, self.target.area.width - 1),
            PanelOrientation::Vertical => (row, self.pressed.1, self.target.area.height - 1),
        };
        let delta = i32::from(position) - i32::from(pressed);
        self.ratio = if delta == 0 {
            self.target.ratio
        } else {
            ClientSplitRatio::new(
                (self.target.ratio.value() + f64::from(delta) / f64::from(share)).clamp(0.1, 0.9),
            )
            .expect("finite displacement over admitted positive pane extent")
        };
        self.preview = divide_panes(self.target.area, self.target.orientation, self.ratio)
            .expect("changing the ratio preserves the admitted minimum-size geometry")
            .divider;
    }

    pub(crate) fn projected_target(&self) -> DrawnDivider {
        let mut target = self.target.clone();
        target.ratio = self.ratio;
        target.divider = self.preview;
        target
    }
}

#[cfg(test)]
mod tests;
