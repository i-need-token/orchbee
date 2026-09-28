//! Commit a complete Host Job sample after physical-generation validation; never infer exit from failure.
use super::SiteCoordinator;
use crate::agent_screen::{Detection, Stability};
use crate::terminal_coordinator::BindingViewerTarget;
use orchbee_agent_observation_protocol::{
    AgentProcessCandidate, AgentProcessSelection, AgentReportProvider, identify_agent_process,
    select_agent_process,
};
use orchbee_server::{TerminalAgentBinding, TerminalAgentProcessObservation};
use orchbee_terminal_protocol::TerminalProcessInfo;
use orchbee_workbench_protocol::PresentationBinding;
use std::{collections::BTreeMap, time::Instant};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Selected {
    pid: u32,
    created: u64,
    pub(crate) provider: AgentReportProvider,
}
pub(super) struct Sample {
    target: TerminalAgentBinding,
    started: Instant,
    selected: Option<Selected>,
    input_revision: u64,
    first_seen: Instant,
    screen: Stability,
    screen_input_revision: u64,
}
pub(super) type Samples = BTreeMap<(String, String), Sample>;
fn key(binding: &PresentationBinding) -> (String, String) {
    (
        binding.worker_id.as_str().into(),
        binding.terminal_id.as_str().into(),
    )
}
pub(crate) fn identify(sample: &TerminalProcessInfo) -> Option<Selected> {
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
    #[cfg(test)]
    pub(crate) fn observe_host_process(
        &mut self,
        target: BindingViewerTarget,
        sample: &TerminalProcessInfo,
        started: Instant,
        now: Instant,
    ) -> bool {
        self.observe_host_screen(target, sample, None, started, now)
    }
    /// Screen evidence must be bounded and associated with the same process on both sides of capture.
    pub(crate) fn observe_host_screen(
        &mut self,
        target: BindingViewerTarget,
        sample: &TerminalProcessInfo,
        detection: Option<Detection>,
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
        let same = prior.filter(|p| p.selected == retained);
        let first_seen = same.map_or(started, |p| p.first_seen);
        let mut screen = same.map_or_else(
            || Stability::new(Detection::fallback(false)),
            |p| p.screen.clone(),
        );
        let mut screen_input_revision = same.map_or(0, |p| p.screen_input_revision);
        let mut fallback = None;
        if departed.is_none()
            && selected.is_some()
            && selected == retained
            && let Some(detection) = detection
            && started.saturating_duration_since(first_seen) >= std::time::Duration::from_secs(3)
        {
            let Ok(tick) = u64::try_from(started.saturating_duration_since(first_seen).as_millis())
            else {
                return false;
            };
            let Ok(prepared) = screen.prepare(detection, tick, false) else {
                return false;
            };
            screen = prepared.next;
            let republish = screen_input_revision != input_revision
                && !detection.skip
                && screen.published() == detection;
            if !detection.skip && screen.published() == detection {
                screen_input_revision = input_revision;
            }
            fallback = prepared
                .publish
                .or(republish.then_some(detection))
                .map(|p| orchbee_server::TerminalAgentScreenObservation {
                    state: p.state,
                    visible_blocker: p.visible_blocker,
                });
        }
        let event = event.or_else(|| fallback.and(selected.map(|p| (p.provider, false))));
        if let Some((provider, exited)) = event
            && workbench
                .observe_terminal_agent(
                    &physical,
                    &facts,
                    TerminalAgentProcessObservation {
                        observed_at: started,
                        agent: Some(provider),
                        fallback,
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
                first_seen,
                screen,
                screen_input_revision,
            },
        );
        true
    }
}

/// A committed target captured before sampling; admission must still match this exact physical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessObservationTarget {
    pub(crate) site_id: orchbee_workbench_protocol::SiteId,
    pub(crate) binding: PresentationBinding,
    pub(crate) target: BindingViewerTarget,
}
impl ProcessObservationTarget {
    /// Stable logical ordering for fair sampling across membership changes.
    pub(crate) fn key(&self) -> (String, String) {
        key(&self.binding)
    }
}
impl SiteCoordinator {
    /// Returns current unfenced registrations and prunes tracking only after a successful full scan.
    pub(crate) fn process_observation_targets(
        &mut self,
    ) -> Result<Vec<ProcessObservationTarget>, orchbee_server::WorkbenchTerminalInvariantError>
    {
        use orchbee_server::WorkbenchTerminalInvariantError as Invalid;
        let Some(workbench) = self.workbench.as_ref() else {
            self.process_samples.clear();
            return Ok(vec![]);
        };
        let site_id = workbench.site_id().clone();
        let facts = self.agent_registration_snapshot()?;
        let mut targets = BTreeMap::new();
        let mut retained = Vec::new();
        for fact in facts {
            let binding = fact.binding().clone();
            retained.push(TerminalAgentBinding {
                binding: binding.clone(),
                worker_generation: fact.worker_generation(),
                identity: fact.identity(),
            });
            let terminal_id = u128::from_str_radix(binding.terminal_id.as_str(), 16)
                .ok()
                .and_then(|n| {
                    orchbee_capability_node_protocol::TerminalId::from_bytes(n.to_be_bytes()).ok()
                })
                .ok_or(Invalid)?;
            let target = BindingViewerTarget {
                terminal_id,
                identity: fact.identity(),
                worker: crate::terminal_coordinator::WorkerPlacement {
                    worker_id: fact.worker_id(),
                    generation: fact.worker_generation(),
                },
            };
            if self.target_lifecycle(target) != super::TargetLifecycle::Unfenced {
                continue;
            }
            let candidate = ProcessObservationTarget {
                site_id: site_id.clone(),
                binding,
                target,
            };
            if let Some(previous) = targets.insert(candidate.key(), candidate.clone())
                && previous != candidate
            {
                return Err(Invalid);
            }
        }
        self.process_samples
            .retain(|_, s| retained.contains(&s.target));
        Ok(targets.into_values().collect())
    }
}
