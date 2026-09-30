//! Presentation-only physical title reads; never a report sequence or acknowledgement.
use super::nullable;
use crate::WorkbenchRevision;
use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentLaunchBinding};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_TERMINAL_TITLE_ROWS: usize = 4;
pub const MAXIMUM_TERMINAL_TITLE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalTitleObservation {
    Unavailable {},
    Observed {
        revision: WorkbenchRevision,
        #[serde(deserialize_with = "nullable")]
        title: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalTitleRow {
    pub binding: AgentLaunchBinding,
    pub observation: TerminalTitleObservation,
}
pub(super) fn valid_bindings(bindings: &[AgentLaunchBinding]) -> bool {
    bindings.len() <= MAXIMUM_TERMINAL_TITLE_ROWS
        && bindings.iter().enumerate().all(|(i, b)| {
            AgentHostEpoch::parse(b.generation.as_str()).is_ok()
                && !bindings[..i]
                    .iter()
                    .any(|p| p.presentation_id == b.presentation_id)
        })
}
impl TerminalTitleRow {
    pub(super) fn valid(&self) -> bool {
        AgentHostEpoch::parse(self.binding.generation.as_str()).is_ok()
            && match &self.observation {
                TerminalTitleObservation::Unavailable {} => true,
                TerminalTitleObservation::Observed { revision, title } => {
                    revision.value() != 0
                        && title
                            .as_ref()
                            .is_none_or(|t| t.len() <= MAXIMUM_TERMINAL_TITLE_BYTES)
                }
            }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use crate::ContractError;
    use serde_json::json;
    fn binding(n: u8) -> AgentLaunchBinding {
        serde_json::from_value(json!({"presentationId":format!("{n:032x}"),"generation":format!("{:032x}",2),"hostEpoch":format!("{:032x}",3),"hostTerminalId":format!("{:032x}",4)})).unwrap()
    }
    #[test]
    fn title_reads_are_bounded_exact_and_do_not_answer_state_requests() {
        let mut request = PanelAgentRequest {
            protocol_version: crate::ProtocolVersion,
            request_id: crate::WorkbenchRequestId::parse(&format!("{:032x}", 5)).unwrap(),
            site_id: crate::SiteId::parse(&format!("{:032x}", 6)).unwrap(),
            action: PanelAgentAction::Titles {
                bindings: vec![binding(1)],
            },
        };
        let mut result = PanelAgentResult {
            protocol_version: crate::ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: PanelAgentOutcome::Titles {
                workbench_revision: WorkbenchRevision::new(1),
                presentations: vec![TerminalTitleRow {
                    binding: binding(1),
                    observation: TerminalTitleObservation::Observed {
                        revision: WorkbenchRevision::new(2),
                        title: None,
                    },
                }],
            },
        };
        assert!(request.validate().is_ok());
        assert!(result.answers(&request));
        if let PanelAgentOutcome::Titles { presentations, .. } = &mut result.outcome {
            presentations[0].binding = binding(2);
        }
        assert!(!result.answers(&request));
        request.action = PanelAgentAction::Titles {
            bindings: vec![binding(1); 2],
        };
        assert_eq!(request.validate(), Err(ContractError::InvalidDocument));
        request.action = PanelAgentAction::Titles {
            bindings: (1..=5).map(binding).collect(),
        };
        assert!(request.validate().is_err());
        request.action = PanelAgentAction::States {
            presentation_ids: vec![],
        };
        assert!(!result.answers(&request));
    }
    #[test]
    fn title_observations_require_null_and_positive_revision_and_preserve_full_unicode() {
        for value in [
            json!({"kind":"observed","revision":"1"}),
            json!({"kind":"observed","revision":"1","title":null,"extra":true}),
        ] {
            assert!(serde_json::from_value::<TerminalTitleObservation>(value).is_err());
        }
        let mut row = TerminalTitleRow {
            binding: binding(1),
            observation: TerminalTitleObservation::Observed {
                revision: WorkbenchRevision::new(0),
                title: None,
            },
        };
        assert!(!row.valid());
        row.observation = TerminalTitleObservation::Observed {
            revision: WorkbenchRevision::new(1),
            title: Some("🙂".repeat(16384)),
        };
        assert!(row.valid());
        if let TerminalTitleObservation::Observed { title: Some(t), .. } = &mut row.observation {
            t.push('a');
        }
        assert!(!row.valid());
        row.observation = TerminalTitleObservation::Unavailable {};
        assert!(row.valid());
    }
}
