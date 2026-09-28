//! Shared native Agent and Panel observation read lane: one worker thread holding its own
//! admitted Client Session (re-established from `LocalClientOrigin`, the
//! `status_recovery` precedent). Commands are sequential; a wake pipe lets
//! `close` interrupt an in-flight exchange, and every outcome is delivered
//! through a channel the board drains from `tick`.
//!
//! Reads never run on the native worker's primary connection: a
//! post-submission ambiguity retires the session that submitted it, and
//! that session must be this lane's own.

use orchbee_local_ipc::AsLocalIo;
#[cfg(windows)]
use orchbee_local_ipc::LocalStream as OwnedFd;
#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentListCursor, AgentListOutcome, AgentListRejectionReason, AgentReadRequest,
    AgentReadResult, AgentSafeInteger, AgentSessionId, AgentSessionSummary, MAXIMUM_LIST_LIMIT,
    ObapRequestId,
};

use crate::local_client::{
    LocalClient, LocalClientOrigin, LocalOperationClock, agent::AgentExchangeError,
};

/// 512 sessions at 100 items per page, rounded up — the walk's page bound.
/// A cursor that outlives the bound yields `complete: false`, never a
/// silently truncated listing presented as whole.
const MAXIMUM_LIST_PAGES: usize = 6;

#[cfg(windows)]
mod attention;
#[cfg(windows)]
pub(super) use attention::AttentionRound;

#[cfg(windows)]
mod panel_reports;
#[cfg(windows)]
mod physical_attention;

pub(super) enum LaneCommand {
    #[cfg(windows)]
    PhysicalAttention {
        epoch: u64,
        deadline: Instant,
    },
    #[cfg(windows)]
    PanelReports {
        epoch: u64,
        scope: super::panel_reports::Scope,
        deadline: Instant,
    },
    #[cfg(windows)]
    Attention {
        epoch: u64,
        cursor: Option<orchbee_agent_observation_protocol::AgentChangesCursor>,
        revision: Option<u64>,
        deadline: Instant,
    },
    ListRound {
        epoch: u64,
        deadline: Instant,
    },
    GetSession {
        epoch: u64,
        serial: u64,
        session: AgentSessionId,
        deadline: Instant,
    },
}

pub(super) enum LaneOutcome {
    #[cfg(windows)]
    PhysicalAttention {
        epoch: u64,
        result: Result<super::agent_attention::PhysicalRound, LaneFailure>,
    },
    #[cfg(windows)]
    PanelReports {
        epoch: u64,
        result: Result<super::panel_reports::Round, LaneFailure>,
    },
    #[cfg(windows)]
    Attention {
        epoch: u64,
        result: Result<AttentionRound, LaneFailure>,
    },
    List {
        epoch: u64,
        result: Result<ListRound, LaneFailure>,
    },
    Get {
        epoch: u64,
        serial: u64,
        result: Result<AgentGetOutcome, LaneFailure>,
    },
}

/// One completed list walk — `complete` is false when the page bound or
/// the restart budget was hit; the accumulated pages are still canonical
/// and may be shown with the incomplete marker.
pub(super) struct ListRound {
    pub(super) items: Vec<AgentSessionSummary>,
    pub(super) complete: bool,
    pub(super) revision: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LaneFailure {
    /// The runtime answered `unavailable` for the operation.
    Unavailable,
    Denied,
    /// Connect, transport, or post-submission ambiguity — the session
    /// handle is already dropped; the next request reconnects.
    Transport,
}

/// The board's lane handle: command channel, outcome drain, the wake
/// pipe's write end, and the thread join for bounded shutdown. The
/// sender is optional so `close` can drop it (waking `recv`) before
/// joining — a `Drop` type cannot move fields out otherwise.
pub(super) struct AgentReadLane {
    commands: Option<Sender<LaneCommand>>,
    outcomes: Receiver<LaneOutcome>,
    wake: OwnedFd,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl AgentReadLane {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
    ) -> std::io::Result<Self> {
        let (commands, command_rx) = mpsc::channel();
        let (outcome_tx, outcomes) = mpsc::channel();
        let (wake_read, wake_write) = wake_pair()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let join = thread::Builder::new()
            .name("orchbee-agent-lane".to_owned())
            .spawn({
                let shutdown = Arc::clone(&shutdown);
                move || run(origin, clock, command_rx, outcome_tx, wake_read, shutdown)
            })?;
        Ok(Self {
            commands: Some(commands),
            outcomes,
            wake: wake_write,
            shutdown,
            join: Some(join),
        })
    }

    /// Queue the next lane operation; `false` means the lane thread is
    /// gone and the caller should drop this handle (the next intent
    /// respawns the lane from the origin).
    pub(super) fn send(&self, command: LaneCommand) -> bool {
        self.commands
            .as_ref()
            .is_some_and(|commands| commands.send(command).is_ok())
    }

    /// A lane whose thread already exited, carrying a pre-queued outcome
    /// stream — tests drain real `LaneOutcome`s and exercise the
    /// `finished()` witness without a socket or a live thread. The
    /// command receiver is deliberately leaked so `send` still reports
    /// success: that is exactly a dead lane holding an issued credit.
    #[cfg(test)]
    pub(super) fn for_test(outcomes: Vec<LaneOutcome>) -> Self {
        let (commands, command_rx) = mpsc::channel();
        let (outcome_tx, outcome_rx) = mpsc::channel();
        for outcome in outcomes {
            let _ = outcome_tx.send(outcome);
        }
        drop(outcome_tx);
        std::mem::forget(command_rx);
        let (_wake_read, wake_write) = wake_pair().expect("test wake pipe");
        Self {
            commands: Some(commands),
            outcomes: outcome_rx,
            wake: wake_write,
            shutdown: Arc::new(AtomicBool::new(true)),
            join: None,
        }
    }

    /// Non-blocking liveness probe: the spawn path defers while a
    /// predecessor thread is still exiting so at most one lane thread is
    /// alive at a time — the UI never blocks on a join outside `close`.
    pub(super) fn finished(&self) -> bool {
        self.join.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub(super) fn take_outcome(&mut self) -> Option<LaneOutcome> {
        self.outcomes.try_recv().ok()
    }

    /// Ask the thread to exit: the shutdown flag ends the loop at its
    /// next boundary, the wake byte interrupts any in-flight exchange,
    /// and the dropped sender ends `recv` even when the thread is idle.
    /// Used by board close — the handle then lives in `dying_lanes` until
    /// the thread has actually exited.
    pub(super) fn interrupt(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = signal_wake(&self.wake);
        drop(self.commands.take());
    }

    /// Bounded close: the wake byte interrupts any in-flight exchange and
    /// the dropped command channel ends the loop after the current
    /// operation — `join` returns without waiting out a full deadline.
    pub(super) fn close(mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = signal_wake(&self.wake);
        drop(self.commands.take());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for AgentReadLane {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = signal_wake(&self.wake);
        drop(self.commands.take());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run(
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    commands: Receiver<LaneCommand>,
    outcomes: Sender<LaneOutcome>,
    wake_read: OwnedFd,
    shutdown: Arc<AtomicBool>,
) {
    let mut client: Option<LocalClient> = None;
    while let Ok(command) = commands.recv() {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let outcome = match command {
            #[cfg(windows)]
            LaneCommand::PhysicalAttention { epoch, deadline } => LaneOutcome::PhysicalAttention {
                epoch,
                result: physical_attention::read_round(
                    &origin,
                    &clock,
                    &mut client,
                    deadline,
                    wake_read.as_local_io(),
                    &shutdown,
                ),
            },
            #[cfg(windows)]
            LaneCommand::PanelReports {
                epoch,
                scope,
                deadline,
            } => LaneOutcome::PanelReports {
                epoch,
                result: panel_reports::read_round(
                    &origin,
                    &clock,
                    &mut client,
                    scope,
                    deadline,
                    wake_read.as_local_io(),
                    &shutdown,
                ),
            },
            #[cfg(windows)]
            LaneCommand::Attention {
                epoch,
                cursor,
                revision,
                deadline,
            } => LaneOutcome::Attention {
                epoch,
                result: attention::read_round(
                    &origin,
                    &clock,
                    &mut client,
                    cursor,
                    revision,
                    deadline,
                    wake_read.as_local_io(),
                    &shutdown,
                ),
            },
            LaneCommand::ListRound { epoch, deadline } => LaneOutcome::List {
                epoch,
                result: list_round(
                    &origin,
                    &clock,
                    &mut client,
                    deadline,
                    wake_read.as_local_io(),
                    &shutdown,
                ),
            },
            LaneCommand::GetSession {
                epoch,
                serial,
                session,
                deadline,
            } => LaneOutcome::Get {
                epoch,
                serial,
                result: get_session(
                    &origin,
                    &clock,
                    &mut client,
                    session,
                    deadline,
                    wake_read.as_local_io(),
                ),
            },
        };
        if outcomes.send(outcome).is_err() {
            break;
        }
    }
}

/// Walk the full list within the caller's deadline: bounded pages, all
/// pages on one revision, cursor rejections and revision splices restart
/// the walk once — a second failure yields `complete: false` rather than
/// a spliced or truncated listing passed off as whole.
fn list_round(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client: &mut Option<LocalClient>,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
    shutdown: &Arc<AtomicBool>,
) -> Result<ListRound, LaneFailure> {
    let mut restarted = false;
    'round: loop {
        let mut items = Vec::new();
        let mut cursor: Option<AgentListCursor> = None;
        let mut revision: Option<u64> = None;
        for _page in 0..MAXIMUM_LIST_PAGES {
            if shutdown.load(Ordering::SeqCst) {
                return Err(LaneFailure::Transport);
            }
            let request = AgentReadRequest::List {
                request_id: mint_request_id()?,
                include_ended: true,
                limit: AgentSafeInteger::new(MAXIMUM_LIST_LIMIT),
                cursor,
            };
            let AgentReadResult::List { result, .. } =
                submit(origin, clock, client, request, deadline, wake)?
            else {
                // The submit path already enforces operation correlation;
                // a different result kind means the transport lied.
                return Err(LaneFailure::Transport);
            };
            match result.outcome {
                AgentListOutcome::Listed {
                    items: page,
                    next_cursor,
                    revision: page_revision,
                } => {
                    match revision {
                        None => revision = Some(page_revision.value()),
                        Some(held) if held == page_revision.value() => {}
                        Some(_) => {
                            // The runtime moved under the walk: never
                            // splice two revisions into one sequence.
                            if restarted {
                                return Ok(ListRound {
                                    items,
                                    complete: false,
                                    revision,
                                });
                            }
                            restarted = true;
                            continue 'round;
                        }
                    }
                    items.extend(page);
                    match next_cursor {
                        Some(next) => cursor = Some(next),
                        None => {
                            return Ok(ListRound {
                                items,
                                complete: true,
                                revision,
                            });
                        }
                    }
                }
                AgentListOutcome::Rejected {
                    reason:
                        AgentListRejectionReason::CursorInvalid | AgentListRejectionReason::CursorStale,
                } => {
                    if restarted {
                        return Ok(ListRound {
                            items,
                            complete: false,
                            revision,
                        });
                    }
                    restarted = true;
                    continue 'round;
                }
                AgentListOutcome::Rejected {
                    reason: AgentListRejectionReason::ReadDenied,
                } => return Err(LaneFailure::Denied),
                AgentListOutcome::Unavailable { .. } => {
                    return Err(LaneFailure::Unavailable);
                }
            }
        }
        // Page bound reached with a live cursor — honest truncation.
        return Ok(ListRound {
            items,
            complete: false,
            revision,
        });
    }
}

fn get_session(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client: &mut Option<LocalClient>,
    session: AgentSessionId,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
) -> Result<AgentGetOutcome, LaneFailure> {
    let request = AgentReadRequest::Get {
        request_id: mint_request_id()?,
        session_id: session,
    };
    let AgentReadResult::Get { result, .. } =
        submit(origin, clock, client, request, deadline, wake)?
    else {
        return Err(LaneFailure::Transport);
    };
    Ok(result.outcome)
}

/// One exchange on the lane session, reconnecting first when the handle
/// is empty (startup, or after a failure retired it). Any submit error —
/// pre-handoff or the ambiguous post-submission kind — drops the handle
/// so the next request re-admits cleanly.
fn submit(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client_slot: &mut Option<LocalClient>,
    request: AgentReadRequest,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
) -> Result<AgentReadResult, LaneFailure> {
    if client_slot.is_none() {
        *client_slot = Some(
            origin
                .reconnect_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let client = client_slot.as_mut().ok_or(LaneFailure::Transport)?;
    match client.submit_agent_read_before_cancellable(request, deadline, Some(wake)) {
        Ok(result) => Ok(result),
        Err(
            AgentExchangeError::SessionUnavailable
            | AgentExchangeError::RequestEncoding(_)
            | AgentExchangeError::RequestNotSubmitted(_)
            | AgentExchangeError::Failed { .. },
        ) => {
            *client_slot = None;
            Err(LaneFailure::Transport)
        }
    }
}

/// 16 bytes of operating-system entropy rendered as 32 lowercase hex characters —
/// the same request-id discipline the CLI uses.
fn mint_request_id() -> Result<ObapRequestId, LaneFailure> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| LaneFailure::Transport)?;
    ObapRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| LaneFailure::Transport)
}

#[cfg(unix)]
fn wake_pair() -> std::io::Result<(OwnedFd, OwnedFd)> {
    nix::unistd::pipe().map_err(Into::into)
}
#[cfg(windows)]
fn wake_pair() -> std::io::Result<(OwnedFd, OwnedFd)> {
    OwnedFd::pair()
}
#[cfg(unix)]
fn signal_wake(wake: &OwnedFd) -> std::io::Result<usize> {
    nix::unistd::write(wake, &[1]).map_err(Into::into)
}
#[cfg(windows)]
fn signal_wake(mut wake: &OwnedFd) -> std::io::Result<usize> {
    std::io::Write::write(&mut wake, &[1])
}
