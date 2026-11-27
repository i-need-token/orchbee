//! The Session's File Bar: what it reads of the serving Worker and of each Workspace's scope. A
//! Workspace has a root only while its primary Directory is on the serving Worker, and the root is
//! where the Workspace stands, as Navigation states it.

use super::LocalTerminalService;
use crate::{
    file_bar::{FileBar, Host},
    worker_obcn_carrier::WorkerRequestHandle,
};
use orchbee_files_protocol::{MAXIMUM_ROOT_BYTES, WorkspaceId, WorkspaceScope};
use orchbee_workbench_protocol::{WorkbenchWorkspaceId, WorkerId};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, Weak},
    time::Instant,
};

impl LocalTerminalService {
    /// Starts the Session's File Bar, whose process endpoints live in `runtime_directory`.
    pub(crate) fn start_file_bar(self: &Arc<Self>, runtime_directory: PathBuf) -> io::Result<()> {
        let worker: Weak<Self> = Arc::downgrade(self);
        let scopes: Weak<Self> = Arc::downgrade(self);
        let host = Host {
            worker: Box::new(move || worker.upgrade()?.file_bar_worker()),
            scopes: Box::new(move |workspaces| match scopes.upgrade() {
                Some(service) => service.file_bar_scopes(workspaces),
                None => vec![None; workspaces.len()],
            }),
        };
        let bar = FileBar::start(runtime_directory, host, self.event_journal().as_ref())?;
        self.file_bar
            .set(bar)
            .map_err(|_| io::Error::other("the File Bar already started"))
    }

    /// The Session's File Bar, once it started.
    pub(crate) fn file_bar(&self) -> Option<&Arc<FileBar>> {
        self.file_bar.get()
    }

    /// Stops the File Bar, draining its process within `deadline`.
    pub(crate) fn stop_file_bar_before(&self, deadline: Instant) {
        if let Some(bar) = self.file_bar.get() {
            bar.stop_before(deadline);
        }
    }

    fn file_bar_worker(&self) -> Option<WorkerRequestHandle> {
        self.workers
            .lock()
            .ok()?
            .request_handle()
            .filter(WorkerRequestHandle::is_active)
    }

    fn file_bar_scopes(&self, workspaces: &[WorkspaceId]) -> Vec<Option<WorkspaceScope>> {
        let unknown = || vec![None; workspaces.len()];
        let Ok(workers) = self.workers.lock() else {
            return unknown();
        };
        let serving = workers
            .request_handle()
            .filter(WorkerRequestHandle::is_active)
            .and_then(|handle| {
                WorkerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(handle.worker_id().as_bytes())
                ))
                .ok()
            });
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return unknown();
        };
        let Some(authority) = coordinator.worktree_authority() else {
            return unknown();
        };
        let navigation = &authority.snapshot().navigation;
        workspaces
            .iter()
            .map(|workspace| {
                let workspace = WorkbenchWorkspaceId::parse(&workspace.to_string()).ok()?;
                navigation.workspace(&workspace)?;
                let root = navigation
                    .workspace_directory(&workspace)
                    .filter(|directory| Some(&directory.worker_id) == serving.as_ref())
                    .and_then(|_| navigation.workspace_current_path(&workspace))
                    .map(|path| path.as_str())
                    .filter(|path| path.len() <= MAXIMUM_ROOT_BYTES);
                Some(root.map_or(WorkspaceScope::NoRoot, |root| {
                    WorkspaceScope::Root(root.to_owned())
                }))
            })
            .collect()
    }
}
