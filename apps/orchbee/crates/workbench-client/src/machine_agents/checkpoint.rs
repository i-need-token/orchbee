//! Settled Agent observation values: one machine's bounded Agent rows, their exact targets,
//! and the checkpoint that fences them. Observation jobs produce them; Client state holds them.
use crate::client_context::ClientContextTarget;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    AgentViewSnapshot, MetadataTokens, PanelAgentState, PanelMetadataEffective,
    PresentationBinding, SessionId, TerminalPresentationId, TerminalTitleObservation,
    WorkbenchPanelId, WorkbenchRevision, WorkbenchTabId,
};
use std::sync::Arc;

pub(crate) const MAXIMUM_ROWS: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub(crate) context: ClientContextTarget,
    pub(crate) tab: WorkbenchTabId,
    pub(crate) panel: WorkbenchPanelId,
    pub(crate) member: TerminalPresentationId,
    pub(crate) terminal: PresentationBinding,
    pub(crate) physical: AgentLaunchBinding,
    pub(crate) agent: String,
}
impl Target {
    /// The Context whose terminal runs this Agent, where a navigation to it lands.
    pub fn destination(&self) -> &ClientContextTarget {
        &self.context
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub(crate) seen: bool,
    pub(crate) name: Option<String>,
    pub(crate) order: PlacementOrder,
    pub(crate) target: Target,
    pub(crate) label: String,
    /// Whether `label` is the Presentation's custom label rather than its creation label.
    pub(crate) label_custom: bool,
    pub(crate) workspace: String,
    pub(crate) tab_label: Option<String>,
    pub(crate) terminal_title: TerminalTitleObservation,
    pub(crate) state: PanelAgentState,
    pub(crate) sequence: WorkbenchRevision,
    pub(crate) metadata: PanelMetadataEffective,
    pub(crate) tokens: MetadataTokens,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlacementOrder {
    pub(crate) workspace: Option<u64>,
    pub(crate) tab: u64,
    pub(crate) pane: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    pub(crate) session_id: SessionId,
    pub(crate) report_revision: WorkbenchRevision,
    pub(crate) contexts: Arc<[ClientContextTarget]>,
    pub(crate) view: AgentViewSnapshot,
    pub(crate) rows: Arc<[Row]>,
}
impl std::ops::Deref for Checkpoint {
    type Target = [Row];
    fn deref(&self) -> &[Row] {
        &self.rows
    }
}
pub type Observation = crate::machine_observation::outcome::Observation<Checkpoint>;
