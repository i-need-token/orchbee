//! Native UI admission for exact link clicks; this adapter revalidates the current painted source
//! and translates lane state into notices without resolving content or performing effects.

use super::{WorkbenchClientNotice, WorkbenchClientUi};
use crate::{
    terminal_link_activation::PaintedLinkSource,
    workbench_client::input::{PaneAdmission, PointerSelectionCell},
};

impl WorkbenchClientUi {
    /// Revalidate and admit one pointer-owned source without changing focus or terminal control.
    pub(in crate::workbench_client) fn pointer_link(
        &mut self,
        source: PaintedLinkSource,
        cell: PointerSelectionCell,
    ) -> PaneAdmission {
        if self.retirement_failure.is_some() {
            return PaneAdmission::Rejected;
        }
        let current = self.view().painted_link_source(source.pane.pane().panel());
        if current.as_ref() != Some(&source) {
            self.report_link_notice(WorkbenchClientNotice::LinkFailed);
            return PaneAdmission::Rejected;
        }
        if !self
            .capabilities
            .supports(crate::tui_capabilities::Capability::Browser)
        {
            self.report_link_notice(WorkbenchClientNotice::Unsupported(
                crate::tui_capabilities::Capability::Browser,
            ));
            return PaneAdmission::Rejected;
        }
        match self.links.admit(source, cell.column, cell.row) {
            crate::terminal_link_activation::Admission::Accepted => {
                self.clear_synchronous_link_notice();
                PaneAdmission::Admitted
            }
            crate::terminal_link_activation::Admission::Busy => {
                self.report_link_notice(WorkbenchClientNotice::LinkBusy);
                PaneAdmission::Busy
            }
            crate::terminal_link_activation::Admission::Unavailable => {
                self.report_link_notice(WorkbenchClientNotice::LinkFailed);
                PaneAdmission::Rejected
            }
        }
    }

    fn report_link_notice(&mut self, notice: WorkbenchClientNotice) {
        self.links.clear_notice();
        self.command_notice.set(
            self.clock
                .now()
                .checked_add(std::time::Duration::from_secs(3))
                .map(|until| (notice, until)),
        );
    }

    fn clear_synchronous_link_notice(&mut self) {
        self.command_notice.clear_if(|notice| {
            matches!(
                notice,
                WorkbenchClientNotice::LinkBusy | WorkbenchClientNotice::LinkFailed
            )
        });
    }
}

#[cfg(test)]
mod tests;
