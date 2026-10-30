//! Bounded Agent/Panel domain jobs share one multiplexed Client carrier.
//! The worker owns only its logical lane, sequential read state and cancellation wake.
//! Physical admission and reconnect belong to the connection actor.

use orchbee_local_ipc::AsLocalIo;

use crate::background_thread::BackgroundThread;
#[cfg(windows)]
use orchbee_local_ipc::LocalStream as OwnedFd;
#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
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
mod attention;
pub(super) use attention::AttentionRound;

pub(super) mod pane_graphics;
pub(super) mod panel_reports;
pub(super) mod physical_attention;

pub(super) enum LaneCommand {
    Graphics {
        epoch: u64,
        job: pane_graphics::Job,
        deadline: Instant,
    },
    PhysicalAttention {
        epoch: u64,
        deadline: Instant,
    },
    PanelReports {
        epoch: u64,
        scope: panel_reports::Scope,
        deadline: Instant,
    },
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
    Graphics {
        epoch: u64,
        result: Result<pane_graphics::Round, LaneFailure>,
    },
    PhysicalAttention {
        epoch: u64,
        result: Result<physical_attention::PhysicalRound, LaneFailure>,
    },
    PanelReports {
        epoch: u64,
        result: Result<panel_reports::Round, LaneFailure>,
    },
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
    /// logical lane is already dropped; the connection actor owns any reconnect.
    Transport,
}

/// The board's lane handle: command channel, outcome drain, the wake
/// pipe's write end, and the thread join for bounded shutdown. The
/// sender is optional so `close` can drop it (waking `recv`) before
/// joining — a `Drop` type cannot move fields out otherwise.
pub(super) struct AgentReadLane {
    commands: Option<SyncSender<LaneCommand>>,
    outcomes: Receiver<LaneOutcome>,
    wake: OwnedFd,
    shutdown: Arc<AtomicBool>,
    thread: BackgroundThread,
}

impl AgentReadLane {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
    ) -> std::io::Result<Self> {
        let (commands, command_rx) = mpsc::sync_channel(1);
        let (outcome_tx, outcomes) = mpsc::sync_channel(1);
        let (wake_read, wake_write) = wake_pair()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread = BackgroundThread::spawn("orchbee-agent-lane", {
            let shutdown = Arc::clone(&shutdown);
            move || run(origin, clock, command_rx, outcome_tx, wake_read, shutdown)
        })?;
        Ok(Self {
            commands: Some(commands),
            outcomes,
            wake: wake_write,
            shutdown,
            thread,
        })
    }

    /// Queue the next lane operation; `false` means the lane thread is
    /// gone and the caller should drop this handle (the next intent
    /// respawns the lane from the origin).
    pub(super) fn send(&self, command: LaneCommand) -> bool {
        self.commands
            .as_ref()
            .is_some_and(|commands| commands.try_send(command).is_ok())
    }

    /// A lane whose thread already exited, carrying a pre-queued outcome
    /// stream — tests drain real `LaneOutcome`s and exercise the
    /// `finished()` witness without a socket or a live thread. The
    /// command receiver is deliberately leaked so `send` still reports
    /// success: that is exactly a dead lane holding an issued credit.
    #[cfg(test)]
    pub(super) fn for_test(outcomes: Vec<LaneOutcome>) -> Self {
        let (commands, command_rx) = mpsc::sync_channel(1);
        let (outcome_tx, outcome_rx) = mpsc::sync_channel(outcomes.len().max(1));
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
            thread: BackgroundThread::ended(),
        }
    }

    /// Non-blocking liveness probe: the spawn path defers while a
    /// predecessor thread is still exiting so at most one lane thread is
    /// alive at a time — the UI never blocks on a join outside `close`.
    pub(super) fn finished(&self) -> bool {
        self.thread.is_finished()
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

    /// Process exit: observes the thread's end against the shared deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.interrupt();
        self.thread.join_before(deadline)
    }

    /// Bounded close: the wake byte interrupts any in-flight exchange and
    /// the dropped command channel ends the loop after the current
    /// operation — `join` returns without waiting out a full deadline.
    pub(super) fn close(mut self) {
        self.interrupt();
        let _ = self.thread.join();
    }
}

impl Drop for AgentReadLane {
    fn drop(&mut self) {
        // Only close and stop_before wait; an unfinished thread is detached here.
        self.interrupt();
    }
}

fn run(
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    commands: Receiver<LaneCommand>,
    outcomes: SyncSender<LaneOutcome>,
    wake_read: OwnedFd,
    shutdown: Arc<AtomicBool>,
) {
    let mut client: Option<LocalClient> = None;
    while let Ok(command) = commands.recv() {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let outcome = match command {
            LaneCommand::Graphics {
                epoch,
                job,
                deadline,
            } => LaneOutcome::Graphics {
                epoch,
                result: pane_graphics::read_round(
                    &origin,
                    &clock,
                    &mut client,
                    job,
                    deadline,
                    wake_read.as_local_io(),
                    &shutdown,
                ),
            },
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
        if outcomes.try_send(outcome).is_err() {
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

/// One exchange on the shared carrier, acquiring a logical scope when its handle
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
                .lane_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let client = client_slot.as_mut().ok_or(LaneFailure::Transport)?;
    match client.submit_agent_read_before_cancellable(request, deadline, Some(wake)) {
        Ok(result) => Ok(result),
        Err(
            AgentExchangeError::ConnectionUnavailable
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
