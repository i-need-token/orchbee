//! Settled Workspace observation values: canonical Workspace metadata rows, their last known
//! Git facts, and the checkpoint that fences them. Readers produce them; Client state holds them.
use crate::client_context::ClientContextTarget;
use orchbee_workbench_protocol::{
    GitDivergence, MetadataTokens, SessionId, WorkbenchDirectoryPath, WorkbenchProjectId,
    WorkbenchRevision,
};
use std::sync::Arc;

/// Directories whose last Git answer a Session keeps for an immediate paint on return.
const MAXIMUM_KNOWN_GIT: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Git {
    pub(crate) branch: Option<String>,
    pub(crate) divergence: Option<GitDivergence>,
}
/// What a row keeps presenting, as one settled whole, while the Git facts of its new directory are
/// unknown: a name that moved with the commit waits for the line below it, so a directory change
/// moves the row once and never mixes one directory's name with another's facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeldRow {
    pub(crate) label: Option<String>,
    pub(crate) folder: Option<WorkbenchDirectoryPath>,
    pub(crate) git: Option<Git>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) target: ClientContextTarget,
    /// Fenced with this checkpoint, independent of the stable terminal target.
    pub(crate) project_id: Option<WorkbenchProjectId>,
    pub(crate) label: Option<String>,
    pub(crate) tokens: MetadataTokens,
    /// Canonical directory identity fences reuse of a last known Git observation.
    pub(crate) directory: Option<orchbee_workbench_protocol::Directory>,
    /// The first terminal's observed directory while it differs from `directory`.
    pub(crate) terminal_directory: Option<WorkbenchDirectoryPath>,
    pub(crate) git: Option<Git>,
    pub(crate) git_stale: bool,
    /// Present only while `git` is unknown for a directory the row just moved to.
    pub(crate) held: Option<HeldRow>,
}
impl Row {
    /// The directory `git` describes: the first terminal's, otherwise the Workspace's own.
    pub(crate) fn source_path(&self) -> Option<&WorkbenchDirectoryPath> {
        self.terminal_directory
            .as_ref()
            .or(self.directory.as_ref().map(|directory| &directory.path))
    }

    fn same_source(&self, other: &Self) -> bool {
        self.directory == other.directory && self.terminal_directory == other.terminal_directory
    }

    /// Completes a row without a Git answer from what `previous` knew. The same source keeps its
    /// fact. A changed source takes the last answer for its directory, or else holds what the row
    /// last settled on. `asked` says a complete round wanted Git and got no answer: it makes a
    /// retained fact stale and ends a hold, since no answer is coming soon. A rename under an unmoved
    /// source also ends one, because the person who renamed it expects to see that at once.
    pub(super) fn carry_git(&mut self, known: Option<&Self>, previous: &Checkpoint, asked: bool) {
        match known.filter(|known| known.same_source(self)) {
            Some(known) => {
                self.git.clone_from(&known.git);
                self.git_stale = self.git.is_some() && (asked || known.git_stale);
                self.held = known
                    .held
                    .clone()
                    .filter(|_| !asked && self.label == known.label);
            }
            None => {
                if let Some(git) = self
                    .source_path()
                    .and_then(|path| previous.git_at(path))
                    .cloned()
                {
                    self.git = Some(git);
                    self.git_stale = asked;
                } else if !asked {
                    self.held = known.map(|known| {
                        known.held.clone().unwrap_or_else(|| HeldRow {
                            label: known.label.clone(),
                            folder: known.source_path().cloned(),
                            git: known.git.clone(),
                        })
                    });
                }
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectSummary {
    pub(crate) project_id: WorkbenchProjectId,
    pub(crate) label: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    pub(crate) session_id: SessionId,
    pub(crate) revision: WorkbenchRevision,
    pub(crate) metadata_revision: WorkbenchRevision,
    pub(crate) git_requested: bool,
    pub(crate) rows: Arc<[Row]>,
    pub(crate) projects: Arc<[ProjectSummary]>,
    /// The Session's last Git answers by directory, oldest first. Hints for presentation only.
    pub(crate) known_git: Arc<[(WorkbenchDirectoryPath, Git)]>,
}
pub(crate) type Observation = crate::machine_observation::outcome::Observation<Checkpoint>;

impl Checkpoint {
    /// The last Git answer for `path`: a current row's, otherwise an earlier round's.
    pub(super) fn git_at(&self, path: &WorkbenchDirectoryPath) -> Option<&Git> {
        self.rows
            .iter()
            .filter(|row| row.source_path() == Some(path))
            .find_map(|row| row.git.as_ref())
            .or_else(|| {
                self.known_git
                    .iter()
                    .find(|(known, _)| known == path)
                    .map(|(_, git)| git)
            })
    }

    /// Reads without an answer complete their rows from `previous`, by exact Workspace and
    /// observed source, then by the directory's last answer. A read that asked for Git and got
    /// none makes the retained fact stale; a metadata-only or partial read says nothing about the
    /// rows it left unread, so a retained fact keeps its freshness and a hold continues.
    pub(crate) fn retaining_git(&self, previous: &Self) -> Self {
        let mut checkpoint = self.clone();
        if self.session_id != previous.session_id {
            return checkpoint;
        }
        let mut rows = self.rows.to_vec();
        for row in rows.iter_mut().filter(|row| row.git.is_none()) {
            let known = previous
                .rows
                .iter()
                .find(|known| known.target == row.target);
            row.carry_git(known, previous, self.git_requested);
        }
        let mut known_git = previous.known_git.to_vec();
        for row in previous.rows.iter().chain(rows.iter()) {
            let (Some(path), Some(git)) = (row.source_path(), row.git.as_ref()) else {
                continue;
            };
            match known_git.iter_mut().find(|(known, _)| known == path) {
                Some((_, known)) => known.clone_from(git),
                None => known_git.push((path.clone(), git.clone())),
            }
        }
        let excess = known_git.len().saturating_sub(MAXIMUM_KNOWN_GIT);
        known_git.drain(..excess);
        checkpoint.rows = rows.into();
        checkpoint.known_git = known_git.into();
        checkpoint
    }
}
