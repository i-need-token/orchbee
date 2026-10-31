//! Workspace summaries compose the existing read-only Git owner without caching its facts.
use super::{
    Failure, FailureKind,
    console_interrupt::SignalWatch,
    output::{Bounded, TooLarge},
    query,
    results::{Projection, WorkspaceRow},
};
use crate::cli_text::CliText;
use crate::{
    local_client::{LocalClient, WorktreeQueryError},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    PluginWorktreeContext, Snapshot, WorkbenchProjectId, WorkbenchRequestId, WorkbenchWorkspaceId,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome, WorktreeEntryText, WorktreeQueryFailure,
    WorktreeSource,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug)]
enum Observation {
    NoDirectory,
    Unavailable {
        request_id: WorkbenchRequestId,
        reason: WorktreeQueryFailure,
    },
    Observed {
        source: Value,
        summary: Value,
    },
}
#[derive(Debug)]
pub(super) struct Document(BTreeMap<String, Observation>);

fn classify(
    snapshot: &Snapshot,
    row: &WorkspaceRow,
    request_id: WorkbenchRequestId,
    outcome: WorkbenchWorktreeQueryOutcome,
) -> Result<Observation, Failure> {
    let invalid = || Failure::new(FailureKind::ProtocolInvalid);
    let WorkbenchWorktreeQueryOutcome::Found {
        session_id,
        workbench_revision,
        worker_id,
        source,
        entries,
    } = outcome
    else {
        let WorkbenchWorktreeQueryOutcome::Failed { reason } = outcome else {
            unreachable!()
        };
        let kind = match reason {
            WorktreeQueryFailure::NotFound {} | WorktreeQueryFailure::SourceChanged {} => {
                return Err(super::query::workbench_failure("workbench-changed"));
            }
            WorktreeQueryFailure::Denied {} => Some(FailureKind::Denied),
            WorktreeQueryFailure::MalformedListing {} => Some(FailureKind::ProtocolInvalid),
            WorktreeQueryFailure::TransportLoss {} => Some(FailureKind::TransportLoss),
            WorktreeQueryFailure::Timeout {} => Some(FailureKind::Timeout),
            WorktreeQueryFailure::Unsupported { .. }
            | WorktreeQueryFailure::Unavailable { .. }
            | WorktreeQueryFailure::GitFailed { .. }
            | WorktreeQueryFailure::SpawnUnavailable { .. }
            | WorktreeQueryFailure::SpawnFailed { .. }
            | WorktreeQueryFailure::TooLarge { .. }
            | WorktreeQueryFailure::OwnerIo { .. } => None,
        };
        return match kind {
            Some(kind) => Err(Failure::new(kind)),
            None => Ok(Observation::Unavailable { request_id, reason }),
        };
    };
    if session_id != snapshot.session_id || workbench_revision != snapshot.revision {
        return Err(super::query::workbench_failure("workbench-changed"));
    }
    let directory = snapshot
        .navigation
        .directories
        .iter()
        .find(|directory| {
            row.primary_directory_id.as_deref() == Some(directory.directory_id.as_str())
        })
        .ok_or_else(invalid)?;
    if source
        .workspace_id
        .as_ref()
        .map(WorkbenchWorkspaceId::as_str)
        != Some(row.workspace_id.as_str())
        || source.requested_path != directory.path
        || worker_id != directory.worker_id
    {
        return Err(invalid());
    }
    let checkout = entries
        .iter()
        .find(|entry| {
            entry.checkout.path == WorktreeEntryText::Utf8(source.checkout_path.as_str().to_owned())
        })
        .ok_or_else(invalid)?;
    if !checkout.workspaces.iter().any(|association| {
        association.workspace_id.as_str() == row.workspace_id
            && association
                .project_id
                .as_ref()
                .map(WorkbenchProjectId::as_str)
                == row.project_id.as_deref()
    }) {
        return Err(invalid());
    }
    let name = source
        .repository_root
        .as_str()
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .ok_or_else(invalid)?;
    Ok(Observation::Observed {
        source: json!({"kind":"worktree-query","requestId":request_id,"sessionId":session_id,"workbenchRevision":workbench_revision}),
        summary: json!({"workerId":worker_id,"repositoryRoot":source.repository_root,
            "commonDirectory":source.common_directory,"repositoryName":name,
            "checkoutPath":source.checkout_path,"linked":checkout.linked}),
    })
}

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    projection: &Projection,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Option<Document>, Failure> {
    let workspaces: Vec<_> = match projection {
        Projection::WorkspaceList(rows) => rows.iter().collect(),
        Projection::WorkspaceGet(doc) => vec![&doc.row],
        _ => return Ok(None),
    };
    let mut rows = BTreeMap::new();
    for row in workspaces {
        signals.check_before(deadline)?;
        let observation = if row.primary_directory_id.is_none() {
            Observation::NoDirectory
        } else {
            let request_id = query::request_id()?;
            let result = client.query_worktrees(
                WorkbenchWorktreeQuery {
                    protocol_version: snapshot.protocol_version,
                    request_id: request_id.clone(),
                    session_id: snapshot.session_id.clone(),
                    source: WorktreeSource::Workspace {
                        workspace_id: WorkbenchWorkspaceId::parse(&row.workspace_id)
                            .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?,
                    },
                    trust_repository: false,
                },
                deadline,
                signals.poll_fd(),
            );
            signals.check_before(deadline)?;
            let result = result.map_err(|error| {
                Failure::new(match error {
                    WorktreeQueryError::Unavailable => FailureKind::ServiceUnavailable,
                    WorktreeQueryError::Encoding => FailureKind::InternalFailure,
                    WorktreeQueryError::Protocol => FailureKind::ProtocolInvalid,
                    WorktreeQueryError::Transport => FailureKind::TransportLoss,
                    WorktreeQueryError::Timeout => FailureKind::Timeout,
                })
            })?;
            classify(snapshot, row, request_id, result.outcome)?
        };
        rows.insert(row.workspace_id.clone(), observation);
    }
    Ok(Some(Document(rows)))
}

impl Observation {
    fn fields(&self) -> Value {
        match self {
            Self::NoDirectory => {
                json!({"worktree":null,"worktreeObservation":{"state":"no-directory"}})
            }
            Self::Unavailable { request_id, reason } => json!({"worktree":null,
                "worktreeObservation":{"state":"unavailable","requestId":request_id,"reason":reason}}),
            Self::Observed { source, summary } => json!({"worktree":summary,
                "worktreeObservation":{"state":"observed","source":source}}),
        }
    }
}
impl Document {
    pub(super) fn plugin_context(
        &self,
        workspace: &str,
    ) -> Result<Option<PluginWorktreeContext>, Failure> {
        match self.0.get(workspace) {
            Some(Observation::Observed { summary, .. }) => serde_json::from_value(summary.clone())
                .map(Some)
                .map_err(|_| Failure::new(FailureKind::ProtocolInvalid)),
            Some(_) => Ok(None),
            None => Err(Failure::new(FailureKind::ProtocolInvalid)),
        }
    }
    pub(super) fn enrich(&self, result: &mut Value) {
        let mut row = |row: &mut Value| {
            let observation = self
                .0
                .get(row["workspaceId"].as_str().expect("typed Workspace ID"))
                .expect("complete worktree observations");
            row.as_object_mut()
                .expect("typed Workspace row")
                .extend(observation.fields().as_object().unwrap().clone());
        };
        if let Some(rows) = result.get_mut("workspaces").and_then(Value::as_array_mut) {
            rows.iter_mut().for_each(&mut row);
        }
        if let Some(workspace) = result.get_mut("workspace") {
            row(workspace);
        }
    }
    pub(super) fn render(&self, out: &mut Bounded, locale: Locale) -> Result<(), TooLarge> {
        out.write(format_args!(
            "{}\n",
            CliText::new(locale).cli_workspace_worktrees_heading()
        ))?;
        for (id, observation) in &self.0 {
            let (state, path) = match observation {
                Observation::NoDirectory => {
                    (CliText::new(locale).cli_workspace_worktree_states()[0], "-")
                }
                Observation::Unavailable { .. } => {
                    (CliText::new(locale).cli_workspace_worktree_states()[1], "-")
                }
                Observation::Observed { summary, .. } => (
                    CliText::new(locale).cli_workspace_worktree_states()
                        [if summary["linked"] == true { 3 } else { 2 }],
                    summary["checkoutPath"]
                        .as_str()
                        .expect("typed checkout path"),
                ),
            };
            out.write(format_args!(
                "{}  {}  {}\n",
                id,
                state,
                crate::cli_text::escape::escape(path)
            ))?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests;
