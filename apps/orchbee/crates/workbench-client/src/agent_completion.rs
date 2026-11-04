//! Client presentation of a quiescent Agent after work; never rewrites provider observations or
//! asserts success. Creation-qualified observation owners retain this value and acknowledge input.
use orchbee_workbench_protocol::PanelAgentState;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Completion {
    #[default]
    None,
    Pending,
    Edited,
}

impl Completion {
    /// Fold an observation of the same physical Agent. A different version of explicit Done is
    /// a new completion; quiet Idle samples preserve the prior completion or input acknowledgement.
    pub(crate) fn observe(
        self,
        before: PanelAgentState,
        after: PanelAgentState,
        same_version: bool,
    ) -> Self {
        match after {
            PanelAgentState::Done if !same_version => Self::Pending,
            PanelAgentState::Idle if before == PanelAgentState::Working => Self::Pending,
            PanelAgentState::Idle | PanelAgentState::Done => self,
            _ => Self::None,
        }
    }

    /// Project only display state. The caller keeps the raw state and semantic sequence intact.
    pub(crate) fn display(self, observed: PanelAgentState) -> PanelAgentState {
        match (self, observed) {
            (Self::Pending, PanelAgentState::Idle) => PanelAgentState::Done,
            (Self::Edited, PanelAgentState::Done) => PanelAgentState::Idle,
            _ => observed,
        }
    }
}
