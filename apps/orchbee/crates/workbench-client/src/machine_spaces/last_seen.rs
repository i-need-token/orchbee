//! The last-seen list: what this computer last read of each saved machine's Workspaces, kept in
//! `internal/state/machine-spaces.json` so a machine that is not connected still lists them, also
//! after a restart. Any Client writes the whole list; the last writer wins. A list this build cannot
//! read counts as empty and is never rewritten.

// module-layer: workflow

use super::{Checkpoint, ProjectSummary, Row};
use crate::client_context::ClientContextTarget;
use crate::machines::ProfileId;
use crate::workbench_projection::ClientContextLocation;
use orchbee_local_runtime::{DataRoot, OpenedRecord, VersionedFile, VersionedRecord};
use orchbee_workbench_protocol::{
    MetadataTokens, SessionId, WorkbenchProjectId, WorkbenchRevision, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::Arc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MAX_BYTES: usize = 1024 * 1024;
/// A change waits this long for the next one, so a burst of renames writes once.
const SETTLE: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u64,
    machines: BTreeMap<String, Entry>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Entry {
    session_id: String,
    projects: Vec<Project>,
    workspaces: Vec<Workspace>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    id: String,
    label: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Workspace {
    id: String,
    terminal_context_id: String,
    label: Option<String>,
    project_id: Option<String>,
}

impl Entry {
    fn of(checkpoint: &Checkpoint) -> Self {
        Self {
            session_id: checkpoint.session_id.as_str().to_owned(),
            projects: checkpoint
                .projects
                .iter()
                .map(|project| Project {
                    id: project.project_id.as_str().to_owned(),
                    label: project.label.clone(),
                })
                .collect(),
            workspaces: checkpoint
                .rows
                .iter()
                .filter_map(|row| match &row.target.location {
                    ClientContextLocation::Workspace {
                        workspace_id,
                        terminal_context_id,
                    } => Some(Workspace {
                        id: workspace_id.as_str().to_owned(),
                        terminal_context_id: terminal_context_id.as_str().to_owned(),
                        label: row.label.clone(),
                        project_id: row.project_id.as_ref().map(|id| id.as_str().to_owned()),
                    }),
                    ClientContextLocation::NoWorkspace => None,
                })
                .collect(),
        }
    }

    /// The checkpoint a machine that is not connected lists. Revision zero never counts as newer
    /// than what its machine answers next.
    fn checkpoint(&self) -> Option<Checkpoint> {
        let session_id = SessionId::parse(&self.session_id).ok()?;
        let projects = self
            .projects
            .iter()
            .map(|project| {
                Some(ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&project.id).ok()?,
                    label: project.label.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let rows = self
            .workspaces
            .iter()
            .map(|workspace| {
                Some(Row {
                    target: ClientContextTarget {
                        session_id: session_id.clone(),
                        location: ClientContextLocation::Workspace {
                            workspace_id: WorkbenchWorkspaceId::parse(&workspace.id).ok()?,
                            terminal_context_id: WorkbenchTerminalContextId::parse(
                                &workspace.terminal_context_id,
                            )
                            .ok()?,
                        },
                    },
                    project_id: match &workspace.project_id {
                        Some(id) => Some(WorkbenchProjectId::parse(id).ok()?),
                        None => None,
                    },
                    label: workspace.label.clone(),
                    tokens: MetadataTokens::default(),
                    directory: None,
                    terminal_directory: None,
                    git: None,
                    git_stale: false,
                    held: None,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Checkpoint {
            session_id,
            revision: WorkbenchRevision::new(0),
            metadata_revision: WorkbenchRevision::new(0),
            git_requested: false,
            rows: rows.into(),
            projects: projects.into(),
            known_git: Vec::new().into(),
        })
    }
}

fn admits(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Record>(bytes).is_ok()
}

/// Reads the list and writes it again when what the machines answer changes.
pub struct LastSeen {
    record: Option<VersionedFile>,
    /// The list as this process last read or wrote it; `None` while the stored list is one this
    /// build cannot use, which is then never rewritten.
    written: Option<BTreeMap<String, Entry>>,
    changed: Option<Instant>,
    writing: Option<JoinHandle<()>>,
}

impl LastSeen {
    /// The list of the data root this process's environment names; without one, nothing is read or
    /// written.
    pub fn configured() -> Self {
        Self::open(
            DataRoot::resolve(|name| std::env::var_os(name))
                .ok()
                .map(|root| VersionedFile::device(root, VersionedRecord::MachineSpaces)),
        )
    }

    /// The list of `root`.
    pub fn in_data_root(root: DataRoot) -> Self {
        Self::open(Some(VersionedFile::device(
            root,
            VersionedRecord::MachineSpaces,
        )))
    }

    fn open(record: Option<VersionedFile>) -> Self {
        let written = match record.as_ref().map(|record| record.read(MAX_BYTES, admits)) {
            None | Some(Ok(OpenedRecord::Absent) | Err(_)) => Some(BTreeMap::new()),
            Some(Ok(OpenedRecord::Readable(bytes))) => Some(
                serde_json::from_slice::<Record>(&bytes)
                    .map(|record| record.machines)
                    .unwrap_or_default(),
            ),
            // A list this build cannot use stays as it is; one it cannot read is replaced.
            Some(Ok(OpenedRecord::Unusable(_))) => None,
        };
        Self {
            record,
            written,
            changed: None,
            writing: None,
        }
    }

    /// What the list holds for each of `machines`.
    pub fn checkpoints(&self, machines: &[ProfileId]) -> Vec<(ProfileId, Arc<Checkpoint>)> {
        let Some(written) = &self.written else {
            return Vec::new();
        };
        machines
            .iter()
            .filter_map(|id| {
                let checkpoint = written.get(id.as_str())?.checkpoint()?;
                Some((id.clone(), Arc::new(checkpoint)))
            })
            .collect()
    }

    /// The saved machines whose last-seen entry lists at least one Workspace, which the hub
    /// connects first.
    pub fn machines_with_workspaces(&self) -> Vec<String> {
        self.written
            .iter()
            .flatten()
            .filter(|(_, entry)| !entry.workspaces.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Takes the machines' current answers: `fresh` holds each connected machine's checkpoint,
    /// `machines` every saved machine. A machine without a fresh answer keeps its entry; a machine
    /// no longer saved loses it. The list is written once answers settle.
    pub fn observe<'a>(
        &mut self,
        fresh: impl Iterator<Item = (&'a ProfileId, &'a Checkpoint)>,
        machines: &[ProfileId],
        now: Instant,
    ) {
        let Some(written) = &self.written else {
            return;
        };
        let mut next = written.clone();
        next.retain(|id, _| machines.iter().any(|machine| machine.as_str() == id));
        for (id, checkpoint) in fresh {
            next.insert(id.as_str().to_owned(), Entry::of(checkpoint));
        }
        if &next == written {
            self.changed = None;
            return;
        }
        let changed = *self.changed.get_or_insert(now);
        if now.saturating_duration_since(changed) < SETTLE
            || self
                .writing
                .as_ref()
                .is_some_and(|writing| !writing.is_finished())
        {
            return;
        }
        let Some(record) = self.record.clone() else {
            self.written = Some(next);
            self.changed = None;
            return;
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&Record {
            version: VersionedRecord::MachineSpaces.current_version(),
            machines: next.clone(),
        }) else {
            return;
        };
        if bytes.len() > MAX_BYTES {
            return;
        }
        self.written = Some(next);
        self.changed = None;
        // The write flushes to disk, which the screen must not wait for.
        self.writing = std::thread::Builder::new()
            .name("machine-spaces".into())
            .spawn(move || write(&record, &bytes))
            .ok();
    }
}

/// Replaces the list under its writer lock unless the stored list is one this build cannot use.
fn write(record: &VersionedFile, bytes: &[u8]) {
    let Ok((mut writer, opened)) = record.acquire(MAX_BYTES, admits) else {
        return;
    };
    if matches!(opened, Ok(OpenedRecord::Unusable(_))) {
        return;
    }
    let _ = writer.replace(bytes);
}

#[cfg(all(test, windows))]
mod tests;
