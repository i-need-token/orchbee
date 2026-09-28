//! Bounded pure provider screen rules; neither terminal identity nor explicit report authority is inferred.
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_workbench_protocol::{AgentScreenExplanation, PanelAgentState};

mod loading;
mod manifest;
mod regions;
mod stability;
#[cfg(windows)]
pub(crate) mod updating;
mod version;
pub(crate) use stability::Stability;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScreenInput<'a> {
    pub(crate) screen: &'a str,
    pub(crate) title: &'a str,
    pub(crate) progress: &'a str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Detection {
    pub(crate) state: PanelAgentState,
    pub(crate) skip: bool,
    pub(crate) visible_idle: bool,
    pub(crate) visible_blocker: bool,
    pub(crate) visible_working: bool,
}
impl Detection {
    pub(crate) fn fallback(known: bool) -> Self {
        Self {
            state: if known {
                PanelAgentState::Idle
            } else {
                PanelAgentState::Unknown
            },
            skip: false,
            visible_idle: false,
            visible_blocker: false,
            visible_working: false,
        }
    }
}
/// Fully validated compiled rules; callers retain the engine instead of recompiling per sample.
pub(crate) struct ScreenDetector(Vec<manifest::Manifest>);
fn bounded(input: ScreenInput<'_>) -> Result<(), &'static str> {
    if input.screen.len() > 64 * 1024 || input.title.len() > 4096 || input.progress.len() > 4096 {
        Err("Screen detection input exceeds bound")
    } else {
        Ok(())
    }
}
fn fallback_explanation(provider: AgentReportProvider) -> AgentScreenExplanation {
    AgentScreenExplanation {
        manifest: None,
        agent: provider.report_label().into(),
        state: PanelAgentState::Idle,
        manifest_source: None,
        manifest_version: None,
        matched_rule: None,
        visible_idle: false,
        visible_blocker: false,
        visible_working: false,
        screen_detection_skipped: false,
        screen_detection_skip_reason: None,
        skip_state_update: false,
        skipped_update_reason: None,
        fallback_reason: Some("default-known-agent-idle-fallback".into()),
        evaluated_rules: vec![],
    }
}
impl ScreenDetector {
    pub(crate) fn summaries(&self) -> Vec<orchbee_workbench_protocol::AgentManifestSummary> {
        self.0
            .iter()
            .map(|m| orchbee_workbench_protocol::AgentManifestSummary {
                agent: m.provider.report_label().into(),
                version: m.version.clone(),
                manifest: m.provenance.clone(),
            })
            .collect()
    }
    /// Parses all pinned rules atomically; a bad bundled asset fails the entire engine.
    pub(crate) fn bundled() -> Result<Self, String> {
        manifest::bundled().map(Self)
    }
    /// Interprets bounded decoded content for a separately proven provider; no I/O or state change.
    pub(crate) fn detect(
        &self,
        provider: Option<AgentReportProvider>,
        input: ScreenInput<'_>,
    ) -> Result<Detection, &'static str> {
        bounded(input)?;
        let Some(provider) = provider else {
            return Ok(Detection::fallback(false));
        };
        Ok(self
            .0
            .iter()
            .find(|m| m.provider == provider)
            .map_or_else(|| Detection::fallback(true), |m| m.detect(input)))
    }

    pub(crate) fn explain(
        &self,
        provider: AgentReportProvider,
        input: ScreenInput<'_>,
    ) -> Result<AgentScreenExplanation, &'static str> {
        bounded(input)?;
        let explanation = self
            .0
            .iter()
            .find(|m| m.provider == provider)
            .map_or_else(|| fallback_explanation(provider), |m| m.explain(input));
        explanation
            .validate()
            .map_err(|_| "Invalid screen explanation")?;
        Ok(explanation)
    }
}
