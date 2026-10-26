//! Native UI admission for exact link clicks; this adapter revalidates the current painted source
//! and translates lane state into notices without resolving content or performing effects.

use super::{NativeUi, WorkbenchClientNotice};
use crate::{
    terminal_input::{PointerLinkAdmission, PointerSelectionCell},
    terminal_link_activation::PaintedLinkSource,
};

impl NativeUi {
    /// Revalidate and admit one pointer-owned source without changing focus or terminal control.
    pub(in crate::workbench_client) fn pointer_link(
        &mut self,
        source: PaintedLinkSource,
        cell: PointerSelectionCell,
    ) -> PointerLinkAdmission {
        if self.retirement_failure.is_some() {
            return PointerLinkAdmission::Rejected;
        }
        let current = self.view().painted_link_source(source.pane.pane().panel());
        if current.as_ref() != Some(&source) {
            self.report_link_notice(WorkbenchClientNotice::LinkFailed);
            return PointerLinkAdmission::Rejected;
        }
        if !self
            .capabilities
            .supports(crate::tui_capabilities::Capability::Browser)
        {
            self.report_link_notice(WorkbenchClientNotice::Unsupported(
                crate::tui_capabilities::Capability::Browser,
            ));
            return PointerLinkAdmission::Rejected;
        }
        match self.links.admit(source, cell.column, cell.row) {
            crate::terminal_link_activation::Admission::Accepted => {
                self.clear_synchronous_link_notice();
                PointerLinkAdmission::Admitted
            }
            crate::terminal_link_activation::Admission::Busy => {
                self.report_link_notice(WorkbenchClientNotice::LinkBusy);
                PointerLinkAdmission::Busy
            }
            crate::terminal_link_activation::Admission::Unavailable => {
                self.report_link_notice(WorkbenchClientNotice::LinkFailed);
                PointerLinkAdmission::Rejected
            }
        }
    }

    fn report_link_notice(&mut self, notice: WorkbenchClientNotice) {
        self.links.clear_notice();
        self.command_notice = self
            .clock
            .now()
            .checked_add(std::time::Duration::from_secs(3))
            .map(|until| (notice, until));
    }

    fn clear_synchronous_link_notice(&mut self) {
        if self.command_notice.as_ref().is_some_and(|(notice, _)| {
            matches!(
                notice,
                WorkbenchClientNotice::LinkBusy | WorkbenchClientNotice::LinkFailed
            )
        }) {
            self.command_notice = None;
        }
    }
}

#[cfg(test)]
mod tests;
