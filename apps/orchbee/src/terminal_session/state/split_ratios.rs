//! Volatile per-Client split weights, reconciled only by a full Snapshot at a changed commit.
use orchbee_workbench_protocol::{
    ClientLayoutAdjustment, ClientLayoutChange, ClientSplitRatio,
    ClientViewControlReason as Reason, PanelNode, Snapshot, WorkbenchPanelSlotId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::collections::BTreeMap;

const MAXIMUM_OVERRIDES: usize = 4096;
#[cfg(test)]
mod tests;
type Key = (WorkbenchTerminalContextId, WorkbenchTabId, WorkbenchSplitId);
#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    first: WorkbenchPanelSlotId,
    second: WorkbenchPanelSlotId,
    ratio: ClientSplitRatio,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct SplitRatios {
    entries: BTreeMap<Key, Entry>,
    revision: Option<WorkbenchRevision>,
    sequence: u64,
}
impl SplitRatios {
    #[cfg(windows)]
    pub(super) fn sequence(&self) -> WorkbenchRevision {
        WorkbenchRevision::new(self.sequence)
    }
    pub(super) fn get(
        &self,
        context: &WorkbenchTerminalContextId,
        tab: &WorkbenchTabId,
        split: &WorkbenchSplitId,
        first: &WorkbenchPanelSlotId,
        second: &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio {
        self.entries
            .get(&(context.clone(), tab.clone(), split.clone()))
            .filter(|entry| &entry.first == first && &entry.second == second)
            .map_or(ClientSplitRatio::HALF, |entry| entry.ratio)
    }

    pub(super) fn commit(
        &mut self,
        snapshot: &Snapshot,
        adjustment: &ClientLayoutAdjustment,
    ) -> Result<(), Reason> {
        if self
            .revision
            .is_some_and(|revision| snapshot.revision < revision)
            || snapshot.revision < adjustment.workbench_revision
        {
            return Err(Reason::StaleView);
        }
        let ClientLayoutChange::Split { split } = &adjustment.change else {
            return Ok(());
        };
        let key = (
            adjustment.terminal_context_id.clone(),
            adjustment.tab_id.clone(),
            split.split_id.clone(),
        );
        let Some((first, second)) = slots(snapshot, &key) else {
            return Err(Reason::TargetUnavailable);
        };
        if first != &split.first_slot_id || second != &split.second_slot_id {
            return Err(Reason::TargetUnavailable);
        }
        if self.get(&key.0, &key.1, &key.2, first, second) != split.previous_ratio {
            return Err(Reason::StaleView);
        }
        if !split.changed() {
            return Ok(());
        }
        // Stage cleanup and insertion together. A refusal never partially prunes state.
        let mut entries = self.entries.clone();
        entries.retain(|key, entry| {
            slots(snapshot, key)
                .is_some_and(|(first, second)| first == &entry.first && second == &entry.second)
        });
        if split.ratio == ClientSplitRatio::HALF {
            entries.remove(&key);
        } else {
            if entries.len() >= MAXIMUM_OVERRIDES && !entries.contains_key(&key) {
                return Err(Reason::Busy);
            }
            entries.insert(
                key,
                Entry {
                    first: first.clone(),
                    second: second.clone(),
                    ratio: split.ratio,
                },
            );
        }
        let sequence = self.sequence.checked_add(1).ok_or(Reason::Busy)?;
        self.entries = entries;
        self.revision = Some(snapshot.revision);
        self.sequence = sequence;
        Ok(())
    }
}

fn slots<'a>(
    snapshot: &'a Snapshot,
    key: &Key,
) -> Option<(&'a WorkbenchPanelSlotId, &'a WorkbenchPanelSlotId)> {
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == key.0)?
        .tabs
        .iter()
        .find(|tab| tab.tab_id == key.1)?;
    find_slots(&tab.root, &key.2)
}
fn find_slots<'a>(
    node: &'a PanelNode,
    target: &WorkbenchSplitId,
) -> Option<(&'a WorkbenchPanelSlotId, &'a WorkbenchPanelSlotId)> {
    match node {
        PanelNode::Split {
            split_id, children, ..
        } => {
            if split_id == target {
                Some((&children[0].slot_id, &children[1].slot_id))
            } else {
                find_slots(&children[0].node, target)
                    .or_else(|| find_slots(&children[1].node, target))
            }
        }
        PanelNode::Panel { .. } => None,
    }
}
