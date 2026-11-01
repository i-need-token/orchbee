//! Counts what a confirmed close ends from the terminal facts this Client holds: a committed
//! launch failure, or a Host settlement on the attachment bound to that exact terminal. It grants
//! no close authority and never counts an unobserved terminal as ended.

use super::{PaneRef, WorkbenchClientState};
use crate::localization::CloseImpact;
use crate::workbench_projection::{ClientPanel, ClientTerminalBinding};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};

/// One terminal a close would end that has not ended, with the pane showing it in this Client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseTerminal {
    pub(crate) binding: ClientTerminalBinding,
    pub(crate) pane: Option<PaneRef>,
}

impl WorkbenchClientState {
    /// The terminals of `panels` that have not ended, by the same facts [`Self::close_impact`]
    /// counts; `None` when a Presentation holds no terminal yet, so nothing can be read about it.
    fn live_close_terminals<'p>(
        &self,
        panels: impl IntoIterator<Item = &'p ClientPanel>,
        settled: &dyn Fn(&PaneRef) -> bool,
    ) -> Option<Vec<CloseTerminal>> {
        let mut terminals = Vec::new();
        for panel in panels {
            let record = self.pane_by_panel(panel.panel_id());
            let ended = record
                .filter(|record| settled(&record.pane_ref()))
                .map(super::PaneRecord::binding);
            for member in panel.presentations() {
                if member.launch_failure().is_some()
                    || ended.is_some_and(|binding| member.binding() == Some(binding))
                {
                    continue;
                }
                let binding = member.binding()?;
                terminals.push(CloseTerminal {
                    binding: binding.clone(),
                    pane: record
                        .filter(|record| record.binding() == binding)
                        .map(super::PaneRecord::pane_ref),
                });
            }
        }
        Some(terminals)
    }

    /// The live terminals every Presentation of `tab` holds; `None` for an unknown Tab.
    pub(crate) fn tab_live_terminals(
        &self,
        tab: &WorkbenchTabId,
        settled: &dyn Fn(&PaneRef) -> bool,
    ) -> Option<Vec<CloseTerminal>> {
        let tab = &self.cache.tabs()[self.cache.tab_index(tab)?];
        self.live_close_terminals(
            (0..tab.panel_count()).filter_map(|panel| tab.panel(panel)),
            settled,
        )
    }

    /// The live terminals every Presentation of one Panel holds; `None` for an unknown Panel.
    pub(crate) fn panel_live_terminals(
        &self,
        tab: &WorkbenchTabId,
        panel: &WorkbenchPanelId,
        settled: &dyn Fn(&PaneRef) -> bool,
    ) -> Option<Vec<CloseTerminal>> {
        let tab = &self.cache.tabs()[self.cache.tab_index(tab)?];
        let panel = tab.panel(tab.panel_index(panel)?)?;
        self.live_close_terminals([panel], settled)
    }

    /// Counts every Presentation of `panels`. `settled` answers whether an exact attachment
    /// reported a Host settlement; the pane record ties that attachment to the one binding it
    /// attached, so a sibling Presentation of the same Panel stays unobserved.
    fn close_impact<'p>(
        &self,
        panels: impl IntoIterator<Item = &'p crate::workbench_projection::ClientPanel>,
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
