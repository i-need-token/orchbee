//! Ephemeral Client-owned selection and connection-scoped, acknowledged navigation.
use super::client_attention::{
    ClientAgentAttentionSnapshot, ClientTerminalAgentAttentionSnapshot, validate_sessions,
    validate_terminal_bindings,
};
use super::{
    client_layout::{ClientPanelDirection, ClientPanelLayout, ClientSplitRatio},
    client_layout_adjustment::{
        ClientLayoutAdjustment, ClientLayoutChange, ClientResizeAmount, ClientSplitIdentity,
        ClientSplitPath,
    },
    client_navigation::{ClientNavigationSnapshot, validate_contexts},
    json_fields::deserialize_nullable,
};
use crate::identity::WorkbenchClientId;
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchPanelId, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
};
use orchbee_agent_observation_protocol::AgentSessionId;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{collections::BTreeSet, num::NonZeroU64};

pub const MAXIMUM_CLIENT_VIEWS: usize = 32;
pub const MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientViewSequence(NonZeroU64);

impl ClientViewSequence {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(ContractError::InvalidDocument)
    }
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0.get()
    }
}
impl Serialize for ClientViewSequence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value().to_string())
    }
}
impl<'de> Deserialize<'de> for ClientViewSequence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = WorkbenchRevision::deserialize(deserializer)?;
        Self::new(value.value()).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewPhase {
    Starting,
    Ready,
    Busy,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewSelection {
    pub terminal_context_id: WorkbenchTerminalContextId,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub tab_id: Option<WorkbenchTabId>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub panel_id: Option<WorkbenchPanelId>,
    pub zoomed: bool,
    pub panel_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewObservation {
    pub workbench_revision: WorkbenchRevision,
    pub phase: ClientViewPhase,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub selection: Option<ClientViewSelection>,
}
impl ClientViewObservation {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.phase == ClientViewPhase::Ready && self.selection.is_none() {
            return Err(ContractError::InvalidDocument);
        }
        if self.selection.as_ref().is_some_and(|selection| {
            (selection.tab_id.is_some() != (selection.panel_count > 0))
                || (selection.panel_id.is_some() && selection.tab_id.is_none())
                || (selection.zoomed && selection.panel_id.is_none())
        }) {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewAvailability {
    Current,
    Stale,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewRecord {
    pub client_id: WorkbenchClientId,
    pub heartbeat: ClientViewSequence,
    pub view_revision: ClientViewSequence,
    pub view: ClientViewObservation,
    pub availability: ClientViewAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewIntent {
    AgentFocus {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        presentation_id: crate::TerminalPresentationId,
        target: orchbee_agent_observation_protocol::AgentNameTarget,
    },
    Viewports {
        panel_ids: Vec<WorkbenchPanelId>,
    },
    Selections {
        terminal_context_ids: Vec<WorkbenchTerminalContextId>,
    },
    TerminalAgentAttention {
        bindings: Vec<orchbee_agent_observation_protocol::AgentLaunchBinding>,
    },
    AgentAttention {
        session_ids: Vec<AgentSessionId>,
    },
    Resize {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        direction: ClientPanelDirection,
        amount: ClientResizeAmount,
    },
    SetSplitRatio {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        path: ClientSplitPath,
        ratio: ClientSplitRatio,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_expected_split"
        )]
        expected_split: Option<ClientSplitIdentity>,
    },
    FocusDirection {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        direction: ClientPanelDirection,
    },
    Layout {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    },
    Context {
        terminal_context_id: WorkbenchTerminalContextId,
    },
    Tab {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
    Panel {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    },
    ToggleZoom {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    },
    Zoom {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        enabled: bool,
    },
}
impl ClientViewIntent {
    #[must_use]
    pub fn is_passive_read(&self) -> bool {
        matches!(
            self,
            Self::Viewports { .. }
                | Self::AgentAttention { .. }
                | Self::TerminalAgentAttention { .. }
        )
    }
    #[must_use]
    pub fn is_attention_read(&self) -> bool {
        matches!(
            self,
            Self::AgentAttention { .. } | Self::TerminalAgentAttention { .. }
        )
    }
    /// Pin one previously resolved directional neighbor as an exact navigation target.
    #[must_use]
    pub fn directional_target(&self, destination: &WorkbenchPanelId) -> Option<Self> {
        let Self::FocusDirection {
            terminal_context_id,
            tab_id,
            panel_id,
            ..
        } = self
        else {
            return None;
        };
        (destination != panel_id).then(|| Self::Panel {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: destination.clone(),
        })
    }
    #[must_use]
    pub fn is_single_panel_zoom_observed(
        &self,
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
    ) -> bool {
        let (terminal_context_id, tab_id, panel_id) = match self {
            Self::Zoom {
                terminal_context_id,
                tab_id,
                panel_id,
                enabled: true,
            }
            | Self::ToggleZoom {
                terminal_context_id,
                tab_id,
                panel_id,
            } => (terminal_context_id, tab_id, panel_id),
            _ => return false,
        };
        view.selection
            .as_ref()
            .is_some_and(|selection| selection.panel_count == 1 && !selection.zoomed)
            && Self::Panel {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
            }
            .is_observed(view, minimum)
    }
    #[must_use]
    pub fn is_toggle_observed(
        &self,
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
        previously_zoomed: bool,
    ) -> bool {
        let Self::ToggleZoom {
            terminal_context_id,
            tab_id,
            panel_id,
        } = self
        else {
            return false;
        };
        Self::Zoom {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            enabled: !previously_zoomed,
        }
        .is_observed(view, minimum)
            && view
                .selection
                .as_ref()
                .is_some_and(|selection| selection.panel_count > 1)
    }
    #[must_use]
    pub fn is_observed(&self, view: &ClientViewObservation, minimum: WorkbenchRevision) -> bool {
        if matches!(self, Self::AgentFocus { .. }) && view.phase != ClientViewPhase::Ready {
            return false;
        }
        if view.workbench_revision.value() < minimum.value()
            || matches!(
                view.phase,
                ClientViewPhase::Starting | ClientViewPhase::Busy
            )
        {
            return false;
        }
        let Some(selection) = &view.selection else {
            return false;
        };
        match self {
            Self::TerminalAgentAttention { .. }
            | Self::AgentAttention { .. }
            | Self::Viewports { .. }
            | Self::Selections { .. }
            | Self::ToggleZoom { .. }
            | Self::Layout { .. }
            | Self::FocusDirection { .. }
            | Self::Resize { .. }
            | Self::SetSplitRatio { .. } => false,
            Self::Context {
                terminal_context_id,
            } => selection.terminal_context_id == *terminal_context_id,
            Self::Tab {
                terminal_context_id,
                tab_id,
            } => {
                selection.terminal_context_id == *terminal_context_id
                    && selection.tab_id.as_ref() == Some(tab_id)
            }
            Self::Panel {
                terminal_context_id,
                tab_id,
                panel_id,
            }
            | Self::AgentFocus {
                terminal_context_id,
                tab_id,
                panel_id,
                ..
            } => {
                selection.terminal_context_id == *terminal_context_id
                    && selection.tab_id.as_ref() == Some(tab_id)
                    && selection.panel_id.as_ref() == Some(panel_id)
            }
            Self::Zoom {
                terminal_context_id,
                tab_id,
                panel_id,
                enabled,
            } => {
                selection.terminal_context_id == *terminal_context_id
                    && selection.tab_id.as_ref() == Some(tab_id)
                    && selection.panel_id.as_ref() == Some(panel_id)
                    && selection.zoomed == *enabled
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewControl {
    pub operation_id: WorkbenchRequestId,
    pub client_id: WorkbenchClientId,
    pub expected_view_revision: ClientViewSequence,
    pub minimum_workbench_revision: WorkbenchRevision,
    pub intent: ClientViewIntent,
}
impl ClientViewControl {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let ClientViewIntent::AgentFocus {
            presentation_id,
            target: orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding },
            ..
        } = &self.intent
        {
            validate_terminal_bindings(std::slice::from_ref(binding))?;
            if presentation_id.as_str() != binding.presentation_id.as_str() {
                return Err(ContractError::InvalidDocument);
            }
        }
        if let ClientViewIntent::Viewports { panel_ids } = &self.intent {
            super::client_viewport::validate_panels(panel_ids)?;
        }
        if let ClientViewIntent::Selections {
            terminal_context_ids,
        } = &self.intent
        {
            validate_contexts(terminal_context_ids)?;
        }
        if let ClientViewIntent::TerminalAgentAttention { bindings } = &self.intent {
            validate_terminal_bindings(bindings)?;
        }
        if let ClientViewIntent::AgentAttention { session_ids } = &self.intent {
            validate_sessions(session_ids)?;
        }
        if let ClientViewIntent::SetSplitRatio {
            expected_split: Some(identity),
            ..
        } = &self.intent
        {
            identity.validate()?;
        }
        Ok(())
    }
}

fn deserialize_expected_split<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ClientSplitIdentity>, D::Error> {
    let identity = ClientSplitIdentity::deserialize(deserializer)?;
    identity.validate().map_err(de::Error::custom)?;
    Ok(Some(identity))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewControlReason {
    Busy,
    StaleView,
    TargetUnavailable,
    ProjectionUnavailable,
    Interrupted,
    ClientDisconnected,
    Expired,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewCompletionOutcome {
    ViewportRead {
        viewport: Box<super::client_viewport::ClientViewportSnapshot>,
    },
    NavigationRead {
        navigation: Box<ClientNavigationSnapshot>,
    },
    TerminalAttentionRead {
        attention: Box<ClientTerminalAgentAttentionSnapshot>,
    },
    AttentionRead {
        attention: Box<ClientAgentAttentionSnapshot>,
    },
    LayoutAdjusted {
        adjustment: ClientLayoutAdjustment,
    },
    DirectionFocused {
        panel_id: WorkbenchPanelId,
    },
    DirectionUnchanged {
        layout_revision: WorkbenchRevision,
    },
    LayoutRead {
        layout: Box<ClientPanelLayout>,
    },
    Applied {},
    ZoomUnchanged {},
    ZoomToggled {
        previously_zoomed: bool,
    },
    Rejected {
        reason: ClientViewControlReason,
    },
    Incomplete {
        reason: ClientViewControlReason,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewCompletion {
    pub operation_id: WorkbenchRequestId,
    pub outcome: ClientViewCompletionOutcome,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewControlStatus {
    ViewportRead {
        viewport: Box<super::client_viewport::ClientViewportSnapshot>,
        view: ClientViewObservation,
    },
    NavigationRead {
        navigation: Box<ClientNavigationSnapshot>,
        view: ClientViewObservation,
    },
    TerminalAttentionRead {
        attention: Box<ClientTerminalAgentAttentionSnapshot>,
        view: ClientViewObservation,
    },
    AttentionRead {
        attention: Box<ClientAgentAttentionSnapshot>,
        view: ClientViewObservation,
    },
    LayoutAdjusted {
        adjustment: ClientLayoutAdjustment,
        view: ClientViewObservation,
    },
    DirectionFocused {
        panel_id: WorkbenchPanelId,
        view: ClientViewObservation,
    },
    DirectionUnchanged {
        layout_revision: WorkbenchRevision,
        view: ClientViewObservation,
    },
    LayoutRead {
        layout: Box<ClientPanelLayout>,
        view: ClientViewObservation,
    },
    Queued {},
    Delivered {},
    Applied {
        view: ClientViewObservation,
    },
    ZoomUnchanged {
        view: ClientViewObservation,
    },
    ZoomToggled {
        previously_zoomed: bool,
        view: ClientViewObservation,
    },
    Rejected {
        reason: ClientViewControlReason,
    },
    Incomplete {
        reason: ClientViewControlReason,
        #[serde(deserialize_with = "deserialize_nullable")]
        view: Option<ClientViewObservation>,
    },
}
impl ClientViewControlStatus {
    #[must_use]
    pub fn pending(&self) -> bool {
        matches!(self, Self::Queued {} | Self::Delivered {})
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewOperation {
    pub command: ClientViewControl,
    pub status: ClientViewControlStatus,
}
impl ClientViewOperation {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.command.validate()?;
        match &self.status {
            ClientViewControlStatus::ViewportRead { viewport, view } => {
                view.validate()?;
                let ClientViewIntent::Viewports { panel_ids } = &self.command.intent else {
                    return Err(ContractError::InvalidDocument);
                };
                if !viewport.answers(panel_ids, view, self.command.minimum_workbench_revision) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::NavigationRead { navigation, view } => {
                view.validate()?;
                let ClientViewIntent::Selections {
                    terminal_context_ids,
                } = &self.command.intent
                else {
                    return Err(ContractError::InvalidDocument);
                };
                if !navigation.answers(
                    terminal_context_ids,
                    view,
                    self.command.minimum_workbench_revision,
                ) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::TerminalAttentionRead { attention, view } => {
                view.validate()?;
                let ClientViewIntent::TerminalAgentAttention { bindings } = &self.command.intent
                else {
                    return Err(ContractError::InvalidDocument);
                };
                if !attention.answers(bindings) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::AttentionRead { attention, view } => {
                view.validate()?;
                let ClientViewIntent::AgentAttention { session_ids } = &self.command.intent else {
                    return Err(ContractError::InvalidDocument);
                };
                if !attention.answers(session_ids) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::LayoutAdjusted { adjustment, view } => {
                adjustment.validate()?;
                view.validate()?;
                let (context, tab, expected) = match (&self.command.intent, &adjustment.change) {
                    (
                        ClientViewIntent::SetSplitRatio {
                            terminal_context_id,
                            tab_id,
                            ratio,
                            expected_split,
                            ..
                        },
                        ClientLayoutChange::Split { split },
                    ) => (
                        terminal_context_id,
                        tab_id,
                        split.ratio == *ratio
                            && expected_split.as_ref().is_none_or(|identity| {
                                identity.split_id == split.split_id
                                    && identity.first_slot_id == split.first_slot_id
                                    && identity.second_slot_id == split.second_slot_id
                            }),
                    ),
                    (
                        ClientViewIntent::Resize {
                            terminal_context_id,
                            tab_id,
                            direction,
                            amount,
                            ..
                        },
                        change,
                    ) => (
                        terminal_context_id,
                        tab_id,
                        match change {
                            ClientLayoutChange::NoSplit {} => true,
                            ClientLayoutChange::Split { split } => {
                                amount.apply(split.previous_ratio, *direction) == split.ratio
                            }
                        },
                    ),
                    _ => return Err(ContractError::InvalidDocument),
                };
                let selected = view.selection.as_ref().is_some_and(|selection| {
                    &selection.terminal_context_id == context
                        && selection.tab_id.as_ref() == Some(tab)
                });
                if !expected
                    || &adjustment.terminal_context_id != context
                    || &adjustment.tab_id != tab
                    || adjustment.workbench_revision < self.command.minimum_workbench_revision
                    || adjustment.selected != selected
                    || (selected && adjustment.workbench_revision != view.workbench_revision)
                    || matches!(
                        view.phase,
                        ClientViewPhase::Starting | ClientViewPhase::Busy
                    )
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::DirectionFocused { panel_id, view } => {
                view.validate()?;
                if self
                    .command
                    .intent
                    .directional_target(panel_id)
                    .is_none_or(|target| {
                        !target.is_observed(view, self.command.minimum_workbench_revision)
                    })
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::DirectionUnchanged {
                layout_revision,
                view,
            } => {
                view.validate()?;
                let ClientViewIntent::FocusDirection {
                    terminal_context_id,
                    tab_id,
                    ..
                } = &self.command.intent
                else {
                    return Err(ContractError::InvalidDocument);
                };
                let selected = view.selection.as_ref().is_some_and(|selection| {
                    &selection.terminal_context_id == terminal_context_id
                        && selection.tab_id.as_ref() == Some(tab_id)
                });
                if *layout_revision < self.command.minimum_workbench_revision
                    || matches!(
                        view.phase,
                        ClientViewPhase::Starting | ClientViewPhase::Busy
                    )
                    || (selected && *layout_revision != view.workbench_revision)
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::LayoutRead { layout, view } => {
                layout.validate()?;
                view.validate()?;
                let ClientViewIntent::Layout {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = &self.command.intent
                else {
                    return Err(ContractError::InvalidDocument);
                };
                let selected = view.selection.as_ref().is_some_and(|selection| {
                    &selection.terminal_context_id == terminal_context_id
                        && selection.tab_id.as_ref() == Some(tab_id)
                });
                if &layout.terminal_context_id != terminal_context_id
                    || &layout.tab_id != tab_id
                    || &layout.source_panel_id != panel_id
                    || layout.workbench_revision < self.command.minimum_workbench_revision
                    || layout.selected != selected
                    || (selected
                        && view.selection.as_ref().is_none_or(|selection| {
                            layout.focused_panel_id != selection.panel_id
                                || layout.zoomed != selection.zoomed
                                || layout.panels.len() != usize::from(selection.panel_count)
                                || layout.workbench_revision != view.workbench_revision
                        }))
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::ZoomToggled {
                previously_zoomed,
                view,
            } => {
                view.validate()?;
                if !self.command.intent.is_toggle_observed(
                    view,
                    self.command.minimum_workbench_revision,
                    *previously_zoomed,
                ) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::Applied { view } => {
                view.validate()?;
                if !self
                    .command
                    .intent
                    .is_observed(view, self.command.minimum_workbench_revision)
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::ZoomUnchanged { view } => {
                view.validate()?;
                if !self
                    .command
                    .intent
                    .is_single_panel_zoom_observed(view, self.command.minimum_workbench_revision)
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewControlStatus::Incomplete {
                view: Some(view), ..
            } => view.validate()?,
            _ => {}
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
pub enum ClientViewAction {
    Synchronize {
        heartbeat: ClientViewSequence,
        view_revision: ClientViewSequence,
        view: Box<ClientViewObservation>,
        #[serde(deserialize_with = "deserialize_nullable")]
        completion: Option<ClientViewCompletion>,
    },
    List {},
    Get {
        client_id: WorkbenchClientId,
    },
    Submit {
        command: Box<ClientViewControl>,
    },
    Read {
        operation_id: WorkbenchRequestId,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchClientViewRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: ClientViewAction,
}
impl WorkbenchClientViewRequest {
    pub(crate) fn carries_attention(&self) -> bool {
        matches!(&self.action, ClientViewAction::Submit { command } if command.intent.is_attention_read())
            || matches!(
                &self.action,
                ClientViewAction::Synchronize {
                    completion: Some(ClientViewCompletion {
                        outcome: ClientViewCompletionOutcome::AttentionRead { .. }
                            | ClientViewCompletionOutcome::TerminalAttentionRead { .. },
                        ..
                    }),
                    ..
                }
            )
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if let ClientViewAction::Submit { command } = &self.action {
            command.validate()?;
        }
        if let ClientViewAction::Synchronize {
            view, completion, ..
        } = &self.action
        {
            view.validate()?;
            if let Some(ClientViewCompletion {
                outcome: ClientViewCompletionOutcome::ViewportRead { viewport },
                ..
            }) = completion
            {
                viewport.validate()?;
            }
            if let Some(ClientViewCompletion {
                outcome: ClientViewCompletionOutcome::NavigationRead { navigation },
                ..
            }) = completion
            {
                navigation.validate()?;
            }
            if let Some(ClientViewCompletion {
                outcome: ClientViewCompletionOutcome::TerminalAttentionRead { attention },
                ..
            }) = completion
            {
                attention.validate()?;
            }
            if let Some(ClientViewCompletion {
                outcome: ClientViewCompletionOutcome::AttentionRead { attention },
                ..
            }) = completion
            {
                attention.validate()?;
            }
            if let Some(ClientViewCompletion {
                outcome: ClientViewCompletionOutcome::LayoutRead { layout },
                ..
            }) = completion
            {
                layout.validate()?;
            }
            if let Some(ClientViewCompletion {
                outcome: ClientViewCompletionOutcome::LayoutAdjusted { adjustment },
                ..
            }) = completion
            {
                adjustment.validate()?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewUnavailable {
    ClientUnavailable,
    ServerBusy,
    ReceiptCapacity,
    Unsupported,
    SequenceExhausted,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientViewOutcome {
    Synchronized {
        client: Box<ClientViewRecord>,
        #[serde(deserialize_with = "deserialize_nullable")]
        control: Option<Box<ClientViewControl>>,
    },
    Listed {
        clients: Vec<ClientViewRecord>,
    },
    Found {
        client: Box<ClientViewRecord>,
    },
    Operation {
        operation: Box<ClientViewOperation>,
    },
    NotFound {},
    Conflict {},
    Denied {},
    Unavailable {
        reason: ClientViewUnavailable,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchClientViewResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: ClientViewOutcome,
}
impl WorkbenchClientViewResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            ClientViewOutcome::Synchronized { client, control } => {
                client.view.validate()?;
                if let Some(control) = control {
                    control.validate()?;
                }
                if control.as_ref().is_some_and(|control| {
                    control.client_id != client.client_id
                        || control.expected_view_revision != client.view_revision
                        || client.availability != ClientViewAvailability::Current
                }) {
                    return Err(ContractError::InvalidDocument);
                }
            }
            ClientViewOutcome::Found { client } => client.view.validate()?,
            ClientViewOutcome::Listed { clients } => {
                if clients.len() > MAXIMUM_CLIENT_VIEWS {
                    return Err(ContractError::InvalidDocument);
                }
                let mut seen = BTreeSet::new();
                for client in clients {
                    client.view.validate()?;
                    if !seen.insert(&client.client_id) {
                        return Err(ContractError::InvalidDocument);
                    }
                }
            }
            ClientViewOutcome::Operation { operation } => operation.validate()?,
            _ => {}
        }
        Ok(())
    }
}
