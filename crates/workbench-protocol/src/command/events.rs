//! Bounded semantic observations, connection-owned subscriptions and explicit history gaps.
use crate::{
    ClientViewObservation, ClientViewportObservation, ContractError, MetadataTokens,
    PanelAgentState, PanelAgentView, PanelNode, PresentationBinding, ProtocolVersion, SiteId,
    TerminalPresentationId, WorkbenchClientId, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkbenchWorktreeResult, Workspace,
};
use serde::{Deserialize, Serialize};
mod matching;
pub use matching::matches_semantic_filter;

pub const MAXIMUM_EVENTS_BYTES: usize = 256 * 1024;
pub const MAXIMUM_EVENT_FILTERS: usize = 32;
pub const MAXIMUM_EVENT_PAGE: usize = 32;

macro_rules! event_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
        pub enum SemanticEventKind { $(#[serde(rename=$name)] $variant),+ }
        impl SemanticEventKind {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str { match self { $(Self::$variant => $name),+ } }
        }
    };
}
event_kinds! {
    WorkspaceCreated => "workspace.created", WorkspaceUpdated => "workspace.updated",
    WorkspaceMetadataUpdated => "workspace.metadata_updated", WorkspaceRenamed => "workspace.renamed",
    WorkspaceMoved => "workspace.moved", WorkspaceReordered => "workspace.reordered",
    WorkspaceClosed => "workspace.closed", WorkspaceFocused => "workspace.focused",
    WorktreeCreated => "worktree.created", WorktreeOpened => "worktree.opened", WorktreeRemoved => "worktree.removed",
    TabCreated => "tab.created", TabClosed => "tab.closed", TabFocused => "tab.focused",
    TabRenamed => "tab.renamed", TabMoved => "tab.moved",
    PaneCreated => "pane.created", PaneClosed => "pane.closed", PaneUpdated => "pane.updated",
    PaneFocused => "pane.focused", PaneMoved => "pane.moved", PaneExited => "pane.exited",
    PaneAgentDetected => "pane.agent_detected", PaneAgentStatusChanged => "pane.agent_status_changed",
    PaneOutputChanged => "pane.output_changed", PaneOutputMatched => "pane.output_matched",
    PaneScrollChanged => "pane.scroll_changed", LayoutUpdated => "layout.updated",
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEventCursor {
    pub epoch: WorkbenchRequestId,
    pub sequence: WorkbenchRevision,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventPosition {
    Journal {
        cursor: SemanticEventCursor,
    },
    Probe {
        subscription_id: WorkbenchRequestId,
        sequence: WorkbenchRevision,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticPane {
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
    pub panel_id: WorkbenchPanelId,
    pub presentation_id: TerminalPresentationId,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub binding: Option<PresentationBinding>,
    pub label: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventData {
    Workspace {
        project_id: WorkbenchProjectId,
        workspace: Workspace,
    },
    WorkspaceOrder {
        workspace_ids: Vec<WorkbenchWorkspaceId>,
    },
    WorkspaceMetadata {
        workspace_id: WorkbenchWorkspaceId,
        tokens: MetadataTokens,
    },
    Tab {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        label: String,
        index: u32,
    },
    Pane {
        pane: SemanticPane,
    },
    Layout {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        root: Box<PanelNode>,
    },
    Worktree {
        result: Box<WorkbenchWorktreeResult>,
    },
    Client {
        client_id: WorkbenchClientId,
        view: ClientViewObservation,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        workspace_id: Option<WorkbenchWorkspaceId>,
    },
    Agent {
        pane: SemanticPane,
        report: PanelAgentView,
        exited: bool,
    },
    Output {
        pane: SemanticPane,
        content_generation: WorkbenchRevision,
        content_revision: WorkbenchRevision,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        matched_line: Option<String>,
    },
    Exited {
        pane: SemanticPane,
        lifecycle_revision: WorkbenchRevision,
    },
    Scroll {
        client_id: WorkbenchClientId,
        pane: SemanticPane,
        observation: ClientViewportObservation,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEvent {
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub workspace_id: Option<WorkbenchWorkspaceId>,
    pub position: SemanticEventPosition,
    pub workbench_revision: WorkbenchRevision,
    pub event: SemanticEventKind,
    pub data: SemanticEventData,
}
impl SemanticEvent {
    pub fn validate(&self) -> Result<(), ContractError> {
        use SemanticEventData as D;
        use SemanticEventKind as K;
        let compatible = match (&self.data, self.event) {
            (
                D::Workspace { workspace, .. },
                K::WorkspaceCreated
                | K::WorkspaceUpdated
                | K::WorkspaceRenamed
                | K::WorkspaceMoved
                | K::WorkspaceClosed,
            ) => WorkbenchLabel::parse(&workspace.label).is_ok(),
            (D::WorkspaceOrder { workspace_ids }, K::WorkspaceReordered) => {
                workspace_ids.len() <= 4096
                    && workspace_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == workspace_ids.len()
            }
            (D::WorkspaceMetadata { .. }, K::WorkspaceMetadataUpdated) => true,
            (D::Tab { label, .. }, K::TabCreated | K::TabClosed | K::TabRenamed | K::TabMoved) => {
                WorkbenchLabel::parse(label).is_ok()
            }
            (D::Pane { pane }, K::PaneCreated | K::PaneClosed | K::PaneUpdated | K::PaneMoved) => {
                WorkbenchLabel::parse(&pane.label).is_ok()
            }
            (
                D::Layout {
                    terminal_context_id,
                    tab_id,
                    root,
                },
                K::LayoutUpdated,
            ) => {
                let snapshot = crate::Snapshot {
                    protocol_version: ProtocolVersion,
                    site_id: SiteId::parse(&format!("{:032x}", 1))
                        .map_err(|_| ContractError::InvalidDocument)?,
                    revision: self.workbench_revision,
                    navigation: crate::Navigation {
                        projects: Vec::new(),
                        standalone_terminal_context_id: terminal_context_id.clone(),
                        workspace_order: Vec::new(),
                    },
                    terminal_contexts: vec![crate::TerminalContext {
                        terminal_context_id: terminal_context_id.clone(),
                        groups: Vec::new(),
                        tabs: vec![crate::Tab {
                            tab_id: tab_id.clone(),
                            label: "Layout".into(),
                            pinned: false,
                            automatic_label: false,
                            group_id: None,
                            root: (**root).clone(),
                        }],
                    }],
                };
                crate::validation::validate_snapshot(&snapshot).is_ok()
            }
            (
                D::Worktree { result },
                K::WorktreeCreated | K::WorktreeOpened | K::WorktreeRemoved,
            ) => {
                result.validate().is_ok()
                    && matches!(&result.outcome, crate::WorkbenchWorktreeOutcome::Available {status,..}
                    if matches!((&**status,self.event),
                        (crate::WorktreeOperationStatus::Opened {..},K::WorktreeCreated | K::WorktreeOpened)
                        | (crate::WorktreeOperationStatus::Removed {..},K::WorktreeRemoved)))
            }
            (
                D::Client { view, .. },
                K::WorkspaceFocused | K::TabFocused | K::PaneFocused | K::LayoutUpdated,
            ) => view.validate().is_ok(),
            (
                D::Agent {
                    pane,
                    report,
                    exited,
                },
                K::PaneAgentDetected | K::PaneAgentStatusChanged | K::PaneUpdated,
            ) => WorkbenchLabel::parse(&pane.label).is_ok() && report.valid() && !exited,
            (
                D::Output {
                    pane,
                    content_generation,
                    matched_line,
                    ..
                },
                K::PaneOutputChanged | K::PaneOutputMatched | K::PaneUpdated,
            ) => {
                WorkbenchLabel::parse(&pane.label).is_ok()
                    && pane.binding.is_some()
                    && content_generation.value() > 0
                    && matched_line
                        .as_ref()
                        .is_none_or(|line| line.len() <= 64 * 1024 && !line.contains('\0'))
                    && (self.event != K::PaneOutputMatched || matched_line.is_some())
            }
            (D::Exited { pane, .. }, K::PaneExited) => {
                WorkbenchLabel::parse(&pane.label).is_ok() && pane.binding.is_some()
            }
            (
                D::Scroll {
                    pane, observation, ..
                },
                K::PaneScrollChanged,
            ) => WorkbenchLabel::parse(&pane.label).is_ok() && observation.validate().is_ok(),
            _ => false,
        };
        let correlated = match &self.data {
            D::Workspace { workspace, .. } => {
                self.workspace_id.as_ref() == Some(&workspace.workspace_id)
            }
            D::WorkspaceMetadata { workspace_id, .. } => {
                self.workspace_id.as_ref() == Some(workspace_id)
            }
            D::WorkspaceOrder { .. } => self.workspace_id.is_none(),
            D::Client { workspace_id, .. } => &self.workspace_id == workspace_id,
            _ => true,
        };
        if !compatible
            || !correlated
            || matches!(&self.position,SemanticEventPosition::Journal {cursor} if cursor.sequence.value()==0)
            || matches!(&self.position,SemanticEventPosition::Probe {sequence,..} if sequence.value()==0)
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventScope {
    All {},
    Workspace {
        workspace_id: WorkbenchWorkspaceId,
    },
    TerminalContext {
        terminal_context_id: WorkbenchTerminalContextId,
    },
    Tab {
        tab_id: WorkbenchTabId,
    },
    Pane {
        panel_id: WorkbenchPanelId,
    },
    Client {
        client_id: WorkbenchClientId,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventMatch {
    Any {},
    Label { label: WorkbenchLabel },
    OutputRevision { minimum_revision: WorkbenchRevision },
    Agent { agent: String },
    AgentState { state: PanelAgentState },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEventSource {
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub presentation_id: Option<TerminalPresentationId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticOutputSource {
    Recent,
    RecentUnwrapped,
    History,
    Viewport,
    Detection,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SemanticOutputMatch {
    Substring { value: String },
    Regex { value: String },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventFilter {
    Event {
        event: SemanticEventKind,
        scope: SemanticEventScope,
        condition: SemanticEventMatch,
    },
    OutputMatched {
        target: SemanticEventSource,
        source: SemanticOutputSource,
        lines: u16,
        matcher: SemanticOutputMatch,
        strip_ansi: bool,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        client_id: Option<WorkbenchClientId>,
    },
    AgentStatusChanged {
        target: SemanticEventSource,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        state: Option<PanelAgentState>,
    },
    ScrollChanged {
        target: SemanticEventSource,
        client_id: WorkbenchClientId,
    },
}
impl SemanticEventFilter {
    pub fn validate(&self) -> Result<(), ContractError> {
        use SemanticEventKind as K;
        let valid = match self {
            Self::Event {
                event, condition, ..
            } => match condition {
                SemanticEventMatch::Any {} => {
                    !matches!(event, K::PaneOutputMatched | K::PaneScrollChanged)
                }
                SemanticEventMatch::Label { .. } => {
                    matches!(event, K::WorkspaceRenamed | K::TabRenamed | K::PaneUpdated)
                }
                SemanticEventMatch::OutputRevision { .. } => *event == K::PaneOutputChanged,
                SemanticEventMatch::Agent { agent } => {
                    *event == K::PaneAgentDetected
                        && !agent.is_empty()
                        && agent.len() <= 241
                        && !agent.chars().any(char::is_control)
                }
                SemanticEventMatch::AgentState { .. } => *event == K::PaneAgentStatusChanged,
            },
            Self::OutputMatched {
                source,
                lines,
                matcher,
                client_id,
                ..
            } => {
                let value = match matcher {
                    SemanticOutputMatch::Substring { value }
                    | SemanticOutputMatch::Regex { value } => value,
                };
                (1..=4096).contains(lines)
                    && !value.is_empty()
                    && value.len() <= 4096
                    && !value.contains('\0')
                    && (*source != SemanticOutputSource::Viewport || client_id.is_some())
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventsAction {
    Subscribe {
        subscription_id: WorkbenchRequestId,
        filters: Vec<SemanticEventFilter>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        after: Option<SemanticEventCursor>,
    },
    Read {
        subscription_id: WorkbenchRequestId,
        limit: u16,
    },
    Wait {
        subscription_id: WorkbenchRequestId,
        timeout_milliseconds: u16,
    },
    Close {
        subscription_id: WorkbenchRequestId,
    },
}
impl SemanticEventsAction {
    #[must_use]
    pub fn subscription_id(&self) -> &WorkbenchRequestId {
        match self {
            Self::Subscribe {
                subscription_id, ..
            }
            | Self::Read {
                subscription_id, ..
            }
            | Self::Wait {
                subscription_id, ..
            }
            | Self::Close { subscription_id } => subscription_id,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEventsRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: SemanticEventsAction,
}
impl SemanticEventsRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            SemanticEventsAction::Subscribe { filters, .. } => {
                if filters.is_empty() || filters.len() > MAXIMUM_EVENT_FILTERS {
                    return Err(ContractError::InvalidDocument);
                }
                for filter in filters {
                    filter.validate()?;
                }
            }
            SemanticEventsAction::Read { limit, .. }
                if *limit == 0 || usize::from(*limit) > MAXIMUM_EVENT_PAGE =>
            {
                return Err(ContractError::InvalidDocument);
            }
            SemanticEventsAction::Wait {
                timeout_milliseconds,
                ..
            } if *timeout_milliseconds > 1000 => return Err(ContractError::InvalidDocument),
            _ => {}
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticEventsFailure {
    Denied,
    InvalidIntent,
    InvalidRegex,
    SiteMismatch,
    Unavailable,
    Busy,
    Conflict,
    NotFound,
    CursorEpoch,
    CursorAhead,
    SourceChanged,
    SourceUnavailable,
    PhysicalUnknown,
    RevisionExhausted,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEventGap {
    pub after: WorkbenchRevision,
    pub through: WorkbenchRevision,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SemanticEventsOutcome {
    Opened {
        subscription_id: WorkbenchRequestId,
        cursor: SemanticEventCursor,
        existing: bool,
    },
    Events {
        subscription_id: WorkbenchRequestId,
        cursor: SemanticEventCursor,
        events: Vec<SemanticEvent>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        gap: Option<SemanticEventGap>,
        more: bool,
    },
    Matched {
        subscription_id: WorkbenchRequestId,
        cursor: SemanticEventCursor,
        event: Box<SemanticEvent>,
    },
    TimedOut {
        subscription_id: WorkbenchRequestId,
        cursor: SemanticEventCursor,
    },
    Gap {
        subscription_id: WorkbenchRequestId,
        cursor: SemanticEventCursor,
        gap: SemanticEventGap,
    },
    Closed {
        subscription_id: WorkbenchRequestId,
    },
    Failed {
        reason: SemanticEventsFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEventsResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: SemanticEventsOutcome,
}
impl SemanticEventsResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            SemanticEventsOutcome::Events {
                cursor,
                events,
                gap,
                subscription_id,
                ..
            } => {
                if events.len() > MAXIMUM_EVENT_PAGE {
                    return Err(ContractError::InvalidDocument);
                }
                let mut last = 0;
                let mut last_probe = 0;
                for event in events {
                    event.validate()?;
                    match &event.position {
                        SemanticEventPosition::Journal { cursor: position } => {
                            if position.epoch != cursor.epoch
                                || position.sequence.value() <= last
                                || position.sequence > cursor.sequence
                            {
                                return Err(ContractError::InvalidDocument);
                            }
                            last = position.sequence.value();
                        }
                        SemanticEventPosition::Probe {
                            subscription_id: actual,
                            sequence,
                        } => {
                            if actual != subscription_id || sequence.value() <= last_probe {
                                return Err(ContractError::InvalidDocument);
                            }
                            last_probe = sequence.value();
                        }
                    }
                }
                if gap
                    .as_ref()
                    .is_some_and(|gap| gap.after >= gap.through || gap.through > cursor.sequence)
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            SemanticEventsOutcome::Matched {
                cursor,
                event,
                subscription_id,
            } => {
                event.validate()?;
                match &event.position {
                    SemanticEventPosition::Journal { cursor: position }
                        if position.epoch != cursor.epoch
                            || position.sequence > cursor.sequence =>
                    {
                        return Err(ContractError::InvalidDocument);
                    }
                    SemanticEventPosition::Probe {
                        subscription_id: actual,
                        ..
                    } if actual != subscription_id => return Err(ContractError::InvalidDocument),
                    _ => {}
                }
            }
            SemanticEventsOutcome::Gap { cursor, gap, .. }
                if gap.after >= gap.through || gap.through > cursor.sequence =>
            {
                return Err(ContractError::InvalidDocument);
            }
            _ => {}
        }
        Ok(())
    }
    #[must_use]
    pub fn answers(&self, request: &SemanticEventsRequest) -> bool {
        use SemanticEventsAction as A;
        use SemanticEventsOutcome as O;
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && self.validate().is_ok()
            && match (&request.action, &self.outcome) {
                (_, O::Failed { .. }) => true,
                (
                    A::Subscribe {
                        subscription_id, ..
                    },
                    O::Opened {
                        subscription_id: actual,
                        ..
                    },
                )
                | (
                    A::Read {
                        subscription_id, ..
                    },
                    O::Events {
                        subscription_id: actual,
                        ..
                    },
                )
                | (
                    A::Wait {
                        subscription_id, ..
                    },
                    O::Matched {
                        subscription_id: actual,
                        ..
                    }
                    | O::TimedOut {
                        subscription_id: actual,
                        ..
                    }
                    | O::Gap {
                        subscription_id: actual,
                        ..
                    },
                )
                | (
                    A::Close { subscription_id },
                    O::Closed {
                        subscription_id: actual,
                    },
                ) => subscription_id == actual,
                _ => false,
            }
    }
}
