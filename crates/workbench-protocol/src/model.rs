use serde::{Deserialize, Serialize};

use crate::directory::WorkbenchDirectoryPath;
use crate::identity::{
    ProtocolVersion, SiteId, TerminalId, TerminalPresentationId, WorkbenchDirectoryId,
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
    pub label: String,
    pub presentation_id: TerminalPresentationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelOrientation {
    Horizontal,
    Vertical,
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
        child_presentations: Vec<Presentation>,
        panel_id: WorkbenchPanelId,
        primary_presentation: Presentation,
    },
    Split {
        children: Box<[PanelSlot; 2]>,
        orientation: PanelOrientation,
        split_id: WorkbenchSplitId,
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
    pub group_id: Option<WorkbenchTabGroupId>,
    pub label: String,
    pub pinned: bool,
    pub root: PanelNode,
    pub tab_id: WorkbenchTabId,
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
    pub directories: Vec<Directory>,
    pub label: String,
    pub project_id: WorkbenchProjectId,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Navigation {
    pub projects: Vec<Project>,
    pub standalone_terminal_context_id: WorkbenchTerminalContextId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub navigation: Navigation,
    pub protocol_version: ProtocolVersion,
    pub revision: WorkbenchRevision,
    pub site_id: SiteId,
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
    pub site_id: SiteId,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeltaRejectionReason {
    BaseRevisionMismatch,
    DuplicateChangeTarget,
    InvalidProjection,
    NonIncreasingRevision,
    SiteMismatch,
}

fn deserialize_required_nullable<'de, D, Value>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
    Value: Deserialize<'de>,
{
    Option::<Value>::deserialize(deserializer)
}
