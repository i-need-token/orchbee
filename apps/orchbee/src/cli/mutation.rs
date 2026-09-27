//! One-effect Workbench mutation commands. Every invocation admits one
//! Client Session, performs one bounded snapshot read that fixes the Site
//! identity, the expected revision, and every resolved target identity, then
//! submits exactly one `workbench.command` under a fresh request identity.
//! Nothing is retried or replayed: an ambiguous post-submission outcome
//! retires the session and surfaces as a typed `outcome-unknown` failure.

use orchbee_workbench_protocol::{
    LaunchProfileCatalogRevision, WorkbenchRevision, WorkbenchTerminalGeometry,
};
use serde_json::{Map, Value, json};

use super::Output;
#[cfg(not(any(target_os = "linux", windows)))]
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
    /// `close-resume` answers under the resume request's own source echo —
    /// `None` keeps the default `workbench-command` source every command
    /// receipt already reports.
    source_override: Option<Value>,
    schema_id: &'static str,
    action: &'static str,
    outcome: &'static str,
    revision: Option<WorkbenchRevision>,
    accepted_revision: Option<WorkbenchRevision>,
    close_request_id: Option<String>,
    /// The attempt generation a `close-resume` receipt or stale answer
    /// carried — the resume lifecycle's own fence, never a Workbench
    /// revision.
    attempt_generation: Option<String>,
    ids: Vec<(&'static str, String)>,
    direction: Option<&'static str>,
    insert_index: Option<orchbee_workbench_protocol::WorkbenchTabInsertIndex>,
    workspace_insert_index: Option<orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex>,
    workspace_order: Option<orchbee_workbench_protocol::WorkbenchWorkspaceList>,
    workspace_sources: Option<orchbee_workbench_protocol::WorkbenchWorkspaceList>,
    panel_transfer: Option<orchbee_workbench_protocol::WorkbenchPanelTransfer>,
    label: Option<String>,
    custom_label: Option<orchbee_workbench_protocol::PanelLabelUpdate>,
    right_click: Option<orchbee_workbench_protocol::PanelRightClickTarget>,
    path: Option<String>,
    terminal: Option<TerminalFact>,
    /// The catalog-private revision a launch-profile command committed —
    /// sibling of `revision`, present only on profile create/delete.
    catalog_revision: Option<LaunchProfileCatalogRevision>,
    /// The profile name a `launch-profile create` stored — the catalog's
    /// `name`, distinct from a Workbench `label`.
    name: Option<String>,
}

impl Receipt {
    #[cfg(windows)]
    pub(super) fn close_request_id(&self) -> Option<&str> {
        self.close_request_id.as_deref()
    }
    #[cfg(windows)]
    pub(super) fn launch_failure(&self) -> Option<super::FailureKind> {
        match self.terminal.as_ref().map(|terminal| terminal.outcome) {
            Some("pending" | "started") => None,
            Some("outcome-unknown") => Some(super::FailureKind::OutcomeUnknown),
            Some("failed") => Some(super::FailureKind::Rejected),
            _ => Some(super::FailureKind::ServiceUnavailable),
        }
    }

    #[cfg(windows)]
    pub(super) fn committed_revision(&self) -> Option<WorkbenchRevision> {
        self.revision
    }

    #[cfg(windows)]
    pub(super) fn id_value(&self, name: &str) -> Option<&str> {
        self.ids
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    #[cfg(windows)]
    pub(super) fn transfer(&self) -> Option<&orchbee_workbench_protocol::WorkbenchPanelTransfer> {
        self.panel_transfer.as_ref()
    }

    fn source(&self) -> Value {
        self.source_override.clone().unwrap_or_else(|| {
            command_source(
                self.site_id.as_str(),
                self.request_id.as_str(),
                self.base_revision,
            )
        })
    }

    pub(super) fn document(&self) -> Value {
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
        if let Some(attempt_generation) = &self.attempt_generation {
            result.insert("attemptGeneration".to_owned(), json!(attempt_generation));
        }
        for (key, id) in &self.ids {
            result.insert((*key).to_owned(), json!(id));
        }
        if let Some(index) = self.insert_index {
            result.insert("insertIndex".to_owned(), json!(index));
        }
        if let Some(index) = self.workspace_insert_index {
            result.insert("insertIndex".to_owned(), json!(index));
        }
        if let Some(order) = &self.workspace_order {
            result.insert("workspaceOrder".to_owned(), json!(order));
        }
        if let Some(sources) = &self.workspace_sources {
            result.insert("workspaceIds".to_owned(), json!(sources));
        }
        if let Some(transfer) = &self.panel_transfer {
            result.insert("transfer".to_owned(), json!(transfer));
        }
        if let Some(direction) = self.direction {
            result.insert("direction".to_owned(), json!(direction));
        }
        if let Some(label) = &self.label {
            result.insert("label".to_owned(), json!(label));
        }
        if let Some(label) = &self.custom_label {
            result.insert("customLabel".to_owned(), json!(label));
        }
        if let Some(policy) = self.right_click {
            result.insert("rightClick".to_owned(), json!(policy));
        }
        if let Some(path) = &self.path {
            result.insert("path".to_owned(), json!(path));
        }
        if let Some(terminal) = &self.terminal {
            result.insert("terminal".to_owned(), terminal.document());
        }
        if let Some(catalog_revision) = self.catalog_revision {
            result.insert(
                "catalogRevision".to_owned(),
                json!(catalog_revision.to_string()),
            );
        }
        if let Some(name) = &self.name {
            result.insert("name".to_owned(), json!(name));
        }
        json!({
            "schema": self.schema_id,
            // Relocation v5 adds transfer facts. Other shared actions retain
            // v4, including those embedded in existing creation workflows.
            "version": if matches!(self.schema_id, super::schema::PROJECT_RENAME_RESULT_ID | super::schema::PANEL_RENAME_RESULT_ID | super::schema::PANEL_INPUT_RESULT_ID | super::schema::TAB_MOVE_RESULT_ID | super::schema::WORKSPACE_MOVE_RESULT_ID) {
                1
            } else if self.action == "panel.relocate" {
                5
            } else {
                4
            },
            "source": self.source(),
            "result": Value::Object(result),
        })
    }

    fn revision(mut self, revision: WorkbenchRevision) -> Self {
        self.revision = Some(revision);
        self
    }

    /// The dedicated machine document this receipt reports under — the shared
    /// mutation envelope by default; a command whose receipt was admitted as
    /// its own schema names that document instead.
    fn schema(mut self, schema_id: &'static str) -> Self {
        self.schema_id = schema_id;
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

    /// The catalog-private revision the launch-profile command committed.
    fn catalog_revision(mut self, catalog_revision: LaunchProfileCatalogRevision) -> Self {
        self.catalog_revision = Some(catalog_revision);
        self
    }

    /// The catalog name a `launch-profile create` stored — echoed verbatim.
    fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_owned());
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
        if let Some(attempt_generation) = &self.attempt_generation {
            rows.push(("attemptGeneration", attempt_generation.clone()));
        }
        for (key, id) in &self.ids {
            rows.push((key, id.clone()));
        }
        if let Some(index) = self.insert_index {
            rows.push(("insertIndex", index.value().to_string()));
        }
        if let Some(index) = self.workspace_insert_index {
            rows.push(("insertIndex", index.value().to_string()));
        }
        if let Some(order) = &self.workspace_order {
            rows.push((
                "workspaceOrder",
                order
                    .ids()
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
        }
        if let Some(sources) = &self.workspace_sources {
            rows.push((
                "workspaceIds",
                sources
                    .ids()
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
        }
        if let Some(transfer) = &self.panel_transfer {
            rows.push((
                "targetTerminalContextId",
                transfer.target_terminal_context_id.as_str().to_owned(),
            ));
            rows.push(("closedSourceTab", transfer.closed_source_tab.to_string()));
            if let Some(workspace) = &transfer.closed_source_workspace_id {
                rows.push(("closedSourceWorkspaceId", workspace.as_str().to_owned()));
            }
            if let Some(workspace) = &transfer.created_workspace_id {
                rows.push(("createdWorkspaceId", workspace.as_str().to_owned()));
            }
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
        if let Some(label) = &self.custom_label {
            rows.push((
                "customLabel",
                label
                    .custom_label()
                    .map_or_else(|| "null".to_owned(), |value| value.as_str().to_owned()),
            ));
        }
        if let Some(policy) = self.right_click {
            rows.push(("rightClick", policy.as_str().to_owned()));
        }
        if let Some(path) = &self.path {
            rows.push(("path", path.clone()));
        }
        if let Some(catalog_revision) = self.catalog_revision {
            rows.push(("catalogRevision", catalog_revision.to_string()));
        }
        if let Some(name) = &self.name {
            rows.push(("name", name.clone()));
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

#[cfg(any(target_os = "linux", windows))]
pub(crate) fn run(command: &MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    linux::run(command, format, locale)
}

#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) fn run(command: &MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    let _ = command;
    failure_output(
        Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("platform", "unsupported-platform")),
        Some(format),
        locale,
    )
}

#[cfg(windows)]
pub(super) use linux::{submit_action, submit_creation};

#[cfg(any(target_os = "linux", windows))]
mod linux {
    use std::time::Instant;

    use orchbee_workbench_protocol::{
        LaunchProfileId, Project, ProtocolVersion, SiteId, Snapshot, Tab, TerminalContext,
        WorkbenchAction, WorkbenchCommand, WorkbenchCommandUnavailableReason, WorkbenchPanelId,
        WorkbenchPanelRelocateDestination, WorkbenchPanelSplitDirection, WorkbenchProjectId,
        WorkbenchRejectionReason, WorkbenchRequestId, WorkbenchRevision,
        WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseResumeRejectionReason,
        WorkbenchTabCloseUnavailableReason, WorkbenchTabMoveDirection,
        WorkbenchTerminalFailureReason, WorkbenchTerminalOutcome,
        WorkbenchTerminalUnavailableReason, WorkbenchWireError, WorkbenchWorkspaceCloseResume,
        WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceId, Workspace,
    };
    use serde_json::{Map, json};

    use super::super::query::{request_id, snapshot_session};
    use super::super::schema::{
        CLOSE_FAILURE_ID, LAUNCH_PROFILE_FAILURE_ID, MUTATION_RESULT_ID, PROJECT_RENAME_RESULT_ID,
    };
    use super::super::{
        Failure, FailureKind, Output, failure_output, output, present, results, stage_details,
        stage_reason_details,
    };
    use super::{Receipt, TerminalFact, command_source};
    use crate::invocation::{
        MutationCommand, OutputFormat, PanelMoveDestination, Resource, TabContext, Target,
    };
    use crate::local_client::LocalClient;
    use crate::local_client::workbench::label_rename::{
        WorkbenchLabelRenameNotSubmitted, WorkbenchLabelRenameOutcomeUnknownReason,
        WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameRequest, WorkbenchLabelRenameSubmission,
    };
    use crate::local_client::workbench::launch_profile::{
        LaunchProfileMutationNotSubmitted, LaunchProfileOutcomeUnknownReason,
        WorkbenchLaunchProfileCreateRequest, WorkbenchLaunchProfileDeleteRequest,
        WorkbenchLaunchProfileMutationPrecommit, WorkbenchLaunchProfileMutationSubmission,
    };
    use crate::local_client::workbench::panel_close::{
        WorkbenchPanelCloseAccepted, WorkbenchPanelCloseFailure, WorkbenchPanelCloseNotSubmitted,
        WorkbenchPanelClosePrecommit, WorkbenchPanelCloseRequest, WorkbenchPanelCloseSubmission,
    };
    use crate::local_client::workbench::panel_input::{
        WorkbenchPanelInputNotSubmitted, WorkbenchPanelInputOutcomeUnknownReason,
        WorkbenchPanelInputPrecommit, WorkbenchPanelInputRequest, WorkbenchPanelInputSubmission,
    };
    use crate::local_client::workbench::panel_placement::{
        WorkbenchPanelPlacementCommit, WorkbenchPanelPlacementNotSubmitted,
        WorkbenchPanelPlacementOutcomeUnknownReason, WorkbenchPanelPlacementPrecommit,
        WorkbenchPanelPlacementRequest, WorkbenchPanelPlacementSubmission,
    };
    use crate::local_client::workbench::panel_relocate::{
        WorkbenchPanelRelocateCommit, WorkbenchPanelRelocateNotSubmitted,
        WorkbenchPanelRelocateOutcomeUnknownReason, WorkbenchPanelRelocatePrecommit,
        WorkbenchPanelRelocateRequest, WorkbenchPanelRelocateSubmission,
    };
    use crate::local_client::workbench::panel_split::{
        WorkbenchPanelSplitCommit, WorkbenchPanelSplitNotSubmitted,
        WorkbenchPanelSplitOutcomeUnknownReason, WorkbenchPanelSplitPrecommit,
        WorkbenchPanelSplitRequest, WorkbenchPanelSplitSubmission,
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
        WorkbenchWorkspaceCloseAccepted, WorkbenchWorkspaceCloseFailure,
        WorkbenchWorkspaceCloseNotSubmitted, WorkbenchWorkspaceClosePrecommit,
        WorkbenchWorkspaceCloseRequest, WorkbenchWorkspaceCloseResumeSubmission,
        WorkbenchWorkspaceCloseSubmission,
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

    /// Every family's definite pre-commit outcome collapses the same way;
    /// `tab create`/`panel split` additionally settle on the typed
    /// launch-profile resolution outcomes when `--profile` was selected.
    enum Precommit {
        Rejected {
            revision: WorkbenchRevision,
            reason: WorkbenchRejectionReason,
        },
        Denied,
        Unavailable {
            reason: WorkbenchCommandUnavailableReason,
        },
        ProfileUnavailable,
        ProfileAmbiguous {
            candidate_ids: Vec<LaunchProfileId>,
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

    /// The two profile-selecting families collapse identically plus the two
    /// typed profile-resolution settlements only their precommit enums carry.
    macro_rules! submissions_with_profile {
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
                        $precommit::ProfileUnavailable => Self::ProfileUnavailable,
                        $precommit::ProfileAmbiguous { candidate_ids } => {
                            Self::ProfileAmbiguous { candidate_ids }
                        }
                    }
                }
            }
            impl UnknownReason for $unknown {
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
        (WorkbenchPanelInputNotSubmitted, WorkbenchPanelInputPrecommit, WorkbenchPanelInputOutcomeUnknownReason);
        (WorkbenchSetupNotSubmitted, WorkbenchSetupPrecommit, WorkbenchSetupOutcomeUnknownReason);
        (LaunchProfileMutationNotSubmitted, WorkbenchLaunchProfileMutationPrecommit, LaunchProfileOutcomeUnknownReason);
        (WorkbenchLabelRenameNotSubmitted, WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameOutcomeUnknownReason);
        (WorkbenchTabMoveNotSubmitted, WorkbenchTabMovePrecommit, WorkbenchTabMoveOutcomeUnknownReason);
        (WorkbenchTabCloseNotSubmitted, WorkbenchTabClosePrecommit, WorkbenchTabCloseFailure);
        (WorkbenchPanelCloseNotSubmitted, WorkbenchPanelClosePrecommit, WorkbenchPanelCloseFailure);
        (WorkbenchPanelPlacementNotSubmitted, WorkbenchPanelPlacementPrecommit, WorkbenchPanelPlacementOutcomeUnknownReason);
        (WorkbenchPanelRelocateNotSubmitted, WorkbenchPanelRelocatePrecommit, WorkbenchPanelRelocateOutcomeUnknownReason);
        (WorkbenchWorkspaceCloseNotSubmitted, WorkbenchWorkspaceClosePrecommit, WorkbenchWorkspaceCloseFailure);
    }

    submissions_with_profile! {
        (WorkbenchTabCreateNotSubmitted, WorkbenchTabCreatePrecommit, WorkbenchTabCreateOutcomeUnknownReason);
        (WorkbenchPanelSplitNotSubmitted, WorkbenchPanelSplitPrecommit, WorkbenchPanelSplitOutcomeUnknownReason);
    }

    pub(super) fn run(command: &MutationCommand, format: OutputFormat, locale: Locale) -> Output {
        match execute(command) {
            Err(failure) => {
                let failure = if matches!(
                    command,
                    MutationCommand::WorkspaceMove { .. }
                        | MutationCommand::WorkspaceMoveBlock { .. }
                ) {
                    failure.failure_schema(super::super::schema::WORKSPACE_MOVE_FAILURE_ID)
                } else if matches!(command, MutationCommand::TabMoveIndex { .. }) {
                    failure.failure_schema(super::super::schema::TAB_MOVE_FAILURE_ID)
                } else {
                    failure
                };
                failure_output(failure, Some(format), locale)
            }
            Ok(receipt) => success_output(&receipt, format, locale),
        }
    }

    fn execute(command: &MutationCommand) -> Result<Receipt, Failure> {
        if matches!(
            command,
            MutationCommand::TabMoveIndex { .. }
                | MutationCommand::WorkspaceMove { .. }
                | MutationCommand::WorkspaceMoveBlock { .. }
        ) && !cfg!(windows)
        {
            return Err(Failure::new(FailureKind::ServiceUnavailable)
                .details(stage_reason_details("platform", "unsupported-platform")));
        }
        // `close-resume` is a retained-lifecycle request, not a Workbench
        // command — it takes its own exchange on the same session
        // discipline: one admission, one submission, never a replay.
        if let MutationCommand::WorkspaceCloseResume {
            project,
            workspace,
            request,
            generation,
        } = command
        {
            return execute_close_resume(project, workspace, request, *generation);
        }
        let caller = if let MutationCommand::PanelInput { target, .. } = command {
            if !cfg!(windows) {
                return Err(Failure::new(FailureKind::ServiceUnavailable)
                    .details(stage_reason_details("platform", "unsupported-platform")));
            }
            Some(super::super::caller_context::PanelTarget::prepare(
                target.as_ref(),
                |name| std::env::var_os(name),
            )?)
        } else {
            None
        };
        // One admitted session plus one bounded snapshot read; a read failure
        // already dropped the session inside `snapshot_session`.
        let (mut client, deadline, snapshot) = snapshot_session()?;
        let mut exact = command.clone();
        if let (Some(caller), MutationCommand::PanelInput { target, .. }) = (caller, &mut exact) {
            *target = Some(caller.resolve(&snapshot)?);
        }
        let resolved = resolve(&exact, &snapshot)
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
            MutationCommand::ProjectRename { target, label } => {
                let project = resolve_project(snapshot, target)?;
                Ok(WorkbenchAction::ProjectRename {
                    project_id: project.project_id.clone(),
                    label: label.clone(),
                })
            }
            MutationCommand::WorkspaceLaunch {
                project,
                label,
                geometry,
                launch,
                profile,
                options,
            } => {
                let project = resolve_project(snapshot, project)?;
                Ok(WorkbenchAction::WorkspaceLaunch {
                    project_id: project.project_id.clone(),
                    label: label.clone(),
                    geometry: *geometry,
                    launch: launch.clone(),
                    profile: profile.clone(),
                    options: Box::new(options.clone()),
                })
            }
            #[cfg(not(windows))]
            MutationCommand::WorkspaceCreate { project, label } => {
                let project = resolve_project(snapshot, project)?;
                Ok(WorkbenchAction::WorkspaceCreate {
                    project_id: project.project_id.clone(),
                    label: label.clone(),
                })
            }
            MutationCommand::WorkspaceMove {
                target,
                insert_index,
            } => {
                let (_, workspace) = resolve_workspace(snapshot, target)?;
                Ok(WorkbenchAction::WorkspaceMove {
                    workspace_id: workspace.workspace_id.clone(),
                    insert_index: *insert_index,
                })
            }
            MutationCommand::WorkspaceMoveBlock { targets, before } => {
                let ids = targets
                    .iter()
                    .map(|target| {
                        resolve_workspace(snapshot, target)
                            .map(|(_, workspace)| workspace.workspace_id.clone())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let before_workspace_id = before
                    .as_ref()
                    .map(|target| {
                        resolve_workspace(snapshot, target)
                            .map(|(_, workspace)| workspace.workspace_id.clone())
                    })
                    .transpose()?;
                let workspace_ids = orchbee_workbench_protocol::WorkbenchWorkspaceList::new(ids)
                    .map_err(|_| {
                        Failure::new(FailureKind::Rejected).details(Map::from_iter([
                            ("stage".to_owned(), json!("command")),
                            ("reason".to_owned(), json!("duplicate-workspace")),
                            ("revision".to_owned(), json!(snapshot.revision)),
                        ]))
                    })?;
                Ok(WorkbenchAction::WorkspaceMoveBlock {
                    workspace_ids,
                    before_workspace_id,
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
            MutationCommand::WorkspaceClose { target } => {
                let (project, workspace) = resolve_workspace(snapshot, target)?;
                Ok(WorkbenchAction::WorkspaceClose {
                    project_id: project.project_id.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                })
            }
            // `close-resume` never becomes a Workbench action; `execute`
            // routes it to the lifecycle resume exchange before resolution.
            MutationCommand::WorkspaceCloseResume { .. } => {
                Err(Failure::new(FailureKind::InternalFailure))
            }
            MutationCommand::TabCreate {
                automatic_label,
                context,
                label,
                geometry,
                launch,
                profile,
                options,
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
                    automatic_label: *automatic_label,
                    terminal_context_id,
                    label: label.clone(),
                    geometry: *geometry,
                    launch: launch.clone(),
                    profile: profile.clone(),
                    options: Box::new(options.clone()),
                })
            }
            MutationCommand::PanelInput {
                target,
                right_click,
            } => {
                let target = target
                    .as_ref()
                    .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                Ok(WorkbenchAction::PanelInputSet {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id,
                    right_click: *right_click,
                })
            }
            MutationCommand::PanelRename { target, label } => {
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                Ok(WorkbenchAction::PanelRename {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id,
                    label: label.clone(),
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
            MutationCommand::TabMoveIndex {
                target,
                insert_index,
            } => {
                let (context, tab) = resolve_tab(snapshot, target)?;
                Ok(WorkbenchAction::TabMoveIndex {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    insert_index: *insert_index,
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
                right_click,
                direction,
                label,
                geometry,
                launch,
                profile,
                options,
            } => {
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                Ok(WorkbenchAction::PanelSplit {
                    right_click: *right_click,
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id,
                    direction: *direction,
                    label: label.clone(),
                    geometry: *geometry,
                    launch: launch.clone(),
                    profile: profile.clone(),
                    options: Box::new(options.clone()),
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
                destination,
            } => {
                let (context, tab, panel_id) = resolve_panel(snapshot, target)?;
                match destination {
                    PanelMoveDestination::Adjacent { into, direction } => {
                        // The wire shape binds both panels into one tab, so the
                        // target panel resolves inside the source panel's own
                        // tab.
                        let target_panel_id = resolve_panel_in(tab, into)?;
                        Ok(WorkbenchAction::PanelMove {
                            terminal_context_id: context.terminal_context_id.clone(),
                            tab_id: tab.tab_id.clone(),
                            panel_id,
                            target_panel_id,
                            direction: *direction,
                        })
                    }
                    PanelMoveDestination::Tab {
                        tab: into_tab,
                        direction,
                    } => {
                        // Relocation is bounded to the source TerminalContext:
                        // the destination Tab resolves inside it alone, so a
                        // same-labelled Tab elsewhere is never a candidate and
                        // ambiguity is decided within the context.
                        let destination_tab = resolve_tab_in(context, into_tab)?;
                        Ok(WorkbenchAction::PanelRelocate {
                            terminal_context_id: context.terminal_context_id.clone(),
                            tab_id: tab.tab_id.clone(),
                            panel_id,
                            destination: WorkbenchPanelRelocateDestination::Tab {
                                terminal_context_id: None,
                                target_panel_id: None,
                                tab_id: destination_tab.tab_id.clone(),
                                direction: *direction,
                            },
                        })
                    }
                    PanelMoveDestination::NewTab { label } => Ok(WorkbenchAction::PanelRelocate {
                        terminal_context_id: context.terminal_context_id.clone(),
                        tab_id: tab.tab_id.clone(),
                        panel_id,
                        destination: WorkbenchPanelRelocateDestination::NewTab {
                            terminal_context_id: None,
                            label: Some(label.clone()),
                        },
                    }),
                }
            }
            MutationCommand::LaunchProfileCreate { name, launch } => {
                Ok(WorkbenchAction::LaunchProfileCreate {
                    name: name.clone(),
                    launch: launch.clone(),
                })
            }
            MutationCommand::LaunchProfileDelete { selector } => {
                Ok(WorkbenchAction::LaunchProfileDelete {
                    selector: selector.clone(),
                })
            }
            // `terminal send` never becomes a Workbench action; `cli::mutation`
            // routes it to the send executor before this resolver runs.
            MutationCommand::TerminalSend { .. }
            | MutationCommand::AgentSendKeys { .. }
            | MutationCommand::AgentPrompt { .. } => {
                Err(Failure::new(FailureKind::InternalFailure))
            }
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
                context.tabs.iter().enumerate().map(move |(position, tab)| {
                    (
                        tab.tab_id.as_str(),
                        tab.display_label(position),
                        (context, tab),
                    )
                })
            }),
            target,
            Resource::Tab,
        )
    }

    /// The `--into-tab` destination resolves inside the source context alone
    /// — same grammar as `resolve_tab` (typed id or label, ambiguity
    /// rejected) scoped to that context's Tabs.
    fn resolve_tab_in<'a>(
        context: &'a TerminalContext,
        target: &Target,
    ) -> Result<&'a Tab, Failure> {
        results::resolve_or_fail(
            context
                .tabs
                .iter()
                .enumerate()
                .map(|(position, tab)| (tab.tab_id.as_str(), tab.display_label(position), tab)),
            target,
            Resource::Tab,
        )
    }

    /// Panels have no label; complete typed IDs and @panel selectors resolve
    /// through the snapshot panel set, exactly like `panel get`.
    fn resolve_panel<'a>(
        snapshot: &'a Snapshot,
        target: &Target,
    ) -> Result<(&'a TerminalContext, &'a Tab, WorkbenchPanelId), Failure> {
        // A `Label` target is parser-impossible for `Resource::Panel`; keep
        // the arm defensive anyway.
        if let Target::Label(_) = target {
            return Err(Failure::new(FailureKind::InternalFailure));
        }
        let (panel_id, context, tab) = results::resolve_or_fail(
            results::panel_entries(snapshot)
                .into_iter()
                .map(|(id, context, tab)| (id.as_str(), "", (id, context, tab))),
            target,
            Resource::Panel,
        )?;
        Ok((context, tab, panel_id.clone()))
    }

    /// `--target-panel` resolves inside the moved panel's own tab — the local
    /// scope is the complete candidate set; there is no global fallback after
    /// a miss.
    fn resolve_panel_in(tab: &Tab, target: &Target) -> Result<WorkbenchPanelId, Failure> {
        // Same parser-impossible `Label` arm as `resolve_panel`.
        if let Target::Label(_) = target {
            return Err(Failure::new(FailureKind::InternalFailure));
        }
        results::resolve_or_fail(
            results::panel_ids_in(&tab.root)
                .into_iter()
                .map(|id| (id.as_str(), "", id)),
            target,
            Resource::Panel,
        )
        .cloned()
    }

    /// Submit exactly one command on the admitted session and map the typed
    /// submission into the receipt/failure matrix.
    #[cfg(windows)]
    pub(in crate::cli) fn submit_creation(
        client: &mut LocalClient,
        snapshot: &Snapshot,
        command: &MutationCommand,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> Result<Receipt, Failure> {
        let resolved = resolve(command, snapshot)
            .map_err(|failure| failure.source(results::snapshot_source(snapshot)))?;
        let source = command_source(
            resolved.site_id.as_str(),
            resolved.request_id.as_str(),
            resolved.expected_revision,
        );
        submit_with_wake(client, deadline, resolved, wake).map_err(|failure| failure.source(source))
    }

    /// Submit an already resolved action from a phased CLI workflow on the same snapshot.
    #[cfg(windows)]
    pub(in crate::cli) fn submit_action(
        client: &mut LocalClient,
        snapshot: &Snapshot,
        action: WorkbenchAction,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> Result<Receipt, Failure> {
        let resolved = Resolved {
            site_id: snapshot.site_id.clone(),
            expected_revision: snapshot.revision,
            request_id: crate::cli::query::request_id()?,
            action,
        };
        let source = command_source(
            resolved.site_id.as_str(),
            resolved.request_id.as_str(),
            resolved.expected_revision,
        );
        submit_with_wake(client, deadline, resolved, wake).map_err(|failure| failure.source(source))
    }

    fn submit(
        client: &mut LocalClient,
        deadline: Instant,
        resolved: Resolved,
    ) -> Result<Receipt, Failure> {
        submit_with_wake(client, deadline, resolved, None)
    }

    fn submit_with_wake(
        client: &mut LocalClient,
        deadline: Instant,
        resolved: Resolved,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
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
            | WorkbenchAction::ProjectRename { .. }
            | WorkbenchAction::WorkspaceCreate { .. }
            | WorkbenchAction::WorkspaceRename { .. }
            | WorkbenchAction::WorkspaceMove { .. }
            | WorkbenchAction::WorkspaceMoveBlock { .. }
            | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. } => {
                let request = WorkbenchSetupRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_setup_with_wake(request, deadline, wake) {
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
                    WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::ProjectRenamed {
                        revision,
                        project_id,
                        unchanged,
                    }) if matches!(&resolved.action, WorkbenchAction::ProjectRename { .. }) => {
                        Ok(receipt(
                            &resolved,
                            "project.rename",
                            if unchanged { "unchanged" } else { "committed" },
                        )
                        .schema(PROJECT_RENAME_RESULT_ID)
                        .revision(revision)
                        .id("projectId", project_id.as_str())
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
                    WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::WorkspaceOrder {
                        revision,
                        workspace_order,
                    }) => {
                        let unchanged = workspace_order.is_none();
                        let action = match &resolved.action {
                            WorkbenchAction::WorkspaceMove { .. } => "workspace.move",
                            WorkbenchAction::WorkspaceMoveBlock { .. } => "workspace.move-block",
                            _ => return Err(internal()),
                        };
                        let mut result = receipt(
                            &resolved,
                            action,
                            if unchanged { "unchanged" } else { "committed" },
                        )
                        .schema(super::super::schema::WORKSPACE_MOVE_RESULT_ID)
                        .revision(revision);
                        result.workspace_order = workspace_order;
                        match &resolved.action {
                            WorkbenchAction::WorkspaceMove {
                                workspace_id,
                                insert_index,
                            } => {
                                result = result.id("workspaceId", workspace_id.as_str());
                                result.workspace_insert_index = Some(*insert_index);
                            }
                            WorkbenchAction::WorkspaceMoveBlock {
                                workspace_ids,
                                before_workspace_id,
                            } => {
                                result.workspace_sources = Some(workspace_ids.clone());
                                if let Some(before) = before_workspace_id {
                                    result = result.id("beforeWorkspaceId", before.as_str());
                                }
                            }
                            _ => return Err(internal()),
                        }
                        Ok(result)
                    }
                    WorkbenchSetupSubmission::Committed(_) => Err(internal()),
                    WorkbenchSetupSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchSetupSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::WorkspaceLaunch { options, .. } => {
                use crate::local_client::workbench::workspace_launch::{
                    WorkspaceLaunchRequest, WorkspaceLaunchSubmission,
                };
                let request = WorkspaceLaunchRequest::try_from(command).map_err(|_| internal())?;
                let submission = if wake.is_some() {
                    client.submit_workspace_launch_with_wake(request, deadline, wake)
                } else {
                    client.submit_workspace_launch(request, deadline)
                };
                match submission {
                    WorkspaceLaunchSubmission::Committed(commit) => {
                        let terminal = commit.terminal_outcome.as_ref().map_or(
                            TerminalFact {
                                outcome: "pending",
                                reason: None,
                                geometry: None,
                            },
                            terminal_fact,
                        );
                        Ok(receipt(&resolved, "workspace.create", "committed")
                            .revision(commit.revision)
                            .id("projectId", commit.project_id.as_str())
                            .id("workspaceId", commit.workspace_id.as_str())
                            .id("directoryId", commit.directory_id.as_str())
                            .id("terminalContextId", commit.terminal_context_id.as_str())
                            .id("tabId", commit.tab_id.as_str())
                            .id("panelId", commit.panel_id.as_str())
                            .id("presentationId", commit.presentation_id.as_str())
                            .label(resolved_label(&resolved.action))
                            .path(options.cwd.as_ref().ok_or_else(internal)?.as_str())
                            .terminal(terminal))
                    }
                    WorkspaceLaunchSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkspaceLaunchSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkspaceLaunchSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabCreate { .. } => {
                let request =
                    WorkbenchTabCreateRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_tab_create_with_wake(request, deadline, wake) {
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
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchTabCreateSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::PanelInputSet {
                terminal_context_id,
                tab_id,
                panel_id,
                right_click,
            } => {
                let mut base = receipt(&resolved, "panel.input", "configured")
                    .schema(super::super::schema::PANEL_INPUT_RESULT_ID)
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .id("panelId", panel_id.as_str());
                base.right_click = Some(*right_click);
                let request =
                    WorkbenchPanelInputRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_panel_input(request, deadline) {
                    WorkbenchPanelInputSubmission::Committed { revision } => {
                        Ok(base.revision(revision))
                    }
                    WorkbenchPanelInputSubmission::Unchanged { revision } => Ok(Receipt {
                        outcome: "unchanged",
                        ..base
                    }
                    .revision(revision)),
                    WorkbenchPanelInputSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchPanelInputSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelInputSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabRename { .. } | WorkbenchAction::PanelRename { .. } => {
                let request =
                    WorkbenchLabelRenameRequest::try_from(command).map_err(|_| internal())?;
                let base = match &resolved.action {
                    WorkbenchAction::TabRename {
                        terminal_context_id,
                        tab_id,
                        label,
                    } => receipt(&resolved, "tab.rename", "committed")
                        .id("terminalContextId", terminal_context_id.as_str())
                        .id("tabId", tab_id.as_str())
                        .label(label.as_str()),
                    WorkbenchAction::PanelRename {
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        label,
                    } => {
                        let mut base = receipt(&resolved, "panel.rename", "renamed")
                            .schema(super::super::schema::PANEL_RENAME_RESULT_ID)
                            .id("terminalContextId", terminal_context_id.as_str())
                            .id("tabId", tab_id.as_str())
                            .id("panelId", panel_id.as_str());
                        base.custom_label = Some(label.clone());
                        base
                    }
                    _ => return Err(internal()),
                };
                match client.submit_workbench_label_rename(request, deadline) {
                    WorkbenchLabelRenameSubmission::Committed { revision } => {
                        Ok(base.revision(revision))
                    }
                    WorkbenchLabelRenameSubmission::Unchanged { revision } => Ok(Receipt {
                        outcome: "unchanged",
                        ..base
                    }
                    .revision(revision)),
                    WorkbenchLabelRenameSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchLabelRenameSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::TabMove {
                terminal_context_id,
                tab_id,
                ..
            }
            | WorkbenchAction::TabMoveIndex {
                terminal_context_id,
                tab_id,
                ..
            } => {
                let request = WorkbenchTabMoveRequest::try_from(command).map_err(|_| internal())?;
                let mut base = receipt(&resolved, "tab.move", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str());
                match &resolved.action {
                    WorkbenchAction::TabMove { direction, .. } => {
                        base = base.direction(tab_direction(*direction));
                    }
                    WorkbenchAction::TabMoveIndex { insert_index, .. } => {
                        base = base.schema(super::super::schema::TAB_MOVE_RESULT_ID);
                        base.insert_index = Some(*insert_index);
                    }
                    _ => unreachable!("Tab move dispatch"),
                }
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
                        Err(precommit_failure(precommit.into(), &resolved.action))
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
                match client.submit_workbench_tab_close_with_wake(request, deadline, wake) {
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
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchTabCloseSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchTabCloseSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::WorkspaceClose { .. } => {
                let request =
                    WorkbenchWorkspaceCloseRequest::try_from(command).map_err(|_| internal())?;
                match client.submit_workbench_workspace_close(request, deadline) {
                    WorkbenchWorkspaceCloseSubmission::Accepted(
                        WorkbenchWorkspaceCloseAccepted {
                            close_request_id,
                            project_id,
                            workspace_id,
                            terminal_context_id,
                            accepted_revision,
                        },
                    ) => Ok(receipt(&resolved, "workspace.close", "accepted")
                        .accepted(close_request_id.as_str(), accepted_revision)
                        .id("projectId", project_id.as_str())
                        .id("workspaceId", workspace_id.as_str())
                        .id("terminalContextId", terminal_context_id.as_str())),
                    WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchWorkspaceCloseSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchWorkspaceCloseSubmission::OutcomeUnknown { request_id, reason } => {
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
                match client.submit_workbench_panel_split_with_wake(request, deadline, wake) {
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
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelSplitSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::LaunchProfileCreate { .. } => {
                let request = WorkbenchLaunchProfileCreateRequest::try_from(command)
                    .map_err(|_| internal())?;
                match client.submit_workbench_launch_profile_create(request, deadline) {
                    WorkbenchLaunchProfileMutationSubmission::Created {
                        revision,
                        catalog_revision,
                        profile_id,
                    } => Ok(receipt(&resolved, "launch-profile.create", "committed")
                        .revision(revision)
                        .catalog_revision(catalog_revision)
                        .id("profileId", profile_id.as_str())
                        .name(resolved_profile_name(&resolved.action))),
                    WorkbenchLaunchProfileMutationSubmission::Deleted {
                        revision,
                        catalog_revision,
                        profile_id,
                    } => Ok(receipt(&resolved, "launch-profile.delete", "committed")
                        .revision(revision)
                        .catalog_revision(catalog_revision)
                        .id("profileId", profile_id.as_str())),
                    WorkbenchLaunchProfileMutationSubmission::ProfileUnavailable => {
                        Err(profile_unavailable_failure(&resolved.action))
                    }
                    WorkbenchLaunchProfileMutationSubmission::ProfileAmbiguous {
                        candidate_ids,
                    } => Err(profile_ambiguous_failure(&resolved.action, &candidate_ids)),
                    WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchLaunchProfileMutationSubmission::OutcomeUnknown {
                        request_id,
                        reason,
                    } => Err(unknown_failure(request_id, reason.reason())),
                }
            }
            WorkbenchAction::LaunchProfileDelete { .. } => {
                let request = WorkbenchLaunchProfileDeleteRequest::try_from(command)
                    .map_err(|_| internal())?;
                match client.submit_workbench_launch_profile_delete(request, deadline) {
                    WorkbenchLaunchProfileMutationSubmission::Created {
                        revision,
                        catalog_revision,
                        profile_id,
                    } => Ok(receipt(&resolved, "launch-profile.create", "committed")
                        .revision(revision)
                        .catalog_revision(catalog_revision)
                        .id("profileId", profile_id.as_str())
                        .name(resolved_profile_name(&resolved.action))),
                    WorkbenchLaunchProfileMutationSubmission::Deleted {
                        revision,
                        catalog_revision,
                        profile_id,
                    } => Ok(receipt(&resolved, "launch-profile.delete", "committed")
                        .revision(revision)
                        .catalog_revision(catalog_revision)
                        .id("profileId", profile_id.as_str())),
                    WorkbenchLaunchProfileMutationSubmission::ProfileUnavailable => {
                        Err(profile_unavailable_failure(&resolved.action))
                    }
                    WorkbenchLaunchProfileMutationSubmission::ProfileAmbiguous {
                        candidate_ids,
                    } => Err(profile_ambiguous_failure(&resolved.action, &candidate_ids)),
                    WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchLaunchProfileMutationSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchLaunchProfileMutationSubmission::OutcomeUnknown {
                        request_id,
                        reason,
                    } => Err(unknown_failure(request_id, reason.reason())),
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
                        Err(precommit_failure(precommit.into(), &resolved.action))
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
                    WorkbenchPanelPlacementRequest::try_from(command).map_err(|_| internal())?;
                let base = receipt(&resolved, "panel.move", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .id("panelId", panel_id.as_str())
                    .id("targetPanelId", target_panel_id.as_str())
                    .direction(panel_direction(*direction));
                match client.submit_workbench_panel_placement(request, deadline) {
                    WorkbenchPanelPlacementSubmission::Committed(
                        WorkbenchPanelPlacementCommit { revision, .. },
                    ) => Ok(base.revision(revision)),
                    WorkbenchPanelPlacementSubmission::Unchanged { revision } => Ok(Receipt {
                        outcome: "unchanged",
                        ..base
                    }
                    .revision(revision)),
                    WorkbenchPanelPlacementSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelPlacementSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            WorkbenchAction::PanelRelocate {
                terminal_context_id,
                tab_id,
                panel_id,
                destination,
            } => {
                let request =
                    WorkbenchPanelRelocateRequest::try_from(command).map_err(|_| internal())?;
                let base = receipt(&resolved, "panel.relocate", "committed")
                    .id("terminalContextId", terminal_context_id.as_str())
                    .id("tabId", tab_id.as_str())
                    .id("panelId", panel_id.as_str());
                match client.submit_workbench_panel_relocate_with_wake(request, deadline, wake) {
                    WorkbenchPanelRelocateSubmission::Committed(WorkbenchPanelRelocateCommit {
                        revision,
                        target_tab_id,
                        split,
                        transfer,
                        ..
                    }) => {
                        // The receipt echoes the direction only for the
                        // existing-Tab form; `new-tab` has no direction and
                        // reports its Server-minted Tab identity.
                        let mut receipt = base
                            .revision(revision)
                            .id("targetTabId", target_tab_id.as_str());
                        receipt.panel_transfer = transfer;
                        if let Some(split) = &split {
                            receipt = receipt
                                .id("splitId", split.split_id.as_str())
                                .id("firstSlotId", split.first_slot_id.as_str())
                                .id("secondSlotId", split.second_slot_id.as_str());
                        }
                        Ok(match destination {
                            WorkbenchPanelRelocateDestination::Tab { direction, .. } => {
                                receipt.direction(panel_direction(*direction))
                            }
                            WorkbenchPanelRelocateDestination::NewTab { label, .. }
                            | WorkbenchPanelRelocateDestination::NewWorkspace {
                                tab_label: label,
                                ..
                            } => match label {
                                Some(label) => receipt.label(label.as_str()),
                                None => receipt,
                            },
                        })
                    }
                    WorkbenchPanelRelocateSubmission::DefinitePrecommit(precommit) => {
                        Err(precommit_failure(precommit.into(), &resolved.action))
                    }
                    WorkbenchPanelRelocateSubmission::DefinitelyNotSubmitted(reason) => {
                        Err(not_submitted_failure(reason.into()))
                    }
                    WorkbenchPanelRelocateSubmission::OutcomeUnknown { request_id, reason } => {
                        Err(unknown_failure(request_id, reason.reason()))
                    }
                }
            }
            // Swap is a declared canonical-plus-focus workflow, never an ordinary mutation.
            WorkbenchAction::PanelSwap { .. } => Err(Failure::new(FailureKind::InternalFailure)),
        }
    }

    fn resolved_project_id(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::WorkspaceLaunch { project_id, .. }
            | WorkbenchAction::WorkspaceCreate { project_id, .. }
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
            | WorkbenchAction::ProjectRename { label, .. }
            | WorkbenchAction::WorkspaceLaunch { label, .. }
            | WorkbenchAction::WorkspaceCreate { label, .. }
            | WorkbenchAction::WorkspaceRename { label, .. }
            | WorkbenchAction::TabCreate { label, .. }
            | WorkbenchAction::TabRename { label, .. }
            | WorkbenchAction::PanelSplit { label, .. } => label.as_str(),
            _ => "",
        }
    }

    /// `workspace close-resume`: one retained-lifecycle request on the
    /// admitted session. There is no `workbench.command` for it — the honest
    /// source is the resume request's own echo, and every post-submission
    /// answer maps under the dedicated close failure envelope.
    fn execute_close_resume(
        project_id: &WorkbenchProjectId,
        workspace_id: &WorkbenchWorkspaceId,
        close_request_id: &WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
    ) -> Result<Receipt, Failure> {
        let (mut client, deadline, snapshot) = snapshot_session()?;
        let resume = WorkbenchWorkspaceCloseResume {
            protocol_version: ProtocolVersion,
            request_id: request_id()?,
            site_id: snapshot.site_id.clone(),
            project_id: project_id.clone(),
            workspace_id: workspace_id.clone(),
            close_request_id: close_request_id.clone(),
            expected_attempt_generation: generation,
        };
        let resume_request_id = resume.request_id.clone();
        let source = json!({
            "kind": "workspace-close-resume",
            "siteId": snapshot.site_id.as_str(),
            "requestId": resume_request_id.as_str(),
            "closeRequestId": close_request_id.as_str(),
            "expectedAttemptGeneration": generation.value().to_string(),
        });
        let submission = client.submit_workbench_workspace_close_resume(resume, deadline);
        // The Client Session is fully dropped before the outcome is mapped or
        // any output is prepared; an ambiguous submission already retired the
        // connection inside the transport, and nothing is replayed.
        drop(client);
        let failure = |kind| Failure::new(kind).failure_schema(CLOSE_FAILURE_ID);
        match submission {
            WorkbenchWorkspaceCloseResumeSubmission::Returned(
                WorkbenchWorkspaceCloseResumeOutcome::Resumed { attempt_generation },
            ) => Ok(Receipt {
                site_id: snapshot.site_id.as_str().to_owned(),
                request_id: resume_request_id.as_str().to_owned(),
                base_revision: snapshot.revision,
                source_override: Some(source.clone()),
                schema_id: MUTATION_RESULT_ID,
                action: "workspace.close-resume",
                outcome: "resumed",
                revision: None,
                accepted_revision: None,
                close_request_id: Some(close_request_id.as_str().to_owned()),
                attempt_generation: Some(attempt_generation.value().to_string()),
                ids: vec![
                    ("projectId", project_id.as_str().to_owned()),
                    ("workspaceId", workspace_id.as_str().to_owned()),
                ],
                direction: None,
                insert_index: None,
                workspace_insert_index: None,
                workspace_order: None,
                workspace_sources: None,
                panel_transfer: None,
                label: None,
                custom_label: None,
                right_click: None,
                path: None,
                terminal: None,
                catalog_revision: None,
                name: None,
            }),
            WorkbenchWorkspaceCloseResumeSubmission::Returned(
                WorkbenchWorkspaceCloseResumeOutcome::Stale { attempt_generation },
            ) => Err(failure(FailureKind::Rejected).details(Map::from_iter([
                ("stage".to_owned(), json!("close-resume")),
                ("reason".to_owned(), json!("stale-generation")),
                (
                    "attemptGeneration".to_owned(),
                    json!(attempt_generation.value().to_string()),
                ),
            ]))),
            WorkbenchWorkspaceCloseResumeSubmission::Returned(
                WorkbenchWorkspaceCloseResumeOutcome::Denied {},
            ) => Err(failure(FailureKind::Denied).details(stage_details("close-resume"))),
            WorkbenchWorkspaceCloseResumeSubmission::Returned(
                WorkbenchWorkspaceCloseResumeOutcome::Unavailable { reason },
            ) => Err(
                failure(FailureKind::ServiceUnavailable).details(stage_reason_details(
                    "close-resume",
                    close_unavailable_reason(reason),
                )),
            ),
            WorkbenchWorkspaceCloseResumeSubmission::Returned(
                WorkbenchWorkspaceCloseResumeOutcome::Rejected { reason },
            ) => Err(failure(FailureKind::Rejected).details(Map::from_iter([
                ("stage".to_owned(), json!("close-resume")),
                ("reason".to_owned(), json!(resume_rejection_reason(reason))),
            ]))),
            WorkbenchWorkspaceCloseResumeSubmission::DefinitelyNotSubmitted(reason) => {
                Err(not_submitted_failure_at("close-resume", reason.into())
                    .failure_schema(CLOSE_FAILURE_ID))
            }
            WorkbenchWorkspaceCloseResumeSubmission::OutcomeUnknown { request_id, reason } => Err(
                unknown_failure_at("close-resume", request_id, reason.reason())
                    .failure_schema(CLOSE_FAILURE_ID),
            ),
        }
        .map_err(|failure| failure.source(source))
    }

    /// The close read/resume unavailable vocabulary is the lifecycle's own —
    /// today it carries only `server-busy`.
    fn close_unavailable_reason(reason: WorkbenchTabCloseUnavailableReason) -> &'static str {
        match reason {
            WorkbenchTabCloseUnavailableReason::ServerBusy => "server-busy",
        }
    }

    /// The resume rejection vocabulary — the lifecycle's typed reasons plus
    /// the fence verdict the caller can act on.
    fn resume_rejection_reason(reason: WorkbenchTabCloseResumeRejectionReason) -> &'static str {
        match reason {
            WorkbenchTabCloseResumeRejectionReason::OperationUnavailable => "operation-unavailable",
            WorkbenchTabCloseResumeRejectionReason::OperationMismatch => "operation-mismatch",
            WorkbenchTabCloseResumeRejectionReason::RequestIdentityConflict => {
                "request-identity-conflict"
            }
            WorkbenchTabCloseResumeRejectionReason::GenerationMismatch => "generation-mismatch",
            WorkbenchTabCloseResumeRejectionReason::NotResumable => "not-resumable",
            WorkbenchTabCloseResumeRejectionReason::GenerationExhausted => "generation-exhausted",
        }
    }

    fn receipt(resolved: &Resolved, action: &'static str, outcome: &'static str) -> Receipt {
        Receipt {
            site_id: resolved.site_id.as_str().to_owned(),
            request_id: resolved.request_id.as_str().to_owned(),
            base_revision: resolved.expected_revision,
            source_override: None,
            schema_id: MUTATION_RESULT_ID,
            action,
            outcome,
            revision: None,
            accepted_revision: None,
            close_request_id: None,
            attempt_generation: None,
            ids: Vec::new(),
            direction: None,
            insert_index: None,
            workspace_insert_index: None,
            workspace_order: None,
            workspace_sources: None,
            panel_transfer: None,
            label: None,
            custom_label: None,
            right_click: None,
            path: None,
            terminal: None,
            catalog_revision: None,
            name: None,
        }
    }

    /// A command that provably never reached the wire: an encoding or session
    /// defect is internal, a transport refusal before handoff is a clean
    /// transport-loss with no applied effect.
    fn not_submitted_failure(reason: NotSubmitted) -> Failure {
        not_submitted_failure_at("command", reason)
    }

    /// The same definite non-submission under a non-command stage: the
    /// close-resume exchange reports its own stage token so the dedicated
    /// close envelope's stage vocabulary stays closed.
    fn not_submitted_failure_at(stage: &'static str, reason: NotSubmitted) -> Failure {
        match reason {
            NotSubmitted::SessionUnavailable | NotSubmitted::RequestEncoding => {
                Failure::new(FailureKind::InternalFailure)
            }
            NotSubmitted::Request => {
                Failure::new(FailureKind::TransportLoss).details(stage_details(stage))
            }
        }
    }

    /// A definite pre-commit answer the Server typed itself. The typed
    /// profile-resolution settlements project the selector the action
    /// carried — under the launch-profile failure envelope, since the
    /// generic failure schema's resource and candidate grammars are closed
    /// without it.
    fn precommit_failure(precommit: Precommit, action: &WorkbenchAction) -> Failure {
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
            Precommit::ProfileUnavailable => profile_unavailable_failure(action),
            Precommit::ProfileAmbiguous { candidate_ids } => {
                profile_ambiguous_failure(action, &candidate_ids)
            }
        }
    }

    /// The selector the action carried — `tab create`/`panel split` only
    /// admit one when `--profile` was selected; `launch-profile delete`
    /// always has one.
    fn resolved_profile_selector(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::WorkspaceLaunch {
                profile: Some(selector),
                ..
            }
            | WorkbenchAction::TabCreate {
                profile: Some(selector),
                ..
            }
            | WorkbenchAction::PanelSplit {
                profile: Some(selector),
                ..
            } => selector.as_str(),
            WorkbenchAction::LaunchProfileDelete { selector } => selector.as_str(),
            _ => "",
        }
    }

    /// The name a `launch-profile create` carried — echoed on the committed
    /// receipt exactly as the catalog stored it.
    fn resolved_profile_name(action: &WorkbenchAction) -> &str {
        match action {
            WorkbenchAction::LaunchProfileCreate { name, .. } => name.as_str(),
            _ => "",
        }
    }

    /// The Server resolved the selector to nothing: the launch-profile
    /// family's `not-found` — no candidates, just the resource and the
    /// selector it refused.
    fn profile_unavailable_failure(action: &WorkbenchAction) -> Failure {
        Failure::new(FailureKind::NotFound)
            .details(Map::from_iter([
                ("resource".to_owned(), json!("launch-profile")),
                (
                    "target".to_owned(),
                    json!(resolved_profile_selector(action)),
                ),
            ]))
            .failure_schema(LAUNCH_PROFILE_FAILURE_ID)
    }

    /// The selector's name cardinality exceeded one: the family's
    /// `ambiguous` — the bounded same-resolution candidate ids plus the true
    /// total, capped like every other ambiguity list.
    fn profile_ambiguous_failure(
        action: &WorkbenchAction,
        candidate_ids: &[LaunchProfileId],
    ) -> Failure {
        let total = candidate_ids.len();
        let candidates: Vec<&str> = candidate_ids
            .iter()
            .take(8)
            .map(LaunchProfileId::as_str)
            .collect();
        Failure::new(FailureKind::Ambiguous)
            .details(Map::from_iter([
                ("resource".to_owned(), json!("launch-profile")),
                (
                    "target".to_owned(),
                    json!(resolved_profile_selector(action)),
                ),
                ("candidates".to_owned(), json!(candidates)),
                ("totalCount".to_owned(), json!(total)),
            ]))
            .failure_schema(LAUNCH_PROFILE_FAILURE_ID)
    }

    /// Any post-submission outcome the transport could not settle: the
    /// command's effect is undetermined, the session is already retired, and
    /// the request identity is reported so operators can investigate. The CLI
    /// never replays it.
    fn unknown_failure(request_id: WorkbenchRequestId, reason: &'static str) -> Failure {
        unknown_failure_at("command", request_id, reason)
    }

    /// The same undetermined-effect verdict under a non-command stage: the
    /// close-resume exchange reports its own stage token.
    fn unknown_failure_at(
        stage: &'static str,
        request_id: WorkbenchRequestId,
        reason: &'static str,
    ) -> Failure {
        Failure::new(FailureKind::OutcomeUnknown).details(Map::from_iter([
            ("stage".to_owned(), json!(stage)),
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
            WorkbenchRejectionReason::WorkspaceGroupCloseRequired => {
                "workspace-group-close-required"
            }
            WorkbenchRejectionReason::WorkspaceDirectoryUnavailable => {
                "workspace-directory-unavailable"
            }
            WorkbenchRejectionReason::WorkspaceDirectoryConflict => "workspace-directory-conflict",
            WorkbenchRejectionReason::TerminalContextUnavailable => "terminal-context-unavailable",
            WorkbenchRejectionReason::TabUnavailable => "tab-unavailable",
            WorkbenchRejectionReason::TabMoveIndexOutOfBounds => "tab-move-index-out-of-bounds",
            WorkbenchRejectionReason::WorkspaceMoveIndexOutOfBounds => {
                "workspace-move-index-out-of-bounds"
            }
            WorkbenchRejectionReason::WorkspaceMoveTargetInBlock => {
                "workspace-move-target-in-block"
            }
            WorkbenchRejectionReason::PanelUnavailable => "panel-unavailable",
            WorkbenchRejectionReason::PanelNotLeaf => "panel-not-leaf",
            WorkbenchRejectionReason::LastPanelInTab => "last-panel-in-tab",
            WorkbenchRejectionReason::SameTab => "same-tab",
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
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            },
            OutputFormat::Text => match output::render_text(|out| present::receipt(out, receipt)) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
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

        /// `panel move --target-panel` resolves inside the moved panel's own
        /// tab — a selector tail that only exists on another tab's panel is a
        /// miss, never a cross-tab fallback.
        #[test]
        fn panel_move_scopes_target_panel_to_the_source_tab() {
            use orchbee_workbench_protocol::parse_snapshot;

            fn tid(high: u64, tail: u64) -> String {
                format!("{high:016x}{tail:016x}")
            }
            let leaf = |panel: &str, pres: &str| {
                json!({
                    "kind": "panel",
                    "panelId": panel,
                    "primaryPresentation": {
                        "presentationId": pres,
                        "label": "p",
                        "binding": null,
                    },
                    "childPresentations": [],
                })
            };
            let document = json!({
                "kind": "workbench.snapshot",
                "protocolVersion": 1,
                "revision": "9",
                "siteId": tid(0, 0xaa00),
                "navigation": {
                    "projects": [{
                        "projectId": tid(0, 0xbb),
                        "label": "p",
                        "directories": [],
                        "workspaces": [{
                            "workspaceId": tid(0, 0xdd),
                            "label": "w",
                            "primaryDirectoryId": null,
                            "terminalContextId": tid(0, 0x60),
                        }],
                    }],
                    "standaloneTerminalContextId": tid(0, 0x01),
                },
                "terminalContexts": [
                    {
                        "terminalContextId": tid(0, 0x01),
                        "groups": [],
                        "tabs": [{
                            "tabId": tid(0, 0xee),
                            "label": "t",
                            "pinned": false,
                            "groupId": null,
                            // A panel ending `ff` lives here — outside the
                            // source tab's scope.
                            "root": leaf(&tid(0, 0xff), &tid(0xa, 0x01)),
                        }],
                    },
                    {
                        "terminalContextId": tid(0, 0x60),
                        "groups": [],
                        "tabs": [{
                            "tabId": tid(0, 0xe1),
                            "label": "t3",
                            "pinned": false,
                            "groupId": null,
                            "root": {
                                "kind": "split",
                                "splitId": tid(9, 0x92),
                                "orientation": "horizontal",
                                "children": [
                                    {"slotId": tid(8, 0x92), "node": leaf(&tid(0, 0xf2), &tid(0xa, 0x02))},
                                    {"slotId": tid(8, 0x93), "node": leaf(&tid(1, 0x1f2), &tid(0xa, 0x03))},
                                ],
                            },
                        }],
                    },
                ],
            });
            let snapshot = parse_snapshot(&serde_json::to_vec(&document).unwrap()).unwrap();
            let move_command = |into: &str| MutationCommand::PanelMove {
                target: Target::Selector("@panel:0f2".into()),
                destination: PanelMoveDestination::Adjacent {
                    into: Target::Selector(into.into()),
                    direction: WorkbenchPanelSplitDirection::Left,
                },
            };

            // Unique in-scope tail: the action carries the resolved full id.
            let resolved =
                action(&move_command("@panel:1f2"), &snapshot).expect("in-scope selector resolves");
            let WorkbenchAction::PanelMove {
                panel_id,
                target_panel_id,
                ..
            } = resolved
            else {
                panic!("panel move action");
            };
            assert_eq!(panel_id.as_str(), tid(0, 0xf2));
            assert_eq!(target_panel_id.as_str(), tid(1, 0x1f2));

            // In-scope collision: both sibling panels end in `f2`.
            let failure =
                action(&move_command("@panel:f2"), &snapshot).expect_err("in-scope collision");
            assert_eq!(failure.kind, FailureKind::Ambiguous);

            // `ff` exists globally (the standalone tab) but not inside the
            // source tab — no global fallback.
            let failure = action(&move_command("@panel:ff"), &snapshot)
                .expect_err("out-of-scope tail must miss");
            assert_eq!(failure.kind, FailureKind::NotFound);
        }
    }
}
