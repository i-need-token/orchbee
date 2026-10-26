use serde::{Deserialize, Serialize};

use crate::directory::WorkbenchDirectoryPath;
use crate::identity::{
    ProtocolVersion, SessionId, TerminalId, TerminalPresentationId, WorkbenchDirectoryId,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchProjectId, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabGroupId, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotReadIntent {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationBinding {
    pub terminal_id: TerminalId,
    pub worker_id: WorkerId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Presentation {
    pub binding: Option<PresentationBinding>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub custom_label: Option<crate::PanelCustomLabel>,
    pub label: String,
    /// Runtime fact about the current binding, never persisted or carried across a rebinding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_failure: Option<PresentationLaunchFailure>,
    pub presentation_id: TerminalPresentationId,
}

/// The Server's committed fact that the terminal bound to a Presentation never started. It names
/// the launched executable only; arguments and environment never enter the projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationLaunchFailure {
    /// Null when the Worker resolved its default shell rather than an explicit executable.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub executable: Option<String>,
    pub reason: crate::command::WorkbenchTerminalFailureReason,
}

impl Presentation {
    #[must_use]
    pub fn display_label(&self) -> &str {
        self.custom_label
            .as_ref()
            .map_or(self.label.as_str(), crate::PanelCustomLabel::as_str)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelOrientation {
    Horizontal,
    Vertical,
}

/// Shared Panel input routing. It never grants terminal controller authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelRightClickTarget {
    #[default]
    Orchbee,
    Panel,
}

impl PanelRightClickTarget {
    #[must_use]
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Orchbee)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Orchbee => "orchbee",
            Self::Panel => "panel",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelSlot {
    pub node: PanelNode,
    pub slot_id: WorkbenchPanelSlotId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PanelNode {
    Panel {
        #[serde(default, skip_serializing_if = "PanelRightClickTarget::is_default")]
        right_click: PanelRightClickTarget,
        child_presentations: Vec<Presentation>,
        panel_id: WorkbenchPanelId,
        primary_presentation: Presentation,
    },
    Split {
        children: Box<[PanelSlot; 2]>,
        orientation: PanelOrientation,
        split_id: WorkbenchSplitId,
        #[serde(default, skip_serializing_if = "crate::WorkbenchSplitRatio::is_half")]
        ratio: crate::WorkbenchSplitRatio,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabGroupColor {
    Blue,
    Cyan,
    Green,
    Grey,
    Orange,
    Pink,
    Purple,
    Red,
    Yellow,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabGroup {
    pub color: TabGroupColor,
    pub group_id: WorkbenchTabGroupId,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tab {
    #[serde(default, skip_serializing_if = "is_false")]
    pub automatic_label: bool,
    pub group_id: Option<WorkbenchTabGroupId>,
    pub label: String,
    pub pinned: bool,
    pub root: PanelNode,
    pub tab_id: WorkbenchTabId,
}

impl Tab {
    /// Effective name at a zero-based position in the owning terminal context.
    #[must_use]
    pub fn display_label(&self, position: usize) -> std::borrow::Cow<'_, str> {
        tab_display_label(&self.label, self.automatic_label, position)
    }
}

/// Project canonical naming metadata without mutating a Tab or its order.
/// `position` is a valid zero-based index in its bounded terminal context.
#[must_use]
pub fn tab_display_label(
    label: &str,
    automatic: bool,
    position: usize,
) -> std::borrow::Cow<'_, str> {
    if automatic {
        std::borrow::Cow::Owned((position + 1).to_string())
    } else {
        std::borrow::Cow::Borrowed(label)
    }
}

pub(crate) const fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalContext {
    pub groups: Vec<TabGroup>,
    pub tabs: Vec<Tab>,
    pub terminal_context_id: WorkbenchTerminalContextId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workspace {
    pub label: String,
    /// Optional organization; changing it never changes the Workspace's resources.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub project_id: Option<WorkbenchProjectId>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub primary_directory_id: Option<WorkbenchDirectoryId>,
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub workspace_id: WorkbenchWorkspaceId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Directory {
    pub directory_id: WorkbenchDirectoryId,
    pub worker_id: WorkerId,
    pub path: WorkbenchDirectoryPath,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub label: String,
    pub project_id: WorkbenchProjectId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Navigation {
    pub directories: Vec<Directory>,
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_order: Vec<WorkbenchWorkspaceId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_order: Vec<WorkbenchProjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub navigation: Navigation,
    pub protocol_version: ProtocolVersion,
    pub revision: WorkbenchRevision,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub terminal_contexts: Vec<TerminalContext>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DeltaChange {
    NavigationReplaced {
        navigation: Navigation,
    },
    TerminalContextUpserted {
        terminal_context: TerminalContext,
    },
    TerminalContextRemoved {
        terminal_context_id: WorkbenchTerminalContextId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Delta {
    pub base_revision: WorkbenchRevision,
    pub changes: Vec<DeltaChange>,
    pub protocol_version: ProtocolVersion,
    pub revision: WorkbenchRevision,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WorkbenchProtocolDocument {
    #[serde(rename = "workbench.snapshot.read")]
    SnapshotRead(SnapshotReadIntent),
    #[serde(rename = "workbench.snapshot")]
    Snapshot(Snapshot),
    #[serde(rename = "workbench.delta")]
    Delta(Delta),
}

/// A committed projection document inside a `published` outcome; read intents are not admitted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WorkbenchPublicationDocument {
    #[serde(rename = "workbench.snapshot")]
    Snapshot(Snapshot),
    #[serde(rename = "workbench.delta")]
    Delta(Delta),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeltaRejectionReason {
    BaseRevisionMismatch,
    DuplicateChangeTarget,
    InvalidProjection,
    NonIncreasingRevision,
    #[serde(rename = "session-mismatch")]
    SessionMismatch,
}

fn deserialize_required_nullable<'de, D, Value>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
    Value: Deserialize<'de>,
{
    Option::<Value>::deserialize(deserializer)
}
