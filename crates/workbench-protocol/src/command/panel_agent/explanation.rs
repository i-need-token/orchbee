//! Bounded diagnostics from an actual terminal capture, distinct from effective report state.
use super::{AgentDetectionManifest, PanelAgentState, PanelAgentView, nullable};
use crate::{ContractError, WorkbenchRevision};
use orchbee_agent_observation_protocol::{AgentLaunchBinding, AgentReportProvider};
use serde::{Deserialize, Serialize};

fn present_manifest<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<AgentDetectionManifest>, D::Error> {
    AgentDetectionManifest::deserialize(d).map(Some)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDetectionRule {
    pub id: String,
    pub priority: i32,
    pub region: String,
    pub state: PanelAgentState,
}
impl AgentDetectionRule {
    fn valid(&self) -> bool {
        !self.id.trim().is_empty()
            && self.id.chars().count() <= 128
            && !self.region.trim().is_empty()
            && self.region.len() <= 128
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDetectionEvidence {
    pub contains: Vec<String>,
    pub regex: Vec<String>,
    pub line_regex: Vec<String>,
    pub all_count: u16,
    pub any_count: u16,
    pub not_count: u16,
    pub region_bytes: u32,
    pub region_preview: String,
}
impl AgentDetectionEvidence {
    fn valid(&self) -> bool {
        self.contains.len() + self.regex.len() + self.line_regex.len() <= 32
            && self
                .contains
                .iter()
                .chain(&self.regex)
                .chain(&self.line_regex)
                .all(|s| s.chars().count() <= 512)
            && self.all_count <= 512
            && self.any_count <= 512
            && self.not_count <= 512
            && self.region_bytes <= 64 * 1024
            && self.region_preview.chars().count() <= 243
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDetectionEvaluation {
    pub rule: AgentDetectionRule,
    pub matched: bool,
    pub evidence: AgentDetectionEvidence,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentScreenExplanation {
    #[serde(
        default,
        deserialize_with = "present_manifest",
        skip_serializing_if = "Option::is_none"
    )]
    pub manifest: Option<AgentDetectionManifest>,
    pub agent: String,
    pub state: PanelAgentState,
    #[serde(deserialize_with = "nullable")]
    pub manifest_source: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub manifest_version: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub matched_rule: Option<AgentDetectionRule>,
    pub visible_idle: bool,
    pub visible_blocker: bool,
    pub visible_working: bool,
    pub screen_detection_skipped: bool,
    #[serde(deserialize_with = "nullable")]
    pub screen_detection_skip_reason: Option<String>,
    pub skip_state_update: bool,
    #[serde(deserialize_with = "nullable")]
    pub skipped_update_reason: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub fallback_reason: Option<String>,
    pub evaluated_rules: Vec<AgentDetectionEvaluation>,
}
impl AgentScreenExplanation {
    pub fn validate(&self) -> Result<(), ContractError> {
        if AgentReportProvider::lookup(&self.agent)
            .is_none_or(|provider| provider.report_label() != self.agent)
            || self
                .manifest_version
                .as_ref()
                .is_some_and(|v| v.chars().count() > 128)
            || self.evaluated_rules.len() > 128
            || !self
                .evaluated_rules
                .iter()
                .all(|r| r.rule.valid() && r.evidence.valid())
        {
            return Err(ContractError::InvalidDocument);
        }
        let valid = if self.screen_detection_skipped {
            self.screen_detection_skip_reason.as_deref() == Some("full-lifecycle-hook-authority")
                && self.manifest.is_none()
                && self.manifest_source.is_none()
                && self.manifest_version.is_none()
                && self.matched_rule.is_none()
                && self.evaluated_rules.is_empty()
                && !self.visible_idle
                && !self.visible_blocker
                && !self.visible_working
                && !self.skip_state_update
                && self.skipped_update_reason.is_none()
                && self.fallback_reason.is_none()
        } else {
            let mut winner: Option<&AgentDetectionRule> = None;
            for evaluated in &self.evaluated_rules {
                if evaluated.matched && winner.is_none_or(|w| evaluated.rule.priority > w.priority)
                {
                    winner = Some(&evaluated.rule);
                }
            }
            self.screen_detection_skip_reason.is_none()
                && (self.manifest.as_ref().is_some_and(|m| {
                    m.valid()
                        && self.manifest_source.as_deref() == Some(m.source.as_str())
                        && (m.source != super::AgentManifestSource::RemoteCache
                            || self.manifest_version == m.cached_remote_version)
                }) || (self.manifest.is_none()
                    && self.manifest_source.as_deref() == Some("bundled"))
                    || (self.manifest_source.is_none()
                        && self.manifest.is_none()
                        && self.manifest_version.is_none()
                        && self.evaluated_rules.is_empty()))
                && self.matched_rule.as_ref() == winner
                && self.state == winner.map_or(PanelAgentState::Idle, |r| r.state)
                && (!self.visible_idle || self.state == PanelAgentState::Idle)
                && (!self.visible_blocker || self.state == PanelAgentState::Blocked)
                && (!self.visible_working || self.state == PanelAgentState::Working)
                && if let Some(winner) = winner {
                    self.fallback_reason.is_none()
                        && if self.skip_state_update {
                            self.state == PanelAgentState::Unknown
                                && !self.visible_idle
                                && !self.visible_blocker
                                && !self.visible_working
                                && self.skipped_update_reason.as_deref()
                                    == Some(format!("matched-rule:{}", winner.id).as_str())
                        } else {
                            self.skipped_update_reason.is_none()
                        }
                } else {
                    self.fallback_reason.as_deref() == Some("default-known-agent-idle-fallback")
                        && !self.visible_idle
                        && !self.visible_blocker
                        && !self.visible_working
                        && !self.skip_state_update
                        && self.skipped_update_reason.is_none()
                }
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDetectionSample {
    pub frame_sequence: WorkbenchRevision,
    pub frame_generation: WorkbenchRevision,
    pub frame_revision: WorkbenchRevision,
    pub root_pid: u32,
    #[serde(deserialize_with = "nullable")]
    pub agent_pid: Option<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalAgentExplanation {
    pub binding: AgentLaunchBinding,
    pub view: PanelAgentView,
    pub explanation: AgentScreenExplanation,
    #[serde(deserialize_with = "nullable")]
    pub sample: Option<AgentDetectionSample>,
}
impl TerminalAgentExplanation {
    pub(super) fn valid(&self) -> bool {
        orchbee_agent_observation_protocol::AgentHostEpoch::parse(self.binding.generation.as_str())
            .is_ok()
            && self.view.valid()
            && self.explanation.validate().is_ok()
            && if self.explanation.screen_detection_skipped {
                self.sample.is_none() && self.view.agent.as_ref() == Some(&self.explanation.agent)
                    && self.view.state == self.explanation.state
                    && self.view.authority_source.as_ref().is_some_and(|source| {
                        orchbee_agent_observation_protocol::agent_report_source_policy(source, &self.explanation.agent)
                            == orchbee_agent_observation_protocol::AgentReportSourcePolicy::FullLifecycle
                    })
            } else {
                self.sample.as_ref().is_some_and(|s| {
                    s.frame_sequence.value() > 0
                        && s.frame_generation.value() > 0
                        && s.root_pid > 0
                        && s.agent_pid != Some(0)
                })
            }
    }
}
