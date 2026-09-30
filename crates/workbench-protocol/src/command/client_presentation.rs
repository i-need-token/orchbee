//! Exact Client-local Panel member and existing attachment incarnation; no new state.
use super::client_view::{ClientViewObservation, ClientViewPhase};
use super::client_viewport::ClientViewportObservation;
use crate::{
    ContractError, PresentationBinding, TerminalPresentationId, WorkbenchPanelId, WorkbenchRevision,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ClientPresentationSelection {
    Bound {
        presentation_id: TerminalPresentationId,
        binding: PresentationBinding,
        attachment_generation: WorkbenchRevision,
        incarnation: u32,
        viewport: ClientViewportObservation,
    },
    Unbound {
        presentation_id: TerminalPresentationId,
        selection_generation: WorkbenchRevision,
    },
}
impl ClientPresentationSelection {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Bound { viewport, .. } => viewport.validate()?,
            Self::Unbound {
                selection_generation,
                ..
            } if selection_generation.value() == 0 => return Err(ContractError::InvalidDocument),
            _ => {}
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
    pub const fn presentation_id(&self) -> &TerminalPresentationId {
        match self {
            Self::Bound {
                presentation_id, ..
            }
            | Self::Unbound {
                presentation_id, ..
            } => presentation_id,
        }
    }
    #[must_use]
    pub const fn binding(&self) -> Option<&PresentationBinding> {
        match self {
            Self::Bound { binding, .. } => Some(binding),
            Self::Unbound { .. } => None,
        }
    }
    #[must_use]
    pub const fn viewport(&self) -> Option<&ClientViewportObservation> {
        match self {
            Self::Bound { viewport, .. } => Some(viewport),
            Self::Unbound { .. } => None,
        }
    }
    /// Scrolling/frame updates do not change which local member owns this read.
    #[must_use]
    pub fn same_member(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Bound {
                    presentation_id: left_id,
                    binding: left_binding,
                    attachment_generation: left_generation,
                    incarnation: left_incarnation,
                    ..
                },
                Self::Bound {
                    presentation_id: right_id,
                    binding: right_binding,
                    attachment_generation: right_generation,
                    incarnation: right_incarnation,
                    ..
                },
            ) => {
                left_id == right_id
                    && left_binding == right_binding
                    && left_generation == right_generation
                    && left_incarnation == right_incarnation
            }
            (
                Self::Unbound {
                    presentation_id: left_id,
                    selection_generation: left_generation,
                },
                Self::Unbound {
                    presentation_id: right_id,
                    selection_generation: right_generation,
                },
            ) => left_id == right_id && left_generation == right_generation,
            _ => false,
        }
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
        match &self.presentation {
            Some(ClientPresentationSelection::Bound { viewport, .. }) => viewport.validate()?,
            Some(ClientPresentationSelection::Unbound {
                selection_generation,
                ..
            }) if selection_generation.value() == 0 => return Err(ContractError::InvalidDocument),
            _ => {}
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
