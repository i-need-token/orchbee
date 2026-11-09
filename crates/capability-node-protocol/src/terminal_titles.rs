//! Read-only physical title observations, separate from viewer and Agent authority: a bounded read
//! of named terminals, and a cursor-paged read of the changes a Worker announces with an
//! unsolicited note. Titles are untrusted terminal text; projecting them for display, pacing the
//! reads and retaining the results belong to the Server.
use crate::{HostTerminalIdentity, OperationId, ProtocolError, WorkerGeneration, WorkerId};
use std::num::NonZeroU64;

pub const MAXIMUM_TERMINAL_TITLE_ROWS: usize = 4;
pub const MAXIMUM_TERMINAL_TITLE_BYTES: usize = 64 * 1024;
/// The most rows one change page carries.
pub const MAXIMUM_TERMINAL_TITLE_CHANGES: usize = 256;
/// The most title bytes one change page carries in all. One maximal title always fits, so a page
/// is never empty while changes remain.
pub const MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitlesRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub identities: Vec<HostTerminalIdentity>,
}
impl TerminalTitlesRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.identities.len() > MAXIMUM_TERMINAL_TITLE_ROWS
            || self.identities.iter().enumerate().any(|(i, id)| {
                self.identities[..i]
                    .iter()
                    .any(|prior| prior.terminal_id() == id.terminal_id())
            })
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalTitleObservation {
    Unavailable,
    Observed {
        revision: NonZeroU64,
        title: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitleRow {
    pub identity: HostTerminalIdentity,
    pub observation: TerminalTitleObservation,
}
impl TerminalTitleRow {
    fn title_bytes(&self) -> usize {
        match &self.observation {
            TerminalTitleObservation::Observed {
                title: Some(title), ..
            } => title.len(),
            _ => 0,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitlesResult {
    pub request_id: OperationId,
    pub rows: Vec<TerminalTitleRow>,
}
impl TerminalTitlesResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.rows.len() > MAXIMUM_TERMINAL_TITLE_ROWS
            || duplicate_terminal(&self.rows)
            || self
                .rows
                .iter()
                .any(|row| row.title_bytes() > MAXIMUM_TERMINAL_TITLE_BYTES)
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
    pub fn answers(&self, request: &TerminalTitlesRequest) -> bool {
        self.validate().is_ok()
            && request.validate().is_ok()
            && self.request_id == request.request_id
            && request
                .identities
                .iter()
                .eq(self.rows.iter().map(|r| &r.identity))
    }
}

fn duplicate_terminal(rows: &[TerminalTitleRow]) -> bool {
    rows.iter().enumerate().any(|(i, row)| {
        rows[..i]
            .iter()
            .any(|prior| prior.identity.terminal_id() == row.identity.terminal_id())
    })
}

/// A Worker's unsolicited note that its title projection holds changes after the cursor of the
/// last change read it answered. It moves no title and has no response: a read stays the only way
/// to take changes, so a lost or stale note costs only latency. The Worker writes it between
/// requests, never inside another record, and writes no second note before it answers a later
/// change read with a page or a reset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTitlesChanged {
    /// The Worker's newest change sequence when it wrote the note. A reader whose cursor already
    /// reached it has nothing to read.
    pub through: NonZeroU64,
}

/// Reads the physical title changes after `after` in the Worker's change sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTitleChangesRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    /// The `next` of the last page the reader applied, or 0 to read every current title afresh.
    /// A cursor belongs to one Worker generation.
    pub after: u64,
}

/// One page of changes, in change-sequence order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitleChangePage {
    /// Each terminal whose observation changed after the request's cursor, once, with its
    /// observation as of `next`. `Unavailable` means the Worker no longer observes that terminal;
    /// `Observed` with a null title is a title the program cleared. A fresh read lists no
    /// `Unavailable` row.
    pub rows: Vec<TerminalTitleRow>,
    /// The cursor for the next read. It passes the cursor whenever the page has rows, and may
    /// pass it with none when every change after the cursor was superseded or removed.
    pub next: u64,
    /// Whether `next` reached the Worker's newest change when it answered. A page that did not
    /// has rows, and the reader continues with another read.
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalTitleChangesOutcome {
    Page(TerminalTitleChangePage),
    /// The cursor cannot be resumed, because it is older than what the Worker retains or its Host
    /// connection was replaced. The reader discards every title it holds from this Worker
    /// generation and reads again from 0. A fresh read is never reset.
    Reset,
    /// The request named another Worker or generation; nothing was read.
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitleChangesResult {
    /// The request this result answers, echoed whole.
    pub request: TerminalTitleChangesRequest,
    pub outcome: TerminalTitleChangesOutcome,
}
impl TerminalTitleChangesResult {
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayload`] for a reset of a fresh read, or for a page with
    /// more than [`MAXIMUM_TERMINAL_TITLE_CHANGES`] rows, a terminal listed twice, a title over
    /// [`MAXIMUM_TERMINAL_TITLE_BYTES`], titles over [`MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES`] in
    /// all, a `next` behind the cursor or not past it despite rows, an incomplete page without
    /// rows, or an `Unavailable` row in a fresh read.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let after = self.request.after;
        let valid = match &self.outcome {
            TerminalTitleChangesOutcome::Page(page) => {
                let fresh = after == 0;
                page.rows.len() <= MAXIMUM_TERMINAL_TITLE_CHANGES
                    && !duplicate_terminal(&page.rows)
                    && page
                        .rows
                        .iter()
                        .all(|row| row.title_bytes() <= MAXIMUM_TERMINAL_TITLE_BYTES)
                    && page
                        .rows
                        .iter()
                        .map(TerminalTitleRow::title_bytes)
                        .sum::<usize>()
                        <= MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES
                    && page.next >= after
                    && (page.rows.is_empty() || page.next > after)
                    && (page.complete || !page.rows.is_empty())
                    && !(fresh
                        && page
                            .rows
                            .iter()
                            .any(|row| row.observation == TerminalTitleObservation::Unavailable))
            }
            TerminalTitleChangesOutcome::Reset => after != 0,
            TerminalTitleChangesOutcome::RejectedFencing => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ProtocolError::InvalidPayload)
        }
    }

    /// Whether this is a valid result echoing exactly `request`.
    pub fn answers(&self, request: &TerminalTitleChangesRequest) -> bool {
        self.validate().is_ok() && self.request == *request
    }
}
