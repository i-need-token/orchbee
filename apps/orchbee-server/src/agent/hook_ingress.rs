//! Translates admitted provider hook envelopes and settled joins into canonical Agent session
//! events within the owning runtime call. It retains bounded join and settlement bookkeeping
//! without accepting sockets, writing acknowledgements, or managing Worker and Host
//! connections.

mod settlement_queue;

use orchbee_agent_observation_protocol::AgentRunToken;
use serde_json::Value;

use super::AgentObservationEvent;
use super::AgentRuntime;
use super::AgentTransition;
use super::JoinOptions;
use super::hook_contract::{
    AdapterDefinition, HookObservationEvent, MAX_OBSERVATION_BODY_BYTES, parse_envelope,
    parse_hint, resolve_adapter,
};
use super::join::{AgentObservationJoinStateMachine, JoinCommand, JoinHint, SeqMap};
use super::model::RawAttachment;
use super::reducer::AgentTransitionRejection;
#[cfg(test)]
use super::tests::conformance_support::{DispatchTrace, summarize_snapshot};
use super::{
    AgentActivity, AgentLifecycle, AgentTurnOutcomeKind, RawAttentionRequest, RawExecutionLocation,
    RawObservation, RawPresence,
};
use settlement_queue::SettlementQueue;

const MAX_CHILD_SESSIONS_PER_SESSION: usize = 128;
const MAX_PENDING_REQUESTS_PER_SESSION: usize = 128;
const MAX_SESSIONS: usize = 128;
pub const ACCEPTED_ENVELOPE_ACK: &str = "{\"accepted\":true,\"version\":1}\n";
#[cfg(test)]
const OBSERVATION_SOCKET_ENV: &str = "ORCHBEE_AGENT_OBSERVATION_SOCKET";

#[derive(Clone, Debug)]
struct SessionBinding {
    active_turn_id: Option<String>,
    active_turn_key: Option<String>,
    adapter: AdapterDefinition,
    child_sessions: SeqMap<String>,
    execution_location_id: String,
    /// The provider's native session key, verbatim as the admitted envelope carried it.
    #[cfg_attr(
        not(windows),
        expect(
            dead_code,
            reason = "only the managed Windows checkpoint captures resume references until POSIX restore exists"
        )
    )]
    native_key: String,
    requests: SeqMap<String>,
    /// The first run token this session's hooks carried; once set it never changes.
    run_token: Option<AgentRunToken>,
    session_id: String,
}

/// `AgentSessionObservationInput` — the event payload `dispatchFor` receives
/// before it stamps `eventId`/`generation`/`sequence`/`sessionId`.
enum SessionEventInput {
    SessionIdleObserved,
    TurnStarted {
        activity: AgentActivity,
        turn_id: String,
    },
    TurnActivityChanged {
        activity: AgentActivity,
        turn_id: String,
    },
    AttentionRequested {
        request: RawAttentionRequest,
    },
    AttentionResolved {
        request_id: String,
    },
    TurnFinished {
        outcome: AgentTurnOutcomeKind,
        turn_id: String,
    },
    PresentationAttached {
        attachment: RawAttachment,
    },
    PresentationVisibilityChanged {
        presentation_generation: String,
        presentation_id: String,
        visibility: String,
    },
    PresentationDetached {
        presentation_generation: String,
        presentation_id: String,
    },
    SessionEnded,
}

/// One root hook session joined to its native key. The key is private provider state: it may feed
/// a checkpoint reference, never the wire, logs or notices.
#[cfg_attr(
    not(windows),
    expect(
        dead_code,
        reason = "only the managed Windows checkpoint captures resume references until POSIX restore exists"
    )
)]
pub struct NativeSessionRef<'a> {
    pub session_id: &'a str,
    pub provider_id: &'a str,
    pub native_key: &'a str,
    pub run_token: Option<&'a AgentRunToken>,
}

pub struct AgentHookObservationIngress {
    allocate_identifier: u64,
    join: AgentObservationJoinStateMachine,
    sessions: SeqMap<SessionBinding>,
    settlement_queue: SettlementQueue,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "ingress socket identity kept for the test-only overlay seam; the managed endpoint owns the path in production"
        )
    )]
    socket_path: Option<String>,
    #[cfg(test)]
    trace: DispatchTrace,
}

impl AgentHookObservationIngress {
    /// `socket_path = Some` mirrors a started Desktop ingress (the corpus
    /// always starts it); `None` mirrors the disabled overlay path. The
    /// translator never owns a canonical runtime: callers pass the one
    /// `AgentRuntime` cell mutably into every runtime-touching method so
    /// reads and dispatches share a single store.
    pub fn new(socket_path: Option<String>, join_options: JoinOptions) -> Self {
        Self {
            allocate_identifier: 0,
            join: AgentObservationJoinStateMachine::new(join_options),
            sessions: SeqMap::new(),
            settlement_queue: SettlementQueue::default(),
            socket_path,
            #[cfg(test)]
            trace: DispatchTrace::default(),
        }
    }

    fn allocate(&mut self) -> String {
        self.allocate_identifier += 1;
        self.allocate_identifier.to_string()
    }

    /// `environmentOverlayFor` — registers the presentation generation and
    /// returns the socket env overlay; empty when no socket path exists.
    /// Test-only: the production overlay is generated worker-side from the
    /// validated session runtime (see terminal_host_launch_adapter), not here.
    #[cfg(test)]
    pub fn environment_overlay_for(
        &mut self,
        runtime: &mut AgentRuntime,
        presentation_id: String,
        creation_id: String,
    ) -> Vec<(String, String)> {
        let Some(path) = self.socket_path.clone() else {
            return Vec::new();
        };
        let commands = self
            .join
            .register_presentation_generation(presentation_id, creation_id);
        self.handle_join_commands(runtime, commands);
        vec![(OBSERVATION_SOCKET_ENV.to_string(), path)]
    }

    /// Registration half of `environment_overlay_for` without the socket
    /// overlay: the native leaf commits bindings on terminals that have
    /// no managed socket, so registration must not be gated on one. The
    /// existing `environment_overlay_for` behavior is unchanged — it
    /// still registers and returns the env overlay when a socket exists
    /// and returns empty otherwise.
    pub fn register_presentation_binding(
        &mut self,
        runtime: &mut AgentRuntime,
        presentation_id: String,
        creation_id: String,
    ) {
        let commands = self
            .join
            .register_presentation_generation(presentation_id, creation_id);
        self.handle_join_commands(runtime, commands);
    }

    /// `observeTerminalApplicationSignal` — the signal payload is a JSON
    /// string produced by the Host APC parser; `creation_id` and
    /// `presentation_id` come from channel identity, never the payload.
    pub fn observe_terminal_application_signal(
        &mut self,
        runtime: &mut AgentRuntime,
        payload: &str,
        creation_id: String,
        presentation_id: String,
        now: f64,
    ) {
        let Ok(value) = serde_json::from_str::<Value>(payload) else {
            return;
        };
        let Some(hint) = parse_hint(&value) else {
            return;
        };
        let commands = self.join.receive_hint(
            JoinHint {
                correlation_id: hint.correlation_id,
                event_id: hint.event_id,
                generation: creation_id,
                presentation_id,
            },
            now,
        );
        self.handle_join_commands(runtime, commands);
    }

    /// `retirePresentation`.
    pub fn retire_presentation(&mut self, runtime: &mut AgentRuntime, presentation_id: &str) {
        let commands = self.join.retire_presentation(presentation_id);
        self.handle_join_commands(runtime, commands);
    }

    /// `#acceptEnvelope` — the pure body path. Returns whether the ACK is
    /// written; transport (socket, byte streaming, deadline) stays outside.
    /// `toString('utf8')` maps to lossy decode; `JSON.parse` to serde_json.
    pub fn admit_envelope(&mut self, runtime: &mut AgentRuntime, body: &[u8], now: f64) -> bool {
        if body.len() > MAX_OBSERVATION_BODY_BYTES {
            return false;
        }
        let Ok(decoded) = serde_json::from_str::<Value>(&String::from_utf8_lossy(body)) else {
            return false;
        };
        let Some(envelope) = parse_envelope(&decoded) else {
            return false;
        };
        let Some(adapter) = resolve_adapter(&envelope.adapter_id, envelope.observation.event)
        else {
            return false;
        };
        let admission = self.join.admit_event(adapter, envelope, now);
        self.handle_join_commands(runtime, admission.commands);
        admission.accepted
    }

    /// Canonical session identity and the run token its hooks first carried, for every
    /// token-bound root session; bounded by the session cap.
    pub fn run_tokens(&self) -> impl Iterator<Item = (&str, &AgentRunToken)> {
        self.sessions.iter().filter_map(|(_, binding)| {
            binding
                .run_token
                .as_ref()
                .map(|token| (binding.session_id.as_str(), token))
        })
    }

    /// Every root hook session with its canonical identity, provider, native key and first run
    /// token; bounded by the session cap. Child sessions have no entry.
    #[cfg_attr(
        not(windows),
        expect(
            dead_code,
            reason = "only the managed Windows checkpoint captures resume references until POSIX restore exists"
        )
    )]
    pub fn native_session_refs(&self) -> impl Iterator<Item = NativeSessionRef<'_>> {
        self.sessions.iter().map(|(_, binding)| NativeSessionRef {
            session_id: &binding.session_id,
            provider_id: &binding.adapter.provider_id,
            native_key: &binding.native_key,
            run_token: binding.run_token.as_ref(),
        })
    }

    /// Production clock-adapter entry: monotone-syncs the settlement
    /// scheduler to the freshest known time. Only the new Instant-domain
    /// service seams call this at fold entry — the corpus `advance` /
    /// `advance_settlement` paths never reach it, so the recorded
    /// two-clock contract is preserved by non-reachability rather than
    /// any claimed invariance. Structural coverage: every scheduling
    /// producer inside the same fold sees the just-synced base.
    pub fn sync_settlement_clock(&mut self, now: f64) {
        self.settlement_queue.sync(now);
    }

    /// Earliest scheduled settlement in translator-time milliseconds —
    /// the production poll deadline source. `None` while no timer
    /// producer has scheduled anything.
    pub fn next_settlement_at(&self) -> Option<f64> {
        self.settlement_queue.next_fire_at()
    }

    /// Production settlement tick: syncs the scheduler to `now`, then
    /// fires every due timer through `join.settle` with that same `now`,
    /// applying the resulting commands. The `advance_settlement` shape
    /// minus `advance` — the corpus scheduler path is untouched.
    pub fn tick(&mut self, runtime: &mut AgentRuntime, now: f64) {
        self.settlement_queue.sync(now);
        while let Some((event_key, event_expires_at)) = self.settlement_queue.pop_due() {
            let commands = self.join.settle(&event_key, event_expires_at, now);
            self.handle_join_commands(runtime, commands);
        }
    }

    /// Test-scheduler advance replacing `setTimeout` + `settleWait`: the
    /// scheduler clock advances by `milliseconds`; every due timer fires
    /// `join.settle` with the injected `now` of the current step. The two
    /// clocks stay independent exactly like the recorded corpus.
    #[cfg(test)]
    pub fn advance_settlement(&mut self, runtime: &mut AgentRuntime, milliseconds: f64, now: f64) {
        self.settlement_queue.advance(milliseconds);
        while let Some((event_key, event_expires_at)) = self.settlement_queue.pop_due() {
            let commands = self.join.settle(&event_key, event_expires_at, now);
            self.handle_join_commands(runtime, commands);
        }
    }

    /// Drain the per-step dispatch log (corpus `dispatches`).
    #[cfg(test)]
    pub fn take_dispatch_log(&mut self) -> Vec<super::tests::conformance_support::DispatchRecord> {
        self.trace.take()
    }

    #[cfg(test)]
    pub fn snapshot_json(&self, runtime: &AgentRuntime) -> Value {
        summarize_snapshot(runtime.snapshot())
    }

    fn handle_join_commands(&mut self, runtime: &mut AgentRuntime, commands: Vec<JoinCommand>) {
        for command in commands {
            match command {
                JoinCommand::ScheduleSettlement {
                    event_expires_at,
                    event_key,
                    milliseconds,
                } => self.schedule_settlement(event_key, event_expires_at, milliseconds),
                JoinCommand::CancelSettlement {
                    event_expires_at,
                    event_key,
                } => self.cancel_settlement(&event_key, event_expires_at),
                JoinCommand::ApplyObservation {
                    adapter,
                    attachments,
                    envelope,
                } => {
                    let super::hook_contract::HookEnvelope {
                        event_id,
                        observation,
                        run_token,
                        ..
                    } = *envelope;
                    self.observe(
                        runtime,
                        adapter,
                        observation,
                        &event_id,
                        attachments,
                        run_token,
                    );
                }
                JoinCommand::AttachPresentations {
                    adapter_id,
                    attachments,
                    session_key,
                } => {
                    let key = session_key_for(&adapter_id, &session_key);
                    if self.sessions.get(&key).is_some() {
                        self.attach_presentations(runtime, &key, &attachments);
                    }
                }
                JoinCommand::RetirePresentationGeneration {
                    generation,
                    presentation_id,
                } => self.detach_presentation_generation(runtime, &presentation_id, &generation),
            }
        }
    }

    fn schedule_settlement(&mut self, event_key: String, event_expires_at: f64, milliseconds: f64) {
        self.settlement_queue
            .schedule(event_key, event_expires_at, milliseconds);
    }

    fn cancel_settlement(&mut self, event_key: &str, event_expires_at: f64) {
        self.settlement_queue.cancel(event_key, event_expires_at);
    }

    // ---- translator internals (one-to-one with the `#`-private methods) ----

    fn observe(
        &mut self,
        runtime: &mut AgentRuntime,
        adapter: AdapterDefinition,
        observation: super::hook_contract::HookObservation,
        external_event_id: &str,
        attachments: Vec<RawAttachment>,
        run_token: Option<AgentRunToken>,
    ) {
        let Some(binding_key) = self.ensure_session(
            runtime,
            &adapter,
            &observation.session_key,
            &attachments,
            external_event_id,
        ) else {
            return;
        };
        if let Some(binding) = self.sessions.get_mut(&binding_key)
            && binding.run_token.is_none()
        {
            binding.run_token = run_token;
        }
        self.attach_presentations(runtime, &binding_key, &attachments);
        let event_id = external_event_id_for(&adapter.adapter_id, external_event_id);
        match observation.event {
            HookObservationEvent::SessionIdentified => {}
            HookObservationEvent::SessionStarted => {
                // Capsule 0.4.7 Cursor hooks carry identity only.
                if adapter.adapter_id != "cursor" {
                    self.observe_idle(runtime, &binding_key);
                }
            }
            HookObservationEvent::SessionIdle => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.observe_idle(runtime, &binding_key);
            }
            HookObservationEvent::TurnStarted => self.start_turn(
                runtime,
                &binding_key,
                observation.activity.unwrap_or(AgentActivity::Thinking),
                observation.turn_key.clone(),
                Some(event_id),
            ),
            HookObservationEvent::ToolStarted => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.ensure_active_turn(runtime, &binding_key, observation.turn_key.clone());
                self.resolve_request(
                    runtime,
                    &binding_key,
                    &observation.operation_key,
                    Some(event_id),
                );
                self.change_activity(
                    runtime,
                    &binding_key,
                    observation.activity.unwrap_or(AgentActivity::Tool),
                );
            }
            HookObservationEvent::ToolFinished | HookObservationEvent::ToolFailed => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.resolve_request(
                    runtime,
                    &binding_key,
                    &observation.operation_key,
                    Some(event_id),
                );
                if self.active_turn_id(&binding_key).is_some() {
                    self.change_activity(runtime, &binding_key, AgentActivity::Thinking);
                }
            }
            HookObservationEvent::QuestionRequested => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.request_attention(
                    runtime,
                    &binding_key,
                    &observation.operation_key,
                    "question",
                    observation.turn_key.clone(),
                    event_id,
                );
            }
            HookObservationEvent::ApprovalRequested => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.request_attention(
                    runtime,
                    &binding_key,
                    &observation.operation_key,
                    "approval",
                    observation.turn_key.clone(),
                    event_id,
                );
            }
            HookObservationEvent::AttentionResolved => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.resolve_request(
                    runtime,
                    &binding_key,
                    &observation.operation_key,
                    Some(event_id),
                );
                if self.active_turn_id(&binding_key).is_some() {
                    self.change_activity(runtime, &binding_key, AgentActivity::Thinking);
                }
            }
            HookObservationEvent::CompactionStarted => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.ensure_active_turn(runtime, &binding_key, observation.turn_key.clone());
                self.change_activity(runtime, &binding_key, AgentActivity::Compacting);
            }
            HookObservationEvent::CompactionFinished => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                if self.active_turn_id(&binding_key).is_some() {
                    self.change_activity(runtime, &binding_key, AgentActivity::Thinking);
                }
            }
            HookObservationEvent::SubagentStarted => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.observe_subagent(runtime, &binding_key, &observation.agent_key, event_id);
            }
            HookObservationEvent::SubagentStopped => {
                if !self.accept_turn_scoped(&binding_key, &observation.turn_key) {
                    return;
                }
                self.end_subagent(runtime, &binding_key, &observation.agent_key, event_id);
            }
            HookObservationEvent::TurnStopped => {
                self.finish_turn(
                    runtime,
                    &binding_key,
                    AgentTurnOutcomeKind::Succeeded,
                    observation.turn_key.clone(),
                    Some(event_id),
                );
            }
            HookObservationEvent::TurnFailed => {
                self.finish_turn(
                    runtime,
                    &binding_key,
                    AgentTurnOutcomeKind::Failed,
                    observation.turn_key.clone(),
                    Some(event_id),
                );
            }
            HookObservationEvent::TurnCanceled => {
                self.finish_turn(
                    runtime,
                    &binding_key,
                    AgentTurnOutcomeKind::Canceled,
                    observation.turn_key.clone(),
                    Some(event_id),
                );
            }
            HookObservationEvent::TurnInterrupted => {
                self.finish_turn(
                    runtime,
                    &binding_key,
                    AgentTurnOutcomeKind::Interrupted,
                    observation.turn_key.clone(),
                    Some(event_id),
                );
            }
            HookObservationEvent::SessionEnded => {
                let session_id = self
                    .sessions
                    .get(&binding_key)
                    .map(|binding| binding.session_id.clone());
                let Some(session_id) = session_id else {
                    return;
                };
                if self
                    .dispatch_for(
                        runtime,
                        &session_id,
                        SessionEventInput::SessionEnded,
                        Some(event_id),
                    )
                    .kind()
                    == "applied"
                    && let Some(binding) = self.sessions.get_mut(&binding_key)
                {
                    binding.active_turn_id = None;
                    binding.active_turn_key = None;
                    binding.requests.clear();
                }
            }
        }
    }

    fn ensure_session(
        &mut self,
        runtime: &mut AgentRuntime,
        adapter: &AdapterDefinition,
        session_key: &str,
        attachments: &[RawAttachment],
        external_event_id: &str,
    ) -> Option<String> {
        let binding_key = session_key_for(&adapter.adapter_id, session_key);
        if self.sessions.contains_key(&binding_key) {
            return Some(binding_key);
        }
        if self.sessions.len() >= MAX_SESSIONS {
            return None;
        }
        let execution_location_id = format!("desktop:agent:execution:{}", self.allocate());
        let session_id = format!("desktop:agent:session:{}", self.allocate());
        let transition = self.dispatch(
            runtime,
            AgentObservationEvent::SessionObserved {
                capabilities: adapter.capabilities.clone(),
                event_id: external_event_id_for(&adapter.adapter_id, external_event_id),
                execution_location: RawExecutionLocation {
                    execution_location_id: execution_location_id.clone(),
                },
                observation: RawObservation {
                    fidelity: "rich".to_string(),
                    freshness: "fresh".to_string(),
                    generation: 0.0,
                    sequence: 1.0,
                    source: "hook".to_string(),
                },
                parent_session_id: RawPresence::Null,
                presentation_attachments: attachments.to_vec(),
                provider_id: adapter.provider_id.clone(),
                session_id: session_id.clone(),
            },
        );
        if transition.kind() != "applied" {
            return None;
        }
        self.sessions.set(
            binding_key.clone(),
            SessionBinding {
                active_turn_id: None,
                active_turn_key: None,
                adapter: adapter.clone(),
                child_sessions: SeqMap::new(),
                execution_location_id,
                native_key: session_key.to_owned(),
                requests: SeqMap::new(),
                run_token: None,
                session_id,
            },
        );
        Some(binding_key)
    }

    fn attach_presentations(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        attachments: &[RawAttachment],
    ) {
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let mut session_ids = vec![binding.session_id.clone()];
        session_ids.extend(
            binding
                .child_sessions
                .iter()
                .map(|(_, session_id)| session_id.clone()),
        );
        for session_id in session_ids {
            for attachment in attachments {
                let Some(session) = self.find_session(runtime, &session_id) else {
                    continue;
                };
                if session.lifecycle != AgentLifecycle::Live {
                    continue;
                }
                let existing = session
                    .presentation_attachments
                    .iter()
                    .find(|candidate| {
                        candidate.presentation_id.as_str() == attachment.presentation_id
                            && candidate.generation.as_str() == attachment.generation
                    })
                    .cloned();
                match existing {
                    None => {
                        self.dispatch_for(
                            runtime,
                            &session_id,
                            SessionEventInput::PresentationAttached {
                                attachment: attachment.clone(),
                            },
                            None,
                        );
                    }
                    Some(existing) if !existing.navigable => {
                        self.dispatch_for(
                            runtime,
                            &session_id,
                            SessionEventInput::PresentationAttached {
                                attachment: attachment.clone(),
                            },
                            None,
                        );
                    }
                    Some(existing) => {
                        if existing.visibility.as_str() != attachment.visibility.as_str() {
                            self.dispatch_for(
                                runtime,
                                &session_id,
                                SessionEventInput::PresentationVisibilityChanged {
                                    presentation_generation: attachment.generation.clone(),
                                    presentation_id: attachment.presentation_id.clone(),
                                    visibility: attachment.visibility.clone(),
                                },
                                None,
                            );
                        }
                    }
                }
            }
        }
    }

    fn observe_idle(&mut self, runtime: &mut AgentRuntime, binding_key: &str) {
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let session_id = binding.session_id.clone();
        let transition = self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::SessionIdleObserved,
            None,
        );
        if transition.kind() != "applied" {
            return;
        }
        if let Some(binding) = self.sessions.get_mut(binding_key) {
            binding.active_turn_id = None;
            binding.active_turn_key = None;
            binding.requests.clear();
        }
    }

    fn start_turn(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        activity: AgentActivity,
        turn_key: Option<String>,
        event_id: Option<String>,
    ) {
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let session_id = binding.session_id.clone();
        if binding.active_turn_id.is_some() {
            if turn_key.is_some() && binding.active_turn_key == turn_key {
                self.change_activity(runtime, binding_key, activity);
                return;
            }
            if turn_key.is_none() {
                self.change_activity(runtime, binding_key, activity);
                return;
            }
            let active_key = binding.active_turn_key.clone();
            self.finish_turn(
                runtime,
                binding_key,
                AgentTurnOutcomeKind::Interrupted,
                active_key,
                event_id.clone(),
            );
            if self
                .sessions
                .get(binding_key)
                .is_some_and(|binding| binding.active_turn_id.is_some())
            {
                return;
            }
        }
        let turn_id = format!("desktop:agent:turn:{}", self.allocate());
        let transition = self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::TurnStarted {
                activity,
                turn_id: turn_id.clone(),
            },
            event_id,
        );
        if transition.kind() != "applied" {
            return;
        }
        if let Some(binding) = self.sessions.get_mut(binding_key) {
            binding.active_turn_key = turn_key;
            binding.active_turn_id = Some(turn_id);
        }
    }

    fn ensure_active_turn(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        turn_key: Option<String>,
    ) {
        if self.active_turn_id(binding_key).is_none() {
            self.start_turn(
                runtime,
                binding_key,
                AgentActivity::Thinking,
                turn_key,
                None,
            );
        }
    }

    fn accept_turn_scoped(&mut self, binding_key: &str, turn_key: &Option<String>) -> bool {
        let Some(binding) = self.sessions.get_mut(binding_key) else {
            return false;
        };
        if turn_key.is_none() || binding.active_turn_id.is_none() {
            return true;
        }
        if binding.active_turn_key.is_none() {
            binding.active_turn_key = turn_key.clone();
            return true;
        }
        binding.active_turn_key == *turn_key
    }

    fn change_activity(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        activity: AgentActivity,
    ) {
        let Some(turn_id) = self.active_turn_id(binding_key) else {
            return;
        };
        let Some(session_id) = self
            .sessions
            .get(binding_key)
            .map(|binding| binding.session_id.clone())
        else {
            return;
        };
        self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::TurnActivityChanged { activity, turn_id },
            None,
        );
    }

    fn request_attention(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        operation_key: &Option<String>,
        kind: &'static str,
        turn_key: Option<String>,
        event_id: String,
    ) {
        let Some(operation_key) = operation_key else {
            return;
        };
        self.ensure_active_turn(runtime, binding_key, turn_key);
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let Some(turn_id) = binding.active_turn_id.clone() else {
            return;
        };
        if binding.requests.contains_key(operation_key)
            || binding.requests.len() >= MAX_PENDING_REQUESTS_PER_SESSION
        {
            return;
        }
        let session_id = binding.session_id.clone();
        let request_id = format!("desktop:agent:request:{}", self.allocate());
        let transition = self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::AttentionRequested {
                request: RawAttentionRequest {
                    action_path: "navigate-only".to_string(),
                    kind: kind.to_string(),
                    request_id: request_id.clone(),
                    turn_id: RawPresence::Value(turn_id),
                },
            },
            Some(event_id),
        );
        if transition.kind() == "applied"
            && let Some(binding) = self.sessions.get_mut(binding_key)
        {
            binding.requests.set(operation_key.clone(), request_id);
        }
    }

    fn resolve_request(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        operation_key: &Option<String>,
        event_id: Option<String>,
    ) {
        let Some(operation_key) = operation_key else {
            return;
        };
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let Some(request_id) = binding.requests.get(operation_key).cloned() else {
            return;
        };
        let session_id = binding.session_id.clone();
        let transition = self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::AttentionResolved { request_id },
            event_id,
        );
        if transition.kind() == "applied"
            && let Some(binding) = self.sessions.get_mut(binding_key)
        {
            binding.requests.remove(operation_key);
        }
    }

    fn finish_turn(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        outcome: AgentTurnOutcomeKind,
        turn_key: Option<String>,
        event_id: Option<String>,
    ) {
        if !self.accept_turn_scoped(binding_key, &turn_key) {
            return;
        }
        if self.active_turn_id(binding_key).is_none() {
            self.start_turn(
                runtime,
                binding_key,
                AgentActivity::Thinking,
                turn_key.clone(),
                None,
            );
        }
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let Some(turn_id) = binding.active_turn_id.clone() else {
            return;
        };
        let session_id = binding.session_id.clone();
        let transition = self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::TurnFinished { outcome, turn_id },
            event_id,
        );
        if transition.kind() != "applied" {
            return;
        }
        if let Some(binding) = self.sessions.get_mut(binding_key) {
            binding.active_turn_id = None;
            binding.active_turn_key = None;
            binding.requests.clear();
        }
    }

    fn observe_subagent(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        agent_key: &Option<String>,
        event_id: String,
    ) {
        let Some(agent_key) = agent_key else {
            return;
        };
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        if binding.child_sessions.contains_key(agent_key)
            || binding.child_sessions.len() >= MAX_CHILD_SESSIONS_PER_SESSION
        {
            return;
        }
        self.ensure_active_turn(runtime, binding_key, None);
        self.change_activity(runtime, binding_key, AgentActivity::Subagent);
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let parent_session_id = binding.session_id.clone();
        let Some(parent_session) = self.find_session(runtime, &parent_session_id) else {
            return;
        };
        let parent_attachments: Vec<RawAttachment> = parent_session
            .presentation_attachments
            .iter()
            .filter(|attachment| attachment.navigable)
            .map(|attachment| RawAttachment {
                generation: attachment.generation.as_str().to_string(),
                navigable: attachment.navigable,
                presentation_id: attachment.presentation_id.as_str().to_string(),
                visibility: attachment.visibility.as_str().to_string(),
            })
            .collect();
        let execution_location_id = binding.execution_location_id.clone();
        let capabilities = binding.adapter.capabilities.clone();
        let provider_id = binding.adapter.provider_id.clone();
        let session_id = format!("desktop:agent:session:{}", self.allocate());
        let transition = self.dispatch(
            runtime,
            AgentObservationEvent::SessionObserved {
                capabilities,
                event_id,
                execution_location: RawExecutionLocation {
                    execution_location_id,
                },
                observation: RawObservation {
                    fidelity: "rich".to_string(),
                    freshness: "fresh".to_string(),
                    generation: 0.0,
                    sequence: 1.0,
                    source: "hook".to_string(),
                },
                parent_session_id: RawPresence::Value(parent_session_id.clone()),
                presentation_attachments: parent_attachments,
                provider_id,
                session_id: session_id.clone(),
            },
        );
        if transition.kind() != "applied" {
            return;
        }
        if let Some(binding) = self.sessions.get_mut(binding_key) {
            binding
                .child_sessions
                .set(agent_key.clone(), session_id.clone());
        }
        let turn_id = format!("desktop:agent:turn:{}", self.allocate());
        self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::TurnStarted {
                activity: AgentActivity::Subagent,
                turn_id,
            },
            None,
        );
    }

    fn end_subagent(
        &mut self,
        runtime: &mut AgentRuntime,
        binding_key: &str,
        agent_key: &Option<String>,
        event_id: String,
    ) {
        let Some(agent_key) = agent_key else {
            return;
        };
        let Some(binding) = self.sessions.get(binding_key) else {
            return;
        };
        let Some(session_id) = binding.child_sessions.get(agent_key).cloned() else {
            return;
        };
        let transition = self.dispatch_for(
            runtime,
            &session_id,
            SessionEventInput::SessionEnded,
            Some(event_id),
        );
        if transition.kind() != "applied" {
            return;
        }
        if let Some(binding) = self.sessions.get_mut(binding_key) {
            binding.child_sessions.remove(agent_key);
        }
        if self.active_turn_id(binding_key).is_some() {
            self.change_activity(runtime, binding_key, AgentActivity::Thinking);
        }
    }

    fn detach_presentation_generation(
        &mut self,
        runtime: &mut AgentRuntime,
        presentation_id: &str,
        generation: &str,
    ) {
        let targets: Vec<String> = runtime
            .snapshot()
            .sessions
            .iter()
            .filter(|session| {
                session.presentation_attachments.iter().any(|attachment| {
                    attachment.presentation_id.as_str() == presentation_id
                        && attachment.generation.as_str() == generation
                        && attachment.navigable
                })
            })
            .map(|session| session.session_id.as_str().to_string())
            .collect();
        for session_id in targets {
            self.dispatch_for(
                runtime,
                &session_id,
                SessionEventInput::PresentationDetached {
                    presentation_generation: generation.to_string(),
                    presentation_id: presentation_id.to_string(),
                },
                None,
            );
        }
    }

    fn active_turn_id(&self, binding_key: &str) -> Option<String> {
        self.sessions
            .get(binding_key)
            .and_then(|binding| binding.active_turn_id.clone())
    }

    fn find_session(
        &self,
        runtime: &AgentRuntime,
        session_id: &str,
    ) -> Option<super::model::AgentSessionSnapshot> {
        runtime
            .snapshot()
            .sessions
            .iter()
            .find(|session| session.session_id.as_str() == session_id)
            .cloned()
    }

    /// `dispatchFor` — echoes the stored generation and `sequence + 1`. A
    /// missing session yields the synthetic `session-not-found` rejection the
    /// source constructs inline (no dispatch, no log entry).
    fn dispatch_for(
        &mut self,
        runtime: &mut AgentRuntime,
        session_id: &str,
        input: SessionEventInput,
        event_id: Option<String>,
    ) -> AgentTransition {
        // Default-parameter parity with `#dispatchFor`: the fallback event
        // id is allocated at call time, BEFORE the session lookup — an
        // implicit id burns the allocator even when the session is absent;
        // an explicit id is never evaluated.
        let event_id = event_id.unwrap_or_else(|| self.allocate_event_id());
        let Some(session) = runtime
            .snapshot()
            .sessions
            .iter()
            .find(|session| session.session_id.as_str() == session_id)
            .cloned()
        else {
            return runtime.rejected(AgentTransitionRejection::SessionNotFound);
        };
        let generation = session.observation.generation as f64;
        let sequence = session.observation.sequence as f64 + 1.0;
        let session_id_owned = session_id.to_string();
        let event = match input {
            SessionEventInput::SessionIdleObserved => AgentObservationEvent::SessionIdleObserved {
                event_id,
                generation,
                sequence,
                session_id: session_id_owned,
            },
            SessionEventInput::TurnStarted { activity, turn_id } => {
                AgentObservationEvent::TurnStarted {
                    activity: RawPresence::Value(activity.as_str().to_string()),
                    event_id,
                    generation,
                    phase: "working".to_string(),
                    sequence,
                    session_id: session_id_owned,
                    turn_id,
                }
            }
            SessionEventInput::TurnActivityChanged { activity, turn_id } => {
                AgentObservationEvent::TurnActivityChanged {
                    activity: RawPresence::Value(activity.as_str().to_string()),
                    event_id,
                    generation,
                    phase: "working".to_string(),
                    sequence,
                    session_id: session_id_owned,
                    turn_id,
                }
            }
            SessionEventInput::AttentionRequested { request } => {
                AgentObservationEvent::AttentionRequested {
                    event_id,
                    generation,
                    request,
                    sequence,
                    session_id: session_id_owned,
                }
            }
            SessionEventInput::AttentionResolved { request_id } => {
                AgentObservationEvent::AttentionResolved {
                    event_id,
                    generation,
                    request_id,
                    sequence,
                    session_id: session_id_owned,
                }
            }
            SessionEventInput::TurnFinished { outcome, turn_id } => {
                AgentObservationEvent::TurnFinished {
                    event_id,
                    generation,
                    outcome: outcome.as_str().to_string(),
                    sequence,
                    session_id: session_id_owned,
                    turn_id,
                }
            }
            SessionEventInput::PresentationAttached { attachment } => {
                AgentObservationEvent::PresentationAttached {
                    attachment,
                    event_id,
                    generation,
                    sequence,
                    session_id: session_id_owned,
                }
            }
            SessionEventInput::PresentationVisibilityChanged {
                presentation_generation,
                presentation_id,
                visibility,
            } => AgentObservationEvent::PresentationVisibilityChanged {
                event_id,
                generation,
                presentation_generation,
                presentation_id,
                sequence,
                session_id: session_id_owned,
                visibility: visibility.to_string(),
            },
            SessionEventInput::PresentationDetached {
                presentation_generation,
                presentation_id,
            } => AgentObservationEvent::PresentationDetached {
                event_id,
                generation,
                presentation_generation,
                presentation_id,
                sequence,
                session_id: session_id_owned,
            },
            SessionEventInput::SessionEnded => AgentObservationEvent::SessionEnded {
                event_id,
                generation,
                sequence,
                session_id: session_id_owned,
            },
        };
        self.dispatch(runtime, event)
    }

    fn dispatch(
        &mut self,
        runtime: &mut AgentRuntime,
        event: AgentObservationEvent,
    ) -> AgentTransition {
        let transition = runtime.dispatch(&event);
        #[cfg(test)]
        self.trace.record(&event, &transition);
        transition
    }

    fn allocate_event_id(&mut self) -> String {
        format!("desktop:agent:event:{}", self.allocate())
    }
}

fn session_key_for(adapter_id: &str, external_session_key: &str) -> String {
    format!("{adapter_id}\0{external_session_key}")
}

fn external_event_id_for(adapter_id: &str, external_event_id: &str) -> String {
    format!("desktop:agent:hook:{adapter_id}:{external_event_id}")
}

#[cfg(test)]
mod tests;
