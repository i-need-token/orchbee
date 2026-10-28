//! Pure logical-time policy for Client-owned physical Agent notifications.
use std::collections::BTreeMap;

const MAXIMUM_OBSERVATIONS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentNotificationState {
    Idle,
    Done,
    Error,
    Working,
    Blocked,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentNotificationKind {
    Done,
    Request,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum AgentSoundPolicy {
    #[default]
    Default,
    On,
    Off,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct AgentNotificationIdentity {
    pub(super) presentation_id: String,
    pub(super) generation: String,
    pub(super) host_epoch: String,
    pub(super) host_terminal_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentNotificationObservation {
    pub(super) identity: AgentNotificationIdentity,
    pub(super) sequence: u64,
    pub(super) agent: Option<String>,
    pub(super) state: AgentNotificationState,
    pub(super) seen: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentNotificationPolicy {
    pub(super) delay_millis: u64,
    pub(super) popup_enabled: bool,
    pub(super) sound_enabled: bool,
    pub(super) agent_sounds: BTreeMap<String, AgentSoundPolicy>,
}

impl Default for AgentNotificationPolicy {
    fn default() -> Self {
        Self {
            delay_millis: 1000,
            popup_enabled: true,
            sound_enabled: false,
            agent_sounds: BTreeMap::new(),
        }
    }
}

impl AgentNotificationPolicy {
    fn sound_allowed(&self, agent: &str) -> bool {
        self.sound_enabled
            && self.agent_sounds.get(agent).copied().unwrap_or_default() != AgentSoundPolicy::Off
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentNotificationCommand {
    pub(super) identity: AgentNotificationIdentity,
    pub(super) sequence: u64,
    pub(super) agent: String,
    pub(super) kind: AgentNotificationKind,
    pub(super) popup: bool,
    pub(super) sound: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    identity: AgentNotificationIdentity,
    sequence: u64,
    agent: String,
    state: AgentNotificationState,
    kind: AgentNotificationKind,
    due_millis: u64,
    after_draw_serial: u64,
    configuration_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Tracked {
    observation: AgentNotificationObservation,
    candidate: Option<Candidate>,
    displayed: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct AgentNotificationScheduler {
    policy: AgentNotificationPolicy,
    configuration_revision: u64,
    draw_serial: u64,
    tracked: BTreeMap<String, Tracked>,
    retired: bool,
}

impl AgentNotificationScheduler {
    /// Installs one already-validated profile. Invalid reloads never call this boundary.
    pub(super) fn configure(&mut self, revision: u64, policy: AgentNotificationPolicy) -> bool {
        if self.retired || (self.configuration_revision == revision && self.policy == policy) {
            return false;
        }
        self.configuration_revision = revision;
        self.policy = policy;
        for tracked in self.tracked.values_mut() {
            tracked.candidate = None;
        }
        true
    }

    /// Publishes one complete physical round atomically at caller-supplied monotonic time.
    pub(super) fn observe(
        &mut self,
        now_millis: u64,
        observations: Vec<AgentNotificationObservation>,
    ) -> Result<bool, ()> {
        if self.retired {
            return Ok(false);
        }
        if observations.len() > MAXIMUM_OBSERVATIONS {
            return Err(());
        }
        let mut next = BTreeMap::new();
        for current in observations {
            let presentation = current.identity.presentation_id.clone();
            if next.contains_key(&presentation) {
                return Err(());
            }
            let previous = self.tracked.get(&presentation);
            if let Some(previous) = previous
                && previous.observation.identity == current.identity
                && (current.sequence < previous.observation.sequence
                    || (current.sequence == previous.observation.sequence
                        && current != previous.observation))
            {
                return Err(());
            }
            let candidate =
                previous.and_then(|previous| self.next_candidate(now_millis, previous, &current));
            let displayed = previous.is_some_and(|previous| {
                previous.observation.identity == current.identity
                    && previous.observation.sequence == current.sequence
                    && previous.displayed
            });
            next.insert(
                presentation,
                Tracked {
                    observation: current,
                    candidate,
                    displayed,
                },
            );
        }
        let changed = next != self.tracked;
        self.tracked = next;
        Ok(changed)
    }

    fn next_candidate(
        &self,
        now_millis: u64,
        previous: &Tracked,
        current: &AgentNotificationObservation,
    ) -> Option<Candidate> {
        if previous.observation.identity != current.identity
            || previous.observation.agent != current.agent
        {
            return None;
        }
        if previous.observation.sequence == current.sequence {
            return (!current.seen && !previous.displayed)
                .then(|| previous.candidate.clone())
                .flatten();
        }
        if current.seen {
            return None;
        }
        let kind = transition_kind(previous.observation.state, current.state)?;
        let agent = current.agent.clone()?;
        if !self.policy.popup_enabled && !self.policy.sound_allowed(&agent) {
            return None;
        }
        Some(Candidate {
            identity: current.identity.clone(),
            sequence: current.sequence,
            agent,
            state: current.state,
            kind,
            due_millis: now_millis.checked_add(self.policy.delay_millis)?,
            after_draw_serial: self.draw_serial.checked_add(1)?,
            configuration_revision: self.configuration_revision,
        })
    }

    /// A successful complete frame opens the zero-delay gate. Failed draws never call this.
    pub(super) fn after_draw(&mut self) -> bool {
        if self.retired {
            return false;
        }
        let Some(next) = self.draw_serial.checked_add(1) else {
            self.retire();
            return true;
        };
        self.draw_serial = next;
        false
    }

    /// Marks only an exact successfully painted physical version seen and cancels its candidate.
    pub(super) fn acknowledge(
        &mut self,
        identity: &AgentNotificationIdentity,
        sequence: u64,
    ) -> bool {
        let Some(tracked) = self.tracked.get_mut(&identity.presentation_id) else {
            return false;
        };
        if tracked.observation.identity != *identity || tracked.observation.sequence != sequence {
            return false;
        }
        let changed =
            !tracked.observation.seen || !tracked.displayed || tracked.candidate.is_some();
        tracked.observation.seen = true;
        tracked.candidate = None;
        tracked.displayed = true;
        changed
    }

    /// Cancels an exact successfully painted version without changing attention seen state.
    pub(super) fn suppress_painted(
        &mut self,
        identity: &AgentNotificationIdentity,
        sequence: u64,
    ) -> bool {
        let Some(tracked) = self.tracked.get_mut(&identity.presentation_id) else {
            return false;
        };
        if tracked.observation.identity != *identity || tracked.observation.sequence != sequence {
            return false;
        }
        let changed = !tracked.displayed || tracked.candidate.is_some();
        tracked.candidate = None;
        tracked.displayed = true;
        changed
    }

    /// Settles at most one ready candidate. Busy manual/user surfaces retain the candidate.
    pub(super) fn take_ready(
        &mut self,
        now_millis: u64,
        sink_available: bool,
    ) -> Option<AgentNotificationCommand> {
        if self.retired || !sink_available {
            return None;
        }
        let presentation = self
            .tracked
            .iter()
            .filter_map(|(presentation, tracked)| {
                let candidate = tracked.candidate.as_ref()?;
                (candidate.due_millis <= now_millis
                    && candidate.after_draw_serial <= self.draw_serial
                    && candidate.configuration_revision == self.configuration_revision
                    && tracked.observation.identity == candidate.identity
                    && tracked.observation.sequence == candidate.sequence
                    && tracked.observation.state == candidate.state
                    && !tracked.observation.seen
                    && !tracked.displayed)
                    .then_some((presentation, candidate))
            })
            .min_by(|(left_id, left), (right_id, right)| {
                left.due_millis
                    .cmp(&right.due_millis)
                    .then_with(|| left.after_draw_serial.cmp(&right.after_draw_serial))
                    .then_with(|| left_id.cmp(right_id))
            })
            .map(|(presentation, _)| presentation.clone())?;
        let candidate = self.tracked.get_mut(&presentation)?.candidate.take()?;
        let popup = self.policy.popup_enabled;
        let sound = self.policy.sound_allowed(&candidate.agent);
        (popup || sound).then_some(AgentNotificationCommand {
            identity: candidate.identity,
            sequence: candidate.sequence,
            agent: candidate.agent,
            kind: candidate.kind,
            popup,
            sound,
        })
    }

    /// An observation gap drops every baseline and candidate. The next complete
    /// round is a fresh baseline, so no transition is inferred across the gap.
    pub(super) fn unavailable(&mut self) -> bool {
        if self.retired || self.tracked.is_empty() {
            return false;
        }
        self.tracked.clear();
        true
    }

    pub(super) fn retire(&mut self) {
        self.tracked.clear();
        self.retired = true;
    }
}

fn transition_kind(
    previous: AgentNotificationState,
    current: AgentNotificationState,
) -> Option<AgentNotificationKind> {
    match (previous, current) {
        (
            AgentNotificationState::Working,
            AgentNotificationState::Idle | AgentNotificationState::Done,
        ) => Some(AgentNotificationKind::Done),
        (
            AgentNotificationState::Idle | AgentNotificationState::Working,
            AgentNotificationState::Blocked,
        ) => Some(AgentNotificationKind::Request),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
