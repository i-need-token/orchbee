//! Commit a complete Host Job sample after physical-generation validation; never infer exit from failure.
use super::SessionCoordinator;
use crate::agent_screen::{Detection, Stability};
use crate::terminal_coordinator::BindingViewerTarget;
use orchbee_agent_observation_protocol::{
    AgentProcessRow, AgentReportProvider, AgentSampleVerdict, classify_agent_process,
    evaluate_agent_sample,
};
use orchbee_server::{TerminalAgentBinding, TerminalAgentProcessObservation};
use orchbee_terminal_protocol::{
    MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalProcessConsole, TerminalProcessInfo,
};
use orchbee_workbench_protocol::PresentationBinding;
use std::{collections::BTreeMap, time::Instant};
mod command_line;
mod identity;
#[cfg(all(test, windows))]
pub(crate) use identity::SETTLE;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Selected {
    pid: u32,
    created: u64,
    pub(crate) provider: AgentReportProvider,
}
impl Selected {
    pub(crate) fn pid(self) -> u32 {
        self.pid
    }
    /// The process creation time in sample row units.
    pub(crate) fn created(self) -> u64 {
        self.created
    }
}
pub(super) struct Sample {
    target: TerminalAgentBinding,
    started: Instant,
    identity: identity::IdentityState,
    input_revision: u64,
    first_seen: Instant,
    /// Whether a screen rule or program status has recognised this run's state since first sight.
    recognised: bool,
    screen: Stability,
    screen_input_revision: u64,
    directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    /// The published run and the arguments after its program, read from that run's own row when
    /// it is interactive for the run's provider: the only command line a resume source reads.
    #[cfg(windows)]
    command_line: Option<(Selected, Vec<String>)>,
}
pub(super) type Samples = BTreeMap<(String, String), Sample>;
pub(super) fn key(binding: &PresentationBinding) -> (String, String) {
    (
        binding.worker_id.as_str().into(),
        binding.terminal_id.as_str().into(),
    )
}
/// The arguments of one Host-reported command line, or `None` when it is blank, carries a NUL,
/// exceeds the Host's bound, or cannot be split on this build.
pub(crate) fn decoded_argv(command_line: Option<&str>) -> Option<Vec<String>> {
    command_line
        .filter(|s| !s.trim().is_empty() && !s.contains('\0') && s.encode_utf16().count() <= 4096)
        .and_then(command_line::argv)
}
/// Decodes one complete Host sample into identity rows.
fn rows(sample: &TerminalProcessInfo) -> Vec<AgentProcessRow> {
    sample
        .processes
        .iter()
        .map(|p| {
            let argv = decoded_argv(p.command_line.as_deref());
            AgentProcessRow {
                pid: p.pid,
                parent_pid: p.parent_pid,
                creation_time: p.creation_time,
                opaque: p.image.is_none(),
                detached: matches!(
                    p.console,
                    TerminalProcessConsole::Detached | TerminalProcessConsole::None
                ),
                recognition: classify_agent_process(p.image.as_deref(), argv.as_deref()),
            }
        })
        .collect()
}
/// Windows sample rows carry FILETIME creation times: 100 ns units since 1601.
const UNIX_EPOCH_FILETIME: u64 = 116_444_736_000_000_000;

/// Decodes the Host's start time into row creation-time units, rounded up so that a later
/// creation time still proves a process started after the Host read the start marker.
fn open_invocation(
    command: orchbee_terminal_protocol::TerminalObservedCommand,
    shell: u32,
) -> identity::Open {
    identity::Open {
        command: command.id,
        shell,
        started: command.started_at.and_then(|nanoseconds| {
            nanoseconds
                .get()
                .div_ceil(100)
                .checked_add(UNIX_EPOCH_FILETIME)
        }),
    }
}

/// Converts a Host real-time reading to row creation-time units, rounded down so that a process
/// created no later than the reading never compares after it.
pub(crate) fn host_time_as_creation(unix_nanoseconds: std::num::NonZeroU64) -> Option<u64> {
    (unix_nanoseconds.get() / 100).checked_add(UNIX_EPOCH_FILETIME)
}

fn evaluate(sample: &TerminalProcessInfo) -> (Vec<AgentProcessRow>, AgentSampleVerdict) {
    let rows = rows(sample);
    let verdict =
        evaluate_agent_sample(&rows, sample.processes.len() >= MAXIMUM_PROCESS_SAMPLE_ROWS);
    (rows, verdict)
}
/// The anchor one sample names on its own; it only brackets screen capture, while the
/// identity state alone decides which run Panel reports attribute.
pub(crate) fn identify(sample: &TerminalProcessInfo) -> Option<Selected> {
    match evaluate(sample).1 {
        AgentSampleVerdict::Anchor(anchor)
        | AgentSampleVerdict::Unresolved {
            anchor: Some(anchor),
            ..
        } => Some(Selected {
            pid: anchor.pid,
            created: anchor.creation_time,
            provider: anchor.provider,
        }),
        AgentSampleVerdict::Unresolved { anchor: None, .. }
        | AgentSampleVerdict::None
        | AgentSampleVerdict::Remote => None,
    }
}
impl From<identity::Run> for Selected {
    fn from(run: identity::Run) -> Self {
        Self {
            pid: run.pid,
            created: run.created,
            provider: run.provider,
        }
    }
}
impl Sample {
    fn published(&self) -> Option<Selected> {
        self.identity.published().map(Selected::from)
    }
}
impl SessionCoordinator {
    /// Called only after a correlated Host response and current Worker registry fence, under the Session gate.
    #[cfg(all(test, windows))]
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
    #[cfg(all(test, windows))]
    pub(crate) fn observe_host_screen(
        &mut self,
        target: BindingViewerTarget,
        sample: &TerminalProcessInfo,
        detection: Option<Detection>,
        started: Instant,
        now: Instant,
    ) -> bool {
        self.observe_host_screen_with_directory(target, sample, detection, None, started, now)
    }

    pub(crate) fn observe_host_screen_with_directory(
        &mut self,
        target: BindingViewerTarget,
        sample: &TerminalProcessInfo,
        detection: Option<Detection>,
        directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
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
        #[cfg(windows)]
        self.retain_carried_agent_sessions(&facts);
        let key = key(&physical.binding);
        let prior = self.process_samples.get(&key);
        if prior.is_some_and(|p| p.started >= started) {
            return false;
        }
        let directory = directory.or_else(|| prior.and_then(|sample| sample.directory.clone()));
        // The projection read after this sample's rows supplies the open command.
        let open = self
            .open_command(&physical)
            .map(|command| open_invocation(command, sample.root_pid));
        let Some(workbench) = self.workbench.as_mut() else {
            return false;
        };
        let input_revision = workbench.terminal_agent_input_revision(&physical);
        let (rows, verdict) = evaluate(sample);
        let mut identity = prior
            .filter(|p| p.target == physical)
            .map_or_else(identity::IdentityState::default, |p| p.identity.clone());
        let old = prior.and_then(Sample::published);
        let departed = identity
            .observe(started, &rows, verdict, open)
            .filter(|run| run.published)
            .map(Selected::from);
        let retained = identity.published().map(Selected::from);
        // The sample's own anchor must still be the published run's for screen evidence.
        let selected = identify(sample).filter(|selected| Some(*selected) == retained);
        let event = if let Some(old) = departed {
            Some((old.provider, true))
        } else if let Some(retained) = retained {
            let unchanged =
                old == Some(retained) && prior.is_some_and(|p| p.input_revision == input_revision);
            (!unchanged).then_some((retained.provider, false))
        } else {
            None
        };
        let same = prior.filter(|p| p.published() == retained);
        let first_seen = same.map_or(started, |p| p.first_seen);
        let recognised = same.is_some_and(|p| p.recognised);
        // A newly seen run reads as opening until its screen is recognised or settles.
        let mut screen = same.map_or_else(
            || Stability::new(Detection::opening()),
            |p| p.screen.clone(),
        );
        let mut screen_input_revision = same.map_or(0, |p| p.screen_input_revision);
        #[cfg(windows)]
        let command_line = published_command_line(sample, retained, same);
        let mut fallback = None;
        let since_seen = started.saturating_duration_since(first_seen);
        if departed.is_none()
            && selected.is_some()
            && let Some(detection) =
                crate::agent_screen::admit_startup_evidence(detection, since_seen, recognised)
        {
            let Ok(tick) = u64::try_from(since_seen.as_millis()) else {
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
        if same.is_none() && departed.is_none() && retained.is_some() && fallback.is_none() {
            fallback = Some(orchbee_server::TerminalAgentScreenObservation {
                state: screen.published().state,
                visible_blocker: false,
            });
        }
        let recognised = recognised || screen.published().recognised();
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
        let released = departed.is_some() && retained.is_none();
        self.observe_name_owner(&physical.binding, released);
        if let Some(directory) = &directory {
            self.follow_first_terminal_directory(&physical.binding, directory);
        }
        #[cfg(windows)]
        self.bind_carried_agent_session(&physical, retained, identity.result(), sample);
        self.process_samples.insert(
            key,
            Sample {
                target: physical,
                started,
                identity,
                input_revision,
                first_seen,
                recognised,
                screen,
                screen_input_revision,
                directory,
                #[cfg(windows)]
                command_line,
            },
        );
        true
    }
}

/// The published run's arguments after its program: kept from the same run's earlier sample once
/// read, else read from this sample's row with the run's pid and creation time, and only when that
/// row is interactive for the run's provider.
#[cfg(windows)]
fn published_command_line(
    sample: &TerminalProcessInfo,
    run: Option<Selected>,
    same: Option<&Sample>,
) -> Option<(Selected, Vec<String>)> {
    let run = run?;
    if let Some(kept) = same.and_then(|p| p.command_line.clone()) {
        return Some(kept);
    }
    let row = sample
        .processes
        .iter()
        .find(|p| p.pid == run.pid && p.creation_time == run.created)?;
    let argv = decoded_argv(row.command_line.as_deref());
    orchbee_agent_observation_protocol::interactive_agent_arguments(
        row.image.as_deref(),
        argv.as_deref(),
    )
    .filter(|(provider, _)| *provider == run.provider)
    .map(|(_, arguments)| (run, arguments))
}

/// A committed target captured before sampling; admission must still match this exact physical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessObservationTarget {
    pub(crate) session_id: orchbee_workbench_protocol::SessionId,
    pub(crate) binding: PresentationBinding,
    pub(crate) target: BindingViewerTarget,
}
impl ProcessObservationTarget {
    /// Stable logical ordering for fair sampling across membership changes.
    pub(crate) fn key(&self) -> (String, String) {
        key(&self.binding)
    }
}
impl SessionCoordinator {
    /// The identity result of one exact physical Terminal generation. Before its first committed
    /// sample the generation holds the initial `unknown` with `settling`.
    pub(crate) fn process_identity(
        &self,
        physical: &TerminalAgentBinding,
    ) -> orchbee_agent_observation_protocol::AgentProcessIdentity {
        self.process_samples
            .get(&key(&physical.binding))
            .filter(|sample| sample.target == *physical)
            .map_or_else(
                || identity::IdentityState::default().result(),
                |sample| sample.identity.result(),
            )
    }

    /// The run Panel reports attribute to the exact physical generation, if any.
    pub(crate) fn published_run(&self, physical: &TerminalAgentBinding) -> Option<Selected> {
        self.process_samples
            .get(&key(&physical.binding))
            .filter(|sample| sample.target == *physical)
            .and_then(Sample::published)
    }

    /// The published run of every tracked generation, with that generation's identity result.
    #[cfg(windows)]
    pub(crate) fn published_runs(
        &self,
    ) -> impl Iterator<
        Item = (
            Selected,
            orchbee_agent_observation_protocol::AgentProcessIdentity,
        ),
    > + '_ {
        self.process_samples
            .values()
            .filter_map(|sample| Some((sample.published()?, sample.identity.result())))
    }

    /// The exact physical generation's published run with the arguments after its program, as
    /// that run's own sample row gave them, if any.
    #[cfg(windows)]
    pub(crate) fn published_command_line(
        &self,
        physical: &TerminalAgentBinding,
    ) -> Option<(Selected, &[String])> {
        let sample = self
            .process_samples
            .get(&key(&physical.binding))
            .filter(|sample| sample.target == *physical)?;
        let (run, arguments) = sample.command_line.as_ref()?;
        (sample.published() == Some(*run)).then_some((*run, arguments.as_slice()))
    }

    /// The anchor of the run Panel reports attribute to the exact physical generation, if any.
    /// Read under the same lock as [Self::process_identity], so an `agent` result names it.
    pub(crate) fn process_anchor(
        &self,
        physical: &TerminalAgentBinding,
    ) -> Option<orchbee_agent_observation_protocol::AgentProcessAnchor> {
        self.published_run(physical).map(|run| {
            orchbee_agent_observation_protocol::AgentProcessAnchor {
                pid: run.pid,
                provider: run.provider,
            }
        })
    }

    /// Answers `identify` for the presentation's current Terminal generation: the canonical
    /// owner resolves the generation, and this state supplies its identity under one lock.
    pub(crate) fn identify_presentation(
        &mut self,
        request: &orchbee_workbench_protocol::PanelAgentRequest,
        now: Instant,
    ) -> orchbee_workbench_protocol::PanelAgentOutcome {
        use orchbee_workbench_protocol::{
            PanelAgentAction as Action, PanelAgentFailure as Failure, PanelAgentOutcome as Outcome,
        };
        let Action::Identify { presentation_id } = &request.action else {
            return Outcome::Failed {
                reason: Failure::InvalidRequest,
            };
        };
        let probe = orchbee_workbench_protocol::PanelAgentRequest {
            action: Action::States {
                presentation_ids: vec![presentation_id.clone()],
            },
            ..request.clone()
        };
        let Some(states) = self.panel_agent(probe, now) else {
            return Outcome::Failed {
                reason: Failure::Unavailable,
            };
        };
        let (workbench_revision, binding) = match states.outcome {
            Outcome::States {
                workbench_revision,
                presentations,
                ..
            } => match presentations
                .into_iter()
                .next()
                .and_then(|row| row.snapshot)
            {
                Some(snapshot) => (workbench_revision, snapshot.binding),
                None => {
                    return Outcome::Failed {
                        reason: Failure::Unbound,
                    };
                }
            },
            Outcome::Failed { reason } => return Outcome::Failed { reason },
            _ => {
                return Outcome::Failed {
                    reason: Failure::Unavailable,
                };
            }
        };
        let hex = |bytes: [u8; 16]| format!("{:032x}", u128::from_be_bytes(bytes));
        let Some(physical) = self.terminal_agent_bindings().ok().and_then(|facts| {
            facts.into_iter().find(|fact| {
                hex(fact.identity.creation_id().as_bytes()) == binding.generation.as_str()
                    && hex(fact.identity.host_epoch().as_bytes()) == binding.host_epoch.as_str()
                    && hex(fact.identity.terminal_id().as_bytes())
                        == binding.host_terminal_id.as_str()
            })
        }) else {
            return Outcome::Failed {
                reason: Failure::BindingChanged,
            };
        };
        Outcome::Identified {
            workbench_revision,
            identity: self.process_identity(&physical),
            anchor: self.process_anchor(&physical),
            binding,
        }
    }

    /// A failed sample of an exactly tracked physical target ages its identity; it never ends a run.
    pub(crate) fn observe_host_process_failure(
        &mut self,
        target: BindingViewerTarget,
        started: Instant,
    ) {
        if let Some(sample) = self.process_samples.values_mut().find(|sample| {
            sample.target.worker_generation == target.worker.generation
                && sample.target.identity == target.identity
        }) && sample.started < started
        {
            sample.started = started;
            sample.identity.fail(started);
        }
    }
    /// Scheduling hint only; publication still validates the complete physical/process identity.
    pub(crate) fn process_observation_has_agent(&self, target: &ProcessObservationTarget) -> bool {
        self.process_samples
            .get(&key(&target.binding))
            .is_some_and(|sample| {
                sample.identity.tracking()
                    && sample.target.binding == target.binding
                    && sample.target.worker_generation == target.target.worker.generation
                    && sample.target.identity == target.target.identity
            })
    }

    /// The latest exact-physical directory of a Workspace's first terminal: the first Tab's
    /// leading Panel primary, reported by OSC 7 or else its root process. Never attaches a viewer.
    pub(crate) fn workspace_terminal_directory(
        &self,
        workspace_id: &orchbee_workbench_protocol::WorkbenchWorkspaceId,
    ) -> Option<(
        orchbee_workbench_protocol::WorkerId,
        orchbee_workbench_protocol::WorkbenchDirectoryPath,
    )> {
        let snapshot = self.workbench.as_ref()?.snapshot();
        let workspace = snapshot.navigation.workspace(workspace_id)?;
        let binding = super::directory_label::first_terminal(snapshot, workspace)?;
        let fact = self
            .agent_registration_for_bindings(std::slice::from_ref(binding))
            .ok()?
            .into_iter()
            .next()?;
        let sample = self.process_samples.get(&key(binding))?;
        let physical = TerminalAgentBinding {
            binding: binding.clone(),
            worker_generation: fact.worker_generation(),
            identity: fact.identity(),
        };
        if sample.target != physical {
            return None;
        }
        Some((binding.worker_id.clone(), sample.directory.clone()?))
    }

    /// What a current-directory read samples and what it falls back to: the Workspace's first
    /// terminal as an unfenced observation target, and the Workspace's own directory.
    pub(crate) fn current_directory_sources(
        &mut self,
        session_id: &orchbee_workbench_protocol::SessionId,
        workspace_id: &orchbee_workbench_protocol::WorkbenchWorkspaceId,
        panel: Option<&orchbee_workbench_protocol::CurrentDirectoryPanel>,
    ) -> Result<
        (
            Option<ProcessObservationTarget>,
            Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
        ),
        orchbee_workbench_protocol::CurrentDirectoryFailure,
    > {
        use orchbee_workbench_protocol::CurrentDirectoryFailure as Failure;
        let workbench = self.workbench.as_ref().ok_or(Failure::Unavailable)?;
        if workbench.session_id() != session_id {
            return Err(Failure::Denied);
        }
        let snapshot = workbench.snapshot();
        let workspace = snapshot
            .navigation
            .workspace(workspace_id)
            .ok_or(Failure::NotFound)?;
        let primary = snapshot
            .navigation
            .workspace_directory(workspace_id)
            .map(|directory| directory.path.clone());
        let binding = match panel {
            Some(panel) => super::directory_label::displayed_terminal(snapshot, workspace, panel)
                .map_err(|()| Failure::NotFound)?,
            None => super::directory_label::first_terminal(snapshot, workspace),
        };
        let Some(binding) = binding.cloned() else {
            return Ok((None, primary));
        };
        // A registration that cannot be read leaves the Workspace's own directory to answer.
        let target = self
            .process_observation_targets()
            .ok()
            .and_then(|targets| targets.into_iter().find(|target| target.binding == binding));
        Ok((target, primary))
    }

    /// Read existing exact-physical samples only; never attach a viewer or substitute a child cwd.
    #[cfg(windows)]
    pub(crate) fn cached_checkpoint_directories(
        &self,
        targets: &[ProcessObservationTarget],
    ) -> Vec<(
        PresentationBinding,
        orchbee_workbench_protocol::WorkbenchDirectoryPath,
    )> {
        targets
            .iter()
            .filter_map(|target| {
                let sample = self.process_samples.get(&key(&target.binding))?;
                if sample.target.binding != target.binding
                    || sample.target.worker_generation != target.target.worker.generation
                    || sample.target.identity != target.target.identity
                {
                    return None;
                }
                Some((target.binding.clone(), sample.directory.clone()?))
            })
            .collect()
    }

    pub(crate) fn reset_screen_stabilization(&mut self) {
        for sample in self.process_samples.values_mut() {
            sample.screen = Stability::new(if sample.recognised {
                Detection::fallback(false)
            } else {
                Detection::opening()
            });
            sample.screen_input_revision = 0;
        }
    }
    /// Returns current unfenced registrations and prunes tracking only after a successful full scan.
    pub(crate) fn process_observation_targets(
        &mut self,
    ) -> Result<Vec<ProcessObservationTarget>, orchbee_server::WorkbenchTerminalInvariantError>
    {
        use orchbee_server::WorkbenchTerminalInvariantError as Invalid;
        let Some(workbench) = self.workbench.as_ref() else {
            self.process_samples.clear();
            self.terminal_observations.clear();
            #[cfg(windows)]
            self.carried_agent_sessions.clear();
            return Ok(vec![]);
        };
        let session_id = workbench.session_id().clone();
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
                session_id: session_id.clone(),
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
        self.terminal_observations
            .retain(|_, s| retained.contains(&s.physical));
        #[cfg(windows)]
        self.retain_carried_agent_sessions(&retained);
        Ok(targets.into_values().collect())
    }
}
