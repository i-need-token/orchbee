//! One-effect Workbench mutation commands. Every invocation admits one
//! Client Session, performs one bounded snapshot read that fixes the Site
//! identity, the expected revision, and every resolved target identity, then
//! submits exactly one `workbench.command` under a fresh request identity.
//! Nothing is retried or replayed: an ambiguous post-submission outcome
//! retires the session and surfaces as a typed `outcome-unknown` failure.

use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchTerminalGeometry};
use serde_json::{Map, Value, json};

use super::Output;
use super::schema::MUTATION_RESULT_ID;
#[cfg(not(target_os = "linux"))]
use super::{Failure, FailureKind, failure_output, stage_reason_details};
use crate::invocation::{MutationCommand, OutputFormat};
use crate::localization::Locale;

/// The definite one-command answer the CLI may print: request-side facts
/// (label, direction, resolved identities) are echoed for correlation and
/// every server-carried field is preserved — nothing is invented.
pub(super) struct Receipt {
    site_id: String,
    request_id: String,
    base_revision: WorkbenchRevision,
    action: &'static str,
    outcome: &'static str,
    revision: Option<WorkbenchRevision>,
    accepted_revision: Option<WorkbenchRevision>,
    close_request_id: Option<String>,
    ids: Vec<(&'static str, String)>,
    direction: Option<&'static str>,
    label: Option<String>,
    path: Option<String>,
    terminal: Option<TerminalFact>,
}

impl Receipt {
    fn source(&self) -> Value {
        command_source(
            self.site_id.as_str(),
            self.request_id.as_str(),
            self.base_revision,
        )
    }

    fn document(&self) -> Value {
        let mut result = Map::from_iter([
            ("action".to_owned(), json!(self.action)),
            ("outcome".to_owned(), json!(self.outcome)),
        ]);
        if let Some(revision) = self.revision {
            result.insert("revision".to_owned(), json!(revision));
        }
        if let Some(revision) = self.accepted_revision {
            result.insert("acceptedRevision".to_owned(), json!(revision));
        }
        if let Some(close_request_id) = &self.close_request_id {
            result.insert("closeRequestId".to_owned(), json!(close_request_id));
        }
        for (key, id) in &self.ids {
            result.insert((*key).to_owned(), json!(id));
        }
        if let Some(direction) = self.direction {
            result.insert("direction".to_owned(), json!(direction));
        }
        if let Some(label) = &self.label {
            result.insert("label".to_owned(), json!(label));
        }
        if let Some(path) = &self.path {
            result.insert("path".to_owned(), json!(path));
        }
        if let Some(terminal) = &self.terminal {
            result.insert("terminal".to_owned(), terminal.document());
        }
        json!({
            "schema": MUTATION_RESULT_ID,
            "version": 1,
            "source": self.source(),
            "result": Value::Object(result),
        })
    }

    fn revision(mut self, revision: WorkbenchRevision) -> Self {
        self.revision = Some(revision);
        self
    }

    fn accepted(mut self, close_request_id: &str, revision: WorkbenchRevision) -> Self {
        self.accepted_revision = Some(revision);
        self.close_request_id = Some(close_request_id.to_owned());
        self
    }

    fn id(mut self, key: &'static str, id: &str) -> Self {
        self.ids.push((key, id.to_owned()));
        self
    }

    fn direction(mut self, direction: &'static str) -> Self {
        self.direction = Some(direction);
        self
    }

    fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_owned());
        self
    }

    /// The literal path the request carried — echoed verbatim, never
    /// canonicalized, resolved, or probed by the CLI.
    fn path(mut self, path: &str) -> Self {
        self.path = Some(path.to_owned());
        self
    }

    fn terminal(mut self, terminal: TerminalFact) -> Self {
        self.terminal = Some(terminal);
        self
    }

    /// Ordered `(JSON name, text value)` rows for the text rendering; the
    /// text output names the same fields as the JSON envelope.
    pub(super) fn text_fields(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![
            ("action", self.action.to_owned()),
            ("outcome", self.outcome.to_owned()),
        ];
        if let Some(revision) = self.revision {
            rows.push(("revision", revision.to_string()));
        }
        if let Some(revision) = self.accepted_revision {
            rows.push(("acceptedRevision", revision.to_string()));
        }
        rows.push(("requestId", self.request_id.clone()));
        if let Some(close_request_id) = &self.close_request_id {
            rows.push(("closeRequestId", close_request_id.clone()));
        }
        for (key, id) in &self.ids {
            rows.push((key, id.clone()));
        }
        if let Some(direction) = self.direction {
            rows.push(("direction", direction.to_owned()));
        }
        if let Some(terminal) = &self.terminal {
            rows.push(("terminal", terminal.text()));
        }
        if let Some(label) = &self.label {
            rows.push(("label", label.clone()));
        }
        if let Some(path) = &self.path {
            rows.push(("path", path.clone()));
        }
        rows
    }
}

/// The terminal half of a create/split answer: pending while the Server
/// still owes a spawn, or the typed spawn outcome it reported.
pub(super) struct TerminalFact {
    outcome: &'static str,
    reason: Option<&'static str>,
    geometry: Option<WorkbenchTerminalGeometry>,
}

impl TerminalFact {
    fn document(&self) -> Value {
        let mut object = Map::from_iter([("outcome".to_owned(), json!(self.outcome))]);
        if let Some(reason) = self.reason {
            object.insert("reason".to_owned(), json!(reason));
        }
        if let Some(geometry) = self.geometry {
            object.insert(
                "geometry".to_owned(),
                json!({
                    "columns": geometry.columns(),
                    "rows": geometry.rows(),
                    "cellPixelWidth": geometry.cell_pixel_width(),
                    "cellPixelHeight": geometry.cell_pixel_height(),
                }),
            );
        }
        Value::Object(object)
    }

    /// Compact single-line form for the receipt's `terminal` field.
    fn text(&self) -> String {
        match (self.outcome, self.reason, self.geometry) {
            ("started", _, Some(geometry)) => format!(
                "started ({}x{}, {}x{}px)",
                geometry.columns(),
                geometry.rows(),
                geometry.cell_pixel_width(),
                geometry.cell_pixel_height()
            ),
            (outcome, Some(reason), _) => format!("{outcome} ({reason})"),
            (outcome, ..) => outcome.to_owned(),
        }
    }
}

fn command_source(site_id: &str, request_id: &str, base: WorkbenchRevision) -> Value {
    json!({
        "kind": "workbench-command",
        "siteId": site_id,
        "requestId": request_id,
        "expectedRevision": base,
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn run(command: &MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    linux::run(command, format, locale)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn run(command: &MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    let _ = command;
    failure_output(
        Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("platform", "unsupported-platform")),
        Some(format),
        locale,
    )
}

#[cfg(target_os = "linux")]
mod linux {
    use std::time::Instant;

    use orchbee_workbench_protocol::{
        PanelNode, Project, ProtocolVersion, SiteId, Snapshot, Tab, TerminalContext,
        WorkbenchAction, WorkbenchCommand, WorkbenchCommandUnavailableReason, WorkbenchPanelId,
        WorkbenchPanelSplitDirection, WorkbenchRejectionReason, WorkbenchRequestId,
        WorkbenchRevision, WorkbenchTabMoveDirection, WorkbenchTerminalFailureReason,
        WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason, WorkbenchWireError,
        Workspace,
    };
    use serde_json::{Map, json};

    use super::super::query::{request_id, snapshot_session};
    use super::super::{
        Failure, FailureKind, Output, failure_output, output, present, results, stage_details,
        stage_reason_details,
    };
    use super::{Receipt, TerminalFact, command_source};
    use crate::invocation::{MutationCommand, OutputFormat, Resource, TabContext, Target};
    use crate::local_client::LocalClient;
    use crate::local_client::workbench::panel_close::{
        WorkbenchPanelCloseAccepted, WorkbenchPanelCloseFailure, WorkbenchPanelCloseNotSubmitted,
        WorkbenchPanelClosePrecommit, WorkbenchPanelCloseRequest, WorkbenchPanelCloseSubmission,
    };
    use crate::local_client::workbench::panel_move::{
        WorkbenchPanelMoveCommit, WorkbenchPanelMoveNotSubmitted,
        WorkbenchPanelMoveOutcomeUnknownReason, WorkbenchPanelMovePrecommit,
        WorkbenchPanelMoveRequest, WorkbenchPanelMoveSubmission,
    };
    use crate::local_client::workbench::panel_split::{
        WorkbenchPanelSplitCommit, WorkbenchPanelSplitNotSubmitted,
        WorkbenchPanelSplitOutcomeUnknownReason, WorkbenchPanelSplitPrecommit,
        WorkbenchPanelSplitRequest, WorkbenchPanelSplitSubmission,
    };
    use crate::local_client::workbench::tab_rename::{
        WorkbenchTabRenameNotSubmitted, WorkbenchTabRenameOutcomeUnknownReason,
        WorkbenchTabRenamePrecommit, WorkbenchTabRenameRequest, WorkbenchTabRenameSubmission,
    };
    use crate::local_client::workbench::workspace_setup::{
        WorkbenchSetupCommit, WorkbenchSetupNotSubmitted, WorkbenchSetupOutcomeUnknownReason,
        WorkbenchSetupPrecommit, WorkbenchSetupRequest, WorkbenchSetupSubmission,
    };
    use crate::local_client::workbench::{
        WorkbenchTabCloseAccepted, WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted,
        WorkbenchTabClosePrecommit, WorkbenchTabCloseRequest, WorkbenchTabCloseSubmission,
        WorkbenchTabCreateCommit, WorkbenchTabCreateNotSubmitted,
        WorkbenchTabCreateOutcomeUnknownReason, WorkbenchTabCreatePrecommit,
        WorkbenchTabCreateRequest, WorkbenchTabCreateSubmission, WorkbenchTabMoveCommit,
        WorkbenchTabMoveNotSubmitted, WorkbenchTabMoveOutcomeUnknownReason,
        WorkbenchTabMovePrecommit, WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
    };
    use crate::localization::Locale;

    /// The resolved command base: site and expected revision come from the
    /// just-read snapshot, the action carries every resolved identity, and the
    /// request identity is fresh.
    struct Resolved {
        site_id: SiteId,
        expected_revision: WorkbenchRevision,
        request_id: WorkbenchRequestId,
        action: WorkbenchAction,
    }

    /// Every family's "definitely not submitted" reason collapses into the
    /// same three-way shape before it becomes a CLI failure.
    enum NotSubmitted {
        SessionUnavailable,
        RequestEncoding,
        Request,
    }

    /// Every family's definite pre-commit outcome collapses the same way.
    enum Precommit {
        Rejected {
            revision: WorkbenchRevision,
            reason: WorkbenchRejectionReason,
        },
        Denied,
        Unavailable {
            reason: WorkbenchCommandUnavailableReason,
        },
    }

    /// Every family's outcome-unknown reason classifies into the same reason
    /// token set: what broke decides the token, never which action ran.
    trait UnknownReason {
        fn reason(&self) -> &'static str;
    }

    /// The nine command families share one structural submission contract, so
    /// the per-family reason enums collapse into the shared shapes once.
    macro_rules! submissions {
        ($(($not_submitted:ident, $precommit:ident, $unknown:ident);)+) => {$(
            impl From<$not_submitted> for NotSubmitted {
                fn from(reason: $not_submitted) -> Self {
                    match reason {
                        $not_submitted::SessionUnavailable => Self::SessionUnavailable,
                        $not_submitted::RequestEncoding(_) => Self::RequestEncoding,
                        $not_submitted::RequestNotSubmitted(_) => Self::Request,
                    }
                }
            }
            impl From<$precommit> for Precommit {
                fn from(precommit: $precommit) -> Self {
                    match precommit {
                        $precommit::Rejected { revision, reason } => {
                            Self::Rejected { revision, reason }
                        }
                        $precommit::Denied => Self::Denied,
                        $precommit::Unavailable { reason } => Self::Unavailable { reason },
                    }
                }
            }
            impl UnknownReason for $unknown {
                /// Transport ambiguity and peer-side validation failures both
                /// leave the effect undetermined; the token only says why the
                /// definite answer never arrived.
                fn reason(&self) -> &'static str {
                    match self {
                        Self::Write(_) => "write-failure",
                        Self::Response(WorkbenchWireError::Io(_))
                        | Self::ResponseDeadline(_) => "response-loss",
                        Self::Response(_) | Self::UnexpectedResponse => "response-invalid",
                        Self::CorrelationMismatch { .. } => "correlation-mismatch",
                        Self::ActionIncompatible => "action-incompatible",
                        _ => "validation-failed",
                    }
                }
            }
        )+};
    }

    submissions! {
        (WorkbenchSetupNotSubmitted, WorkbenchSetupPrecommit, WorkbenchSetupOutcomeUnknownReason);
        (WorkbenchTabCreateNotSubmitted, WorkbenchTabCreatePrecommit, WorkbenchTabCreateOutcomeUnknownReason);
        (WorkbenchTabRenameNotSubmitted, WorkbenchTabRenamePrecommit, WorkbenchTabRenameOutcomeUnknownReason);
        (WorkbenchTabMoveNotSubmitted, WorkbenchTabMovePrecommit, WorkbenchTabMoveOutcomeUnknownReason);
        (WorkbenchTabCloseNotSubmitted, WorkbenchTabClosePrecommit, WorkbenchTabCloseFailure);
        (WorkbenchPanelSplitNotSubmitted, WorkbenchPanelSplitPrecommit, WorkbenchPanelSplitOutcomeUnknownReason);
        (WorkbenchPanelCloseNotSubmitted, WorkbenchPanelClosePrecommit, WorkbenchPanelCloseFailure);
        (WorkbenchPanelMoveNotSubmitted, WorkbenchPanelMovePrecommit, WorkbenchPanelMoveOutcomeUnknownReason);
    }

    pub(super) fn run(command: &MutationCommand, format: OutputFormat, locale: Locale) -> Output {
        match execute(command) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok(receipt) => success_output(&receipt, format, locale),
        }
    }

    fn execute(command: &MutationCommand) -> Result<Receipt, Failure> {
        // One admitted session plus one bounded snapshot read; a read failure
        // already dropped the session inside `snapshot_session`.
        let (mut client, deadline, snapshot) = snapshot_session()?;
        let resolved = resolve(command, &snapshot)
            .map_err(|failure| failure.source(results::snapshot_source(&snapshot)))?;
        let source = command_source(
            resolved.site_id.as_str(),
            resolved.request_id.as_str(),
            resolved.expected_revision,
        );
        let submission = submit(&mut client, deadline, resolved);
        // The Client Session is fully dropped before the outcome is mapped or
        // any output is prepared; an ambiguous submission already retired the
        // connection inside the transport, and nothing is replayed.
        drop(client);
        submission.map_err(|failure| failure.source(source))
    }

    fn resolve(command: &MutationCommand, snapshot: &Snapshot) -> Result<Resolved, Failure> {
        let action = action(command, snapshot)?;
        Ok(Resolved {
            site_id: snapshot.site_id.clone(),
            expected_revision: snapshot.revision,
            request_id: request_id()?,
            action,
        })
    }

    /// Translate one parsed command into its wire action, resolving every
    /// target against the validated snapshot with the query resolver's exact
    /// semantics.
    fn action(command: &MutationCommand, snapshot: &Snapshot) -> Result<WorkbenchAction, Failure> {
        match command {
            MutationCommand::ProjectCreate { label } => Ok(WorkbenchAction::ProjectCreate {
                label: label.clone(),
            }),
            MutationCommand::WorkspaceCreate { project, label } => {
                let project = resolve_project(snapshot, project)?;
                Ok(WorkbenchAction::WorkspaceCreate {
                    project_id: project.project_id.clone(),
                    label: label.clone(),
                })
            }
            MutationCommand::WorkspaceRename { target, label } => {
                let (project, workspace) = resolve_workspace(snapshot, target)?;
                Ok(WorkbenchAction::WorkspaceRename {
                    project_id: project.project_id.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                    label: label.clone(),
                })
            }
            MutationCommand::WorkspaceAcceptDirectory { target, path } => {
                let (project, workspace) = resolve_workspace(snapshot, target)?;
                Ok(WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    project_id: project.project_id.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                    path: path.clone(),
                })
            }
            MutationCommand::TabCreate {
                context,
                label,
                geometry,
            } => {
                let terminal_context_id = match context {
                    TabContext::Standalone => {
                        snapshot.navigation.standalone_terminal_context_id.clone()
                    }
                    TabContext::Workspace(target) => resolve_workspace(snapshot, target)?
                        .1
                        .terminal_context_id
                        .clone(),
                };
                Ok(WorkbenchAction::TabCreate {
                    terminal_context_id,
                    label: label.clone(),
                    geometry: *geometry,
                })
            }
            MutationCommand::TabRename { target, label } => {
                let (context, tab) = resolve_tab(snapshot, target)?;
                Ok(WorkbenchAction::TabRename {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    label: label.clone(),
                })
            }
            MutationCommand::TabMove { target, direction } => {
                let (context, tab) = resolve_tab(snapshot, target)?;
                Ok(WorkbenchAction::TabMove {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    direction: *direction,
                })
            }
            MutationCommand::TabClose { target } => {
                let (context, tab) = resolve_tab(snapshot, target)?;
                Ok(WorkbenchAction::TabClose {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                })
            }
            MutationCommand::PanelSplit {
                target,
                direction,
                label,
                geometry,
            } => {
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                Ok(WorkbenchAction::PanelSplit {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id,
                    direction: *direction,
                    label: label.clone(),
                    geometry: *geometry,
                })
            }
            MutationCommand::PanelClose { target } => {
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                Ok(WorkbenchAction::PanelClose {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id,
                })
            }
            MutationCommand::PanelMove {
                target,
                into,
                direction,
            } => {
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                // The wire shape binds both panels into one tab, so the target
                // panel resolves inside the source panel's own tab.
                let target_panel_id = resolve_panel_in(tab, into)?;
                Ok(WorkbenchAction::PanelMove {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id,
                    target_panel_id,
                    direction: *direction,
                })
            }
            // `terminal send` never becomes a Workbench action; `cli::mutation`
            // routes it to the send executor before this resolver runs.
            MutationCommand::TerminalSend { .. } => Err(Failure::new(FailureKind::InternalFailure)),
        }
    }

    fn resolve_project<'a>(
        snapshot: &'a Snapshot,
        target: &Target,
    ) -> Result<&'a Project, Failure> {
        results::resolve_or_fail(
            snapshot
                .navigation
                .projects
                .iter()
                .map(|project| (project.project_id.as_str(), project.label.as_str(), project)),
            target,
            Resource::Project,
        )
    }

    fn resolve_workspace<'a>(
        snapshot: &'a Snapshot,
        target: &Target,
    ) -> Result<(&'a Project, &'a Workspace), Failure> {
        results::resolve_or_fail(
            snapshot.navigation.projects.iter().flat_map(|project| {
                project.workspaces.iter().map(move |workspace| {
                    (
                        workspace.workspace_id.as_str(),
                        workspace.label.as_str(),
                        (project, workspace),
                    )
                })
            }),
            target,
            Resource::Workspace,
        )
    }

    fn resolve_tab<'a>(
        snapshot: &'a Snapshot,
        target: &Target,
    ) -> Result<(&'a TerminalContext, &'a Tab), Failure> {
        results::resolve_or_fail(
            snapshot.terminal_contexts.iter().flat_map(|context| {
                context
                    .tabs
                    .iter()
                    .map(move |tab| (tab.tab_id.as_str(), tab.label.as_str(), (context, tab)))
            }),
            target,
            Resource::Tab,
        )
    }

    /// Panels have no label; only complete typed IDs resolve, exactly like
    /// `panel get`.
    fn resolve_panel<'a>(
        snapshot: &'a Snapshot,
        target: &Target,
    ) -> Result<(&'a TerminalContext, &'a Tab, WorkbenchPanelId), Failure> {
        // The grammar admits only typed IDs for `Resource::Panel` targets, so
        // a non-`Id` target cannot arrive here; keep the arm defensive anyway.
        let Target::Id(identity) = target else {
            return Err(Failure::new(FailureKind::InternalFailure));
        };
        for context in &snapshot.terminal_contexts {
            for tab in &context.tabs {
                if let Some(panel_id) = panel_id_in(&tab.root, identity) {
                    return Ok((context, tab, panel_id.clone()));
                }
            }
        }
        Err(results::not_found(Resource::Panel, target))
    }

    fn resolve_panel_in(tab: &Tab, target: &Target) -> Result<WorkbenchPanelId, Failure> {
        // Same typed-ID-only invariant as `resolve_panel`: unreachable unless
        // the parser's panel target rule ever widens.
        let Target::Id(identity) = target else {
            return Err(Failure::new(FailureKind::InternalFailure));
        };
        panel_id_in(&tab.root, identity)
            .cloned()
            .ok_or_else(|| results::not_found(Resource::Panel, target))
    }

    fn panel_id_in<'a>(node: &'a PanelNode, identity: &str) -> Option<&'a WorkbenchPanelId> {
        match node {
            PanelNode::Panel { panel_id, .. } if panel_id.as_str() == identity => Some(panel_id),
            PanelNode::Panel { .. } => None,
            PanelNode::Split { children, .. } => children
                .iter()
                .find_map(|slot| panel_id_in(&slot.node, identity)),
        }
    }

    /// Submit exactly one command on the admitted session and map the typed
    /// submission into the receipt/failure matrix.
    fn submit(
        client: &mut LocalClient,
        deadline: Instant,
        resolved: Resolved,
    ) -> Result<Receipt, Failure> {
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: resolved.request_id.clone(),
            site_id: resolved.site_id.clone(),
            expected_revision: resolved.expected_revision,
            action: resolved.action.clone(),
        };
        let internal = || Failure::new(FailureKind::InternalFailure);
        match &resolved.action {
            WorkbenchAction::ProjectCreate { .. }
            | WorkbenchAction::WorkspaceCreate { .. }
            | WorkbenchAction::WorkspaceRename { .. }
            | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. } => {
                let request = WorkbenchSetupRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_setup(request, deadline) {
                    WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::ProjectCreated {
                        revision,
                        project_id,
                    }) if matches!(&resolved.action, WorkbenchAction::ProjectCreate { .. }) => {
                        Ok(receipt(&resolved, "project.create", "committed")
                            .revision(revision)
                            .id("projectId", project_id.as_str())
                            .label(resolved_label(&resolved.action)))
                    }
                    WorkbenchSetupSubmission::Committed(
                        WorkbenchSetupCommit::WorkspaceCreated {
                            revision,
                            workspace_id,
                            terminal_context_id,
                        },
                    ) if matches!(&resolved.action, WorkbenchAction::WorkspaceCreate { .. }) => {
                        Ok(receipt(&resolved, "workspace.create", "committed")
                            .revision(revision)
                            .id("projectId", resolved_project_id(&resolved.action))
                            .id("workspaceId", workspace_id.as_str())
                            .id("terminalContextId", terminal_context_id.as_str())
                            .label(resolved_label(&resolved.action)))
                    }
                    WorkbenchSetupSubmission::Committed(
                        WorkbenchSetupCommit::WorkspaceRenamed {
                            revision,
                            project_id,
                            workspace_id,
                            unchanged,
                        },
                    ) if matches!(&resolved.action, WorkbenchAction::WorkspaceRename { .. }) => {
                        Ok(receipt(
                            &resolved,
                            "workspace.rename",
                            if unchanged { "unchanged" } else { "committed" },
                        )
                        .revision(revision)
                        .id("projectId", project_id.as_str())
                        .id("workspaceId", workspace_id.as_str())
                        .label(resolved_label(&resolved.action)))
                    }
                    WorkbenchSetupSubmission::Committed(
                        WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
                            revision,
                            directory_id,
                            worker_id,
                        },
                    ) if matches!(
                        &resolved.action,
                        WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
                    ) =>
                    {
                        Ok(
                            receipt(&resolved, "workspace.accept-directory", "committed")
                                .revision(revision)
                                .id("projectId", resolved_project_id(&resolved.action))
                                .id("workspaceId", resolved_workspace_id(&resolved.action))
                                .id("directoryId", directory_id.as_str())
                                .id("workerId", worker_id.as_str())
                                .path(resolved_path(&resolved.action)),
                        )
                    }
                    // Other setup commits are action-incompatible here; the
                    // transport already classifies them as unknown.
                    WorkbenchSetupSubmission::Committed(_) => Err(internal()),
                    WorkbenchSetupSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchSetupSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabCreate { .. } => {
                let request =
                    WorkbenchTabCreateRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_tab_create(request, deadline) {
                    WorkbenchTabCreateSubmission::Committed(commit) => {
                        let (
                            revision,
                            terminal_context_id,
                            tab_id,
                            panel_id,
                            presentation_id,
                            terminal,
                        ) = match commit {
                            WorkbenchTabCreateCommit::Pending {
                                revision,
                                terminal_context_id,
                                tab_id,
                                panel_id,
                                presentation_id,
                            } => (
                                revision,
                                terminal_context_id,
                                tab_id,
                                panel_id,
                                presentation_id,
                                TerminalFact {
                                    outcome: "pending",
                                    reason: None,
                                    geometry: None,
                                },
                            ),
                            WorkbenchTabCreateCommit::Created {
                                revision,
                                terminal_context_id,
                                tab_id,
                                panel_id,
                                presentation_id,
                                terminal_outcome,
                            } => (
                                revision,
                                terminal_context_id,
                                tab_id,
                                panel_id,
                                presentation_id,
                                terminal_fact(&terminal_outcome),
                            ),
                        };
                        Ok(receipt(&resolved, "tab.create", "committed")
                            .revision(revision)
                            .id("terminalContextId", terminal_context_id.as_str())
                            .id("tabId", tab_id.as_str())
                            .id("panelId", panel_id.as_str())
                            .id("presentationId", presentation_id.as_str())
                            .label(resolved_label(&resolved.action))
                            .terminal(terminal))
                    }
                    WorkbenchTabCreateSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchTabCreateSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabRename {
                terminal_context_id,
                tab_id,
                label,
            } => {
                let request =
                    WorkbenchTabRenameRequest::try_from(command).map_err(|_| internal())?;
                let base = receipt(&resolved, "tab.rename", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .label(label.as_str());
                match client.submit_workbench_tab_rename(request, deadline) {
                    WorkbenchTabRenameSubmission::Committed { revision } => {
                        Ok(base.revision(revision))
                    }
                    WorkbenchTabRenameSubmission::Unchanged { revision } => Ok(Receipt {
                        outcome: "unchanged",
                        ..base
                    }
                    .revision(revision)),
                    WorkbenchTabRenameSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchTabRenameSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchTabRenameSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabMove {
                terminal_context_id,
                tab_id,
                direction,
            } => {
                let request = WorkbenchTabMoveRequest::try_from(command).map_err(|_| internal())?;
                let base = receipt(&resolved, "tab.move", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .direction(tab_direction(*direction));
                match client.submit_workbench_tab_move(request, deadline) {
                    WorkbenchTabMoveSubmission::Committed(WorkbenchTabMoveCommit {
                        revision,
                        ..
                    }) => Ok(base.revision(revision)),
                    WorkbenchTabMoveSubmission::Unchanged { revision } => Ok(Receipt {
                        outcome: "unchanged",
                        ..base
                    }
                    .revision(revision)),
                    WorkbenchTabMoveSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchTabMoveSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabClose { .. } => {
                let request =
                    WorkbenchTabCloseRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_tab_close(request, deadline) {
                    WorkbenchTabCloseSubmission::Accepted(WorkbenchTabCloseAccepted {
                        close_request_id,
                        terminal_context_id,
                        tab_id,
                        accepted_revision,
                    }) => Ok(receipt(&resolved, "tab.close", "accepted")
                        .accepted(close_request_id.as_str(), accepted_revision)
                        .id("terminalContextId", terminal_context_id.as_str())
                        .id("tabId", tab_id.as_str())),
                    WorkbenchTabCloseSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchTabCloseSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchTabCloseSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::PanelSplit {
                terminal_context_id,
                tab_id,
                panel_id,
                direction,
                label,
                ..
            } => {
                let request =
                    WorkbenchPanelSplitRequest::try_from(command).map_err(|_| internal())?;
                let base = receipt(&resolved, "panel.split", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .id("panelId", panel_id.as_str())
                    .direction(panel_direction(*direction))
                    .label(label.as_str());
                match client.submit_workbench_panel_split(request, deadline) {
                    WorkbenchPanelSplitSubmission::Committed(commit) => {
                        let (
                            revision,
                            new_panel_id,
                            split_id,
                            first_slot_id,
                            second_slot_id,
                            presentation_id,
                            terminal,
                        ) = match commit {
                            WorkbenchPanelSplitCommit::Pending {
                                revision,
                                new_panel_id,
                                split_id,
                                first_slot_id,
                                second_slot_id,
                                presentation_id,
                                ..
                            } => (
                                revision,
                                new_panel_id,
                                split_id,
                                first_slot_id,
                                second_slot_id,
                                presentation_id,
                                TerminalFact {
                                    outcome: "pending",
                                    reason: None,
                                    geometry: None,
                                },
                            ),
                            WorkbenchPanelSplitCommit::Created {
                                revision,
                                new_panel_id,
                                split_id,
                                first_slot_id,
                                second_slot_id,
                                presentation_id,
                                terminal_outcome,
                                ..
                            } => (
                                revision,
                                new_panel_id,
                                split_id,
                                first_slot_id,
                                second_slot_id,
                                presentation_id,
                                terminal_fact(&terminal_outcome),
                            ),
                        };
                        Ok(base
                            .revision(revision)
                            .id("newPanelId", new_panel_id.as_str())
                            .id("splitId", split_id.as_str())
                            .id("firstSlotId", first_slot_id.as_str())
                            .id("secondSlotId", second_slot_id.as_str())
                            .id("presentationId", presentation_id.as_str())
                            .terminal(terminal))
                    }
                    WorkbenchPanelSplitSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelSplitSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::PanelClose { .. } => {
                let request =
                    WorkbenchPanelCloseRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_panel_close(request, deadline) {
                    WorkbenchPanelCloseSubmission::Accepted(WorkbenchPanelCloseAccepted {
                        close_request_id,
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        accepted_revision,
                    }) => Ok(receipt(&resolved, "panel.close", "accepted")
                        .accepted(close_request_id.as_str(), accepted_revision)
                        .id("terminalContextId", terminal_context_id.as_str())
                        .id("tabId", tab_id.as_str())
                        .id("panelId", panel_id.as_str())),
                    WorkbenchPanelCloseSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchPanelCloseSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelCloseSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::PanelMove {
                terminal_context_id,
                tab_id,
                panel_id,
                target_panel_id,
                direction,
            } => {
                let request =
                    WorkbenchPanelMoveRequest::try_from(command).map_err(|_| internal())?;
                let base = receipt(&resolved, "panel.move", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .id("panelId", panel_id.as_str())
                    .id("targetPanelId", target_panel_id.as_str())
                    .direction(panel_direction(*direction));
                match client.submit_workbench_panel_move(request, deadline) {
                    WorkbenchPanelMoveSubmission::Committed(WorkbenchPanelMoveCommit {
                        revision,
                        ..
                    }) => Ok(base.revision(revision)),
                    WorkbenchPanelMoveSubmission::Unchanged { revision } => Ok(Receipt {
                        outcome: "unchanged",
                        ..base
                    }
                    .revision(revision)),
                    WorkbenchPanelMoveSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into()))
                    }
                    WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelMoveSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            // The grammar only produces the nine action variants above.
            _ => Err(internal()),
        }
    }

    fn resolved_project_id(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::WorkspaceCreate { project_id, .. }
            | WorkbenchAction::WorkspacePrimaryDirectoryAccept { project_id, .. } => {
                project_id.as_str()
            }
            _ => "",
        }
    }

    fn resolved_workspace_id(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { workspace_id, .. } => {
                workspace_id.as_str()
            }
            _ => "",
        }
    }

    /// The literal `--path` value the request carried: echoed for correlation
    /// only, never canonicalized or probed by the CLI.
    fn resolved_path(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } => path.as_str(),
            _ => "",
        }
    }

    fn resolved_label(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::ProjectCreate { label, .. }
            | WorkbenchAction::WorkspaceCreate { label, .. }
            | WorkbenchAction::WorkspaceRename { label, .. }
            | WorkbenchAction::TabCreate { label, .. }
            | WorkbenchAction::TabRename { label, .. }
            | WorkbenchAction::PanelSplit { label, .. } => label.as_str(),
            _ => "",
        }
    }

    fn receipt(resolved: &Resolved, action: &'static str, outcome: &'static str) -> Receipt {
        Receipt {
            site_id: resolved.site_id.as_str().to_owned(),
            request_id: resolved.request_id.as_str().to_owned(),
            base_revision: resolved.expected_revision,
            action,
            outcome,
            revision: None,
            accepted_revision: None,
            close_request_id: None,
            ids: Vec::new(),
            direction: None,
            label: None,
            path: None,
            terminal: None,
        }
    }

    /// A command that provably never reached the wire: an encoding or session
    /// defect is internal, a transport refusal before handoff is a clean
    /// transport-loss with no applied effect.
    fn not_submitted_failure(reason: NotSubmitted) -> Failure {
        match reason {
            NotSubmitted::SessionUnavailable | NotSubmitted::RequestEncoding => {
                Failure::new(FailureKind::InternalFailure)
            }
            NotSubmitted::Request => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("command"))
            }
        }
    }

    /// A definite pre-commit answer the Server typed itself.
    fn precommit_failure(precommit: Precommit) -> Failure {
        match precommit {
            Precommit::Rejected { revision, reason } => Failure::new(FailureKind::Rejected)
                .details(Map::from_iter([
                    ("stage".to_owned(), json!("command")),
                    ("reason".to_owned(), json!(rejection_reason(reason))),
                    ("revision".to_owned(), json!(revision)),
                ])),
            Precommit::Denied => Failure::new(FailureKind::Denied),
            Precommit::Unavailable { reason } => Failure::new(FailureKind::ServiceUnavailable)
                .details(stage_reason_details("command", unavailable_reason(reason))),
        }
    }

    /// Any post-submission outcome the transport could not settle: the
    /// command's effect is undetermined, the session is already retired, and
    /// the request identity is reported so operators can investigate. The CLI
    /// never replays it.
    fn unknown_failure(request_id: WorkbenchRequestId, reason: &'static str) -> Failure {
        Failure::new(FailureKind::OutcomeUnknown).details(Map::from_iter([
            ("stage".to_owned(), json!("command")),
            ("requestId".to_owned(), json!(request_id.as_str())),
            ("reason".to_owned(), json!(reason)),
        ]))
    }

    fn rejection_reason(reason: WorkbenchRejectionReason) -> &'static str {
        match reason {
            WorkbenchRejectionReason::SiteMismatch => "site-mismatch",
            WorkbenchRejectionReason::RevisionMismatch => "revision-mismatch",
            WorkbenchRejectionReason::ProjectUnavailable => "project-unavailable",
            WorkbenchRejectionReason::WorkspaceUnavailable => "workspace-unavailable",
            WorkbenchRejectionReason::WorkspaceDirectoryUnavailable => {
                "workspace-directory-unavailable"
            }
            WorkbenchRejectionReason::WorkspaceDirectoryConflict => "workspace-directory-conflict",
            WorkbenchRejectionReason::TerminalContextUnavailable => "terminal-context-unavailable",
            WorkbenchRejectionReason::TabUnavailable => "tab-unavailable",
            WorkbenchRejectionReason::PanelUnavailable => "panel-unavailable",
            WorkbenchRejectionReason::ResourceLimit => "resource-limit",
            WorkbenchRejectionReason::IdentityExhausted => "identity-exhausted",
            WorkbenchRejectionReason::RevisionExhausted => "revision-exhausted",
            WorkbenchRejectionReason::RequestIdentityConflict => "request-identity-conflict",
        }
    }

    fn unavailable_reason(reason: WorkbenchCommandUnavailableReason) -> &'static str {
        match reason {
            WorkbenchCommandUnavailableReason::RequestLedgerFull => "request-ledger-full",
            WorkbenchCommandUnavailableReason::NoCompatibleWorker => "no-compatible-worker",
            WorkbenchCommandUnavailableReason::TerminalBusy => "terminal-busy",
            WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable => {
                "terminal-runtime-unavailable"
            }
            WorkbenchCommandUnavailableReason::ServerBusy => "server-busy",
            WorkbenchCommandUnavailableReason::ServiceUnavailable => "service-unavailable",
            WorkbenchCommandUnavailableReason::DirectoryUnavailable => "directory-unavailable",
        }
    }

    fn tab_direction(direction: WorkbenchTabMoveDirection) -> &'static str {
        match direction {
            WorkbenchTabMoveDirection::Previous => "previous",
            WorkbenchTabMoveDirection::Next => "next",
            WorkbenchTabMoveDirection::First => "first",
            WorkbenchTabMoveDirection::Last => "last",
        }
    }

    fn panel_direction(direction: WorkbenchPanelSplitDirection) -> &'static str {
        match direction {
            WorkbenchPanelSplitDirection::Above => "above",
            WorkbenchPanelSplitDirection::Below => "below",
            WorkbenchPanelSplitDirection::Left => "left",
            WorkbenchPanelSplitDirection::Right => "right",
        }
    }

    fn terminal_fact(outcome: &WorkbenchTerminalOutcome) -> TerminalFact {
        match outcome {
            WorkbenchTerminalOutcome::Started { geometry } => TerminalFact {
                outcome: "started",
                reason: None,
                geometry: Some(*geometry),
            },
            WorkbenchTerminalOutcome::Unavailable { reason } => TerminalFact {
                outcome: "unavailable",
                reason: Some(match reason {
                    WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable => {
                        "terminal-runtime-unavailable"
                    }
                    WorkbenchTerminalUnavailableReason::ServerBusy => "server-busy",
                }),
                geometry: None,
            },
            WorkbenchTerminalOutcome::Failed { reason } => TerminalFact {
                outcome: "failed",
                reason: Some(match reason {
                    WorkbenchTerminalFailureReason::TerminalCreateFailed => {
                        "terminal-create-failed"
                    }
                }),
                geometry: None,
            },
            WorkbenchTerminalOutcome::OutcomeUnknown {} => TerminalFact {
                outcome: "unknown",
                reason: None,
                geometry: None,
            },
        }
    }

    fn success_output(receipt: &Receipt, format: OutputFormat, locale: Locale) -> Output {
        let source = receipt.source();
        let too_large = || {
            failure_output(
                Failure::new(FailureKind::OutputTooLarge)
                    .details(Map::from_iter([(
                        "limitBytes".to_owned(),
                        json!(output::LIMIT),
                    )]))
                    .source(source),
                Some(format),
                locale,
            )
        };
        match format {
            OutputFormat::Json => match output::encode_document(&receipt.document()) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                },
                Err(output::TooLarge) => too_large(),
            },
            OutputFormat::Text => match output::render_text(|out| present::receipt(out, receipt)) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                },
                Err(output::TooLarge) => too_large(),
            },
        }
    }

    #[cfg(test)]
    mod tests {
        //! Table-driven witnesses for the mapping arms a scripted peer cannot
        //! stage deterministically: pre-handoff refusals, the per-variant
        //! unknown-reason tokens, and every terminal outcome arm.

        use std::io;

        use orchbee_workbench_protocol::{
            WorkbenchTabCloseAttemptGeneration, WorkbenchTabId, WorkbenchTerminalContextId,
            WorkbenchTerminalGeometry,
        };

        use super::*;

        fn io_error() -> io::Error {
            io::Error::new(io::ErrorKind::ConnectionReset, "scripted")
        }

        fn id32(value: u64) -> String {
            format!("{value:032x}")
        }

        fn request(value: u64) -> WorkbenchRequestId {
            WorkbenchRequestId::parse(&id32(value)).unwrap()
        }

        #[test]
        fn not_submitted_maps_only_request_to_transport_loss() {
            let cases = [
                (
                    NotSubmitted::SessionUnavailable,
                    FailureKind::InternalFailure,
                ),
                (NotSubmitted::RequestEncoding, FailureKind::InternalFailure),
                (NotSubmitted::Request, FailureKind::TransportLoss),
            ];
            for (reason, kind) in cases {
                let failure = not_submitted_failure(reason);
                assert_eq!(failure.kind, kind);
                match kind {
                    FailureKind::TransportLoss => {
                        assert_eq!(failure.details.as_ref().unwrap()["stage"], json!("command"));
                    }
                    _ => assert!(failure.details.is_none()),
                }
            }
        }

        #[test]
        fn unknown_failure_carries_stage_request_and_reason() {
            let request = request(0x77);
            let failure = unknown_failure(request.clone(), "response-loss");
            assert_eq!(failure.kind, FailureKind::OutcomeUnknown);
            let details = failure.details.unwrap();
            assert_eq!(details["stage"], json!("command"));
            assert_eq!(details["requestId"], json!(request.as_str()));
            assert_eq!(details["reason"], json!("response-loss"));
        }

        /// Every `reason()` arm on one full reason enum plus the close
        /// families' extra verdict-mismatch variants: the tokens are the same
        /// closed vocabulary the failure schema enumerates.
        #[test]
        fn unknown_reason_tokens_cover_every_classification_arm() {
            let cases: Vec<(WorkbenchTabMoveOutcomeUnknownReason, &str)> = vec![
                (
                    WorkbenchTabMoveOutcomeUnknownReason::Write(io_error()),
                    "write-failure",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::Response(WorkbenchWireError::Io(
                        io_error(),
                    )),
                    "response-loss",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::ResponseDeadline(io_error()),
                    "response-loss",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::Response(
                        WorkbenchWireError::InvalidHeader,
                    ),
                    "response-invalid",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::UnexpectedResponse,
                    "response-invalid",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::CorrelationMismatch {
                        expected: request(0x42),
                        received: request(0x99),
                    },
                    "correlation-mismatch",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::ActionIncompatible,
                    "action-incompatible",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::ExpectedRevisionOverflow {
                        expected: WorkbenchRevision::new(u64::MAX),
                    },
                    "validation-failed",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::RevisionMismatch {
                        expected: WorkbenchRevision::new(1),
                        received: WorkbenchRevision::new(2),
                    },
                    "validation-failed",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::TerminalContextMismatch {
                        expected: WorkbenchTerminalContextId::parse(&id32(1)).unwrap(),
                        received: WorkbenchTerminalContextId::parse(&id32(2)).unwrap(),
                    },
                    "validation-failed",
                ),
                (
                    WorkbenchTabMoveOutcomeUnknownReason::TabMismatch {
                        expected: WorkbenchTabId::parse(&id32(3)).unwrap(),
                        received: WorkbenchTabId::parse(&id32(4)).unwrap(),
                    },
                    "validation-failed",
                ),
            ];
            for (reason, expected) in cases {
                assert_eq!(reason.reason(), expected, "{reason}");
            }

            // The close families' extra verdict-mismatch variants all land in
            // the same catch-all bucket.
            let close_cases: Vec<(WorkbenchTabCloseFailure, &str)> = vec![
                (WorkbenchTabCloseFailure::Write(io_error()), "write-failure"),
                (
                    WorkbenchTabCloseFailure::SiteMismatch {
                        expected: SiteId::parse(&id32(5)).unwrap(),
                        received: SiteId::parse(&id32(6)).unwrap(),
                    },
                    "validation-failed",
                ),
                (
                    WorkbenchTabCloseFailure::OperationMismatch {
                        expected: request(0x11),
                        received: request(0x22),
                    },
                    "validation-failed",
                ),
                (
                    WorkbenchTabCloseFailure::AcceptedRevisionMismatch {
                        admitted: WorkbenchRevision::new(4),
                        received: WorkbenchRevision::new(5),
                    },
                    "validation-failed",
                ),
                (
                    WorkbenchTabCloseFailure::AttemptGenerationMismatch {
                        expected: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                        received: WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
                    },
                    "validation-failed",
                ),
            ];
            for (reason, expected) in close_cases {
                assert_eq!(reason.reason(), expected, "{reason}");
            }
        }

        #[test]
        fn terminal_fact_covers_every_outcome_arm() {
            let geometry = WorkbenchTerminalGeometry::new(80, 24, 9, 18).unwrap();
            let cases = vec![
                (
                    WorkbenchTerminalOutcome::Started { geometry },
                    ("started", None, true),
                ),
                (
                    WorkbenchTerminalOutcome::Unavailable {
                        reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
                    },
                    ("unavailable", Some("terminal-runtime-unavailable"), false),
                ),
                (
                    WorkbenchTerminalOutcome::Unavailable {
                        reason: WorkbenchTerminalUnavailableReason::ServerBusy,
                    },
                    ("unavailable", Some("server-busy"), false),
                ),
                (
                    WorkbenchTerminalOutcome::Failed {
                        reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
                    },
                    ("failed", Some("terminal-create-failed"), false),
                ),
                (
                    WorkbenchTerminalOutcome::OutcomeUnknown {},
                    ("unknown", None, false),
                ),
            ];
            for (outcome, (expected_outcome, expected_reason, has_geometry)) in cases {
                let fact = terminal_fact(&outcome);
                assert_eq!(fact.outcome, expected_outcome, "{outcome:?}");
                assert_eq!(fact.reason, expected_reason, "{outcome:?}");
                assert_eq!(fact.geometry.is_some(), has_geometry, "{outcome:?}");
            }
        }
    }
}
