//! Selects the live Agent session reference each checkpoint retains for a Presentation: a
//! delivered reference carried forward while the process its delivery started lives; else Agent
//! candidates joined to the exact Terminal generation, refuted by that generation's command and
//! process evidence, and admitted only when exactly one remains; else, when none remains, the
//! session the published run's own process registered in its Agent's run table. Reports, process
//! names and screen detection never grant a reference. It does not persist, deduplicate across
//! Presentations or deliver a resume.

use crate::agent_service::AgentResumeCandidate;
use orchbee_agent_observation_protocol::AgentProcessIdentity;
use orchbee_server::RetainedAgentSession;
use std::collections::BTreeSet;

#[cfg(windows)]
mod claude_run_table;
#[cfg(test)]
mod tests;

#[cfg(windows)]
pub(crate) use claude_run_table::read as read_run_table;

/// The shell command evidence of one exact Terminal generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandEvidence {
    /// The latest shell-echoed command, still running, with its command id.
    Open([u8; 16]),
    /// The latest shell-echoed command completed and none is open.
    Closed,
    /// The shell never echoed a command, as in an unmarked shell such as cmd.
    Unmarked,
}

impl CommandEvidence {
    /// The evidence a generation's latest shell-echoed command gives; none means unmarked.
    #[cfg(windows)]
    pub(crate) fn of(latest: Option<orchbee_terminal_protocol::TerminalObservedCommand>) -> Self {
        match latest {
            Some(command) if !command.completed => Self::Open(command.id),
            Some(_) => Self::Closed,
            None => Self::Unmarked,
        }
    }
}

/// The session one Agent process registered in its Agent's own run table, read outside every lock
/// before a capture, and the process it was read for. Only an entry whose process id and creation
/// time are that process's is ever built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunTableEntry {
    pub(crate) anchor: RunAnchor,
    pub(crate) session: RetainedAgentSession,
}

/// One Presentation's reference without a carried one, if any. An open command keeps the
/// candidates its run token names, else the tokenless ones; a completed command keeps only
/// tokenless ones; an unmarked shell keeps all. An Agent process identity keeps its own provider,
/// `None` drops every candidate, and `Unknown` or `Unsupported` refutes nothing. Exactly one
/// distinct `(provider, native key)` that is also a valid id gives the reference; several give
/// none. When none remains, the process source decides: the entry read for the generation's
/// published `run`, while the identity is `Agent` of that entry's provider, gives its session;
/// anything else gives none.
pub(crate) fn select(
    candidates: &[AgentResumeCandidate],
    evidence: CommandEvidence,
    identity: &AgentProcessIdentity,
    run: Option<RunAnchor>,
    entries: &[RunTableEntry],
) -> Option<RetainedAgentSession> {
    let tokenless = || candidates.iter().filter(|c| c.run_token.is_none());
    let kept: Vec<&AgentResumeCandidate> = match evidence {
        CommandEvidence::Open(id) => {
            let open = format!("{:032x}", u128::from_be_bytes(id));
            let named: Vec<_> = candidates
                .iter()
                .filter(|c| c.run_token.as_ref().is_some_and(|t| t.as_str() == open))
                .collect();
            if named.is_empty() {
                tokenless().collect()
            } else {
                named
            }
        }
        CommandEvidence::Closed => tokenless().collect(),
        CommandEvidence::Unmarked => candidates.iter().collect(),
    };
    let provider = match identity {
        AgentProcessIdentity::Agent { provider, .. } => Some(provider.native_provider_id()),
        AgentProcessIdentity::None => return None,
        AgentProcessIdentity::Unknown(_) | AgentProcessIdentity::Unsupported(_) => None,
    };
    let mut distinct = kept
        .into_iter()
        .filter(|c| provider.is_none_or(|provider| c.provider.provider_id() == provider))
        .map(|c| (c.provider, c.native_key.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter();
    match (distinct.next(), distinct.next()) {
        (Some((provider, key)), None) => RetainedAgentSession::new(provider, key),
        (Some(_), Some(_)) => None,
        // No hook session of this run remains: only its own process can name one.
        (None, _) => {
            let (provider, run) = (provider?, run?);
            entries
                .iter()
                .find(|entry| {
                    entry.anchor == run && entry.session.provider().provider_id() == provider
                })
                .map(|entry| entry.session.clone())
        }
    }
}

/// The process a delivery started: its pid and its creation time in sample row units, which
/// defeats pid reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunAnchor {
    pub(crate) pid: u32,
    pub(crate) created: u64,
}

/// A delivered reference carried forward. It stays capturable while, and only while, the process
/// its delivery started lives in the Terminal generation the delivery reached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Carried {
    pub(crate) session: RetainedAgentSession,
    /// The argument tokens the delivery typed after the program.
    pub(crate) arguments: Vec<String>,
    /// The run the delivery started, once a sample commit bound it.
    pub(crate) anchor: Option<RunAnchor>,
}

/// The carried generation's published run, as one sample commit or capture reads it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PublishedRun<'a> {
    pub(crate) anchor: RunAnchor,
    pub(crate) identity: AgentProcessIdentity,
    /// The anchor row's decoded arguments: `None` when unreadable. Capture never reads them.
    pub(crate) argv: Option<&'a [String]>,
}

/// One moment that decides a carried reference.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CarriedMoment<'a> {
    /// A committed sample of the carried generation, with its published run, if any.
    Sample(Option<PublishedRun<'a>>),
    /// A checkpoint capture of the carried generation: whether any hook candidate names it, and
    /// its published run, if any.
    Capture {
        candidate: bool,
        run: Option<PublishedRun<'a>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CarriedOutcome {
    /// Bind the reference to this run.
    Bind(RunAnchor),
    /// Write the reference as the generation's live one, and keep carrying it.
    Write,
    /// Keep carrying it without writing it.
    Keep,
    /// Drop it for good.
    Drop,
}

/// The carry-forward decision. At a sample commit the first published run decides an unbound
/// reference: the reference binds to that run when it is an `Agent` of the carried provider whose
/// anchor row's arguments end with the typed tokens, so a manual start of the same provider never
/// inherits it, and any other run drops it. At a capture any hook candidate drops it, so the live
/// rule decides; otherwise a bound reference whose anchor is still the published run, as an
/// `Agent` of its provider, is written, and any other bound one is dropped. An unbound reference
/// is never written, so a resume that fails fast never repeats.
pub(crate) fn carried_outcome(carried: &Carried, moment: CarriedMoment<'_>) -> CarriedOutcome {
    let own = |run: &PublishedRun<'_>| {
        matches!(run.identity, AgentProcessIdentity::Agent { provider, .. }
            if provider.native_provider_id() == carried.session.provider().provider_id())
    };
    let typed =
        |argv: &[String]| !carried.arguments.is_empty() && argv.ends_with(&carried.arguments);
    match moment {
        CarriedMoment::Sample(run) => match (carried.anchor, run) {
            (Some(_), _) | (None, None) => CarriedOutcome::Keep,
            (None, Some(run)) if own(&run) && run.argv.is_some_and(typed) => {
                CarriedOutcome::Bind(run.anchor)
            }
            (None, Some(_)) => CarriedOutcome::Drop,
        },
        CarriedMoment::Capture {
            candidate: true, ..
        } => CarriedOutcome::Drop,
        CarriedMoment::Capture { run, .. } => match (carried.anchor, run) {
            (None, _) => CarriedOutcome::Keep,
            (Some(anchor), Some(run)) if run.anchor == anchor && own(&run) => CarriedOutcome::Write,
            (Some(_), _) => CarriedOutcome::Drop,
        },
    }
}

/// Carried references keyed like process samples, each with the exact Terminal generation its
/// delivery reached.
#[cfg(windows)]
pub(super) type CarriedSessions =
    std::collections::BTreeMap<(String, String), (orchbee_server::TerminalAgentBinding, Carried)>;

#[cfg(windows)]
impl super::SessionCoordinator {
    /// Records the reference the driver just admitted for delivery into `target`'s exact Terminal
    /// generation, with the tokens that delivery types. At most one delivery reaches a Presentation
    /// per Server lifetime, so nothing earlier is carried there.
    pub(crate) fn carry_agent_session(
        &mut self,
        target: &super::ProcessObservationTarget,
        session: RetainedAgentSession,
        arguments: Vec<String>,
    ) {
        let physical = orchbee_server::TerminalAgentBinding {
            binding: target.binding.clone(),
            worker_generation: target.target.worker.generation,
            identity: target.target.identity,
        };
        let carried = Carried {
            session,
            arguments,
            anchor: None,
        };
        self.carried_agent_sessions
            .insert(target.key(), (physical, carried));
    }

    /// Applies one committed sample of `physical` to its carried reference: the first published
    /// run binds or drops an unbound one. A reference of another generation is dropped.
    pub(super) fn bind_carried_agent_session(
        &mut self,
        physical: &orchbee_server::TerminalAgentBinding,
        run: Option<super::process_observation::Selected>,
        identity: AgentProcessIdentity,
        sample: &orchbee_terminal_protocol::TerminalProcessInfo,
    ) {
        let key = super::process_observation::key(&physical.binding);
        let Some((carried_physical, carried)) = self.carried_agent_sessions.get_mut(&key) else {
            return;
        };
        let outcome = if carried_physical == physical {
            let argv = run
                .and_then(|run| {
                    sample
                        .processes
                        .iter()
                        .find(|p| p.pid == run.pid() && p.creation_time == run.created())
                })
                .and_then(|row| super::decoded_argv(row.command_line.as_deref()));
            let run = run.map(|run| PublishedRun {
                anchor: RunAnchor {
                    pid: run.pid(),
                    created: run.created(),
                },
                identity,
                argv: argv.as_deref(),
            });
            carried_outcome(carried, CarriedMoment::Sample(run))
        } else {
            CarriedOutcome::Drop
        };
        match outcome {
            CarriedOutcome::Bind(anchor) => carried.anchor = Some(anchor),
            CarriedOutcome::Drop => {
                self.carried_agent_sessions.remove(&key);
            }
            CarriedOutcome::Write | CarriedOutcome::Keep => {}
        }
    }

    /// Drops every carried reference whose exact Terminal generation is no longer registered: a
    /// binding or generation change, or a Presentation that went away.
    pub(super) fn retain_carried_agent_sessions(
        &mut self,
        registered: &[orchbee_server::TerminalAgentBinding],
    ) {
        self.carried_agent_sessions
            .retain(|_, (physical, _)| registered.contains(physical));
    }

    /// Applies one capture to `physical`'s carried reference. `None` when no carried reference
    /// decides, because there is none or it drops; otherwise what it decides: itself when written,
    /// nothing while it waits unbound.
    fn capture_carried_agent_session(
        &mut self,
        physical: &orchbee_server::TerminalAgentBinding,
        candidate: bool,
    ) -> Option<Option<RetainedAgentSession>> {
        let key = super::process_observation::key(&physical.binding);
        let outcome = match self.carried_agent_sessions.get(&key)? {
            (carried_physical, _) if carried_physical != physical => CarriedOutcome::Drop,
            (_, carried) => {
                let run = self.published_run(physical).map(|run| PublishedRun {
                    anchor: RunAnchor {
                        pid: run.pid(),
                        created: run.created(),
                    },
                    identity: self.process_identity(physical),
                    argv: None,
                });
                carried_outcome(carried, CarriedMoment::Capture { candidate, run })
            }
        };
        match outcome {
            CarriedOutcome::Write => self
                .carried_agent_sessions
                .get(&key)
                .map(|(_, carried)| Some(carried.session.clone())),
            CarriedOutcome::Keep => Some(None),
            CarriedOutcome::Drop | CarriedOutcome::Bind(_) => {
                self.carried_agent_sessions.remove(&key);
                None
            }
        }
    }

    /// The published run of every tracked generation whose identity is `Agent` of a provider with
    /// a run table, Claude Code alone, for the run table read that precedes a capture. Capture
    /// checks each entry against the generation's published run again, so a run that changed
    /// after this read gains nothing.
    pub(crate) fn run_table_anchors(&self) -> Vec<RunAnchor> {
        use orchbee_agent_observation_protocol::AgentReportProvider;
        self.published_runs()
            .filter(|(run, identity)| {
                matches!(identity, AgentProcessIdentity::Agent { provider, .. }
                    if *provider == AgentReportProvider::Claude && run.provider == *provider)
            })
            .map(|(run, _)| RunAnchor {
                pid: run.pid(),
                created: run.created(),
            })
            .collect()
    }

    /// The live reference of every checkpoint target that has one, keyed by its binding. Targets
    /// join their Presentation through current registration at the exact generation they name;
    /// a stale target, an unreadable registration or an unavailable Agent cell contributes
    /// nothing, so pending restart references still carry over. A target with any hook candidate
    /// drops its carried reference and takes the live rule; otherwise its carried reference
    /// decides. Without one, the live rule decides, and `entries`, the run table read for
    /// published runs before this capture, answer only where no hook candidate remains.
    /// Capture owns cross-Presentation deduplication in its own order.
    pub(crate) fn checkpoint_agent_sessions(
        &mut self,
        targets: &[super::ProcessObservationTarget],
        entries: &[RunTableEntry],
    ) -> Vec<(
        orchbee_workbench_protocol::PresentationBinding,
        RetainedAgentSession,
    )> {
        let Some(agents) = self.agents.as_ref().and_then(std::sync::Weak::upgrade) else {
            return Vec::new();
        };
        let bindings: Vec<_> = targets.iter().map(|t| t.binding.clone()).collect();
        let Ok(facts) = self.agent_registration_for_bindings(&bindings) else {
            return Vec::new();
        };
        let physical: Vec<_> = facts
            .into_iter()
            .filter(|fact| {
                targets.iter().any(|t| {
                    t.binding == *fact.binding()
                        && t.target.worker.generation == fact.worker_generation()
                        && t.target.identity == fact.identity()
                })
            })
            .map(|fact| {
                (
                    fact.presentation_id().clone(),
                    orchbee_server::TerminalAgentBinding {
                        binding: fact.binding().clone(),
                        worker_generation: fact.worker_generation(),
                        identity: fact.identity(),
                    },
                )
            })
            .collect();
        let identities: Vec<_> = physical
            .iter()
            .map(|(presentation, terminal)| (presentation.clone(), terminal.identity))
            .collect();
        let Ok(candidates) = agents.resume_candidates(&identities) else {
            return Vec::new();
        };
        let mut live = Vec::new();
        for (presentation, terminal) in physical {
            let own: Vec<_> = candidates
                .iter()
                .filter(|c| c.presentation == presentation)
                .cloned()
                .collect();
            let session = match self.capture_carried_agent_session(&terminal, !own.is_empty()) {
                Some(decided) => decided,
                None => {
                    let evidence = CommandEvidence::of(self.latest_command(&terminal));
                    let run = self.published_run(&terminal).map(|run| RunAnchor {
                        pid: run.pid(),
                        created: run.created(),
                    });
                    select(
                        &own,
                        evidence,
                        &self.process_identity(&terminal),
                        run,
                        entries,
                    )
                }
            };
            if let Some(session) = session {
                live.push((terminal.binding, session));
            }
        }
        live
    }
}

/// Where a pending restart reference's Presentation stands when its delivery is attempted.
#[cfg(windows)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResumeTarget {
    /// The Presentation is no longer a bound member of the Workbench.
    Gone,
    /// Bound as this, but no started, unfenced physical terminal of the current Worker answers.
    Unavailable(orchbee_workbench_protocol::PresentationBinding),
    /// The started physical terminal its current binding names.
    Started(super::ProcessObservationTarget),
}

#[cfg(windows)]
impl super::SessionCoordinator {
    /// Whether a restart reference still waits for delivery in this Presentation.
    pub(crate) fn has_pending_agent_session(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> bool {
        self.pending_agent_session(presentation).is_some()
    }

    /// The restart reference still pending for this Presentation, if any.
    pub(crate) fn pending_agent_session(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> Option<RetainedAgentSession> {
        self.workbench
            .as_ref()?
            .pending_agent_session(presentation)
            .cloned()
    }

    /// Consumes the pending restart reference, so no later attempt or capture carries it.
    pub(crate) fn take_pending_agent_session(
        &mut self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> Option<RetainedAgentSession> {
        self.workbench
            .as_mut()?
            .take_pending_agent_session(presentation)
    }

    /// Undoes one admitted delivery the Host then refused with nothing typed: the reference is
    /// pending again for the next attempt or capture, and `target` no longer carries it. Never
    /// changes the Workbench revision.
    pub(crate) fn return_agent_session(
        &mut self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
        target: &super::ProcessObservationTarget,
        session: RetainedAgentSession,
    ) {
        let key = target.key();
        if self
            .carried_agent_sessions
            .get(&key)
            .is_some_and(|(_, carried)| carried.session == session && carried.anchor.is_none())
        {
            self.carried_agent_sessions.remove(&key);
        }
        if let Some(workbench) = self.workbench.as_mut() {
            workbench.return_pending_agent_session(presentation, session);
        }
    }

    /// Consumes the pending reference of a resume that typed nothing and, when `at` is known,
    /// publishes that on the Presentation still bound as `binding`, in one Workbench revision.
    pub(crate) fn give_up_agent_resume(
        &mut self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
        binding: Option<&orchbee_workbench_protocol::PresentationBinding>,
        reason: orchbee_workbench_protocol::AgentNotResumedReason,
        at: Option<u64>,
    ) {
        let Some(workbench) = self.workbench.as_mut() else {
            return;
        };
        match at {
            Some(at) => {
                workbench.give_up_agent_resume(presentation, binding, reason, at);
            }
            None => {
                workbench.take_pending_agent_session(presentation);
            }
        }
    }

    /// The Presentation's current binding and, when the current Worker has started it and no
    /// lifecycle fence covers it, the exact physical terminal a delivery may reach.
    pub(crate) fn agent_resume_target(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
        current_worker: Option<crate::terminal_coordinator::WorkerPlacement>,
    ) -> ResumeTarget {
        let Some(workbench) = self.workbench.as_ref() else {
            return ResumeTarget::Gone;
        };
        let binding = workbench
            .snapshot()
            .terminal_contexts
            .iter()
            .filter_map(|context| {
                workbench
                    .presentation_membership(&context.terminal_context_id)
                    .ok()
            })
            .find_map(|membership| {
                membership
                    .members()
                    .iter()
                    .find(|member| member.presentation_id() == presentation)
                    .map(|member| member.binding().clone())
            });
        let Some(binding) = binding else {
            return ResumeTarget::Gone;
        };
        let parse = |text: &str| u128::from_str_radix(text, 16).ok().map(u128::to_be_bytes);
        let target = parse(binding.terminal_id.as_str())
            .zip(parse(binding.worker_id.as_str()))
            .and_then(|(terminal, worker)| {
                Some((
                    orchbee_capability_node_protocol::TerminalId::from_bytes(terminal).ok()?,
                    orchbee_capability_node_protocol::WorkerId::from_bytes(worker).ok()?,
                ))
            })
            .and_then(|(terminal, worker)| {
                self.terminals
                    .binding_send_target(terminal, worker, current_worker)
                    .ok()
            })
            .filter(|target| self.target_lifecycle(*target) == super::TargetLifecycle::Unfenced);
        match target {
            Some(target) => ResumeTarget::Started(super::ProcessObservationTarget {
                session_id: workbench.session_id().clone(),
                binding,
                target,
            }),
            None => ResumeTarget::Unavailable(binding),
        }
    }

    /// Whether the shell of this exact Terminal generation has echoed any command.
    pub(crate) fn agent_resume_command_observed(
        &self,
        target: &super::ProcessObservationTarget,
    ) -> bool {
        self.latest_command(&orchbee_server::TerminalAgentBinding {
            binding: target.binding.clone(),
            worker_generation: target.target.worker.generation,
            identity: target.target.identity,
        })
        .is_some()
    }
}
