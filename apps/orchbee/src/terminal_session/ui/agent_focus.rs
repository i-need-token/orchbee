//! Explicit Agent focus retains subject identity and acknowledges one selected Tab.
use super::NativeUi;
use orchbee_agent_observation_protocol::AgentNameTarget;
use orchbee_workbench_protocol::{
    ClientViewControl, ClientViewControlReason as Reason, ClientViewIntent,
};

impl NativeUi {
    pub(in crate::terminal_session) fn agent_focus_source(
        &self,
        command: &ClientViewControl,
    ) -> Result<(), Reason> {
        let ClientViewIntent::AgentFocus {
            presentation_id,
            target,
            ..
        } = &command.intent
        else {
            return Ok(());
        };
        match self
            .attention
            .focus_matches(target, presentation_id.as_str())
        {
            Some(true) => Ok(()),
            Some(false) => Err(Reason::TargetUnavailable),
            None => Err(Reason::ProjectionUnavailable),
        }
    }

    /// Called only once the requested selection is observed; None-frame is still pending attach.
    pub(in crate::terminal_session) fn agent_focus_ready(
        &self,
        command: &ClientViewControl,
    ) -> Result<bool, Reason> {
        self.agent_focus_source(command)?;
        let ClientViewIntent::AgentFocus {
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            target,
        } = &command.intent
        else {
            return Ok(true);
        };
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        let cache = state.cache();
        if cache.terminal_context_id() != terminal_context_id {
            return Err(Reason::TargetUnavailable);
        }
        let tab = cache
            .tab_index(tab_id)
            .and_then(|index| cache.tabs().get(index))
            .ok_or(Reason::TargetUnavailable)?;
        let panel = (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .find(|panel| panel.panel_id() == panel_id)
            .ok_or(Reason::TargetUnavailable)?;
        if panel.presentation_id() != presentation_id {
            return Err(Reason::TargetUnavailable);
        }
        if let AgentNameTarget::Terminal { binding } = target {
            let view = self.view();
            if view.pane_projection_status(panel_id)
                != super::NativeProjectionStatus::FrameAvailable
            {
                return Ok(false);
            }
            let Some(projection) = view.pane_projection(panel_id) else {
                return Ok(false);
            };
            let Some(frame) = projection.frame() else {
                return Ok(false);
            };
            if frame.route.host_epoch.to_text() != binding.host_epoch.as_str()
                || projection.host_terminal_id() != Some(binding.host_terminal_id.as_str())
            {
                return Err(Reason::TargetUnavailable);
            }
        }
        Ok(true)
    }

    pub(in crate::terminal_session) fn finish_agent_focus(
        &mut self,
        command: &ClientViewControl,
    ) -> Result<(), Reason> {
        let (_, view) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if !command
            .intent
            .is_observed(&view, command.minimum_workbench_revision)
        {
            return Err(Reason::StaleView);
        }
        if !self.agent_focus_ready(command)? {
            return Err(Reason::ProjectionUnavailable);
        }
        let ClientViewIntent::AgentFocus { tab_id, .. } = &command.intent else {
            return Ok(());
        };
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        let tab = state
            .cache()
            .tab_index(tab_id)
            .and_then(|index| state.cache().tabs().get(index))
            .ok_or(Reason::TargetUnavailable)?;
        let presentations: Vec<_> = (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .map(|panel| panel.presentation_id().as_str().to_owned())
            .collect();
        self.attention
            .acknowledge_focused_tab(&presentations.iter().map(String::as_str).collect());
        Ok(())
    }
}
