//! Commit a complete Host Job sample after physical-generation validation; never infer exit from failure.
use super::SiteCoordinator;
use crate::terminal_coordinator::BindingViewerTarget;
use orchbee_agent_observation_protocol::{
    AgentProcessCandidate, AgentProcessSelection, AgentReportProvider, identify_agent_process,
    select_agent_process,
};
use orchbee_server::{TerminalAgentBinding, TerminalAgentProcessObservation};
use orchbee_terminal_protocol::TerminalProcessInfo;
use orchbee_workbench_protocol::{PanelAgentState, PresentationBinding};
use std::{collections::BTreeMap, time::Instant};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Selected {
    pid: u32,
    created: u64,
    provider: AgentReportProvider,
}
pub(super) struct Sample {
    target: TerminalAgentBinding,
    started: Instant,
    selected: Option<Selected>,
    input_revision: u64,
}
pub(super) type Samples = BTreeMap<(String, String), Sample>;
fn key(binding: &PresentationBinding) -> (String, String) {
    (
        binding.worker_id.as_str().into(),
        binding.terminal_id.as_str().into(),
    )
}
fn identify(sample: &TerminalProcessInfo) -> Option<Selected> {
    let candidates: Vec<_> = sample
        .processes
        .iter()
        .map(|p| {
            let argv = p
                .command_line
                .as_deref()
                .filter(|s| {
                    !s.trim().is_empty() && !s.contains('\0') && s.encode_utf16().count() <= 4096
                })
                .and_then(|s| winsafe::CommandLineToArgv(s).ok());
            AgentProcessCandidate {
                pid: p.pid,
                parent_pid: p.parent_pid,
                creation_time: p.creation_time,
                provider: identify_agent_process(p.image.as_deref(), argv.as_deref())
                    .and_then(AgentReportProvider::lookup),
            }
        })
        .collect();
    match select_agent_process(&candidates) {
        AgentProcessSelection::Selected {
            pid,
            creation_time,
            provider,
        } => Some(Selected {
            pid,
            created: creation_time,
            provider,
        }),
        AgentProcessSelection::None | AgentProcessSelection::Ambiguous => None,
    }
}
impl SiteCoordinator {
    /// Called only after a correlated Host response and current Worker registry fence, under the Site gate.
    pub(crate) fn observe_host_process(
        &mut self,
        target: BindingViewerTarget,
        sample: &TerminalProcessInfo,
        started: Instant,
        now: Instant,
    ) -> bool {
        if started > now
            || sample.terminal_id.as_bytes() != &target.identity.terminal_id().as_bytes()
        {
            return false;
        }
        let Ok(facts) = self.terminal_agent_bindings() else {
            return false;
        };
        let worker = format!(
            "{:032x}",
            u128::from_be_bytes(target.worker.worker_id.as_bytes())
        );
        let terminal = format!(
            "{:032x}",
            u128::from_be_bytes(target.terminal_id.as_bytes())
        );
        let Some(physical) = facts
            .iter()
            .find(|f| {
                f.binding.worker_id.as_str() == worker
                    && f.binding.terminal_id.as_str() == terminal
                    && f.worker_generation == target.worker.generation
                    && f.identity == target.identity
            })
            .cloned()
        else {
            return false;
        };
        // Discard tracking only, never emit an exit from missing coordinator facts.
        self.process_samples
            .retain(|_, s| facts.contains(&s.target));
        let key = key(&physical.binding);
        let prior = self.process_samples.get(&key);
        if prior.is_some_and(|p| p.started >= started) {
            return false;
        }
        let Some(workbench) = self.workbench.as_mut() else {
            return false;
        };
        let input_revision = workbench.terminal_agent_input_revision(&physical);
        let selected = identify(sample);
        let old = prior.and_then(|p| p.selected);
        let departed = old.filter(|old| {
            !sample
                .processes
                .iter()
                .any(|p| p.pid == old.pid && p.creation_time == old.created)
        });
        let (event, retained) = if let Some(old) = departed {
            (Some((old.provider, true)), None)
        } else if let Some(selected) = selected {
            let unchanged = prior.is_some_and(|p| {
                p.selected == Some(selected) && p.input_revision == input_revision
            });
            (
                (!unchanged).then_some((selected.provider, false)),
                Some(selected),
            )
        } else {
            (None, old)
        };
        if let Some((provider, exited)) = event
            && workbench
                .observe_terminal_agent(
                    &physical,
                    &facts,
                    TerminalAgentProcessObservation {
                        observed_at: started,
                        agent: Some(provider),
                        state: PanelAgentState::Unknown,
                        visible_blocker: false,
                        exited,
                    },
                    now,
                )
                .is_err()
        {
            return false;
        }
        let input_revision = workbench.terminal_agent_input_revision(&physical);
        self.process_samples.insert(
            key,
            Sample {
                target: physical,
                started,
                selected: retained,
                input_revision,
            },
        );
        true
    }
}
