//! A notice has one wall-clock deadline and one Host input floor. A resize
//! changes the frame used for a send while preserving both of those facts.
use std::{
    num::{NonZeroU32, NonZeroU64},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{HostTerminalIdentity, SendScreenGate};
use orchbee_terminal_protocol::{SCREEN_GATE_QUIET_MILLISECONDS, TerminalFrameRoute};

use crate::session_coordinator::HostAgentRun;

use super::codex::RetryTime;

const RETRY_WINDOW_MILLISECONDS: u64 = 5 * 60_000;
const RETRY_INTERVAL: Duration = Duration::from_millis(SCREEN_GATE_QUIET_MILLISECONDS);

#[derive(Clone, Copy)]
pub(super) enum Screen {
    Clear,
    Working,
    Quota(RetryTime),
}

pub(super) struct Run {
    pub(super) identity: HostTerminalIdentity,
    pub(super) agent: HostAgentRun,
    last_revision: Option<u64>,
    phase: Phase,
}

enum Phase {
    BeforeNotice(Option<Origin>),
    Notice(Notice),
}

struct Notice {
    retry: RetryTime,
    generation: NonZeroU64,
    wait: Option<Wait>,
}

/// Where the current notice can have originated. A birth after the observer
/// started also works for Codex launched with an initial prompt, but in that
/// case the Host must be able to prove there has been no human terminal input.
#[derive(Clone, Copy)]
enum Origin {
    Born(u64),
    Clear(u64),
    Working { at: u64, first_revision: u64 },
}

struct Wait {
    deadline: u64,
    first_revision: u64,
    /// The first sample at or after the deadline. A sleeping machine takes no
    /// samples, so its delivery window opens when it wakes.
    due_since: Option<u64>,
    retry_after: Option<Instant>,
}

impl Origin {
    fn arm(self, retry: RetryTime, now: u64, revision: u64) -> Option<Wait> {
        let (observed_at, first_revision) = match self {
            Self::Born(at) => (at, 0),
            Self::Clear(at) => (at, revision),
            Self::Working { at, first_revision } => (at, first_revision),
        };
        Some(Wait {
            deadline: retry.deadline(now, observed_at)?,
            first_revision,
            due_since: None,
            retry_after: None,
        })
    }
}

impl Run {
    pub(super) fn new(
        identity: HostTerminalIdentity,
        agent: HostAgentRun,
        born_while_observed: Option<u64>,
    ) -> Self {
        Self {
            identity,
            agent,
            last_revision: None,
            phase: Phase::BeforeNotice(born_while_observed.map(Origin::Born)),
        }
    }

    pub(super) fn observe(
        &mut self,
        frame: TerminalFrameRoute,
        screen: Screen,
        now: u64,
        tick: Instant,
        enabled: impl FnOnce() -> bool,
    ) -> Option<SendScreenGate> {
        let TerminalFrameRoute {
            generation,
            revision,
            ..
        } = frame;
        if self.last_revision.is_some_and(|seen| revision < seen) {
            return None;
        }
        let changed = self.last_revision != Some(revision);
        self.last_revision = Some(revision);

        match screen {
            Screen::Clear => {
                // A different viewport may omit the notice. A visible working
                // turn or a positive replacement in the same view can retire it.
                let replaces = match &self.phase {
                    Phase::Notice(notice) => changed && notice.generation == generation,
                    // A clear composer ends an observed turn, so the key that
                    // submits the next turn cannot fall under this turn's floor.
                    Phase::BeforeNotice(_) => true,
                };
                if replaces {
                    self.phase = Phase::BeforeNotice(Some(Origin::Clear(now)));
                }
                None
            }
            Screen::Working => {
                // The first live work screen follows the submitting key. Keep
                // its input floor throughout this turn, even during redraws.
                let first_revision = match self.phase {
                    Phase::BeforeNotice(Some(Origin::Working { first_revision, .. })) => {
                        first_revision
                    }
                    _ => revision,
                };
                self.phase = Phase::BeforeNotice(Some(Origin::Working {
                    at: now,
                    first_revision,
                }));
                None
            }
            Screen::Quota(retry) => {
                let wait = match self.phase {
                    Phase::BeforeNotice(origin) => {
                        origin.and_then(|origin| origin.arm(retry, now, revision))
                    }
                    Phase::Notice(ref notice) if notice.retry == retry => {
                        return self.try_send(frame, now, tick, enabled);
                    }
                    Phase::Notice(_) => None,
                };
                self.phase = Phase::Notice(Notice {
                    retry,
                    generation,
                    wait,
                });
                None
            }
        }
    }

    fn try_send(
        &mut self,
        frame: TerminalFrameRoute,
        now: u64,
        tick: Instant,
        enabled: impl FnOnce() -> bool,
    ) -> Option<SendScreenGate> {
        let Phase::Notice(notice) = &mut self.phase else {
            return None;
        };
        let wait = notice.wait.as_mut()?;
        if now < wait.deadline {
            return None;
        }
        let due_since = *wait.due_since.get_or_insert(now);
        if now >= due_since.saturating_add(RETRY_WINDOW_MILLISECONDS) {
            notice.wait = None;
            return None;
        }
        if wait.retry_after.is_some_and(|after| tick < after) || !enabled() {
            return None;
        }
        wait.retry_after = Some(tick + RETRY_INTERVAL);
        Some(SendScreenGate {
            pid: NonZeroU32::new(self.agent.pid())?,
            creation_time: self.agent.created(),
            generation: frame.generation,
            revision: frame.revision,
            first_revision: wait.first_revision,
        })
    }

    pub(super) fn finish(&mut self) {
        if let Phase::Notice(notice) = &mut self.phase {
            notice.wait = None;
        }
    }
}

pub(super) fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}
