//! Bounded, read-only facts from an interactive Client's installed terminal viewport.
use super::client_view::{ClientViewObservation, ClientViewPhase};
use crate::{ContractError, TerminalId, WorkbenchPanelId, WorkbenchRevision};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAXIMUM_CLIENT_VIEWPORT_PANELS: usize = 16;
pub const MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES: usize = 12 * 1024;

pub(super) fn validate_panels(ids: &[WorkbenchPanelId]) -> Result<(), ContractError> {
    if ids.is_empty()
        || ids.len() > MAXIMUM_CLIENT_VIEWPORT_PANELS
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

mod decimal {
    use super::{Deserialize, Serialize, WorkbenchRevision};
    pub fn serialize<S: serde::Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        WorkbenchRevision::new(*value).serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        WorkbenchRevision::deserialize(deserializer).map(WorkbenchRevision::value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientScrollMetrics {
    #[serde(with = "decimal")]
    pub offset_from_bottom: u64,
    #[serde(with = "decimal")]
    pub max_offset_from_bottom: u64,
    pub viewport_rows: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewportSource {
    pub host_epoch: String,
    pub attachment_id: String,
    #[serde(with = "decimal")]
    pub generation: u64,
    #[serde(with = "decimal")]
    pub revision: u64,
    pub columns: u16,
    pub alternate_screen: bool,
    #[serde(with = "decimal")]
    pub total_rows: u64,
    #[serde(with = "decimal")]
    pub start_row: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewportMode {
    Live,
    History,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewportPending {
    Entering,
    AwaitingWindow,
    Recovering,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientViewportUnavailable {
    NotAttached,
    FrameUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientViewportObservation {
    Unavailable {
        reason: ClientViewportUnavailable,
    },
    Pending {
        phase: ClientViewportPending,
    },
    Observed {
        mode: ClientViewportMode,
        source: ClientViewportSource,
        scroll: ClientScrollMetrics,
    },
}
impl ClientViewportObservation {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let Self::Observed {
            mode,
            source,
            scroll,
        } = self
        {
            let maximum = source
                .total_rows
                .saturating_sub(u64::from(scroll.viewport_rows));
            if crate::SiteId::parse(&source.host_epoch).is_err()
                || crate::SiteId::parse(&source.attachment_id).is_err()
                || source.generation == 0
                || !(1..=4096).contains(&source.columns)
                || !(1..=4096).contains(&scroll.viewport_rows)
                || source.start_row > source.total_rows
                || scroll.max_offset_from_bottom != maximum
                || scroll.offset_from_bottom != maximum.saturating_sub(source.start_row)
                || (*mode == ClientViewportMode::Live && source.start_row != maximum)
                || (source.alternate_screen && (*mode != ClientViewportMode::Live || maximum != 0))
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPanelViewport {
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub primary_terminal_id: Option<TerminalId>,
    pub observation: ClientViewportObservation,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientViewportSnapshot {
    pub workbench_revision: WorkbenchRevision,
    pub panels: Vec<ClientPanelViewport>,
}
impl ClientViewportSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_panels(
            &self
                .panels
                .iter()
                .map(|row| row.panel_id.clone())
                .collect::<Vec<_>>(),
        )?;
        for row in &self.panels {
            row.observation.validate()?;
            if row.primary_terminal_id.is_none()
                && !matches!(
                    row.observation,
                    ClientViewportObservation::Unavailable {
                        reason: ClientViewportUnavailable::NotAttached
                    }
                )
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| ContractError::InvalidDocument)?
            .len()
            > MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
    #[must_use]
    pub fn answers(
        &self,
        ids: &[WorkbenchPanelId],
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
    ) -> bool {
        self.validate().is_ok()
            && matches!(view.phase, ClientViewPhase::Ready | ClientViewPhase::Busy)
            && self.workbench_revision >= minimum
            && self.workbench_revision >= view.workbench_revision
            && self.panels.len() == ids.len()
            && self
                .panels
                .iter()
                .zip(ids)
                .all(|(row, id)| &row.panel_id == id)
    }
}
