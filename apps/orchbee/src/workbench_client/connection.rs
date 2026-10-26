use super::TerminalStatus;
use super::workbench_projection::NativeTerminalBinding;
use crate::{
    local_client::LocalClient,
    workbench_client::state::{
        PendingCloseResult, PendingCreateCommit, PendingMoveCommit, PendingPanelCommit,
    },
};
use orchbee_client_server_protocol::{
    Geometry, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId, TerminalViewerAction as Viewer,
    WorkerId as ObcsWorkerId,
};
use orchbee_workbench_protocol::SiteId as WorkbenchSiteId;
use std::sync::Arc;
mod native;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(super) use native::{RecoveryShutdown, run_status_recovery};

use super::mailbox::CommandMailbox;

type Result<T> = std::result::Result<T, TerminalStatus>;

pub(super) fn run_native(
    client: LocalClient,
    initial_geometry: Geometry,
    mailbox: Arc<CommandMailbox>,
    context: Option<super::workbench_projection::NativeStartupTarget>,
) -> NativeWorkerExit {
    native::run_at(client, initial_geometry, mailbox, context)
}

pub(super) struct NativeWorkerExit {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    unavailable_poll: Option<super::state::retired_membership::UnavailablePoll>,
    pending_create: Option<PendingCreateCommit>,
    pending_move: Option<PendingMoveCommit>,
    pending_panel: Option<PendingPanelCommit>,
    pending_close: Option<PendingCloseResult>,
    pending_rename: Option<super::state::tab_rename::RenameWorkerEvidence>,
    pending_setup: Option<super::state::workspace_setup::SetupWorkerEvidence>,
    pending_workspace_close: Option<super::state::workspace_close::WorkspaceCloseEvidence>,
    pending_takeover: Option<super::state::control_takeover::TakeoverEvidence>,
}

impl NativeWorkerExit {
    fn with_takeover(
        mut self,
        evidence: Option<super::state::control_takeover::TakeoverEvidence>,
    ) -> Self {
        self.pending_takeover = evidence;
        self
    }
    pub(super) fn take_takeover_result(
        &mut self,
    ) -> Option<super::state::control_takeover::TakeoverEvidence> {
        self.pending_takeover.take()
    }
    fn with_workspace_close(
        mut self,
        evidence: Option<super::state::workspace_close::WorkspaceCloseEvidence>,
    ) -> Self {
        self.pending_workspace_close = evidence;
        self
    }
    pub(super) fn take_workspace_close_result(
        &mut self,
    ) -> Option<super::state::workspace_close::WorkspaceCloseEvidence> {
        self.pending_workspace_close.take()
    }
    fn with_setup(
        mut self,
        evidence: Option<super::state::workspace_setup::SetupWorkerEvidence>,
    ) -> Self {
        self.pending_setup = evidence;
        self
    }
    pub(super) fn take_setup_result(
        &mut self,
    ) -> Option<super::state::workspace_setup::SetupWorkerEvidence> {
        self.pending_setup.take()
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn with_unavailable_poll(
        mut self,
        evidence: Option<super::state::retired_membership::UnavailablePoll>,
    ) -> Self {
        self.unavailable_poll = evidence;
        self
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) fn take_unavailable_poll(
        &mut self,
    ) -> Option<super::state::retired_membership::UnavailablePoll> {
        self.unavailable_poll.take()
    }

    const fn new(
        pending_create: Option<PendingCreateCommit>,
        pending_move: Option<PendingMoveCommit>,
        pending_panel: Option<PendingPanelCommit>,
        pending_close: Option<PendingCloseResult>,
        pending_rename: Option<super::state::tab_rename::RenameWorkerEvidence>,
    ) -> Self {
        Self {
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            unavailable_poll: None,
            pending_create,
            pending_move,
            pending_panel,
            pending_close,
            pending_rename,
            pending_setup: None,
            pending_workspace_close: None,
            pending_takeover: None,
        }
    }

    #[cfg(test)]
    pub(super) fn into_pending_create(self) -> Option<PendingCreateCommit> {
        self.pending_create
    }

    #[cfg(test)]
    pub(super) fn into_pending_commits(
        self,
    ) -> (Option<PendingCreateCommit>, Option<PendingMoveCommit>) {
        (self.pending_create, self.pending_move)
    }

    #[cfg(test)]
    pub(super) fn into_pending_close(self) -> Option<PendingCloseResult> {
        self.pending_close
    }

    pub(super) fn into_pending_results(self) -> PendingWorkerResults {
        PendingWorkerResults {
            create: self.pending_create,
            moved: self.pending_move,
            panel: self.pending_panel,
            close: self.pending_close,
            renamed: self.pending_rename,
        }
    }
}

pub(super) struct PendingWorkerResults {
    pub(super) create: Option<PendingCreateCommit>,
    pub(super) moved: Option<PendingMoveCommit>,
    pub(super) panel: Option<PendingPanelCommit>,
    pub(super) close: Option<PendingCloseResult>,
    pub(super) renamed: Option<super::state::tab_rename::RenameWorkerEvidence>,
}

fn attach_binding(site_id: &WorkbenchSiteId, binding: &NativeTerminalBinding) -> Result<Viewer> {
    Ok(Viewer::AttachBinding {
        site_id: ObcsSiteId::parse(site_id.as_str())
            .map_err(|_error| TerminalStatus::Unavailable)?,
        worker_id: ObcsWorkerId::parse(binding.worker_id().as_str())
            .map_err(|_error| TerminalStatus::Unavailable)?,
        terminal_id: ObcsTerminalId::parse(binding.terminal_id().as_str())
            .map_err(|_error| TerminalStatus::Unavailable)?,
    })
}
