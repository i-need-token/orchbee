use std::fmt;

mod publication;
pub use publication::{
    MAXIMUM_RESULT_PUBLICATIONS, PublicationReadIntent, WorkbenchPublicationOutcome,
    WorkbenchPublicationResult,
};
mod tab_close;
pub use tab_close::{
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, de::Visitor};

use crate::{
    directory::WorkbenchDirectoryPath,
    identity::{
        ProtocolVersion, SiteId, TerminalPresentationId, WorkbenchDirectoryId, WorkbenchPanelId,
        WorkbenchPanelSlotId, WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision,
        WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
        WorkerId,
    },
    model::{
        Snapshot, SnapshotReadIntent, WorkbenchProtocolDocument, WorkbenchPublicationDocument,
    },
    validation::{
        ContractError, MAXIMUM_ENCODED_INTENT_BYTES, MAXIMUM_ENCODED_PROJECTION_BYTES,
        MAXIMUM_LABEL_LENGTH, enforce_encoded_limit, validate_delta_structure, validate_snapshot,
    },
};

pub const MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES: usize = MAXIMUM_ENCODED_PROJECTION_BYTES + 1024;
const MAXIMUM_BINARY64_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAXIMUM_TERMINAL_GEOMETRY_DIMENSION: u16 = 4096;
const MAXIMUM_TERMINAL_GRID_CELLS: u32 = 262_144;
const MAXIMUM_TERMINAL_PIXEL_EXTENT: u32 = 65_535;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkbenchLabel(String);

impl WorkbenchLabel {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty() || value.chars().count() > MAXIMUM_LABEL_LENGTH {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for WorkbenchLabel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchTerminalGeometry {
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
}

impl WorkbenchTerminalGeometry {
    pub fn new(
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) -> Result<Self, ContractError> {
        let value = Self {
            columns,
            rows,
            cell_pixel_width,
            cell_pixel_height,
        };
        validate_terminal_geometry(value).map_err(|_error| ContractError::InvalidDocument)?;
        Ok(value)
    }

    #[must_use]
    pub const fn columns(self) -> u16 {
        self.columns
    }

    #[must_use]
    pub const fn rows(self) -> u16 {
        self.rows
    }

    #[must_use]
    pub const fn cell_pixel_width(self) -> u16 {
        self.cell_pixel_width
    }

    #[must_use]
    pub const fn cell_pixel_height(self) -> u16 {
        self.cell_pixel_height
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkbenchTerminalGeometryWire {
    columns: Binary64SafeInteger,
    rows: Binary64SafeInteger,
    cell_pixel_width: Binary64SafeInteger,
    cell_pixel_height: Binary64SafeInteger,
}

impl<'de> Deserialize<'de> for WorkbenchTerminalGeometry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WorkbenchTerminalGeometryWire::deserialize(deserializer)?;
        let dimension = |value: Binary64SafeInteger| {
            u16::try_from(value.0)
                .map_err(|_error| de::Error::custom("terminal geometry dimension exceeds u16"))
        };
        Self::new(
            dimension(wire.columns)?,
            dimension(wire.rows)?,
            dimension(wire.cell_pixel_width)?,
            dimension(wire.cell_pixel_height)?,
        )
        .map_err(de::Error::custom)
    }
}

impl Serialize for WorkbenchTerminalGeometry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        validate_terminal_geometry(*self).map_err(serde::ser::Error::custom)?;
        WorkbenchTerminalGeometryWire {
            columns: Binary64SafeInteger(u64::from(self.columns)),
            rows: Binary64SafeInteger(u64::from(self.rows)),
            cell_pixel_width: Binary64SafeInteger(u64::from(self.cell_pixel_width)),
            cell_pixel_height: Binary64SafeInteger(u64::from(self.cell_pixel_height)),
        }
        .serialize(serializer)
    }
}

fn validate_terminal_geometry(value: WorkbenchTerminalGeometry) -> Result<(), &'static str> {
    if value.columns == 0
        || value.columns > MAXIMUM_TERMINAL_GEOMETRY_DIMENSION
        || value.rows == 0
        || value.rows > MAXIMUM_TERMINAL_GEOMETRY_DIMENSION
        || value.cell_pixel_width == 0
        || value.cell_pixel_width > MAXIMUM_TERMINAL_GEOMETRY_DIMENSION
        || value.cell_pixel_height == 0
        || value.cell_pixel_height > MAXIMUM_TERMINAL_GEOMETRY_DIMENSION
    {
        return Err("terminal geometry dimension is outside its protocol bounds");
    }
    let columns = u32::from(value.columns);
    let rows = u32::from(value.rows);
    if columns * rows > MAXIMUM_TERMINAL_GRID_CELLS {
        return Err("terminal geometry exceeds the maximum grid cell count");
    }
    if columns * u32::from(value.cell_pixel_width) > MAXIMUM_TERMINAL_PIXEL_EXTENT
        || rows * u32::from(value.cell_pixel_height) > MAXIMUM_TERMINAL_PIXEL_EXTENT
    {
        return Err("terminal geometry exceeds the maximum pixel extent");
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Binary64SafeInteger(u64);

impl Serialize for Binary64SafeInteger {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for Binary64SafeInteger {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Binary64SafeIntegerVisitor)
    }
}

struct Binary64SafeIntegerVisitor;

impl Visitor<'_> for Binary64SafeIntegerVisitor {
    type Value = Binary64SafeInteger;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a finite nonnegative binary64 safe integer")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        if value > MAXIMUM_BINARY64_SAFE_INTEGER {
            return Err(E::custom("number exceeds the binary64 safe-integer range"));
        }
        Ok(Binary64SafeInteger(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        let value = u64::try_from(value)
            .map_err(|_error| E::custom("number must be a nonnegative safe integer"))?;
        self.visit_u64(value)
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        if !value.is_finite()
            || value.is_sign_negative()
            || value.trunc() != value
            || value > MAXIMUM_BINARY64_SAFE_INTEGER as f64
        {
            return Err(E::custom(
                "number must normalize to a nonnegative safe integer",
            ));
        }
        self.visit_u64(value as u64)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum WorkbenchAction {
    #[serde(rename = "project.create")]
    ProjectCreate { label: WorkbenchLabel },
    #[serde(rename = "project.rename")]
    ProjectRename {
        project_id: WorkbenchProjectId,
        label: WorkbenchLabel,
    },
    #[serde(rename = "workspace.create")]
    WorkspaceCreate {
        project_id: WorkbenchProjectId,
        label: WorkbenchLabel,
    },
    #[serde(rename = "workspace.rename")]
    WorkspaceRename {
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        label: WorkbenchLabel,
    },
    #[serde(rename = "workspace.primary-directory.accept")]
    WorkspacePrimaryDirectoryAccept {
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        path: WorkbenchDirectoryPath,
    },
    #[serde(rename = "tab.rename")]
    TabRename {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        label: WorkbenchLabel,
    },
    #[serde(rename = "tab.move")]
    TabMove {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        direction: WorkbenchTabMoveDirection,
    },
    #[serde(rename = "tab.close")]
    TabClose {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
    #[serde(rename = "tab.create")]
    TabCreate {
        terminal_context_id: WorkbenchTerminalContextId,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
    },
    #[serde(rename = "panel.split")]
    PanelSplit {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkbenchTabMoveDirection {
    Previous,
    Next,
    First,
    Last,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkbenchPanelSplitDirection {
    Above,
    Below,
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchCommand {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub expected_revision: WorkbenchRevision,
    pub action: WorkbenchAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WorkbenchRequest {
    #[serde(rename = "workbench.snapshot.read")]
    SnapshotRead(SnapshotReadIntent),
    #[serde(rename = "workbench.command")]
    Command(WorkbenchCommand),
    #[serde(rename = "workbench.tab-close.read")]
    TabCloseRead(WorkbenchTabCloseRead),
    #[serde(rename = "workbench.tab-close.resume")]
    TabCloseResume(WorkbenchTabCloseResume),
    #[serde(rename = "workbench.publication.read")]
    PublicationRead(PublicationReadIntent),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchRejectionReason {
    SiteMismatch,
    RevisionMismatch,
    ProjectUnavailable,
    WorkspaceUnavailable,
    WorkspaceDirectoryUnavailable,
    WorkspaceDirectoryConflict,
    TerminalContextUnavailable,
    TabUnavailable,
    PanelUnavailable,
    ResourceLimit,
    IdentityExhausted,
    RevisionExhausted,
    RequestIdentityConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchCommandUnavailableReason {
    RequestLedgerFull,
    NoCompatibleWorker,
    TerminalBusy,
    TerminalRuntimeUnavailable,
    ServerBusy,
    ServiceUnavailable,
    DirectoryUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchSnapshotUnavailableReason {
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTerminalUnavailableReason {
    TerminalRuntimeUnavailable,
    ServerBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTerminalFailureReason {
    TerminalCreateFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkbenchTerminalOutcome {
    Started {
        geometry: WorkbenchTerminalGeometry,
    },
    Unavailable {
        reason: WorkbenchTerminalUnavailableReason,
    },
    Failed {
        reason: WorkbenchTerminalFailureReason,
    },
    OutcomeUnknown {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchCommandOutcome {
    ProjectCreated {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
    },
    WorkspaceCreated {
        revision: WorkbenchRevision,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
    WorkspacePrimaryDirectoryAccepted {
        revision: WorkbenchRevision,
        directory_id: WorkbenchDirectoryId,
        worker_id: WorkerId,
    },
    Renamed {
        revision: WorkbenchRevision,
    },
    Unchanged {
        revision: WorkbenchRevision,
    },
    TabMoved {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
    TabCloseAccepted {
        close_request_id: WorkbenchRequestId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        accepted_revision: WorkbenchRevision,
    },
    TabCreatePending {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
    },
    TabCreated {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
    },
    PanelSplitPending {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        new_panel_id: WorkbenchPanelId,
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        presentation_id: TerminalPresentationId,
    },
    PanelSplitCreated {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        new_panel_id: WorkbenchPanelId,
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
    },
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied {},
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchCommandResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchCommandOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkbenchSnapshotOutcome {
    Available {
        #[serde(with = "snapshot_document")]
        snapshot: Snapshot,
    },
    Denied {},
    Unavailable {
        reason: WorkbenchSnapshotUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchSnapshotResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchSnapshotOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WorkbenchResponse {
    #[serde(rename = "workbench.snapshot.result")]
    Snapshot(WorkbenchSnapshotResult),
    #[serde(rename = "workbench.command.result")]
    Command(WorkbenchCommandResult),
    #[serde(rename = "workbench.tab-close.result")]
    TabCloseRead(WorkbenchTabCloseReadResult),
    #[serde(rename = "workbench.tab-close.resume.result")]
    TabCloseResume(WorkbenchTabCloseResumeResult),
    #[serde(rename = "workbench.publication.result")]
    Publication(WorkbenchPublicationResult),
}

pub fn parse_workbench_request(encoded: &[u8]) -> Result<WorkbenchRequest, ContractError> {
    enforce_encoded_limit(encoded, MAXIMUM_ENCODED_INTENT_BYTES)?;
    serde_json::from_slice(encoded).map_err(|_error| ContractError::InvalidDocument)
}

pub fn serialize_workbench_request(request: &WorkbenchRequest) -> Result<Vec<u8>, ContractError> {
    let encoded = serde_json::to_vec(request).map_err(|_error| ContractError::InvalidDocument)?;
    enforce_encoded_limit(&encoded, MAXIMUM_ENCODED_INTENT_BYTES)?;
    Ok(encoded)
}

pub fn parse_workbench_response(encoded: &[u8]) -> Result<WorkbenchResponse, ContractError> {
    enforce_encoded_limit(encoded, MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES)?;
    let response =
        serde_json::from_slice(encoded).map_err(|_error| ContractError::InvalidDocument)?;
    validate_response(&response, encoded)?;
    Ok(response)
}

pub fn serialize_workbench_response(
    response: &WorkbenchResponse,
) -> Result<Vec<u8>, ContractError> {
    let encoded = serde_json::to_vec(response).map_err(|_error| ContractError::InvalidDocument)?;
    validate_response(response, &encoded)?;
    Ok(encoded)
}

fn validate_response(response: &WorkbenchResponse, encoded: &[u8]) -> Result<(), ContractError> {
    match response {
        WorkbenchResponse::Command(_)
        | WorkbenchResponse::TabCloseRead(_)
        | WorkbenchResponse::TabCloseResume(_) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_INTENT_BYTES)
        }
        WorkbenchResponse::Snapshot(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES)?;
            if let WorkbenchSnapshotOutcome::Available { snapshot } = &result.outcome {
                validate_snapshot(snapshot)?;
                let document = WorkbenchProtocolDocument::Snapshot(snapshot.clone());
                crate::validation::serialize_document(&document)?;
            }
            Ok(())
        }
        WorkbenchResponse::Publication(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES)?;
            if let WorkbenchPublicationOutcome::Published { publications } = &result.outcome {
                if publications.is_empty() || publications.len() > MAXIMUM_RESULT_PUBLICATIONS {
                    return Err(ContractError::InvalidDocument);
                }
                for publication in publications {
                    let document = match publication {
                        WorkbenchPublicationDocument::Snapshot(snapshot) => {
                            validate_snapshot(snapshot)?;
                            WorkbenchProtocolDocument::Snapshot(snapshot.clone())
                        }
                        WorkbenchPublicationDocument::Delta(delta) => {
                            validate_delta_structure(delta)?;
                            WorkbenchProtocolDocument::Delta(delta.clone())
                        }
                    };
                    crate::validation::serialize_document(&document)?;
                }
            }
            Ok(())
        }
    }
}

mod snapshot_document {
    use super::{
        Deserialize, Deserializer, Serialize, Serializer, Snapshot, WorkbenchProtocolDocument, de,
    };

    pub(super) fn serialize<S: Serializer>(
        snapshot: &Snapshot,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        WorkbenchProtocolDocument::Snapshot(snapshot.clone()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Snapshot, D::Error> {
        match WorkbenchProtocolDocument::deserialize(deserializer)? {
            WorkbenchProtocolDocument::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(de::Error::custom("expected a complete Workbench snapshot")),
        }
    }
}

#[cfg(test)]
mod tests;
