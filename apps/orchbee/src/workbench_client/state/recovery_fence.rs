//! The one table of which retained commit recoveries fence which structural flows.
//!
//! A known Move, Panel or Rename commit whose canonical projection has not installed yet keeps a
//! recovery record; an uncertain Rename keeps its operation. Flows ask for their fence by name
//! instead of listing records, so a new recovery kind is decided here once for every flow.

use super::WorkbenchClientState;

/// Structural flows that consult retained commit recoveries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RecoveryFence {
    /// Submitting another mutation: every unprojected Move, Panel or Rename commit.
    Mutation,
    /// Removing a context or opening Workspace Close: every recovery, including an uncertain Rename.
    Removal,
    /// Selecting a context or retiring the current membership: Panel recovery does not fence these.
    Navigation,
    /// Workspace setup authority: only Move and Rename projections fence it.
    Setup,
}

impl WorkbenchClientState {
    pub(super) fn recovery_fences(&self, fence: RecoveryFence) -> bool {
        let projection = self.move_recovery.is_some() || self.rename_recovery.is_some();
        let panel = self.panel_recovery.is_some();
        let uncertain = self.rename_uncertain.is_some();
        match fence {
            RecoveryFence::Mutation => projection || panel,
            RecoveryFence::Removal => projection || panel || uncertain,
            RecoveryFence::Navigation => projection || uncertain,
            RecoveryFence::Setup => projection,
        }
    }

    /// An installed canonical refresh resolves every projection recovery; settled commits and an
    /// uncertain Rename are not projection recoveries and survive it.
    pub(super) fn resolve_projection_recoveries(&mut self) {
        self.move_recovery = None;
        self.panel_recovery = None;
        self.rename_recovery = None;
    }
}
