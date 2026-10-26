//! Nonmodal navigation organization uses the existing canonical setup mailbox.
use super::{NativeUi, OperationDeadlines, WorkbenchClientView};
use crate::tui_pointer::SidebarReorderSource;
use orchbee_workbench_protocol::{NavigationConfiguration, configure_navigation};

impl WorkbenchClientView<'_> {
    /// A retired carrier cannot keep speculative navigation alive in another view.
    pub(crate) fn sidebar_navigation(
        &self,
    ) -> Option<crate::workbench_client::SidebarNavigationProjection> {
        if self.retirement_requested || self.retirement.is_some() {
            return None;
        }
        Some(self.state.as_ref()?.sidebar_navigation())
    }
}

impl NativeUi {
    pub(in crate::workbench_client) fn sidebar_reorder_source(
        &mut self,
    ) -> Option<SidebarReorderSource> {
        if self.retirement_failure.is_some()
            || self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            return None;
        }
        let state = self.state.as_ref()?;
        if state.setup_blocks_structure() || !state.setup_authority_available() {
            return None;
        }
        Some(SidebarReorderSource::from_snapshot(
            state.cache().shared_canonical_snapshot(),
        ))
    }
    pub(in crate::workbench_client) fn commit_sidebar_reorder(
        &mut self,
        source: &SidebarReorderSource,
        operation: NavigationConfiguration,
    ) {
        let admitted = (|| {
            if self.sidebar_reorder_source().as_ref() != Some(source) {
                return None;
            }
            let changed = configure_navigation(source.navigation(), &operation).ok()?;
            if changed.is_none() {
                return Some(());
            }
            let deadlines = OperationDeadlines::from_sample(self.clock.now())?;
            self.mailbox
                .as_ref()?
                .try_enqueue_navigation_configuration_for(
                    self.state.as_mut()?,
                    operation,
                    deadlines,
                    &self.clock,
                )
                .ok()?;
            Some(())
        })();
        if admitted.is_none() {
            self.command_notice = Some((
                super::WorkbenchClientNotice::MenuUnavailable,
                self.clock.now() + std::time::Duration::from_secs(3),
            ));
        }
    }
}
