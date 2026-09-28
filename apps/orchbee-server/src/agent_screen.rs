//! Bounded pure provider screen rules; neither terminal identity nor explicit report authority is inferred.
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_workbench_protocol::PanelAgentState;

mod manifest;
mod regions;
mod stability;
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
    fn fallback(known: bool) -> Self {
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
impl ScreenDetector {
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
        if input.screen.len() > 64 * 1024 || input.title.len() > 4096 || input.progress.len() > 4096
        {
            return Err("Screen detection input exceeds bound");
        }
        let Some(provider) = provider else {
            return Ok(Detection::fallback(false));
        };
        Ok(self
            .0
            .iter()
            .find(|m| m.provider == provider)
            .map_or_else(|| Detection::fallback(true), |m| m.detect(input)))
    }
}
