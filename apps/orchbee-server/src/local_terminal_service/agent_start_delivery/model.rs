//! The decision of one Agent start delivery: from the new terminal's launch kind, each read of it,
//! and the answers to the one line it may type, what to do next and which failure fact, if any,
//! ends it. Time is given by the caller; nothing here reads a clock, takes a lock or types.
use orchbee_terminal_protocol::TerminalLaunchKind;
use orchbee_workbench_protocol::AgentStartFailureReason;
use std::time::{Duration, Instant};

/// How long managed PowerShell's first command is watched from the launch's acceptance. It also
/// covers the shell's own startup, while the root shell has no child and looks idle.
pub(super) const FIRST_COMMAND_WATCH: Duration = Duration::from_secs(10);
/// How long a typed line is watched from its accepted send.
pub(super) const TYPED_WATCH: Duration = Duration::from_secs(3);
/// How long, from the launch's acceptance, a marked cmd may take to accept the line, and a
/// terminal may take to be read at all.
pub(super) const DEADLINE: Duration = Duration::from_secs(30);
/// The time between two reads, and after a refused send before the next read.
pub(super) const POLL: Duration = Duration::from_millis(250);
/// The wait after the Host found the prompt younger than its quiet period.
pub(super) const SETTLING: Duration =
    Duration::from_millis(orchbee_terminal_protocol::PROMPT_LINE_QUIET_MILLISECONDS);

/// What one process sample of the terminal showed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Observation {
    /// The Host's launch record; `None` from a Host that predates the prompt gate.
    pub(super) kind: Option<TerminalLaunchKind>,
    /// Nothing but the root shell and console hosts runs, and the sample is complete.
    pub(super) idle: bool,
    /// A process of the started provider runs.
    pub(super) provider: bool,
    /// The shell is worth one gated send now, by the readiness rule resume uses.
    pub(super) prompt: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Event {
    /// One read of the terminal, whose binding and generation are still the launch's: the sample,
    /// when one was read, and whether Client input or a shell command reached the terminal.
    Read {
        observation: Option<Observation>,
        touched: bool,
    },
    /// The Host typed the line.
    Accepted,
    /// The Host typed nothing: its prompt was not confirmed, or only settling.
    Refused { settling: bool },
    /// The Host typed nothing because it predates the prompt gate.
    GateUnsupported,
    /// Any other send result; part of the line may have been typed.
    Failed,
    /// The Presentation left or rebound, its terminal stopped answering, or a guard refused the
    /// send before anything was typed.
    Refuted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Command {
    /// Read the terminal again at this instant.
    ReadAt(Instant),
    /// Send the line once through the Host's prompt gate and report the answer.
    Send,
    /// The candidate ends, publishing this fact when there is one.
    End(Option<AgentStartFailureReason>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Unread,
    Offering { not_before: Instant },
    Sending,
    Watching { until: Instant, seen: bool },
    Ended,
}

/// One launch's candidate, from its acceptance on.
#[derive(Clone, Copy, Debug)]
pub(super) struct Delivery {
    accepted: Instant,
    state: State,
    /// The Host accepted the line; it is never sent again.
    typed: bool,
}

impl Delivery {
    pub(super) const fn new(accepted: Instant) -> Self {
        Self {
            accepted,
            state: State::Unread,
            typed: false,
        }
    }

    /// Whether the Host accepted the line.
    pub(super) const fn typed(&self) -> bool {
        self.typed
    }

    /// Advances the candidate by `event` at `now` and names what the driver does next.
    pub(super) fn step(&mut self, event: Event, now: Instant) -> Command {
        let command = self.transition(event, now);
        if matches!(command, Command::End(_)) {
            self.state = State::Ended;
        }
        command
    }

    fn transition(&mut self, event: Event, now: Instant) -> Command {
        let deadline = self.accepted + DEADLINE;
        match (self.state, event) {
            (State::Ended, _) | (_, Event::Refuted) => Command::End(None),
            (
                State::Unread,
                Event::Read {
                    observation: Some(observation),
                    touched,
                },
            ) => match observation.kind {
                None => Command::End(Some(AgentStartFailureReason::TerminalTooOld)),
                Some(TerminalLaunchKind::Other) => {
                    Command::End(Some(AgentStartFailureReason::UnsupportedShell))
                }
                Some(TerminalLaunchKind::ManagedPowershell) => {
                    self.state = State::Watching {
                        until: self.accepted + FIRST_COMMAND_WATCH,
                        seen: false,
                    };
                    self.watch(Some(observation), now)
                }
                Some(TerminalLaunchKind::Cmd) => {
                    self.state = State::Offering {
                        not_before: self.accepted,
                    };
                    self.offer(Some(observation), touched, now)
                }
            },
            (
                State::Unread,
                Event::Read {
                    observation: None, ..
                },
            ) if now >= deadline => Command::End(None),
            (State::Unread, Event::Read { .. }) => Command::ReadAt(now + POLL),
            (
                State::Offering { .. },
                Event::Read {
                    observation,
                    touched,
                },
            ) => self.offer(observation, touched, now),
            (State::Sending, Event::Accepted) => {
                self.typed = true;
                self.state = State::Watching {
                    until: now + TYPED_WATCH,
                    seen: false,
                };
                Command::ReadAt(now + POLL)
            }
            (State::Sending, Event::Refused { settling }) => {
                let not_before = now + if settling { SETTLING } else { POLL };
                if not_before >= deadline {
                    return Command::End(Some(AgentStartFailureReason::PromptUnconfirmed));
                }
                self.state = State::Offering { not_before };
                Command::ReadAt(not_before)
            }
            (State::Sending, Event::GateUnsupported) => {
                Command::End(Some(AgentStartFailureReason::TerminalTooOld))
            }
            (State::Watching { .. }, Event::Read { observation, .. }) => {
                self.watch(observation, now)
            }
            // A send's answer outside a send, a read during one, or a failed send: whatever was
            // typed stays as it is, and nothing is sent again.
            _ => Command::End(None),
        }
    }

    /// A marked cmd before its line was accepted: Client input or a command ends it silently, a
    /// confirmed prompt takes the line, and the deadline ends it as unconfirmed.
    fn offer(&mut self, observation: Option<Observation>, touched: bool, now: Instant) -> Command {
        let State::Offering { not_before } = self.state else {
            return Command::End(None);
        };
        if touched {
            return Command::End(None);
        }
        if now >= self.accepted + DEADLINE {
            return Command::End(Some(AgentStartFailureReason::PromptUnconfirmed));
        }
        if now >= not_before && observation.is_some_and(|observation| observation.prompt) {
            self.state = State::Sending;
            return Command::Send;
        }
        Command::ReadAt(not_before.max(now + POLL))
    }

    /// The early check: the shell idle again after a process of the provider was seen is
    /// `exited`; idle at the end of the watch with none seen is `not-found`; anything else, an
    /// unread sample included, ends with no fact.
    fn watch(&mut self, observation: Option<Observation>, now: Instant) -> Command {
        let State::Watching { until, seen } = &mut self.state else {
            return Command::End(None);
        };
        let settled = now >= *until;
        let Some(observation) = observation else {
            return if settled {
                Command::End(None)
            } else {
                Command::ReadAt(now + POLL)
            };
        };
        *seen |= observation.provider;
        if observation.idle && (*seen || settled) {
            return Command::End(Some(if *seen {
                AgentStartFailureReason::Exited
            } else {
                AgentStartFailureReason::NotFound
            }));
        }
        if settled {
            Command::End(None)
        } else {
            Command::ReadAt(now + POLL)
        }
    }
}
