//! Counts what a confirmed close ends from the terminal facts this Client holds: a committed
//! launch failure, or a Host settlement on the attachment bound to that exact terminal. It grants
//! no close authority and never counts an unobserved terminal as ended.

use super::{PaneRef, WorkbenchClientState};
use crate::localization::CloseImpact;
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};

impl WorkbenchClientState {
    /// Counts every Presentation of `panels`. `settled` answers whether an exact attachment
    /// reported a Host settlement; the pane record ties that attachment to the one binding it
    /// attached, so a sibling Presentation of the same Panel stays unobserved.
    fn close_impact<'p>(
        &self,
        panels: impl IntoIterator<Item = &'p super::super::ClientPanel>,
        settled: &dyn Fn(&PaneRef) -> bool,
    ) -> CloseImpact {
        let mut impact = CloseImpact::default();
        for panel in panels {
            let settled = self
                .pane_by_panel(panel.panel_id())
                .filter(|record| settled(&record.pane_ref()))
                .map(super::PaneRecord::binding);
            for member in panel.presentations() {
                if member.launch_failure().is_some()
                    || settled.is_some_and(|binding| member.binding() == Some(binding))
                {
                    impact.ended += 1;
                } else {
                    impact.running += 1;
                }
            }
        }
        impact
    }

    /// Counts every terminal of `tabs`. A Tab missing from the current projection counts as one
    /// running terminal, so an unresolved scope never reads as already ended.
    pub(super) fn tabs_close_impact(
        &self,
        tabs: &[WorkbenchTabId],
        settled: &dyn Fn(&PaneRef) -> bool,
    ) -> CloseImpact {
        tabs.iter().fold(CloseImpact::default(), |total, id| {
            let impact = self.cache.tab_index(id).map_or(
                CloseImpact {
                    running: 1,
                    ended: 0,
                },
                |index| {
                    let tab = &self.cache.tabs()[index];
                    self.close_impact(
                        (0..tab.panel_count()).filter_map(|panel| tab.panel(panel)),
                        settled,
                    )
                },
            );
            CloseImpact {
                running: total.running + impact.running,
                ended: total.ended + impact.ended,
            }
        })
    }

    /// Counts the terminals of one Panel, or one running terminal when it is no longer projected.
    pub(super) fn panel_close_impact(
        &self,
        tab: &WorkbenchTabId,
        panel: &WorkbenchPanelId,
        settled: &dyn Fn(&PaneRef) -> bool,
    ) -> CloseImpact {
        self.cache
            .tab_index(tab)
            .map(|index| &self.cache.tabs()[index])
            .and_then(|tab| tab.panel(tab.panel_index(panel)?))
            .map_or(
                CloseImpact {
                    running: 1,
                    ended: 0,
                },
                |panel| self.close_impact([panel], settled),
            )
    }
}
