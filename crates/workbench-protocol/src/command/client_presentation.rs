//! Exact Client-local Panel member and existing attachment incarnation; no new state.
use super::client_view::{ClientViewObservation, ClientViewPhase};
use super::client_viewport::ClientViewportObservation;
use crate::{
    ContractError, PresentationBinding, TerminalPresentationId, WorkbenchPanelId, WorkbenchRevision,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPresentationSelection {
    pub presentation_id: TerminalPresentationId,
    pub binding: PresentationBinding,
    pub attachment_generation: WorkbenchRevision,
    pub incarnation: u32,
    pub viewport: ClientViewportObservation,
}
impl ClientPresentationSelection {
    /// Scrolling/frame updates do not change which local member owns this read.
    #[must_use]
    pub fn same_member(&self, other: &Self) -> bool {
        self.presentation_id == other.presentation_id
            && self.binding == other.binding
            && self.attachment_generation == other.attachment_generation
            && self.incarnation == other.incarnation
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPresentationSnapshot {
    pub workbench_revision: WorkbenchRevision,
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub presentation: Option<ClientPresentationSelection>,
}
impl ClientPresentationSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let Some(member) = &self.presentation {
            member.viewport.validate()?;
        }
        if serde_json::to_vec(self)
            .map_err(|_| ContractError::InvalidDocument)?
            .len()
            > MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
    #[must_use]
    pub fn answers(
        &self,
        panel: &WorkbenchPanelId,
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
    ) -> bool {
        self.validate().is_ok()
            && matches!(view.phase, ClientViewPhase::Ready | ClientViewPhase::Busy)
            && self.workbench_revision >= minimum
            && self.workbench_revision >= view.workbench_revision
            && &self.panel_id == panel
    }
}
