//! Answers an Agent's routine confirmations for a person. The screen sampler hands each
//! observation of a supported Agent's terminal to the rules in `crate::agent_confirmation`; this
//! module keeps one decision state per Agent run, sends the screen-gated key or the hold the rules
//! ask for, and answers the Panel Agent reads and choices about it. It types nothing without the
//! Terminal Host's screen gate, and a failure anywhere leaves the confirmation waiting for the
//! person. `../../docs/agent-confirmation.md` owns the rules.
use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_capability_node_protocol::{
    SendKey, SendKeyStroke, SendPrecondition, SendRecord, SendRecords, SendRejection,
    SendScreenGate, SendScreenState, SendStrokeKey, TerminalSendOutcome, TerminalSendRequest,
};
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use orchbee_terminal_protocol::TerminalProcessInfo;
use orchbee_workbench_protocol::{
    AgentConfirmationChoice, AgentConfirmationKind, AgentConfirmationPress, AgentConfirmationView,
    AgentConfirmationWait, PanelAgentAction, PanelAgentFailure, PanelAgentOutcome,
    PanelAgentRequest, PanelAgentResult, PresentationBinding, PresentationConfirmationRow,
    ProtocolVersion, SessionId, SettingChange, SettingPath, SettingValue, SettingsOperation,
    SettingsOutcome, TerminalPresentationId,
};

use super::{LocalTerminalService, placement};
use crate::agent_confirmation::{
    Choice, Facts, Frame, Kind, Layouts, Outcome, Refusal, Run, Scope, Seen, Settings, Step, Wait,
    judge,
};
use crate::session_coordinator::HostAgentRun;
use crate::terminal_coordinator::BindingViewerTarget;
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
};

/// The saved setting that turns answering on for every Claude Code run.
const CLAUDE_KEY: &str = "agents.auto_allow.claude";
/// The saved setting that turns answering on for every Codex run.
const CODEX_KEY: &str = "agents.auto_allow.codex";
/// The saved setting a person's "no" to an offer turns off.
const OFFER_KEY: &str = "agents.auto_allow.offer";
/// A run no sampler turn has observed for this long is forgotten when another run is first seen.
const UNOBSERVED: Duration = Duration::from_secs(600);

/// The screen one sampler turn captured, as the Terminal Host numbers its frame.
pub(in crate::local_terminal_service) struct ConfirmationScreen<'a> {
    pub(in crate::local_terminal_service) text: &'a str,
    pub(in crate::local_terminal_service) generation: u64,
    pub(in crate::local_terminal_service) revision: u64,
}

/// The saved settings, read in one step before any other lock.
#[derive(Clone, Copy)]
struct Saved {
    claude: bool,
    codex: bool,
    offer: bool,
}
impl Saved {
    fn settings(self, provider: AgentReportProvider) -> Settings {
        Settings {
            agent_type: match provider {
                AgentReportProvider::Claude => self.claude,
                AgentReportProvider::Codex => self.codex,
                _ => false,
            },
            offer: self.offer,
        }
    }
}

/// The saved setting of one Agent type, for the types a person can turn on.
fn type_key(provider: AgentReportProvider) -> Option<&'static str> {
    match provider {
        AgentReportProvider::Claude => Some(CLAUDE_KEY),
        AgentReportProvider::Codex => Some(CODEX_KEY),
        _ => None,
    }
}

struct Entry {
    target: BindingViewerTarget,
    agent: HostAgentRun,
    run: Run,
    /// What the last publication showed, to tell a change from a repeat.
    shown: Option<Facts>,
    observed: Instant,
}
impl Entry {
    /// Whether what Clients show changed since the last publication.
    fn publish(&mut self, settings: Settings) -> bool {
        let facts = self.run.facts(settings);
        let changed = self.shown.as_ref() != Some(&facts);
        self.shown = Some(facts);
        changed
    }
}

/// One decision state per terminal that runs a supported Agent, keyed by the terminal's id.
pub(super) struct AgentConfirmations {
    /// The bundled layouts. Without them no dialog is read and every confirmation waits.
    layouts: Option<Layouts>,
    /// A leaf lock: no other lock is taken while it is held.
    runs: Mutex<BTreeMap<[u8; 16], Entry>>,
    changes: SessionChangeSource,
}
impl Default for AgentConfirmations {
    fn default() -> Self {
        let layouts = Layouts::bundled();
        if layouts.is_err() {
            eprintln!("orchbee-server diagnostic component=agent-confirmation outcome=no-layouts");
        }
        Self {
            layouts: layouts.ok(),
            runs: Mutex::default(),
            changes: SessionChangeSource::default(),
        }
    }
}
impl AgentConfirmations {
    fn supports(&self, provider: AgentReportProvider) -> bool {
        self.layouts
            .as_ref()
            .is_some_and(|layouts| layouts.supports(provider))
    }

    /// Runs `change` on the entry of exactly this terminal and run, if it is still kept.
    fn with_entry<T>(
        &self,
        target: BindingViewerTarget,
        agent: HostAgentRun,
        change: impl FnOnce(&mut Entry) -> T,
    ) -> Option<T> {
        let mut runs = self.runs.lock().ok()?;
        runs.get_mut(&target.terminal_id.as_bytes())
            .filter(|entry| entry.target == target && entry.agent == agent)
            .map(change)
    }

    fn forget(&self, target: BindingViewerTarget) {
        let removed = self
            .runs
            .lock()
            .is_ok_and(|mut runs| runs.remove(&target.terminal_id.as_bytes()).is_some());
        if removed {
            self.changes.notify_agent_confirmations();
        }
    }
}

fn enter() -> Option<SendRecords> {
    SendRecords::new(vec![SendRecord::Stroke(
        SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0).ok()?,
    )])
    .ok()
}

fn refusal(state: SendScreenState) -> Refusal {
    match state {
        SendScreenState::AgentGone => Refusal::AgentGone,
        SendScreenState::Changed => Refusal::Changed,
        SendScreenState::AlreadyTyped => Refusal::AlreadyTyped,
        SendScreenState::InputPending => Refusal::InputPending,
        SendScreenState::PersonInput => Refusal::PersonInput,
        SendScreenState::PersonRecent => Refusal::PersonRecent,
    }
}

/// Reads one gated send's answer. Only an answer that proves the key was written, refused or
/// never sent is read as that; anything else leaves the key's fate unknown.
fn outcome(result: Result<WorkerOperationResult, WorkerRequestError>) -> Outcome {
    match result {
        Ok(WorkerOperationResult::Send(result)) => match result.outcome() {
            TerminalSendOutcome::Accepted { .. } => Outcome::Written,
            TerminalSendOutcome::Rejected { reason, .. } => match reason {
                SendRejection::ScreenRefused(state) => Outcome::Refused(refusal(state)),
                SendRejection::HostScreenGateUnsupported => Outcome::Unsupported,
                // Every rejection proves the record did not reach the Host's queue.
                SendRejection::WrongWorker
                | SendRejection::StaleWorkerGeneration
                | SendRejection::StaleHostEpoch
                | SendRejection::TerminalUnavailable
                | SendRejection::HostRejected
                | SendRejection::HostInputInvalid
                | SendRejection::HostInputLimitExceeded
                | SendRejection::HostInputBackpressure
                | SendRejection::HostInputUnavailable
                | SendRejection::PromptUnconfirmed(_)
                | SendRejection::HostPromptGateUnsupported => Outcome::NotSent,
            },
            TerminalSendOutcome::Unavailable { progress, .. }
                if progress.records_accepted() == 0 && progress.accepted_encoded_bytes() == 0 =>
            {
                Outcome::NotSent
            }
            TerminalSendOutcome::Unavailable { .. }
            | TerminalSendOutcome::OutcomeUnknown { .. } => Outcome::Unknown,
        },
        Ok(_) | Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Outcome::Unknown,
        // A Worker that does not read the screen gate declines the request before any effect.
        Err(WorkerRequestError::Declined) => Outcome::Unsupported,
        Err(
            WorkerRequestError::Unavailable
            | WorkerRequestError::Backpressure
            | WorkerRequestError::WrongWorker
            | WorkerRequestError::WrongGeneration
            | WorkerRequestError::InvalidRequest(_)
            | WorkerRequestError::NotDelivered(_),
        ) => Outcome::NotSent,
    }
}

fn diagnose(outcome: Outcome) {
    // A refusal for a changing screen can repeat on every sampler turn, so only outcomes that
    // happen at most once per prompt are written.
    let name = match outcome {
        Outcome::Written => "pressed",
        Outcome::Unknown => "press-unknown",
        Outcome::Unsupported => "gate-unsupported",
        Outcome::Refused(Refusal::PersonInput) => "refused-person",
        Outcome::Refused(
            Refusal::AgentGone
            | Refusal::Changed
            | Refusal::AlreadyTyped
            | Refusal::InputPending
            | Refusal::PersonRecent,
        )
        | Outcome::NotSent => return,
    };
    eprintln!("orchbee-server diagnostic component=agent-confirmation outcome={name}");
}

fn wait(wait: Wait) -> AgentConfirmationWait {
    match wait {
        Wait::Destructive => AgentConfirmationWait::Destructive,
        Wait::LeavesWorkspace => AgentConfirmationWait::LeavesWorkspace,
        Wait::ProtectedFile => AgentConfirmationWait::ProtectedFile,
        Wait::NotUnderstood => AgentConfirmationWait::NotUnderstood,
        Wait::NotVisible => AgentConfirmationWait::NotVisible,
        Wait::MarkedDangerous => AgentConfirmationWait::MarkedDangerous,
        Wait::NotAnswered => AgentConfirmationWait::NotAnswered,
        Wait::Person => AgentConfirmationWait::Person,
        Wait::Changing => AgentConfirmationWait::Changing,
        Wait::Repeating => AgentConfirmationWait::Repeating,
        Wait::HostTooOld => AgentConfirmationWait::HostTooOld,
    }
}

/// The row a Client shows. A press whose text the contract would refuse is left out, so one odd
/// screen cannot make the whole answer invalid.
fn view(provider: AgentReportProvider, facts: Facts) -> AgentConfirmationView {
    let choice = match facts.choice {
        Choice::Off => AgentConfirmationChoice::Off,
        Choice::Run => AgentConfirmationChoice::Run,
        Choice::AgentType => AgentConfirmationChoice::AgentType,
    };
    AgentConfirmationView {
        agent: provider.report_label().to_owned(),
        choice,
        offer: facts.offer && choice == AgentConfirmationChoice::Off,
        wait: facts.wait.map(wait),
        pressed: facts
            .pressed
            .filter(|pressed| {
                pressed.serial != 0
                    && !pressed.text.is_empty()
                    && pressed.text.len()
                        <= orchbee_workbench_protocol::MAXIMUM_AGENT_CONFIRMATION_TEXT_BYTES
                    && !pressed.text.chars().any(char::is_control)
            })
            .map(|pressed| AgentConfirmationPress {
                serial: pressed.serial,
                kind: match pressed.kind {
                    Kind::Command => AgentConfirmationKind::Command,
                    Kind::FileCreate => AgentConfirmationKind::Create,
                    Kind::FileEdit => AgentConfirmationKind::Edit,
                },
                text: pressed.text,
            }),
        stopped: facts.stopped,
    }
}

fn terminal_key(binding: &PresentationBinding) -> Option<[u8; 16]> {
    u128::from_str_radix(binding.terminal_id.as_str(), 16)
        .ok()
        .map(u128::to_be_bytes)
}

impl LocalTerminalService {
    /// Wakes `signal` when what the Server shows about an Agent's confirmations changes.
    pub(crate) fn watch_agent_confirmations(&self, signal: &Arc<SessionChangeSignal>) {
        self.agent_confirmations.changes.watch(signal);
    }

    /// Whether the sampler captures the screen of `provider`'s terminal for a confirmation
    /// decision even when a program status report already decides the Agent's state.
    pub(in crate::local_terminal_service) fn reads_agent_confirmation(
        &self,
        provider: AgentReportProvider,
    ) -> bool {
        self.agent_confirmations.supports(provider)
    }

    /// Before startup installs configuration every setting reads as off.
    fn confirmation_saved(&self) -> Saved {
        self.configuration
            .with_settings(|settings| Saved {
                claude: settings.values.auto_allow_claude,
                codex: settings.values.auto_allow_codex,
                offer: settings.auto_allow_offer,
            })
            .unwrap_or(Saved {
                claude: false,
                codex: false,
                offer: false,
            })
    }

    /// One sampler turn's observation of `target`: `agent` is the run both process samples around
    /// the capture name, `sample` the later of them, and `screen` the capture, absent when it
    /// failed. Decides, and sends what the decision asks for before it returns.
    pub(in crate::local_terminal_service) fn observe_agent_confirmation(
        &self,
        handle: &WorkerRequestHandle,
        target: BindingViewerTarget,
        agent: Option<HostAgentRun>,
        sample: &TerminalProcessInfo,
        screen: Option<ConfirmationScreen<'_>>,
        blocked: bool,
    ) {
        let confirmations = &self.agent_confirmations;
        let Some((agent, layouts)) = agent.zip(confirmations.layouts.as_ref()) else {
            confirmations.forget(target);
            return;
        };
        if !layouts.supports(agent.provider) {
            confirmations.forget(target);
            return;
        }
        let root = {
            let Ok(workers) = self.workers.lock() else {
                return;
            };
            if !handle.is_active()
                || placement(workers.request_handle().as_ref()) != Some(target.worker)
                || placement(Some(handle)) != Some(target.worker)
            {
                return;
            }
            let Ok(coordinator) = self.coordinator.lock() else {
                return;
            };
            coordinator.confirmation_root(target, agent)
        };
        // Only the run the Session publishes for this terminal is answered.
        let Some(root) = root else {
            confirmations.forget(target);
            return;
        };
        let settings = self.confirmation_saved().settings(agent.provider);
        let directory = sample
            .processes
            .iter()
            .find(|process| process.pid == agent.pid() && process.creation_time == agent.created())
            .and_then(|process| process.current_directory.as_deref())
            .and_then(orchbee_local_runtime::reported_directory);
        let now = Instant::now();
        let decided = {
            let Ok(mut runs) = confirmations.runs.lock() else {
                return;
            };
            let key = target.terminal_id.as_bytes();
            if !runs
                .get(&key)
                .is_some_and(|entry| entry.target == target && entry.agent == agent)
            {
                runs.retain(|_, entry| now.saturating_duration_since(entry.observed) < UNOBSERVED);
                runs.insert(
                    key,
                    Entry {
                        target,
                        agent,
                        run: Run::new(),
                        shown: None,
                        observed: now,
                    },
                );
            }
            runs.get_mut(&key).map(|entry| {
                entry.observed = now;
                let frame = screen.as_ref().and_then(|screen| {
                    Some(Frame {
                        generation: NonZeroU64::new(screen.generation)?,
                        revision: screen.revision,
                    })
                });
                let step = match (frame, &screen) {
                    (Some(frame), Some(screen)) => {
                        let dialog = layouts.read(agent.provider, screen.text);
                        let scope = Scope {
                            root: root.as_ref().map(|root| root.as_str()),
                            directory: directory.as_deref(),
                        };
                        entry.run.observe(
                            now,
                            settings,
                            Seen {
                                frame,
                                blocked,
                                dialog: dialog
                                    .as_ref()
                                    .map(|dialog| (dialog, judge(dialog, scope))),
                            },
                        )
                    }
                    // Without a frame nothing is known about the screen; only a hold can be owed.
                    _ if entry.run.owes_hold(settings) => Step::Hold,
                    _ => Step::Nothing,
                };
                (step, entry.publish(settings))
            })
        };
        let Some((step, mut changed)) = decided else {
            return;
        };
        match step {
            Step::Nothing => {}
            Step::Press(gate) => {
                let sent = NonZeroU32::new(agent.pid()).map_or(Outcome::NotSent, |pid| {
                    self.send_confirmation(
                        handle,
                        target,
                        SendPrecondition::Screen(SendScreenGate {
                            pid,
                            creation_time: agent.created(),
                            generation: gate.frame.generation,
                            revision: gate.frame.revision,
                            first_revision: gate.first_revision,
                        }),
                        || {
                            let settings = self.confirmation_saved().settings(agent.provider);
                            confirmations
                                .with_entry(target, agent, |entry| entry.run.sending(settings))
                                .unwrap_or(false)
                        },
                    )
                });
                diagnose(sent);
                changed |= confirmations
                    .with_entry(target, agent, |entry| {
                        entry.run.pressed(Instant::now(), sent);
                        entry.publish(settings)
                    })
                    .unwrap_or(false);
            }
            Step::Hold => changed |= self.hold_confirmation(handle, target, agent),
        }
        if changed {
            confirmations.changes.notify_agent_confirmations();
        }
    }

    /// Sends one hold and records its answer. True when what Clients show changed.
    fn hold_confirmation(
        &self,
        handle: &WorkerRequestHandle,
        target: BindingViewerTarget,
        agent: HostAgentRun,
    ) -> bool {
        // A Host without the gate cannot type a gated key, so nothing is left to hold there.
        let held = matches!(
            self.send_confirmation(handle, target, SendPrecondition::ScreenHold, || true),
            Outcome::Written | Outcome::Unsupported
        );
        if !held {
            return false;
        }
        let settings = self.confirmation_saved().settings(agent.provider);
        self.agent_confirmations
            .with_entry(target, agent, |entry| {
                entry.run.held();
                entry.publish(settings)
            })
            .unwrap_or(false)
    }

    /// Sends the hold every run owes after its choice or its Agent type's setting went off, and
    /// returns once each is answered or has failed. A hold that failed is sent again by the run's
    /// next observation, and the run shows as on until then.
    pub(super) fn hold_agent_confirmations(&self) {
        let saved = self.confirmation_saved();
        let owed: Vec<(BindingViewerTarget, HostAgentRun)> = {
            let Ok(mut runs) = self.agent_confirmations.runs.lock() else {
                return;
            };
            runs.values_mut()
                .filter_map(|entry| {
                    entry
                        .run
                        .owes_hold(saved.settings(entry.agent.provider))
                        .then_some((entry.target, entry.agent))
                })
                .collect()
        };
        if owed.is_empty() {
            return;
        }
        let handle = self
            .workers
            .lock()
            .ok()
            .and_then(|workers| workers.request_handle());
        let mut changed = false;
        if let Some(handle) = handle {
            for (target, agent) in owed {
                changed |= self.hold_confirmation(&handle, target, agent);
            }
        }
        if changed {
            self.agent_confirmations
                .changes
                .notify_agent_confirmations();
        }
    }

    /// Sends one Enter or one hold through the Host's screen gate to the exact terminal a sampler
    /// read. `admit` runs last under both admission locks, and a false answer sends nothing.
    fn send_confirmation(
        &self,
        handle: &WorkerRequestHandle,
        target: BindingViewerTarget,
        precondition: SendPrecondition,
        admit: impl FnOnce() -> bool,
    ) -> Outcome {
        let Some(records) = enter() else {
            return Outcome::NotSent;
        };
        let identities = {
            let Ok(workers) = self.workers.lock() else {
                return Outcome::NotSent;
            };
            if !handle.is_active()
                || placement(workers.request_handle().as_ref()) != Some(target.worker)
                || placement(Some(handle)) != Some(target.worker)
            {
                return Outcome::NotSent;
            }
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return Outcome::NotSent;
            };
            let Some(identities) =
                coordinator.prepare_confirmation_send(target, Some(target.worker))
            else {
                return Outcome::NotSent;
            };
            if !admit() {
                return Outcome::NotSent;
            }
            identities
        };
        outcome(
            handle.request(WorkerOperationRequest::Send(TerminalSendRequest {
                request_id: identities.operation,
                worker_id: target.worker.worker_id,
                worker_generation: target.worker.generation,
                target: target.identity,
                attachment_id: identities.attachment,
                precondition,
                records,
            })),
        )
    }

    /// Writes one saved setting through the settings file, as an edit from a Client does.
    fn save_confirmation_setting(&self, key: &str, value: bool) -> Result<(), PanelAgentFailure> {
        let key = SettingPath::parse(key).map_err(|_| PanelAgentFailure::Unavailable)?;
        match self.settings_outcome(&SettingsOperation::Edit {
            changes: vec![SettingChange::Set {
                key,
                value: SettingValue::Boolean(value),
            }],
        }) {
            SettingsOutcome::Edited { .. } => Ok(()),
            SettingsOutcome::Current { .. } | SettingsOutcome::Refused { .. } => {
                Err(PanelAgentFailure::Unavailable)
            }
        }
    }

    /// One row per requested Presentation, in request order.
    fn confirmation_rows(
        &self,
        session: &SessionId,
        presentation_ids: &[TerminalPresentationId],
    ) -> Result<Vec<PresentationConfirmationRow>, PanelAgentFailure> {
        let saved = self.confirmation_saved();
        let targets = self
            .coordinator
            .lock()
            .map_err(|_| PanelAgentFailure::Unavailable)?
            .presentation_title_targets(session, presentation_ids)?;
        let runs = self
            .agent_confirmations
            .runs
            .lock()
            .map_err(|_| PanelAgentFailure::Unavailable)?;
        Ok(presentation_ids
            .iter()
            .zip(targets)
            .map(|(presentation_id, target)| {
                let confirmation =
                    target
                        .binding
                        .as_ref()
                        .zip(target.physical)
                        .and_then(|(binding, physical)| {
                            let entry = runs.get(&terminal_key(binding)?)?;
                            (physical == (entry.target.worker.generation, entry.target.identity))
                                .then(|| {
                                    view(
                                        entry.agent.provider,
                                        entry.run.facts(saved.settings(entry.agent.provider)),
                                    )
                                })
                        });
                PresentationConfirmationRow {
                    presentation_id: presentation_id.clone(),
                    binding: target.binding,
                    confirmation,
                }
            })
            .collect())
    }

    /// Applies a person's choice for the run in one Presentation's terminal, or their "no" to its
    /// offer when `choice` is absent. A choice that turns answering off is answered only after the
    /// Host has the hold, so "off" in the answer means no key decided earlier can still be typed.
    fn choose_confirmation(
        &self,
        session: &SessionId,
        presentation_id: &TerminalPresentationId,
        expected: &PresentationBinding,
        choice: Option<AgentConfirmationChoice>,
    ) -> Result<PanelAgentOutcome, PanelAgentFailure> {
        let presentation = std::slice::from_ref(presentation_id);
        let target = self
            .coordinator
            .lock()
            .map_err(|_| PanelAgentFailure::Unavailable)?
            .presentation_title_targets(session, presentation)?
            .pop()
            .ok_or(PanelAgentFailure::Unavailable)?;
        let binding = target.binding.ok_or(PanelAgentFailure::Unbound)?;
        if binding != *expected {
            return Err(PanelAgentFailure::BindingChanged);
        }
        let key = terminal_key(&binding).ok_or(PanelAgentFailure::Unavailable)?;
        let run = |change: &mut dyn FnMut(&mut Entry)| {
            let mut runs = self
                .agent_confirmations
                .runs
                .lock()
                .map_err(|_| PanelAgentFailure::Unavailable)?;
            let entry = runs
                .get_mut(&key)
                .filter(|entry| {
                    target.physical == Some((entry.target.worker.generation, entry.target.identity))
                })
                .ok_or(PanelAgentFailure::Unavailable)?;
            change(entry);
            Ok::<_, PanelAgentFailure>(entry.agent.provider)
        };
        let provider = run(&mut |_| {})?;
        match choice {
            None => self.save_confirmation_setting(OFFER_KEY, false)?,
            Some(choice) => {
                let always = choice == AgentConfirmationChoice::AgentType;
                if self.confirmation_saved().settings(provider).agent_type != always {
                    let key = type_key(provider).ok_or(PanelAgentFailure::Unavailable)?;
                    self.save_confirmation_setting(key, always)?;
                }
                run(&mut |entry| entry.run.choose(choice == AgentConfirmationChoice::Run))?;
            }
        }
        self.hold_agent_confirmations();
        self.agent_confirmations
            .changes
            .notify_agent_confirmations();
        let row = self
            .confirmation_rows(session, presentation)?
            .pop()
            .filter(|row| row.binding.as_ref() == Some(expected))
            .ok_or(PanelAgentFailure::BindingChanged)?;
        Ok(PanelAgentOutcome::ConfirmationChosen {
            presentation: Box::new(row),
        })
    }

    pub(super) fn execute_agent_confirmation(
        &self,
        request: &PanelAgentRequest,
    ) -> PanelAgentResult {
        let session = &request.session_id;
        let outcome = match self.workbench_session_matches(session) {
            None => Err(PanelAgentFailure::Unavailable),
            Some(false) => Err(PanelAgentFailure::SessionMismatch),
            Some(true) => {
                if let PanelAgentAction::Confirmations { presentation_ids } = &request.action {
                    self.confirmation_rows(session, presentation_ids)
                        .map(|presentations| PanelAgentOutcome::Confirmations { presentations })
                } else if let PanelAgentAction::ConfirmationChoose {
                    presentation_id,
                    expected_binding,
                    choice,
                } = &request.action
                {
                    self.choose_confirmation(
                        session,
                        presentation_id,
                        expected_binding,
                        Some(*choice),
                    )
                } else if let PanelAgentAction::ConfirmationDecline {
                    presentation_id,
                    expected_binding,
                } = &request.action
                {
                    self.choose_confirmation(session, presentation_id, expected_binding, None)
                } else {
                    Err(PanelAgentFailure::InvalidRequest)
                }
            }
        };
        PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome: outcome.unwrap_or_else(|reason| PanelAgentOutcome::Failed { reason }),
        }
    }
}

#[cfg(test)]
mod tests;
