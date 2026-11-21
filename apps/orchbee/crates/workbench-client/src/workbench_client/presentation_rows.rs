//! UI-owned State streams of one Panel Agent read on the shared connection, without a bridge
//! thread: the Session's bound Presentations in groups of at most the read's row bound, each group
//! one stream. They keep no row. A failed or closed stream changes nothing, since its consumer
//! keeps what it last read, and it opens again after a delay.
use crate::local_client::{
    LocalClientOrigin, LocalOperationClock,
    multiplex::{StreamEvent, Subscription as Stream},
};
use orchbee_workbench_protocol::{
    MAXIMUM_AGENT_CONFIRMATION_ROWS, MAXIMUM_PRESENTATION_TITLE_ROWS, PanelAgentAction,
    PanelAgentOutcome, PanelAgentRequest, PresentationConfirmationRow, PresentationTitleRow,
    ProtocolVersion, SessionId, StreamMode, TerminalPresentationId, WorkbenchRequest,
    WorkbenchResponse, read_workbench_response,
};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

/// An ended stream opens again after this long, so a Server that refused it for capacity or
/// restarted publishes again without waiting for its Presentations to change.
const REOPEN_DELAY: Duration = Duration::from_secs(5);

/// One read whose answer is a row per requested Presentation.
pub(super) struct Read<Row> {
    /// The most Presentations one request may name.
    limit: usize,
    action: fn(Vec<TerminalPresentationId>) -> PanelAgentAction,
    /// The rows of the read's own outcome; `None` for any other outcome but a failure.
    rows: fn(PanelAgentOutcome) -> Option<Vec<Row>>,
}

/// The title each Presentation's terminal last reported.
pub(super) const TITLES: Read<PresentationTitleRow> = Read {
    limit: MAXIMUM_PRESENTATION_TITLE_ROWS,
    action: |presentation_ids| PanelAgentAction::PresentationTitles { presentation_ids },
    rows: |outcome| match outcome {
        PanelAgentOutcome::PresentationTitles { presentations } => Some(presentations),
        _ => None,
    },
};

/// How the Server answers the routine confirmations of the Agent in each Presentation. Only a
/// Session whose handshake answer lists `agent-confirmation` serves it.
pub(super) const CONFIRMATIONS: Read<PresentationConfirmationRow> = Read {
    limit: MAXIMUM_AGENT_CONFIRMATION_ROWS,
    action: |presentation_ids| PanelAgentAction::Confirmations { presentation_ids },
    rows: |outcome| match outcome {
        PanelAgentOutcome::Confirmations { presentations } => Some(presentations),
        _ => None,
    },
};

struct Group {
    presentations: Vec<TerminalPresentationId>,
    /// The open stream and the request it answers.
    open: Option<(PanelAgentRequest, Stream)>,
    reopen_at: Option<Instant>,
}

/// Why a stream's records can no longer be trusted.
struct Invalid;

pub(super) struct Subscription<Row> {
    read: Read<Row>,
    source: Option<(LocalClientOrigin, LocalOperationClock, SessionId)>,
    groups: Vec<Group>,
}

impl<Row> Subscription<Row> {
    pub(super) fn spawn(
        read: Read<Row>,
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session: SessionId,
    ) -> Self {
        Self {
            read,
            source: Some((origin, clock, session)),
            groups: Vec::new(),
        }
    }

    /// Follows `presentations` in groups of at most the read's bound, reopening only the groups
    /// whose members changed, and returns every row that arrived since the last call, in arrival
    /// order.
    pub(super) fn poll(&mut self, presentations: &[TerminalPresentationId]) -> Vec<Row> {
        let Some((origin, clock, session)) = &self.source else {
            return Vec::new();
        };
        let now = clock.now();
        let chunks = presentations.chunks(self.read.limit);
        if chunks.len() != self.groups.len()
            || chunks
                .clone()
                .zip(&self.groups)
                .any(|(chunk, group)| chunk != group.presentations)
        {
            let mut previous = std::mem::take(&mut self.groups);
            self.groups = chunks
                .map(|chunk| {
                    previous
                        .iter()
                        .position(|group| group.presentations == chunk)
                        .map_or_else(
                            || Group {
                                presentations: chunk.to_vec(),
                                open: None,
                                reopen_at: Some(now),
                            },
                            |index| previous.swap_remove(index),
                        )
                })
                .collect();
        }
        let mut rows = Vec::new();
        for group in &mut self.groups {
            if group.open.is_none() && group.reopen_at.is_some_and(|at| now >= at) {
                group.open = open(
                    origin,
                    session,
                    (self.read.action)(group.presentations.clone()),
                );
                group.reopen_at = group.open.is_none().then(|| now + REOPEN_DELAY);
            }
            if read(group, self.read.rows, now, &mut rows).is_err() {
                // A Server of this cohort never sends such a record, so nothing it reports can
                // be trusted.
                self.close();
                return Vec::new();
            }
        }
        rows
    }

    /// Opens every stream the Server ended at the next poll instead of after [`REOPEN_DELAY`], as
    /// it does once it serves again after replacing a lost Worker.
    pub(super) fn reopen_now(&mut self) {
        let Some((_, clock, _)) = &self.source else {
            return;
        };
        let now = clock.now();
        for group in &mut self.groups {
            if group.open.is_none() && group.reopen_at.is_some() {
                group.reopen_at = Some(now);
            }
        }
    }

    /// Whether these open streams follow a Session other than `session`, so they must be
    /// replaced. Closed streams follow none and stay closed.
    pub(super) fn follows_another(&self, session: &SessionId) -> bool {
        self.source
            .as_ref()
            .is_some_and(|(_, _, followed)| followed != session)
    }

    pub(super) fn close(&mut self) {
        self.groups.clear();
        self.source = None;
    }
}

impl<Row> Drop for Subscription<Row> {
    fn drop(&mut self) {
        self.close();
    }
}

fn open(
    origin: &LocalClientOrigin,
    session: &SessionId,
    action: PanelAgentAction,
) -> Option<(PanelAgentRequest, Stream)> {
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: crate::machine_observation::request_id().ok()?,
        session_id: session.clone(),
        action,
    };
    let stream = origin
        .subscribe(
            &WorkbenchRequest::PanelAgent(request.clone()),
            StreamMode::State,
        )
        .ok()?;
    Some((request, stream))
}

/// Drains one group's stream. A failed outcome, a gap or a disconnect keeps every row; an ended
/// stream opens again after [`REOPEN_DELAY`].
fn read<Row>(
    group: &mut Group,
    rows_of: fn(PanelAgentOutcome) -> Option<Vec<Row>>,
    now: Instant,
    rows: &mut Vec<Row>,
) -> Result<(), Invalid> {
    while let Some((request, stream)) = &group.open {
        match stream.poll() {
            Ok(Some(StreamEvent::Record(bytes))) => {
                let Ok(WorkbenchResponse::PanelAgent(result)) =
                    read_workbench_response(&mut Cursor::new(bytes))
                else {
                    return Err(Invalid);
                };
                if !result.answers(request) {
                    return Err(Invalid);
                }
                if !matches!(result.outcome, PanelAgentOutcome::Failed { .. }) {
                    rows.extend(rows_of(result.outcome).ok_or(Invalid)?);
                }
            }
            Ok(Some(StreamEvent::Gap | StreamEvent::Disconnected)) => {}
            Ok(None) => break,
            Err(_) => {
                group.open = None;
                group.reopen_at = Some(now + REOPEN_DELAY);
            }
        }
    }
    Ok(())
}
