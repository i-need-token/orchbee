//! Source-owned Panel presentation reports with strict bounded read and write envelopes.
use crate::{
    ContractError, MetadataReportDisposition, MetadataSource, MetadataTokenPatch,
    MetadataTokenValue, MetadataTokens, PresentationBinding, ProtocolVersion, SessionId,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
};

fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

pub const MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES: usize = 512 * 1024;

pub const MAXIMUM_PANEL_METADATA_READ_ROWS: usize = 4;
pub const MAXIMUM_PANEL_METADATA_SOURCES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataState {
    Idle,
    Working,
    Blocked,
    Done,
    Error,
    Unknown,
}

fn state_labels<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<BTreeMap<MetadataState, T>, D::Error> {
    struct Labels<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Labels<T> {
        type Value = BTreeMap<MetadataState, T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unique display-state labels")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<MetadataState, T>()? {
                if values.insert(key, value).is_some() {
                    return Err(de::Error::custom("duplicate display-state label"));
                }
            }
            Ok(values)
        }
    }
    d.deserialize_map(Labels(PhantomData))
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<MetadataTokenValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_to_source: Option<MetadataSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<MetadataTokenValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_agent: Option<MetadataTokenValue>,
    #[serde(
        default,
        deserialize_with = "state_labels",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub state_labels: BTreeMap<MetadataState, MetadataTokenValue>,
    #[serde(default)]
    pub clear_title: bool,
    #[serde(default)]
    pub clear_display_agent: bool,
    #[serde(default)]
    pub clear_state_labels: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<MetadataTokenPatch>,
}
impl PanelMetadataPatch {
    pub fn clears_presentation(&self) -> bool {
        self.clear_title || self.clear_display_agent || self.clear_state_labels
    }
    pub fn sets_presentation(&self) -> bool {
        self.title.is_some() || self.display_agent.is_some() || !self.state_labels.is_empty()
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if (self.clear_title && self.title.is_some())
            || (self.clear_display_agent && self.display_agent.is_some())
            || (self.clear_state_labels && !self.state_labels.is_empty())
            || !(self.clears_presentation() || self.sets_presentation() || self.tokens.is_some())
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
pub enum PanelMetadataAction {
    ReadPresentations {
        presentation_ids: Vec<TerminalPresentationId>,
    },
    Read {
        panel_ids: Vec<WorkbenchPanelId>,
    },
    ReportPresentation {
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        expected_binding: PresentationBinding,
        source: MetadataSource,
        patch: Box<PanelMetadataPatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<WorkbenchRevision>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttl_milliseconds: Option<u32>,
    },
    Report {
        panel_id: WorkbenchPanelId,
        expected_binding: PresentationBinding,
        source: MetadataSource,
        patch: Box<PanelMetadataPatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<WorkbenchRevision>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttl_milliseconds: Option<u32>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: PanelMetadataAction,
}
impl PanelMetadataRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            PanelMetadataAction::ReadPresentations { presentation_ids } => {
                if presentation_ids.len() > MAXIMUM_PANEL_METADATA_READ_ROWS
                    || presentation_ids.iter().collect::<BTreeSet<_>>().len()
                        != presentation_ids.len()
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            PanelMetadataAction::Read { panel_ids } => {
                if panel_ids.len() > MAXIMUM_PANEL_METADATA_READ_ROWS
                    || panel_ids.iter().collect::<BTreeSet<_>>().len() != panel_ids.len()
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
            PanelMetadataAction::Report {
                patch,
                ttl_milliseconds,
                ..
            }
            | PanelMetadataAction::ReportPresentation {
                patch,
                ttl_milliseconds,
                ..
            } => {
                patch.validate()?;
                if ttl_milliseconds
                    .is_some_and(|v| !(1..=crate::MAXIMUM_METADATA_TTL_MILLISECONDS).contains(&v))
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataField {
    pub value: MetadataTokenValue,
    pub order: WorkbenchRevision,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataPresentation {
    pub source: MetadataSource,
    #[serde(deserialize_with = "required_nullable")]
    pub agent: Option<MetadataTokenValue>,
    #[serde(deserialize_with = "required_nullable")]
    pub applies_to_source: Option<MetadataSource>,
    #[serde(deserialize_with = "required_nullable")]
    pub title: Option<PanelMetadataField>,
    #[serde(deserialize_with = "required_nullable")]
    pub display_agent: Option<PanelMetadataField>,
    #[serde(deserialize_with = "state_labels")]
    pub state_labels: BTreeMap<MetadataState, PanelMetadataField>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataRow {
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "required_nullable")]
    pub binding: Option<PresentationBinding>,
    pub tokens: MetadataTokens,
    pub presentations: Vec<PanelMetadataPresentation>,
}
/// An exact primary or child member; the nested row describes this member, not its Panel primary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationMetadataRow {
    pub presentation_id: TerminalPresentationId,
    pub panel: PanelMetadataRow,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataEffective {
    #[serde(deserialize_with = "required_nullable")]
    pub title: Option<MetadataTokenValue>,
    #[serde(deserialize_with = "required_nullable")]
    pub display_agent: Option<MetadataTokenValue>,
    #[serde(deserialize_with = "state_labels")]
    pub state_labels: BTreeMap<MetadataState, MetadataTokenValue>,
}
impl PanelMetadataRow {
    /// Caller supplies actual Agent/authority facts. Missing facts never satisfy a guard.
    /// Ordering comes from the owner's accepted writes, not caller clock or vector order.
    pub fn presentation(
        &self,
        agent: Option<&str>,
        authority_source: Option<&str>,
    ) -> PanelMetadataEffective {
        let mut title: Option<&PanelMetadataField> = None;
        let mut display: Option<&PanelMetadataField> = None;
        let mut labels: BTreeMap<MetadataState, &PanelMetadataField> = BTreeMap::new();
        for record in &self.presentations {
            if record
                .agent
                .as_ref()
                .is_some_and(|v| Some(v.as_str()) != agent)
                || record
                    .applies_to_source
                    .as_ref()
                    .is_some_and(|v| Some(v.as_str()) != authority_source)
            {
                continue;
            }
            if let Some(field) = &record.title
                && title.is_none_or(|old| field.order > old.order)
            {
                title = Some(field);
            }
            if let Some(field) = &record.display_agent
                && display.is_none_or(|old| field.order > old.order)
            {
                display = Some(field);
            }
            for (state, field) in &record.state_labels {
                if labels.get(state).is_none_or(|old| field.order > old.order) {
                    labels.insert(*state, field);
                }
            }
        }
        PanelMetadataEffective {
            title: title.map(|f| f.value.clone()),
            display_agent: display.map(|f| f.value.clone()),
            state_labels: labels
                .into_iter()
                .map(|(s, f)| (s, f.value.clone()))
                .collect(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelMetadataFailure {
    InvalidRequest,
    #[serde(rename = "session-mismatch")]
    SessionMismatch,
    NotFound,
    Unbound,
    BindingChanged,
    Denied,
    Unavailable,
    TokenLimit,
    SourceLimit,
    ResourceLimit,
    RevisionExhausted,
    DeadlineOverflow,
    ResultTooLarge,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PanelMetadataOutcome {
    PresentationsRead {
        workbench_revision: WorkbenchRevision,
        metadata_revision: WorkbenchRevision,
        presentations: Vec<PresentationMetadataRow>,
    },
    Read {
        workbench_revision: WorkbenchRevision,
        metadata_revision: WorkbenchRevision,
        panels: Vec<PanelMetadataRow>,
    },
    PresentationReported {
        workbench_revision: WorkbenchRevision,
        metadata_revision: WorkbenchRevision,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        binding: PresentationBinding,
        disposition: MetadataReportDisposition,
    },
    Reported {
        workbench_revision: WorkbenchRevision,
        metadata_revision: WorkbenchRevision,
        panel_id: WorkbenchPanelId,
        binding: PresentationBinding,
        disposition: MetadataReportDisposition,
    },
    Failed {
        reason: PanelMetadataFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelMetadataResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: PanelMetadataOutcome,
}
impl PanelMetadataResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            PanelMetadataOutcome::Read {
                metadata_revision,
                panels,
                ..
            } => {
                if panels.len() > MAXIMUM_PANEL_METADATA_READ_ROWS
                    || panels
                        .iter()
                        .map(|p| &p.panel_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != panels.len()
                {
                    return Err(ContractError::InvalidDocument);
                }
                for panel in panels {
                    panel.validate_at(*metadata_revision)?;
                }
            }
            PanelMetadataOutcome::PresentationsRead {
                metadata_revision,
                presentations,
                ..
            } => {
                if presentations.len() > MAXIMUM_PANEL_METADATA_READ_ROWS
                    || presentations
                        .iter()
                        .map(|p| &p.presentation_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != presentations.len()
                {
                    return Err(ContractError::InvalidDocument);
                }
                for row in presentations {
                    row.panel.validate_at(*metadata_revision)?;
                }
            }
            PanelMetadataOutcome::Reported { .. }
            | PanelMetadataOutcome::PresentationReported { .. }
            | PanelMetadataOutcome::Failed { .. } => {}
        }
        Ok(())
    }
    pub fn answers(&self, request: &PanelMetadataRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
            && {
                let shared_failure = matches!(&self.outcome, PanelMetadataOutcome::Failed { .. });
                match &request.action {
                    PanelMetadataAction::ReadPresentations { presentation_ids } => {
                        matches!(&self.outcome, PanelMetadataOutcome::PresentationsRead { presentations, .. } if presentation_ids
                    .iter()
                    .eq(presentations.iter().map(|row| &row.presentation_id)))
                            || shared_failure
                    }
                    PanelMetadataAction::Read { panel_ids } => {
                        matches!(&self.outcome, PanelMetadataOutcome::Read { panels, .. } if panel_ids.iter().eq(panels.iter().map(|p| &p.panel_id)))
                            || shared_failure
                    }
                    PanelMetadataAction::ReportPresentation {
                        panel_id,
                        presentation_id,
                        expected_binding,
                        ..
                    } => {
                        matches!(&self.outcome, PanelMetadataOutcome::PresentationReported {
                                panel_id: actual,
                                presentation_id: actual_presentation,
                                binding,
                                ..
                            } if panel_id == actual
                                && presentation_id == actual_presentation
                                && expected_binding == binding)
                            || shared_failure
                    }
                    PanelMetadataAction::Report {
                        panel_id,
                        expected_binding,
                        ..
                    } => {
                        matches!(&self.outcome, PanelMetadataOutcome::Reported {
                        panel_id: actual,
                        binding,
                        ..
                    } if panel_id == actual && expected_binding == binding)
                            || shared_failure
                    }
                }
            }
    }
}
impl PanelMetadataRow {
    fn validate_at(&self, metadata_revision: WorkbenchRevision) -> Result<(), ContractError> {
        if self.presentations.len() > MAXIMUM_PANEL_METADATA_SOURCES
            || self
                .presentations
                .iter()
                .map(|p| &p.source)
                .collect::<BTreeSet<_>>()
                .len()
                != self.presentations.len()
            || (self.binding.is_none()
                && (!self.presentations.is_empty() || !self.tokens.as_map().is_empty()))
        {
            return Err(ContractError::InvalidDocument);
        }
        let mut writers = BTreeMap::new();
        for p in &self.presentations {
            if p.title.is_none() && p.display_agent.is_none() && p.state_labels.is_empty() {
                return Err(ContractError::InvalidDocument);
            }
            for f in p
                .title
                .iter()
                .chain(p.display_agent.iter())
                .chain(p.state_labels.values())
            {
                if writers
                    .insert(f.order, &p.source)
                    .is_some_and(|previous| previous != &p.source)
                    || f.order == WorkbenchRevision::new(0)
                    || f.order > metadata_revision
                {
                    return Err(ContractError::InvalidDocument);
                }
            }
        }
        Ok(())
    }
}
