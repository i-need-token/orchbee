mod configured_command;
mod events;
pub use events::*;
mod graphics;
pub use graphics::{
    MAXIMUM_GRAPHICS_CHUNK_BYTES, MAXIMUM_GRAPHICS_DECODED_BYTES, MAXIMUM_GRAPHICS_LAYERS,
    MAXIMUM_GRAPHICS_LAYERS_PER_PANE, MAXIMUM_GRAPHICS_REQUEST_BYTES,
    MAXIMUM_GRAPHICS_RESIDENT_BYTES, MAXIMUM_GRAPHICS_RESULT_BYTES, MAXIMUM_GRAPHICS_SET_BYTES,
    MAXIMUM_GRAPHICS_STREAM_BYTES, PaneGraphicsAction, PaneGraphicsCapabilities,
    PaneGraphicsCellPixels, PaneGraphicsClientObservation, PaneGraphicsClientPane,
    PaneGraphicsDamage, PaneGraphicsFailure, PaneGraphicsFormat, PaneGraphicsFrame,
    PaneGraphicsLayer, PaneGraphicsOutcome, PaneGraphicsPlacement, PaneGraphicsRequest,
    PaneGraphicsResult, PaneGraphicsStreamImage, PaneGraphicsTarget, PaneGraphicsTransport,
    graphics_base64_bytes, graphics_layer_id, graphics_rgba_bytes,
};
mod terminal_editor;
pub use configured_command::*;
pub use terminal_editor::{
    EditorPopupCapture, MAXIMUM_TERMINAL_EDITOR_BYTES, ScrollbackEditorSpec, TerminalEditorAction,
    TerminalEditorFailure, TerminalEditorOutcome, TerminalEditorRequest, TerminalEditorResult,
};
mod plugin_popup;
pub use plugin_popup::{ClientPopupSource, PluginPopupSize, PluginPopupSpec};
mod plugins;
pub use plugins::{
    MAXIMUM_ENCODED_PLUGIN_REQUEST_BYTES, MAXIMUM_ENCODED_PLUGIN_RESULT_BYTES, PluginAction,
    PluginCommandFailure, PluginCommandLog, PluginFailure, PluginInvocationContext,
    PluginLinkCatalogGuard, PluginLogCursor, PluginOutcome, PluginPaneInfo, PluginRequest,
    PluginResult, PluginWorker, PluginWorktreeContext, plugin_identifier, plugin_job_identifier,
};
mod window_title;
pub use window_title::{
    WindowTitle, WindowTitleAction, WindowTitleContext, WindowTitleFailure, WindowTitleOutcome,
    WindowTitleReason, WindowTitleRequest, WindowTitleResult, WindowTitleSnapshot,
};
pub(crate) mod notification;
use notification::*;
pub(crate) mod configuration;
use configuration::*;
pub(crate) mod settings;
use settings::*;
pub(crate) mod agent_manifest_update;
pub(crate) mod agent_manifests;
use agent_manifests::*;
mod panel_agent;
pub use panel_agent::{
    AgentDetectionEvaluation, AgentDetectionEvidence, AgentDetectionManifest, AgentDetectionRule,
    AgentDetectionSample, AgentManifestSource, AgentManifestWarning, AgentManifestWarningKind,
    AgentScreenExplanation, MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES,
    MAXIMUM_PANEL_AGENT_READ_ROWS, MAXIMUM_TERMINAL_AGENT_CHANGES,
    MAXIMUM_TERMINAL_AGENT_STATE_ROWS, MAXIMUM_TERMINAL_TITLE_BYTES, MAXIMUM_TERMINAL_TITLE_ROWS,
    PanelAgentAction, PanelAgentDisposition, PanelAgentFailure, PanelAgentOutcome,
    PanelAgentReference, PanelAgentReport, PanelAgentRequest, PanelAgentResult, PanelAgentRow,
    PanelAgentSession, PanelAgentState, PanelAgentView, PresentationAgentRow, TerminalAgentChange,
    TerminalAgentExplanation, TerminalAgentObservation, TerminalAgentStateRow,
    TerminalAgentStateSnapshot, TerminalTitleObservation, TerminalTitleRow,
};
mod panel_metadata;
pub use panel_metadata::{
    MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES, MAXIMUM_PANEL_METADATA_READ_ROWS,
    MAXIMUM_PANEL_METADATA_SOURCES, MetadataState, PanelMetadataAction, PanelMetadataEffective,
    PanelMetadataFailure, PanelMetadataField, PanelMetadataOutcome, PanelMetadataPatch,
    PanelMetadataPresentation, PanelMetadataRequest, PanelMetadataResult, PanelMetadataRow,
    PresentationMetadataRow,
};
mod client_navigation;
mod client_presentation;
mod client_viewport;
pub use client_navigation::{
    ClientContextSelection, ClientNavigationSnapshot, MAXIMUM_CLIENT_SELECTION_CONTEXTS,
    MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES,
};
pub use client_presentation::{
    ClientPresentationSelection, ClientPresentationSnapshot,
    MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES,
};
pub use client_viewport::{
    ClientPanelViewport, ClientScrollMetrics, ClientViewportMode, ClientViewportObservation,
    ClientViewportPending, ClientViewportSnapshot, ClientViewportSource, ClientViewportUnavailable,
    MAXIMUM_CLIENT_VIEWPORT_PANELS, MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES,
};
mod client_attention;
pub use client_attention::{
    ClientAgentAttentionEntry, ClientAgentAttentionSnapshot, ClientAgentAttentionVersion,
    ClientTerminalAgentAttentionEntry, ClientTerminalAgentAttentionSnapshot,
    ClientTerminalAgentAttentionVersion, MAXIMUM_CLIENT_ATTENTION_SESSIONS,
    MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES,
};
mod client_layout;
mod client_layout_adjustment;
pub use client_layout::{
    ClientCellRect, ClientPanelArea, ClientPanelDirection, ClientPanelLayout, ClientPanelNeighbors,
    ClientSplitArea, ClientSplitRatio, MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES,
};
pub use client_layout_adjustment::{
    ClientLayoutAdjustment, ClientLayoutChange, ClientResizeAmount, ClientSplitChange,
    ClientSplitIdentity, ClientSplitPath,
};
mod client_view;
pub use client_view::{
    ClientOverlayMember, ClientViewAction, ClientViewAvailability, ClientViewCompletion,
    ClientViewCompletionOutcome, ClientViewControl, ClientViewControlReason,
    ClientViewControlStatus, ClientViewIntent, ClientViewObservation, ClientViewOperation,
    ClientViewOutcome, ClientViewPhase, ClientViewRecord, ClientViewSelection, ClientViewSequence,
    ClientViewUnavailable, MAXIMUM_CLIENT_VIEWS, MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES,
    WorkbenchClientViewRequest, WorkbenchClientViewResult,
};
mod workspace_group_close;
pub use workspace_group_close::{
    MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES, MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS,
    MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS, WorkbenchWorkspaceGroupCloseRequest,
    WorkbenchWorkspaceGroupCloseResult, WorkspaceGroupCloseAction, WorkspaceGroupCloseChild,
    WorkspaceGroupCloseFailure, WorkspaceGroupCloseMember, WorkspaceGroupCloseObservation,
    WorkspaceGroupCloseOutcome, WorkspaceGroupCloseStatus, WorkspaceGroupCloseUnavailable,
};
mod json_fields;
mod worktree_query;
pub use worktree_query::{
    MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES, WorkbenchWorktreeQuery,
    WorkbenchWorktreeQueryOutcome, WorkbenchWorktreeQueryResult, WorktreeQueryEntry,
    WorktreeQueryFailure, WorktreeQuerySource, WorktreeWorkspaceAssociation,
};
mod workspace_git;
mod workspace_metadata;
pub use workspace_git::{
    GitDivergence, MAXIMUM_WORKSPACE_GIT_BYTES, WorkspaceGitFailure, WorkspaceGitOutcome,
    WorkspaceGitRequest, WorkspaceGitResult,
};

mod worktree_operation;
use std::fmt;
pub use workspace_metadata::{
    MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES, MAXIMUM_METADATA_PATCH_KEYS,
    MAXIMUM_METADATA_TOKEN_KEYS, MAXIMUM_METADATA_TTL_MILLISECONDS,
    MAXIMUM_WORKSPACE_METADATA_READ_ROWS, MetadataReportDisposition, MetadataSource,
    MetadataTokenKey, MetadataTokenPatch, MetadataTokenValue, MetadataTokens,
    WorkspaceMetadataAction, WorkspaceMetadataFailure, WorkspaceMetadataOutcome,
    WorkspaceMetadataRequest, WorkspaceMetadataResult, WorkspaceMetadataRow,
};
pub use worktree_operation::{
    MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES, MAXIMUM_WORKTREE_OPERATION_ATTEMPTS,
    WorkbenchWorktreeAction, WorkbenchWorktreeOutcome, WorkbenchWorktreeRequest,
    WorkbenchWorktreeResult, WorktreeAdmissionFailure, WorktreeCheckoutFacts, WorktreeGitEffect,
    WorktreeGitError, WorktreeGitStage, WorktreeIntent, WorktreeOperationFailure,
    WorktreeOperationPhase, WorktreeOperationStatus, WorktreeProgress, WorktreeReference,
    WorktreeSelector, WorktreeShellRestoration, WorktreeSource, WorktreeWorkspaceDisposition,
};

mod publication;
pub use publication::{
    MAXIMUM_RESULT_PUBLICATIONS, PublicationReadIntent, WorkbenchPublicationOutcome,
    WorkbenchPublicationResult,
};
mod launch_profile;
mod tab_close;
mod workspace_close;
mod worktree_list;
pub use launch_profile::{
    LaunchProfileCatalogOutcome, LaunchProfileCatalogRevision, LaunchProfileEntry, LaunchProfileId,
    LaunchProfileName, LaunchProfileSelector, MAXIMUM_ENCODED_LAUNCH_PROFILE_CATALOG_RESULT_BYTES,
    MAXIMUM_LAUNCH_PROFILE_CANDIDATES, MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES,
    MAXIMUM_LAUNCH_PROFILE_ENTRIES, WorkbenchLaunchProfileCatalogRead,
    WorkbenchLaunchProfileCatalogResult,
};
pub use tab_close::{
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason,
};
pub use workspace_close::{
    MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES, WorkbenchWorkspaceCloseCounts,
    WorkbenchWorkspaceCloseMember, WorkbenchWorkspaceCloseMemberStatus,
    WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseResumeResult,
};
pub use worktree_list::{
    MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES, WorkbenchWorktreeListOutcome,
    WorkbenchWorktreeListRead, WorkbenchWorktreeListResult, WorktreeEntry, WorktreeEntryText,
    WorktreeListStage, WorktreeListTooLargeOrigin, WorktreeListUnavailableReason,
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

/// Worst-case escaped cwd, argv, label and environment for native launch commands.
pub const MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES: usize = 96 * 1024;

pub const MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES: usize = MAXIMUM_ENCODED_PROJECTION_BYTES + 1024;
const MAXIMUM_BINARY64_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAXIMUM_TERMINAL_GEOMETRY_DIMENSION: u16 = 4096;
const MAXIMUM_TERMINAL_GRID_CELLS: u32 = 262_144;
const MAXIMUM_TERMINAL_PIXEL_EXTENT: u32 = 65_535;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkbenchLabel(String);

/// Group names may be empty, matching an unnamed GUI group; other labels remain nonempty.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkbenchTabGroupLabel(String);

impl WorkbenchTabGroupLabel {
    /// # Errors
    /// Rejects names longer than the shared Unicode scalar bound.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.chars().count() > MAXIMUM_LABEL_LENGTH {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for WorkbenchTabGroupLabel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

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

/// Maximum argument values one explicit terminal launch selection carries.
/// The wire `arguments` list is argv[1..] — the Host synthesizes argv[0]
/// from the executable; argv0 is never transmitted.
pub const MAXIMUM_LAUNCH_SELECTION_ARGUMENTS: usize = 128;
/// Maximum combined UTF-8 bytes of the executable path plus every argument
/// value in one explicit launch selection. The same budget also bounds worst-case
/// JSON string contents: controls cost six bytes, quotes/backslashes two, others one.
pub const MAXIMUM_LAUNCH_SELECTION_BYTES: usize = 30 * 1024;

/// An explicit terminal launch program: one absolute executable path and its
/// verbatim argument list. Construction and decode run the same closed
/// lexical contract — a non-empty absolute NUL-free executable, NUL-free
/// arguments, at most the argument count, and the aggregate byte bound —
/// so a malformed wire selection is `InvalidDocument` before any effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSelection {
    executable: String,
    arguments: Vec<String>,
}

impl LaunchSelection {
    pub fn new(executable: String, arguments: Vec<String>) -> Result<Self, ContractError> {
        if executable.is_empty()
            || !crate::directory::is_absolute_path(executable.as_bytes())
            || executable.as_bytes().contains(&0)
        {
            return Err(ContractError::InvalidDocument);
        }
        if arguments.len() > MAXIMUM_LAUNCH_SELECTION_ARGUMENTS {
            return Err(ContractError::InvalidDocument);
        }
        let mut bytes = executable.len();
        for argument in &arguments {
            if argument.as_bytes().contains(&0) {
                return Err(ContractError::InvalidDocument);
            }
            bytes = bytes
                .checked_add(argument.len())
                .ok_or(ContractError::InvalidDocument)?;
        }
        if bytes > MAXIMUM_LAUNCH_SELECTION_BYTES {
            return Err(ContractError::InvalidDocument);
        }
        let encoded = std::iter::once(executable.as_str())
            .chain(arguments.iter().map(String::as_str))
            .flat_map(str::bytes)
            .map(|b| match b {
                0..=31 => 6,
                b'"' | b'\\' => 2,
                _ => 1,
            })
            .sum::<usize>();
        if encoded > MAXIMUM_LAUNCH_SELECTION_BYTES {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self {
            executable,
            arguments,
        })
    }

    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchSelectionRef<'a> {
    exec: &'a str,
    args: &'a [String],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LaunchSelectionOwned {
    exec: String,
    args: Vec<String>,
}

impl Serialize for LaunchSelection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LaunchSelectionRef {
            exec: &self.executable,
            args: &self.arguments,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LaunchSelection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = LaunchSelectionOwned::deserialize(deserializer)?;
        Self::new(wire.exec, wire.args).map_err(de::Error::custom)
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
        || value.cell_pixel_width > MAXIMUM_TERMINAL_GEOMETRY_DIMENSION
        || value.cell_pixel_height > MAXIMUM_TERMINAL_GEOMETRY_DIMENSION
    {
        return Err("terminal geometry dimension is outside its protocol bounds");
    }
    if (value.cell_pixel_width == 0) != (value.cell_pixel_height == 0) {
        return Err("terminal geometry mixes known and unknown cell pixel dimensions");
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
pub(super) struct Binary64SafeInteger(pub(super) u64);

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
    /// Remove organization only, retaining every Workspace and terminal resource.
    #[serde(rename = "project.dissolve")]
    ProjectDissolve { project_id: WorkbenchProjectId },
    #[serde(rename = "workspace.create")]
    WorkspaceCreate {
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
        label: WorkbenchLabel,
    },
    /// Atomically register a directory Workspace and its first terminal. The
    /// required cwd belongs to the selected Worker and is probed before commit.
    #[serde(rename = "workspace.launch")]
    WorkspaceLaunch {
        placement: crate::WorkspacePlacement,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch: Option<LaunchSelection>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<LaunchProfileSelector>,
        options: Box<crate::LaunchOptions>,
    },
    #[serde(rename = "workspace.rename")]
    WorkspaceRename {
        workspace_id: WorkbenchWorkspaceId,
        label: WorkbenchLabel,
    },
    /// Change only optional Project membership, preserving display order and resources.
    #[serde(rename = "workspace.set-project")]
    WorkspaceSetProject {
        workspace_id: WorkbenchWorkspaceId,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
    },
    /// Create a Project and move exactly one existing Workspace in one commit.
    #[serde(rename = "workspace.create-project")]
    WorkspaceCreateProject {
        workspace_id: WorkbenchWorkspaceId,
        label: WorkbenchLabel,
    },
    #[serde(rename = "workspace.move")]
    WorkspaceMove {
        workspace_id: WorkbenchWorkspaceId,
        insert_index: crate::WorkbenchWorkspaceInsertIndex,
    },
    #[serde(rename = "workspace.move-block")]
    WorkspaceMoveBlock {
        workspace_ids: crate::WorkbenchWorkspaceList,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before_workspace_id: Option<WorkbenchWorkspaceId>,
    },
    /// Close one Workspace semantically: every captured terminal leaf member
    /// runs the retained Close lifecycle, then one authoritative commit
    /// removes the Workspace and its TerminalContext together. The Project,
    /// global Directory registry and Git checkout remain unchanged.
    #[serde(rename = "workspace.close")]
    WorkspaceClose { workspace_id: WorkbenchWorkspaceId },
    #[serde(rename = "workspace.primary-directory.accept")]
    WorkspacePrimaryDirectoryAccept {
        workspace_id: WorkbenchWorkspaceId,
        path: WorkbenchDirectoryPath,
    },
    #[serde(rename = "tab.rename")]
    TabRename {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        label: WorkbenchLabel,
    },
    /// Atomic metadata/order configuration; terminal and presentation identities are preserved.
    #[serde(rename = "tabs.configure")]
    TabsConfigure {
        terminal_context_id: WorkbenchTerminalContextId,
        operation: crate::TabConfiguration,
    },
    #[serde(rename = "panel.rename")]
    PanelRename {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        #[serde(default)]
        label: crate::PanelLabelUpdate,
    },
    #[serde(rename = "panel.input.set")]
    PanelInputSet {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        right_click: crate::PanelRightClickTarget,
    },
    #[serde(rename = "layout.set-split-ratio")]
    SplitRatioSet {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        split: ClientSplitIdentity,
        previous_ratio: crate::WorkbenchSplitRatio,
        ratio: crate::WorkbenchSplitRatio,
    },
    #[serde(rename = "tab.move")]
    TabMove {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        direction: WorkbenchTabMoveDirection,
    },
    #[serde(rename = "tab.move-index")]
    TabMoveIndex {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        insert_index: WorkbenchTabInsertIndex,
    },
    #[serde(rename = "tab.close")]
    TabClose {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
    #[serde(rename = "tab.create-layout")]
    TabCreateLayout {
        terminal_context_id: WorkbenchTerminalContextId,
        label: WorkbenchLabel,
        automatic_label: bool,
        layout: crate::WorkbenchLayoutTemplate,
    },
    #[serde(rename = "tab.create")]
    TabCreate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placement: Option<crate::TabCreationPlacement>,
        #[serde(default, skip_serializing_if = "crate::model::is_false")]
        automatic_label: bool,
        terminal_context_id: WorkbenchTerminalContextId,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
        /// An explicit launch program; absent runs the resolved default
        /// shell. Serialized only when present — a default launch emits
        /// no key rather than a null placeholder. Mutually exclusive
        /// with `profile`; the pair is rejected at request decode.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch: Option<LaunchSelection>,
        /// A Launch Profile selector resolved Server-side into the same
        /// explicit launch selection; mutually exclusive with `launch`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<LaunchProfileSelector>,
        #[serde(default, skip_serializing_if = "crate::LaunchOptions::is_empty")]
        options: Box<crate::LaunchOptions>,
    },
    #[serde(rename = "panel.split")]
    PanelSplit {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
        #[serde(
            default,
            skip_serializing_if = "crate::PanelRightClickTarget::is_default"
        )]
        right_click: crate::PanelRightClickTarget,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
        /// An explicit launch program; absent runs the resolved default
        /// shell. Serialized only when present — a default launch emits
        /// no key rather than a null placeholder. Mutually exclusive
        /// with `profile`; the pair is rejected at request decode.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch: Option<LaunchSelection>,
        /// A Launch Profile selector resolved Server-side into the same
        /// explicit launch selection; mutually exclusive with `launch`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<LaunchProfileSelector>,
        #[serde(default, skip_serializing_if = "crate::LaunchOptions::is_empty")]
        options: Box<crate::LaunchOptions>,
    },
    #[serde(rename = "panel.create-terminal")]
    PanelCreateTerminal {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target: WorkbenchPanelTerminalTarget,
        geometry: WorkbenchTerminalGeometry,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch: Option<LaunchSelection>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<LaunchProfileSelector>,
        #[serde(default, skip_serializing_if = "crate::LaunchOptions::is_empty")]
        options: Box<crate::LaunchOptions>,
    },
    #[serde(rename = "panel.close")]
    PanelClose {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    },
    #[serde(rename = "panel.move")]
    PanelMove {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target_panel_id: WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
    },
    /// Exchange complete leaf nodes without changing their existing Split/slot structure.
    #[serde(rename = "panel.swap")]
    PanelSwap {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target_panel_id: WorkbenchPanelId,
    },
    /// Relocate one leaf Panel into an existing or new Tab/Workspace. The leaf
    /// is retained whole — Panel, Presentation, and terminal binding
    /// identities move unchanged.
    #[serde(rename = "panel.relocate")]
    PanelRelocate {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        destination: WorkbenchPanelRelocateDestination,
    },
    /// Create one ephemeral per-Site Launch Profile catalog entry. The
    /// selection passes the same closed lexical contract as an explicit
    /// `launch`, so a stored profile is always launchable.
    #[serde(rename = "launch-profile.create")]
    LaunchProfileCreate {
        name: LaunchProfileName,
        launch: LaunchSelection,
    },
    /// Delete one catalog entry by selector: exact profile id first, then
    /// name cardinality (not-found/single/ambiguous).
    #[serde(rename = "launch-profile.delete")]
    LaunchProfileDelete { selector: LaunchProfileSelector },
}

/// Structural destination for a retained Panel. An omitted context uses the
/// source context; an omitted target Panel grafts at the destination Tab root.
/// New Workspaces have optional Project membership and global Directory references.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchPanelRelocateDestination {
    #[serde(rename = "tab")]
    Tab {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        terminal_context_id: Option<WorkbenchTerminalContextId>,
        tab_id: WorkbenchTabId,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        target_panel_id: Option<WorkbenchPanelId>,
        direction: WorkbenchPanelSplitDirection,
    },
    #[serde(rename = "new-tab")]
    NewTab {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        terminal_context_id: Option<WorkbenchTerminalContextId>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        label: Option<WorkbenchLabel>,
    },
    NewWorkspace {
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        primary_directory_id: Option<WorkbenchDirectoryId>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        primary_directory_path: Option<WorkbenchDirectoryPath>,
        label: WorkbenchLabel,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        tab_label: Option<WorkbenchLabel>,
    },
}

/// Additional effects when relocation crosses a context or removes its source
/// Tab. The original terminal and Presentation identities are retained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchPanelTransfer {
    pub target_terminal_context_id: WorkbenchTerminalContextId,
    pub closed_source_tab: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub closed_source_workspace_id: Option<WorkbenchWorkspaceId>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub created_workspace_id: Option<WorkbenchWorkspaceId>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub created_directory_id: Option<WorkbenchDirectoryId>,
}

fn deserialize_present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// A zero-based insertion boundary in the original Tab order. The syntactic
/// bound admits the end of a maximum-size context; the authority checks its
/// current Tab count after exact context and target admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchTabInsertIndex(usize);

impl WorkbenchTabInsertIndex {
    pub fn new(value: usize) -> Result<Self, ContractError> {
        if value > crate::MAXIMUM_TABS_PER_CONTEXT {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

impl Serialize for WorkbenchTabInsertIndex {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0 as u64)
    }
}

impl<'de> Deserialize<'de> for WorkbenchTabInsertIndex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Binary64SafeInteger::deserialize(deserializer)?;
        let value = usize::try_from(value.0).map_err(de::Error::custom)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkbenchTabMoveDirection {
    Previous,
    Next,
    First,
    Last,
}

/// A new child or an exact existing unbound member; never a new layout Panel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchPanelTerminalTarget {
    Child {
        label: WorkbenchLabel,
    },
    Unbound {
        presentation_id: TerminalPresentationId,
    },
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
    #[serde(rename = "workbench.pane-graphics")]
    PaneGraphics(PaneGraphicsRequest),
    #[serde(rename = "workbench.events")]
    Events(SemanticEventsRequest),
    #[serde(rename = "workbench.configured-command")]
    ConfiguredCommand(ConfiguredCommandRequest),
    #[serde(rename = "agent.view")]
    AgentView(crate::AgentViewRequest),
    #[serde(rename = "workbench.terminal-editor")]
    TerminalEditor(TerminalEditorRequest),
    #[serde(rename = "workbench.plugin")]
    Plugin(PluginRequest),
    #[serde(rename = "workbench.configuration")]
    Configuration(ConfigurationRequest),
    #[serde(rename = "workbench.settings")]
    Settings(NativeSettingsRequest),
    #[serde(rename = "workbench.notification")]
    Notification(NotificationRequest),
    #[serde(rename = "workbench.window-title")]
    WindowTitle(WindowTitleRequest),
    #[serde(rename = "workbench.agent-manifests")]
    AgentManifests(AgentManifestsRequest),
    #[serde(rename = "workbench.panel-agent")]
    PanelAgent(PanelAgentRequest),
    #[serde(rename = "workbench.panel-metadata")]
    PanelMetadata(PanelMetadataRequest),
    #[serde(rename = "workbench.workspace-metadata")]
    WorkspaceMetadata(WorkspaceMetadataRequest),
    #[serde(rename = "workbench.workspace-git")]
    WorkspaceGit(WorkspaceGitRequest),
    #[serde(rename = "workbench.client-view")]
    ClientView(WorkbenchClientViewRequest),
    #[serde(rename = "workbench.workspace-group-close")]
    WorkspaceGroupClose(WorkbenchWorkspaceGroupCloseRequest),
    #[serde(rename = "workbench.worktree.query")]
    WorktreeQuery(WorkbenchWorktreeQuery),
    #[serde(rename = "workbench.worktree.operation")]
    WorktreeOperation(WorkbenchWorktreeRequest),
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
    #[serde(rename = "workbench.worktree-list.read")]
    WorktreeListRead(WorkbenchWorktreeListRead),
    #[serde(rename = "workbench.launch-profile-catalog.read")]
    LaunchProfileCatalogRead(WorkbenchLaunchProfileCatalogRead),
    #[serde(rename = "workbench.workspace-close.read")]
    WorkspaceCloseRead(WorkbenchWorkspaceCloseRead),
    #[serde(rename = "workbench.workspace-close.resume")]
    WorkspaceCloseResume(WorkbenchWorkspaceCloseResume),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchRejectionReason {
    SiteMismatch,
    RevisionMismatch,
    ProjectUnavailable,
    WorkspaceUnavailable,
    WorkspaceMoveIndexOutOfBounds,
    WorkspaceMoveTargetInBlock,
    WorkspaceGroupCloseRequired,
    WorkspaceDirectoryUnavailable,
    WorkspaceDirectoryConflict,
    TerminalContextUnavailable,
    TabUnavailable,
    TabGroupUnavailable,
    TabMoveIndexOutOfBounds,
    /// Exact placement would break the pinned prefix or a contiguous Tab group.
    TabMoveOrderConflict,
    PanelUnavailable,
    SplitUnavailable,
    ResourceLimit,
    IdentityExhausted,
    RevisionExhausted,
    RequestIdentityConflict,
    /// Legacy stacked-Presentation refusal retained for earlier peers. Current
    /// relocation preserves every child as part of its complete Panel.
    PanelNotLeaf,
    /// Legacy last-leaf refusal retained for decoding earlier peers. Current
    /// relocation removes the empty source Tab without disposing the terminal.
    LastPanelInTab,
    /// The relocation destination resolved to the source Tab itself.
    SameTab,
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
    /// A committed configuration, with a new identity only for group creation.
    TabsConfigured {
        revision: WorkbenchRevision,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        group_id: Option<crate::WorkbenchTabGroupId>,
    },
    ProjectCreated {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
    },
    ProjectDissolved {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
    },
    WorkspaceProjectChanged {
        revision: WorkbenchRevision,
        workspace_id: WorkbenchWorkspaceId,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
    },
    WorkspaceCreated {
        revision: WorkbenchRevision,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
    WorkspacesReordered {
        revision: WorkbenchRevision,
        workspace_order: crate::WorkbenchWorkspaceList,
    },
    WorkspacePrimaryDirectoryAccepted {
        revision: WorkbenchRevision,
        directory_id: WorkbenchDirectoryId,
        worker_id: WorkerId,
    },
    Renamed {
        revision: WorkbenchRevision,
    },
    PanelInputConfigured {
        revision: WorkbenchRevision,
    },
    SplitRatioConfigured {
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
    /// The Workspace Close was retained: `close_request_id` names the
    /// operation for `workspace-close.read`/`resume`, and the echoed scope
    /// triple is the join identity later requests replay or join against.
    WorkspaceCloseAccepted {
        close_request_id: WorkbenchRequestId,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
        accepted_revision: WorkbenchRevision,
    },
    WorkspaceLaunchPending {
        revision: WorkbenchRevision,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
        workspace_id: WorkbenchWorkspaceId,
        directory_id: WorkbenchDirectoryId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
    },
    WorkspaceLaunched {
        revision: WorkbenchRevision,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
        workspace_id: WorkbenchWorkspaceId,
        directory_id: WorkbenchDirectoryId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
    },
    TabLayoutCreated {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
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
    PanelTerminalPending {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
    },
    PanelTerminalCreated {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
    },
    PanelCloseAccepted {
        close_request_id: WorkbenchRequestId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        accepted_revision: WorkbenchRevision,
    },
    PanelMoved {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target_panel_id: WorkbenchPanelId,
    },
    PanelSwapped {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target_panel_id: WorkbenchPanelId,
    },
    /// The leaf relocated into an existing Tab: `tab_id` names the source,
    /// `target_tab_id` names the destination, and fresh graft identities are
    /// echoed. Transfer facts describe context changes or source removal.
    PanelRelocated {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        target_tab_id: WorkbenchTabId,
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        transfer: Option<WorkbenchPanelTransfer>,
    },
    /// The leaf relocated into a freshly appended Tab: `new_tab_id` is the
    /// minted Tab identity; the moved leaf is that Tab's root.
    PanelRelocatedToNewTab {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        new_tab_id: WorkbenchTabId,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        transfer: Option<WorkbenchPanelTransfer>,
    },
    /// One catalog entry committed; `revision` is the new Workbench revision
    /// (every committed command advances it) and `catalog_revision` is the
    /// new catalog-private counter.
    LaunchProfileCreated {
        revision: WorkbenchRevision,
        catalog_revision: LaunchProfileCatalogRevision,
        profile_id: LaunchProfileId,
    },
    LaunchProfileDeleted {
        revision: WorkbenchRevision,
        catalog_revision: LaunchProfileCatalogRevision,
        profile_id: LaunchProfileId,
    },
    /// The selector resolved to nothing at this Server resolution — a
    /// legitimate settled outcome journaled like every other result.
    LaunchProfileUnavailable {},
    /// The selector's name cardinality exceeded one; the bounded candidate
    /// id list (at most the catalog cap) is captured at this same resolution.
    LaunchProfileAmbiguous {
        candidate_ids: Vec<LaunchProfileId>,
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
    #[serde(rename = "workbench.pane-graphics.result")]
    PaneGraphics(PaneGraphicsResult),
    #[serde(rename = "workbench.events.result")]
    Events(SemanticEventsResult),
    #[serde(rename = "workbench.configured-command.result")]
    ConfiguredCommand(ConfiguredCommandResult),
    #[serde(rename = "agent.view.result")]
    AgentView(crate::AgentViewResult),
    #[serde(rename = "workbench.terminal-editor.result")]
    TerminalEditor(TerminalEditorResult),
    #[serde(rename = "workbench.plugin.result")]
    Plugin(PluginResult),
    #[serde(rename = "workbench.configuration.result")]
    Configuration(ConfigurationResult),
    #[serde(rename = "workbench.settings.result")]
    Settings(NativeSettingsResult),
    #[serde(rename = "workbench.notification.result")]
    Notification(NotificationResult),
    #[serde(rename = "workbench.window-title.result")]
    WindowTitle(WindowTitleResult),
    #[serde(rename = "workbench.agent-manifests.result")]
    AgentManifests(AgentManifestsResult),
    #[serde(rename = "workbench.panel-agent.result")]
    PanelAgent(PanelAgentResult),
    #[serde(rename = "workbench.panel-metadata.result")]
    PanelMetadata(PanelMetadataResult),
    #[serde(rename = "workbench.workspace-metadata.result")]
    WorkspaceMetadata(WorkspaceMetadataResult),
    #[serde(rename = "workbench.workspace-git.result")]
    WorkspaceGit(WorkspaceGitResult),
    #[serde(rename = "workbench.client-view.result")]
    ClientView(WorkbenchClientViewResult),
    #[serde(rename = "workbench.workspace-group-close.result")]
    WorkspaceGroupClose(WorkbenchWorkspaceGroupCloseResult),
    #[serde(rename = "workbench.worktree.query.result")]
    WorktreeQuery(WorkbenchWorktreeQueryResult),
    #[serde(rename = "workbench.worktree.operation.result")]
    WorktreeOperation(WorkbenchWorktreeResult),
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
    #[serde(rename = "workbench.worktree-list.result")]
    WorktreeList(WorkbenchWorktreeListResult),
    #[serde(rename = "workbench.launch-profile-catalog.result")]
    LaunchProfileCatalog(WorkbenchLaunchProfileCatalogResult),
    #[serde(rename = "workbench.workspace-close.result")]
    WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult),
    #[serde(rename = "workbench.workspace-close.resume.result")]
    WorkspaceCloseResume(WorkbenchWorkspaceCloseResumeResult),
}

pub fn parse_workbench_request(encoded: &[u8]) -> Result<WorkbenchRequest, ContractError> {
    enforce_encoded_limit(
        encoded,
        MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES
            .max(crate::MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES)
            .max(MAXIMUM_GRAPHICS_REQUEST_BYTES),
    )?;
    let request: WorkbenchRequest =
        serde_json::from_slice(encoded).map_err(|_error| ContractError::InvalidDocument)?;
    enforce_encoded_limit(encoded, request_byte_limit(&request))?;
    validate_request(&request)?;
    Ok(request)
}

/// Wire-level invariants serde cannot express inside the action enum: an
/// explicit `launch` and a `profile` selector are mutually exclusive on the
/// terminal-launch actions.
fn validate_request(request: &WorkbenchRequest) -> Result<(), ContractError> {
    if let WorkbenchRequest::Settings(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::PaneGraphics(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::Events(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::ConfiguredCommand(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::AgentView(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::TerminalEditor(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::Plugin(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::WindowTitle(request) = request {
        request.validate()?;
    }
    if let WorkbenchRequest::Notification(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::PanelAgent(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::PanelMetadata(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::WorkspaceMetadata(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::ClientView(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::WorkspaceGroupClose(request) = request {
        return request.validate();
    }
    if let WorkbenchRequest::WorktreeOperation(request) = request {
        return request.validate();
    }
    let WorkbenchRequest::Command(command) = request else {
        return Ok(());
    };
    if let WorkbenchAction::SplitRatioSet { split, .. } = &command.action {
        split.validate()?;
    }
    if matches!(
        &command.action,
        WorkbenchAction::PanelRelocate {
            destination: WorkbenchPanelRelocateDestination::NewWorkspace {
                primary_directory_id: Some(_),
                primary_directory_path: Some(_),
                ..
            },
            ..
        }
    ) {
        return Err(ContractError::InvalidDocument);
    }
    if let WorkbenchAction::WorkspaceLaunch { options, .. } = &command.action
        && options.cwd.is_none()
    {
        return Err(ContractError::InvalidDocument);
    }
    let conflicting = match &command.action {
        WorkbenchAction::TabCreate {
            launch,
            profile,
            options,
            ..
        }
        | WorkbenchAction::PanelSplit {
            launch,
            profile,
            options,
            ..
        } => {
            if let Some(selector) = &options.plugin_pane {
                crate::PluginPaneSelector::new(
                    selector.plugin_id.clone(),
                    selector.entrypoint_id.clone(),
                )
                .map_err(|_| ContractError::InvalidDocument)?;
            }
            if let Some(context) = &options.plugin_context {
                context.validate()?;
            }
            (launch.is_some() && profile.is_some())
                || (options.plugin_context.is_some() && options.plugin_pane.is_none())
                || (options.plugin_pane.is_some() && (launch.is_some() || profile.is_some()))
        }
        WorkbenchAction::WorkspaceLaunch {
            launch,
            profile,
            options,
            ..
        }
        | WorkbenchAction::PanelCreateTerminal {
            launch,
            profile,
            options,
            ..
        } => {
            options.plugin_pane.is_some()
                || options.plugin_context.is_some()
                || (launch.is_some() && profile.is_some())
        }
        _ => false,
    };
    if conflicting {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

fn request_byte_limit(request: &WorkbenchRequest) -> usize {
    if matches!(request, WorkbenchRequest::Settings(_)) {
        return MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES;
    }
    if matches!(request, WorkbenchRequest::PaneGraphics(_)) {
        return MAXIMUM_GRAPHICS_REQUEST_BYTES;
    }
    if matches!(request, WorkbenchRequest::Events(_)) {
        return MAXIMUM_EVENTS_BYTES;
    }
    if matches!(request, WorkbenchRequest::ConfiguredCommand(_)) {
        return MAXIMUM_CONFIGURED_COMMAND_BYTES;
    }
    if matches!(request, WorkbenchRequest::AgentView(_)) {
        return crate::MAXIMUM_AGENT_VIEW_BYTES;
    }
    if matches!(request, WorkbenchRequest::TerminalEditor(_)) {
        return MAXIMUM_TERMINAL_EDITOR_BYTES;
    }
    if matches!(request, WorkbenchRequest::Plugin(_)) {
        return MAXIMUM_ENCODED_PLUGIN_REQUEST_BYTES;
    }
    if matches!(request, WorkbenchRequest::Configuration(_)) {
        return MAXIMUM_CONFIGURATION_BYTES;
    }
    if matches!(
        request,
        WorkbenchRequest::Command(WorkbenchCommand {
            action: WorkbenchAction::TabCreateLayout { .. },
            ..
        })
    ) {
        return crate::MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES;
    }
    if matches!(
        request,
        WorkbenchRequest::Command(WorkbenchCommand {
            action: WorkbenchAction::WorkspaceMoveBlock { .. },
            ..
        })
    ) {
        return crate::MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES;
    }
    if matches!(request, WorkbenchRequest::ClientView(request) if request.carries_attention()) {
        return MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES;
    }
    if matches!(
        request,
        WorkbenchRequest::ClientView(WorkbenchClientViewRequest {
            action: ClientViewAction::Synchronize {
                completion: Some(ClientViewCompletion {
                    outcome: ClientViewCompletionOutcome::LayoutRead { .. },
                    ..
                }),
                ..
            },
            ..
        })
    ) {
        return MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES;
    }

    if matches!(request, WorkbenchRequest::WorktreeOperation(_)) {
        return MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES;
    }
    if matches!(
        request,
        WorkbenchRequest::Command(WorkbenchCommand {
            action: WorkbenchAction::TabCreate { .. }
                | WorkbenchAction::PanelSplit { .. }
                | WorkbenchAction::PanelCreateTerminal { .. }
                | WorkbenchAction::WorkspaceLaunch { .. },
            ..
        })
    ) {
        MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES
    } else {
        MAXIMUM_ENCODED_INTENT_BYTES
    }
}

pub fn serialize_workbench_request(request: &WorkbenchRequest) -> Result<Vec<u8>, ContractError> {
    validate_request(request)?;
    let encoded = serde_json::to_vec(request).map_err(|_error| ContractError::InvalidDocument)?;
    enforce_encoded_limit(&encoded, request_byte_limit(request))?;
    Ok(encoded)
}

pub fn parse_workbench_response(encoded: &[u8]) -> Result<WorkbenchResponse, ContractError> {
    enforce_encoded_limit(
        encoded,
        MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES.max(MAXIMUM_ENCODED_PLUGIN_RESULT_BYTES),
    )?;
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
    if let WorkbenchResponse::Command(WorkbenchCommandResult {
        outcome: WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids },
        ..
    }) = response
        && candidate_ids.len() > MAXIMUM_LAUNCH_PROFILE_CANDIDATES
    {
        return Err(ContractError::InvalidDocument);
    }
    match response {
        WorkbenchResponse::PaneGraphics(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_GRAPHICS_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::Events(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_EVENTS_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::ConfiguredCommand(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_CONFIGURED_COMMAND_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::AgentView(result) => {
            enforce_encoded_limit(encoded, crate::MAXIMUM_AGENT_VIEW_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::TerminalEditor(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_TERMINAL_EDITOR_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::Plugin(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_PLUGIN_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::WindowTitle(result) => result.validate(),
        WorkbenchResponse::Notification(result) => result.validate(),
        WorkbenchResponse::Configuration(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_CONFIGURATION_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::Settings(result) => {
            enforce_encoded_limit(encoded, crate::MAXIMUM_USER_PREFERENCES_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::AgentManifests(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::PanelAgent(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::PanelMetadata(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::WorkspaceGit(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_WORKSPACE_GIT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::WorkspaceMetadata(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::ClientView(result) => {
            let maximum = if matches!(&result.outcome, ClientViewOutcome::Operation { operation }
                if matches!(operation.status, ClientViewControlStatus::LayoutRead { .. }))
            {
                MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES
            } else {
                MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES
            };
            enforce_encoded_limit(encoded, maximum)?;
            result.validate()
        }
        WorkbenchResponse::WorkspaceGroupClose(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::WorktreeQuery(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::WorktreeOperation(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::WorkspacesReordered { .. },
            ..
        }) => enforce_encoded_limit(encoded, crate::MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES),
        WorkbenchResponse::Command(_)
        | WorkbenchResponse::TabCloseRead(_)
        | WorkbenchResponse::TabCloseResume(_)
        | WorkbenchResponse::WorkspaceCloseResume(_) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_INTENT_BYTES)
        }
        WorkbenchResponse::WorkspaceCloseRead(result) => {
            // Dedicated variant cap; member structure is part of the contract.
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES)?;
            result.validate()
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
        WorkbenchResponse::WorktreeList(result) => {
            // The dedicated 32 KiB variant cap is enforced on parse and
            // serialize alike; entry structure is part of the contract.
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES)?;
            result.validate()
        }
        WorkbenchResponse::LaunchProfileCatalog(result) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_LAUNCH_PROFILE_CATALOG_RESULT_BYTES)?;
            result.validate()
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
