//! The Host exchanges of one move, from the quiesce to the claim, and the abort before the claim
//! (`proposals/platform/host-handover.md`, The steps of a move). The record is relayed without
//! being parsed; the kernel, the attachments, and the turn's answer stay with the caller.
//!
//! Every step has its own budget, since a Host exchange that passes its deadline ends the
//! connection, and a forward step starts only while both abort steps still fit in the request's
//! bound after it.

use std::{
    error::Error,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{HostEpoch, HostTerminalId, HostTerminalIdentity};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::{
    CreationId, DiscardOutcome, DiscardTarget, HandoverControl, HandoverProcess,
    HandoverRecordChunk, ImportTerminal, QuiesceTerminal, ReadHandoverRecord, ResumeOutcome,
    TerminalId,
};

use crate::{
    host_connection::{HostBrokerConnection, HostFailure, NotStarted},
    session_keeper::{HostRefusal, Serving, request},
};

/// The mark's deadline and the import's, which the Worker sets to 5 seconds.
const DEADLINE_SECONDS: u8 = 5;
/// The quiesce: the Host's three waits of up to a second each, the export, and margin.
const QUIESCE_BUDGET: Duration = Duration::from_secs(6);
/// The restore builds the terminal and pulls the Keeper's duplicates.
const RESTORE_BUDGET: Duration = Duration::from_secs(5);
/// The claim and each abort step wait on the Keeper, which a Host bounds at 2 seconds.
const KEEPER_BUDGET: Duration = Duration::from_secs(3);
/// The import and each record chunk.
const STEP_BUDGET: Duration = Duration::from_secs(2);
/// What a forward step leaves of the request's bound: both abort steps.
const ABORT_RESERVE: Duration = Duration::from_secs(6);

/// Why a step did not move the terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refused {
    /// The terminal stays on its Host for the rest of this Worker's life.
    Ineligible(&'static str),
    /// A later turn may pass.
    Passing(&'static str),
    /// The step failed, which stops moves.
    Failed(&'static str),
}

impl Refused {
    pub(super) const fn class(self) -> &'static str {
        match self {
            Self::Ineligible(_) => "ineligible",
            Self::Passing(_) => "passing",
            Self::Failed(_) => "failed",
        }
    }

    pub(super) const fn reason(self) -> &'static str {
        match self {
            Self::Ineligible(reason) | Self::Passing(reason) | Self::Failed(reason) => reason,
        }
    }
}

/// A move that committed.
#[derive(Clone, Copy, Debug)]
pub(super) struct Moved {
    pub(super) to: HostTerminalIdentity,
    pub(super) settled: bool,
    pub(super) record_bytes: u32,
}

/// How a move ended.
pub(super) enum Ended {
    Moved(Moved),
    /// Nothing moved: the step that stopped it and why.
    Refused {
        step: &'static str,
        refused: Refused,
    },
}

/// One move of the terminal that runs as `from` in `old` into `new`, whose process `target` is.
pub(super) struct Move<'a> {
    pub(super) server: &'a UnixStream,
    pub(super) old: &'a HostBrokerConnection,
    pub(super) new: &'a HostBrokerConnection,
    pub(super) target: HandoverProcess,
    pub(super) from: HostTerminalIdentity,
}

/// What the new Host holds of the move when it aborts.
#[derive(Clone, Copy)]
enum Imported {
    /// An import or none: a discard of the creation is harmless either way.
    Record,
    /// A restored terminal, which a discard may find claimed.
    Restored(TerminalId),
}

impl Move<'_> {
    pub(super) fn run(&self) -> Result<Ended, Box<dyn Error>> {
        let terminal = TerminalId::from_bytes(self.from.terminal_id().as_bytes())?;
        let creation = CreationId::from_bytes(self.from.creation_id().as_bytes())?;
        let quiesce = HandoverControl::QuiesceTerminal(QuiesceTerminal {
            terminal_id: terminal,
            target: self.target,
            deadline_seconds: DEADLINE_SECONDS,
        });
        let record_bytes = match self.forward(self.old, QUIESCE_BUDGET, &quiesce) {
            Ok(HandoverControl::TerminalQuiesced(quiesced)) if quiesced.terminal_id == terminal => {
                quiesced.record_bytes
            }
            // A quiesce refused before its export changed nothing; one that failed may have left
            // a mark, which the resume settles.
            Ok(_) => return self.abort("quiesce", Refused::Failed("unexpected"), None, 0),
            Err(error) => {
                let refused = classify(error)?;
                if matches!(refused, Refused::Failed(_)) {
                    return self.abort("quiesce", refused, None, 0);
                }
                return Ok(Ended::Refused {
                    step: "quiesce",
                    refused,
                });
            }
        };
        let import = HandoverControl::ImportTerminal(ImportTerminal {
            creation_id: creation,
            record_bytes,
            deadline_seconds: DEADLINE_SECONDS,
        });
        match self.forward(self.new, STEP_BUDGET, &import) {
            Ok(HandoverControl::TerminalImported(imported)) if imported == creation => {}
            Ok(_) => {
                let unexpected = Refused::Failed("unexpected");
                return self.abort("import", unexpected, Some(Imported::Record), record_bytes);
            }
            Err(error) => {
                let refused = on_new(classify(error)?);
                return self.abort("import", refused, Some(Imported::Record), record_bytes);
            }
        }
        if let Err(refused) = self.relay(terminal, creation, record_bytes)? {
            return self.abort("record", refused, Some(Imported::Record), record_bytes);
        }
        let restore = HandoverControl::RestoreTerminal(creation);
        let to = match self.forward(self.new, RESTORE_BUDGET, &restore) {
            Ok(HandoverControl::TerminalRestored(identity))
                if identity.creation_id == creation
                    && identity.host_epoch == self.new.host_epoch() =>
            {
                identity
            }
            Ok(_) => {
                let unexpected = Refused::Failed("unexpected");
                return self.abort("restore", unexpected, Some(Imported::Record), record_bytes);
            }
            Err(error) => {
                let refused = on_new(classify(error)?);
                return self.abort("restore", refused, Some(Imported::Record), record_bytes);
            }
        };
        let moved = |settled| self.moved(to.terminal_id, settled, record_bytes);
        let restored = Some(Imported::Restored(to.terminal_id));
        match self.forward(
            self.new,
            KEEPER_BUDGET,
            &HandoverControl::ClaimTerminal(to.terminal_id),
        ) {
            Ok(HandoverControl::TerminalClaimed(claimed)) if claimed == to.terminal_id => {
                moved(false)
            }
            Ok(_) => self.abort(
                "claim",
                Refused::Failed("unexpected"),
                restored,
                record_bytes,
            ),
            Err(error) => match classify(error)? {
                // A settled import is the new Host's once restored, and refuses its claim.
                Refused::Ineligible("not-running") => moved(true),
                refused => self.abort("claim", refused, restored, record_bytes),
            },
        }
    }

    /// The move's end once the new Host runs it as `terminal`, under the creation it kept.
    fn moved(
        &self,
        terminal: TerminalId,
        settled: bool,
        record_bytes: u32,
    ) -> Result<Ended, Box<dyn Error>> {
        Ok(Ended::Moved(Moved {
            to: HostTerminalIdentity::new(
                self.from.creation_id(),
                HostEpoch::from_bytes(*self.new.host_epoch().as_bytes())?,
                HostTerminalId::from_bytes(*terminal.as_bytes())?,
            ),
            settled,
            record_bytes,
        }))
    }

    /// Relays the record chunk by chunk; `Err` names why a chunk did not pass.
    fn relay(
        &self,
        terminal: TerminalId,
        creation: CreationId,
        record_bytes: u32,
    ) -> Result<Result<(), Refused>, Box<dyn Error>> {
        let mut offset = 0_u32;
        while offset < record_bytes {
            let read = HandoverControl::ReadHandoverRecord(ReadHandoverRecord {
                terminal_id: terminal,
                offset,
            });
            let chunk = match self.forward(self.old, STEP_BUDGET, &read) {
                Ok(HandoverControl::HandoverRecordChunk(chunk))
                    if chunk.id == *terminal.as_bytes()
                        && chunk.offset == offset
                        && !chunk.chunk.is_empty()
                        && u64::from(offset) + chunk.chunk.len() as u64
                            <= u64::from(record_bytes) =>
                {
                    chunk.chunk
                }
                Ok(_) => return Ok(Err(Refused::Failed("unexpected"))),
                Err(error) => return classify(error).map(Err),
            };
            let end = offset + u32::try_from(chunk.len())?;
            let write = HandoverControl::WriteHandoverRecord(HandoverRecordChunk {
                id: *creation.as_bytes(),
                offset,
                chunk,
            });
            match self.forward(self.new, STEP_BUDGET, &write) {
                Ok(HandoverControl::HandoverRecordWritten(written))
                    if written.creation_id == creation && written.received_bytes == end => {}
                Ok(_) => return Ok(Err(Refused::Failed("unexpected"))),
                Err(error) => return classify(error).map(|refused| Err(on_new(refused))),
            }
            offset = end;
        }
        Ok(Ok(()))
    }

    /// A forward step, which starts only while both abort steps still fit after its budget.
    fn forward(
        &self,
        host: &HostBrokerConnection,
        budget: Duration,
        value: &HandoverControl,
    ) -> Result<HandoverControl, Box<dyn Error>> {
        if let Some(deadline) = self.old.pacing.request_deadline()
            && Instant::now() + budget + ABORT_RESERVE > deadline
        {
            return Err(NotStarted.into());
        }
        request(host, Serving::Within(self.server, budget), value)
    }

    /// Undoes a move before its claim: the new Host discards what it imported, and the old Host
    /// resumes the terminal. A claim either answer reveals completes the move instead. A step
    /// refused here stops moves and leaves the rest to the mark's and the import's deadlines; only
    /// answers that contradict each other end the Worker.
    fn abort(
        &self,
        step: &'static str,
        refused: Refused,
        imported: Option<Imported>,
        record_bytes: u32,
    ) -> Result<Ended, Box<dyn Error>> {
        let creation = CreationId::from_bytes(self.from.creation_id().as_bytes())?;
        let terminal = TerminalId::from_bytes(self.from.terminal_id().as_bytes())?;
        // The abort step that was refused, which stops moves.
        let mut undone: Result<(), (&'static str, Refused)> = Ok(());
        let mut discarded = false;
        if let Some(imported) = imported {
            let target = match imported {
                Imported::Record => DiscardTarget::Import(creation),
                Imported::Restored(terminal) => DiscardTarget::Terminal(terminal),
            };
            let discard = HandoverControl::DiscardImport(target);
            match request(
                self.new,
                Serving::Within(self.server, KEEPER_BUDGET),
                &discard,
            ) {
                Ok(HandoverControl::ImportDiscarded(DiscardOutcome::Discarded)) => discarded = true,
                Ok(HandoverControl::ImportDiscarded(DiscardOutcome::Claimed)) => {
                    // Only a restored terminal can be claimed: its claim answer was out of
                    // contract, and the move completed.
                    let Imported::Restored(terminal) = imported else {
                        return Err(contradiction());
                    };
                    return self.moved(terminal, false, record_bytes);
                }
                Ok(_) => undone = Err(("discard", Refused::Failed("unexpected"))),
                Err(error) => undone = Err(("discard", Refused::Failed(classify(error)?.reason()))),
            }
        }
        let resume = HandoverControl::ResumeTerminal(terminal);
        match request(
            self.old,
            Serving::Within(self.server, KEEPER_BUDGET),
            &resume,
        ) {
            Ok(HandoverControl::TerminalResumed(resumed)) if resumed.terminal_id == terminal => {
                if resumed.outcome == ResumeOutcome::ClaimedElsewhere {
                    // The new Host claimed the terminal while its discard was refused.
                    return match imported {
                        Some(Imported::Restored(terminal)) if !discarded => {
                            self.moved(terminal, false, record_bytes)
                        }
                        _ => Err(contradiction()),
                    };
                }
            }
            Ok(_) => undone = undone.and(Err(("resume", Refused::Failed("unexpected")))),
            Err(error) => {
                let failed = Refused::Failed(classify(error)?.reason());
                undone = undone.and(Err(("resume", failed)));
            }
        }
        match undone {
            Ok(()) => Ok(Ended::Refused { step, refused }),
            // The move's own refusal is recorded first; the refused abort step stops moves, and
            // the mark's and the import's deadlines settle what it left.
            Err((undo, failed)) => {
                crate::diagnostic_events::terminal_move_failed(
                    step,
                    refused.class(),
                    refused.reason(),
                    false,
                );
                Ok(Ended::Refused {
                    step: undo,
                    refused: failed,
                })
            }
        }
    }
}

/// A refusal from the new Host: a record it finds ineligible fails the move, since every other
/// terminal's record would meet the same Host.
const fn on_new(refused: Refused) -> Refused {
    match refused {
        Refused::Ineligible(reason) => Refused::Failed(reason),
        refused => refused,
    }
}

fn contradiction() -> Box<dyn Error> {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "the two Hosts disagree on whether a terminal moved",
    )
    .into()
}

/// The class of a step's error, or the error itself when the exchange was lost, which ends the
/// Worker as any lost Host exchange does.
pub(super) fn classify(error: Box<dyn Error>) -> Result<Refused, Box<dyn Error>> {
    if HostFailure::find(error.as_ref()).is_some() {
        return Err(error);
    }
    if NotStarted::is(error.as_ref()) {
        return Ok(Refused::Passing("no-room"));
    }
    let Some(HostRefusal(refusal)) = error.downcast_ref::<HostRefusal>() else {
        // An answer arrived whole but out of contract.
        return Ok(Refused::Failed("unexpected"));
    };
    let reason = token(refusal.reason.as_deref().unwrap_or(refusal.code.as_str()));
    Ok(match refusal.code.as_str() {
        "move-ineligible" => Refused::Ineligible(reason),
        "move-passing" => Refused::Passing(reason),
        _ => Refused::Failed(reason),
    })
}

/// A refusal's reason or code as one of the tokens a diagnostic record may carry.
fn token(value: &str) -> &'static str {
    const KNOWN: [&str; 17] = [
        "not-running",
        "unheld",
        "system-console",
        "unknown-console-build",
        "holds-image",
        "too-large",
        "two-keepers",
        "input-pending",
        "signal-pending",
        "mid-sequence",
        "moving",
        "invalid-phase",
        "missing-resource",
        "invalid-field",
        "host-draining",
        "resource-exhausted",
        "terminal-quiesced",
    ];
    KNOWN
        .into_iter()
        .find(|known| *known == value)
        .unwrap_or("other")
}
