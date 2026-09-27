mod client_navigation;
pub use client_navigation::{
    ClientContextSelection, ClientNavigationSnapshot, MAXIMUM_CLIENT_SELECTION_CONTEXTS,
    MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES,
};
mod client_attention;
pub use client_attention::{
    ClientAgentAttentionEntry, ClientAgentAttentionSnapshot, ClientAgentAttentionVersion,
    MAXIMUM_CLIENT_ATTENTION_SESSIONS, MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES,
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
    ClientViewAction, ClientViewAvailability, ClientViewCompletion, ClientViewCompletionOutcome,
    ClientViewControl, ClientViewControlReason, ClientViewControlStatus, ClientViewIntent,
    ClientViewObservation, ClientViewOperation, ClientViewOutcome, ClientViewPhase,
    ClientViewRecord, ClientViewSelection, ClientViewSequence, ClientViewUnavailable,
    MAXIMUM_CLIENT_VIEWS, MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES, WorkbenchClientViewRequest,
    WorkbenchClientViewResult,
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
mod worktree_operation;
use std::fmt;
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
pub const MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES: usize = 48 * 1024;

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

/// Maximum argument values one explicit terminal launch selection carries.
/// The wire `arguments` list is argv[1..] — the Host synthesizes argv[0]
/// from the executable; argv0 is never transmitted.
pub const MAXIMUM_LAUNCH_SELECTION_ARGUMENTS: usize = 128;
/// Maximum combined UTF-8 bytes of the executable path plus every argument
/// value in one explicit launch selection.
pub const MAXIMUM_LAUNCH_SELECTION_BYTES: usize = 2048;

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
    #[serde(rename = "workspace.create")]
    WorkspaceCreate {
        project_id: WorkbenchProjectId,
        label: WorkbenchLabel,
    },
    /// Atomically register a directory Workspace and its first terminal. The
    /// required cwd belongs to the selected Worker and is probed before commit.
    #[serde(rename = "workspace.launch")]
    WorkspaceLaunch {
        project_id: WorkbenchProjectId,
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
        project_id: WorkbenchProjectId,
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
    /// its directories, the directory association, and the Git checkout are
    /// never touched.
    #[serde(rename = "workspace.close")]
    WorkspaceClose {
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
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
    #[serde(rename = "tab.create")]
    TabCreate {
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
/// New Workspaces retain an existing Project and optional directory ownership.
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
        project_id: WorkbenchProjectId,
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
    TabMoveIndexOutOfBounds,
    PanelUnavailable,
    ResourceLimit,
    IdentityExhausted,
    RevisionExhausted,
    RequestIdentityConflict,
    /// The named Panel exists but carries stacked child Presentations; only a
    /// leaf Panel relocates.
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
    ProjectCreated {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
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
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
        accepted_revision: WorkbenchRevision,
    },
    WorkspaceLaunchPending {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        directory_id: WorkbenchDirectoryId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
    },
    WorkspaceLaunched {
        revision: WorkbenchRevision,
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        directory_id: WorkbenchDirectoryId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
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
    enforce_encoded_limit(encoded, MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES)?;
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
        WorkbenchAction::WorkspaceLaunch {
            launch, profile, ..
        }
        | WorkbenchAction::TabCreate {
            launch, profile, ..
        }
        | WorkbenchAction::PanelSplit {
            launch, profile, ..
        } => launch.is_some() && profile.is_some(),
        _ => false,
    };
    if conflicting {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

fn request_byte_limit(request: &WorkbenchRequest) -> usize {
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
