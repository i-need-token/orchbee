//! UI-owned `presentation-titles` State streams on the shared connection, without a bridge
//! thread: the Session's bound Presentations in groups of at most 64, each group one stream. They
//! keep no title. A failed or closed stream changes nothing, since a Presentation keeps the name
//! it last showed, and it opens again after a delay.
use crate::local_client::{
    LocalClientOrigin, LocalOperationClock,
    multiplex::{StreamEvent, Subscription as Stream},
};
use orchbee_workbench_protocol::{
    MAXIMUM_PRESENTATION_TITLE_ROWS, PanelAgentAction, PanelAgentOutcome, PanelAgentRequest,
    PresentationTitleRow, ProtocolVersion, SessionId, StreamMode, TerminalPresentationId,
    WorkbenchRequest, WorkbenchResponse, read_workbench_response,
};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

/// An ended stream opens again after this long, so a Server that refused it for capacity or
/// restarted publishes again without waiting for its Presentations to change.
const REOPEN_DELAY: Duration = Duration::from_secs(5);

struct Group {
    presentations: Vec<TerminalPresentationId>,
    /// The open stream and the request it answers.
    open: Option<(PanelAgentRequest, Stream)>,
    reopen_at: Option<Instant>,
}

/// Why a stream's records can no longer be trusted.
struct Invalid;

pub(super) struct Subscription {
    source: Option<(LocalClientOrigin, LocalOperationClock, SessionId)>,
    groups: Vec<Group>,
}

impl Subscription {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session: SessionId,
    ) -> Self {
        Self {
            source: Some((origin, clock, session)),
            groups: Vec::new(),
        }
    }

    /// Follows `presentations` in groups of at most 64, reopening only the groups whose members
    /// changed, and returns every row that arrived since the last call, in arrival order.
    pub(super) fn poll(
        &mut self,
        presentations: &[TerminalPresentationId],
    ) -> Vec<PresentationTitleRow> {
        let Some((origin, clock, session)) = &self.source else {
            return Vec::new();
        };
        let now = clock.now();
        let chunks = presentations.chunks(MAXIMUM_PRESENTATION_TITLE_ROWS);
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
                group.open = open(origin, session, &group.presentations);
                group.reopen_at = group.open.is_none().then(|| now + REOPEN_DELAY);
            }
            if read(group, now, &mut rows).is_err() {
                // A Server of this cohort never sends such a record, so nothing it reports can
                // be trusted.
                self.close();
                return Vec::new();
            }
        }
        rows
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

impl Drop for Subscription {
    fn drop(&mut self) {
        self.close();
    }
}

fn open(
    origin: &LocalClientOrigin,
    session: &SessionId,
    presentations: &[TerminalPresentationId],
) -> Option<(PanelAgentRequest, Stream)> {
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: crate::machine_observation::request_id().ok()?,
        session_id: session.clone(),
        action: PanelAgentAction::PresentationTitles {
            presentation_ids: presentations.to_vec(),
        },
    };
    let stream = origin
        .subscribe(
            &WorkbenchRequest::PanelAgent(request.clone()),
            StreamMode::State,
        )
        .ok()?;
    Some((request, stream))
}

/// Drains one group's stream. A failed outcome, a gap or a disconnect keeps every title; an ended
/// stream opens again after [`REOPEN_DELAY`].
fn read(
    group: &mut Group,
    now: Instant,
    rows: &mut Vec<PresentationTitleRow>,
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
                match result.outcome {
                    PanelAgentOutcome::PresentationTitles { presentations } => {
                        rows.extend(presentations);
                    }
                    PanelAgentOutcome::Failed { .. } => {}
                    _ => return Err(Invalid),
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
