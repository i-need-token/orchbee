//! Retained per-Presentation directory intent; physical observation admission belongs to the Site.
use super::{WorkbenchAuthority, WorkbenchCheckpointError as Error, agent_sessions::members};
use orchbee_workbench_protocol::{
    PresentationBinding, Snapshot, TerminalPresentationId, WorkbenchDirectoryPath, WorkerId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredDirectory {
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) path: WorkbenchDirectoryPath,
}
pub(super) fn validate(rows: &[StoredDirectory], snapshot: &Snapshot) -> Result<(), Error> {
    let mut retained = members(snapshot);
    for row in rows {
        if retained.remove(&row.presentation_id).is_none() {
            return Err(Error::InvalidTerminalDirectory);
        }
    }
    Ok(())
}
pub(super) fn capture(
    authority: &WorkbenchAuthority,
    worker: &WorkerId,
    observed: &[(PresentationBinding, WorkbenchDirectoryPath)],
) -> Result<Vec<StoredDirectory>, Error> {
    let retained = members(authority.snapshot());
    let mut paths = BTreeMap::new();
    for (binding, path) in observed {
        if &binding.worker_id != worker
            || !retained.values().any(|b| b.as_ref() == Some(binding))
            || paths.insert(binding.terminal_id.clone(), path).is_some()
        {
            return Err(Error::InvalidTerminalDirectory);
        }
    }
    Ok(retained
        .into_iter()
        .filter_map(|(id, binding)| {
            let path = binding
                .as_ref()
                .and_then(|b| paths.get(&b.terminal_id).copied())
                .or_else(|| authority.restart_directories.get(&id))?
                .clone();
            Some(StoredDirectory {
                presentation_id: id,
                path,
            })
        })
        .collect())
}
impl WorkbenchAuthority {
    /// Complete checkpoint membership, without the canonical Agent registration read quota.
    pub fn terminal_restart_bindings(&self) -> Vec<PresentationBinding> {
        members(self.snapshot()).into_values().flatten().collect()
    }
}
