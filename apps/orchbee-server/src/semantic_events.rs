//! Site-lifetime semantic history. Actual owners publish; readers never infer lost commits.
use orchbee_workbench_protocol::{
    MAXIMUM_EVENT_PAGE, MAXIMUM_EVENTS_BYTES, SemanticEvent, SemanticEventCursor,
    SemanticEventData, SemanticEventFilter, SemanticEventGap, SemanticEventKind,
    SemanticEventPosition, SemanticEventsFailure as Failure, Snapshot, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
mod projection;
mod source;
pub use source::{semantic_pane, semantic_panes_for_binding};
const RETAINED_EVENTS: usize = 512;
const RETAINED_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct SiteEventJournal(Arc<Mutex<State>>);
#[derive(Debug)]
struct State {
    epoch: WorkbenchRequestId,
    head: u64,
    lost_through: u64,
    bytes: usize,
    exhausted: bool,
    events: VecDeque<(SemanticEvent, usize)>,
    workspaces: BTreeMap<WorkbenchTerminalContextId, WorkbenchWorkspaceId>,
}
pub struct SiteEventPage {
    pub cursor: SemanticEventCursor,
    pub events: Vec<SemanticEvent>,
    pub gap: Option<SemanticEventGap>,
    pub more: bool,
}
impl SiteEventJournal {
    pub(crate) fn new(
        snapshot: &Snapshot,
    ) -> Result<Self, orchbee_workbench_protocol::ContractError> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)
            .map_err(|_| orchbee_workbench_protocol::ContractError::InvalidDocument)?;
        Ok(Self::with_epoch(
            snapshot,
            WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
                .map_err(|_| orchbee_workbench_protocol::ContractError::InvalidDocument)?,
        ))
    }
    fn with_epoch(snapshot: &Snapshot, epoch: WorkbenchRequestId) -> Self {
        Self(Arc::new(Mutex::new(State {
            epoch,
            head: 0,
            lost_through: 0,
            bytes: 0,
            exhausted: false,
            events: VecDeque::new(),
            workspaces: workspace_index(snapshot),
        })))
    }
    pub fn head(&self) -> Result<SemanticEventCursor, Failure> {
        let state = self.0.lock().map_err(|_| Failure::Unavailable)?;
        if state.exhausted {
            return Err(Failure::RevisionExhausted);
        }
        Ok(state.cursor())
    }
    pub fn validate_cursor(&self, cursor: &SemanticEventCursor) -> Result<(), Failure> {
        self.0
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .validate_cursor(cursor)
    }
    pub fn workspace_for_context(
        &self,
        id: &WorkbenchTerminalContextId,
    ) -> Option<WorkbenchWorkspaceId> {
        self.0.lock().ok()?.workspaces.get(id).cloned()
    }
    pub fn publish(
        &self,
        revision: WorkbenchRevision,
        workspace: Option<WorkbenchWorkspaceId>,
        kind: SemanticEventKind,
        data: SemanticEventData,
    ) -> bool {
        self.0
            .lock()
            .is_ok_and(|mut state| state.publish(revision, workspace, kind, data))
    }
    pub(crate) fn projection_committed(&self, before: &Snapshot, after: &Snapshot) {
        if let Ok(mut state) = self.0.lock() {
            projection::publish(&mut state, before, after);
            state.workspaces = workspace_index(after);
        }
    }
    pub fn read(
        &self,
        after: &SemanticEventCursor,
        filters: &[SemanticEventFilter],
        limit: usize,
    ) -> Result<SiteEventPage, Failure> {
        if limit == 0 || limit > MAXIMUM_EVENT_PAGE || filters.is_empty() || filters.len() > 32 {
            return Err(Failure::InvalidIntent);
        }
        let state = self.0.lock().map_err(|_| Failure::Unavailable)?;
        state.validate_cursor(after)?;
        let gap = (after.sequence.value() < state.lost_through).then(|| SemanticEventGap {
            after: after.sequence,
            through: WorkbenchRevision::new(state.lost_through),
        });
        let mut scanned = after.sequence.value().max(state.lost_through);
        let mut events = Vec::new();
        let mut bytes = 0usize;
        let mut more = false;
        for (event, length) in &state.events {
            let SemanticEventPosition::Journal { cursor } = &event.position else {
                unreachable!()
            };
            if cursor.sequence.value() <= scanned {
                continue;
            }
            let selected = filters.iter().any(|filter| matches_filter(event, filter));
            if selected
                && (events.len() >= limit
                    || bytes.saturating_add(*length) > MAXIMUM_EVENTS_BYTES - 4096)
            {
                more = true;
                break;
            }
            scanned = cursor.sequence.value();
            if selected {
                bytes += *length;
                events.push(event.clone());
            }
        }
        if !more {
            scanned = state.head;
        }
        Ok(SiteEventPage {
            cursor: SemanticEventCursor {
                epoch: state.epoch.clone(),
                sequence: WorkbenchRevision::new(scanned),
            },
            events,
            gap,
            more,
        })
    }
}
impl State {
    fn cursor(&self) -> SemanticEventCursor {
        SemanticEventCursor {
            epoch: self.epoch.clone(),
            sequence: WorkbenchRevision::new(self.head),
        }
    }
    fn validate_cursor(&self, cursor: &SemanticEventCursor) -> Result<(), Failure> {
        if self.exhausted {
            return Err(Failure::RevisionExhausted);
        }
        if cursor.epoch != self.epoch {
            return Err(Failure::CursorEpoch);
        }
        if cursor.sequence.value() > self.head {
            return Err(Failure::CursorAhead);
        }
        Ok(())
    }
    fn publish(
        &mut self,
        revision: WorkbenchRevision,
        workspace: Option<WorkbenchWorkspaceId>,
        event: SemanticEventKind,
        data: SemanticEventData,
    ) -> bool {
        let Some(next) = self.head.checked_add(1) else {
            self.exhausted = true;
            return false;
        };
        self.head = next;
        let event = SemanticEvent {
            workspace_id: workspace,
            position: SemanticEventPosition::Journal {
                cursor: self.cursor(),
            },
            workbench_revision: revision,
            event,
            data,
        };
        let length = event
            .validate()
            .ok()
            .and_then(|()| serde_json::to_vec(&event).ok())
            .map(|bytes| bytes.len());
        let Some(length) = length.filter(|length| *length <= MAXIMUM_EVENTS_BYTES - 4096) else {
            self.events.clear();
            self.bytes = 0;
            self.lost_through = self.head;
            return false;
        };
        self.bytes += length;
        self.events.push_back((event, length));
        while self.events.len() > RETAINED_EVENTS || self.bytes > RETAINED_BYTES {
            if let Some((event, length)) = self.events.pop_front() {
                self.bytes -= length;
                if let SemanticEventPosition::Journal { cursor } = event.position {
                    self.lost_through = cursor.sequence.value();
                }
            }
        }
        true
    }
}
fn workspace_index(
    snapshot: &Snapshot,
) -> BTreeMap<WorkbenchTerminalContextId, WorkbenchWorkspaceId> {
    snapshot
        .navigation
        .projects
        .iter()
        .flat_map(|project| &project.workspaces)
        .map(|workspace| {
            (
                workspace.terminal_context_id.clone(),
                workspace.workspace_id.clone(),
            )
        })
        .collect()
}

#[must_use]
pub fn matches_filter(event: &SemanticEvent, filter: &SemanticEventFilter) -> bool {
    orchbee_workbench_protocol::matches_semantic_filter(event, filter)
}

#[cfg(test)]
mod tests;
