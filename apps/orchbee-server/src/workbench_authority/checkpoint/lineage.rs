//! The `layoutLineage` rows a checkpoint carries: an open Workspace's or Tab's identity and an
//! earlier identity it stands in for. The Server adds no rows. Restore admits a checkpoint's rows
//! only when each is valid, and every save writes back the rows whose Workspace or Tab is still
//! open, so the field reads and writes as it always has.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use orchbee_workbench_protocol::Snapshot;

use super::super::identity;
use super::{Counter, WorkbenchCheckpointError};

/// The most rows a checkpoint may carry.
const MAXIMUM_LINEAGE: usize = 4096;

/// Each row's Workspace or Tab identity with the earlier identity it stands in for, in the order
/// the checkpoint listed them.
#[derive(Debug, Default)]
pub(in crate::workbench_authority) struct LayoutLineage {
    origins: BTreeMap<String, String>,
    order: VecDeque<String>,
}

impl LayoutLineage {
    /// The rows a checkpoint keeps: those whose Workspace or Tab is still open, in order. Closed
    /// ones are dropped here, so the list stays within what is open.
    pub(super) fn capture(&self, snapshot: &Snapshot) -> Vec<StoredLineage> {
        let open = open_structure(snapshot);
        self.order
            .iter()
            .filter(|reopened| open.contains(reopened.as_str()))
            .filter_map(|reopened| {
                Some(StoredLineage {
                    reopened: Counter::parse(reopened).ok()?,
                    saved: Counter::parse(self.origins.get(reopened)?).ok()?,
                })
            })
            .collect()
    }

    /// Admits a checkpoint's rows. Each names a distinct open Workspace or Tab, no later than
    /// `last_identity`, and a different saved identity. The saved identity may predate this
    /// Workbench's allocation, so it is not ordered against the other one.
    pub(super) fn restore(
        rows: &[StoredLineage],
        snapshot: &Snapshot,
        last_identity: u128,
    ) -> Result<Self, WorkbenchCheckpointError> {
        if rows.len() > MAXIMUM_LINEAGE {
            return Err(WorkbenchCheckpointError::InvalidIdentity);
        }
        let open = open_structure(snapshot);
        let mut lineage = Self::default();
        for row in rows {
            let reopened = identity(row.reopened.0);
            if row.saved.0 == row.reopened.0
                || row.reopened.0 > last_identity
                || !open.contains(reopened.as_str())
                || lineage.origins.contains_key(&reopened)
            {
                return Err(WorkbenchCheckpointError::InvalidIdentity);
            }
            lineage.order.push_back(reopened.clone());
            lineage.origins.insert(reopened, identity(row.saved.0));
        }
        Ok(lineage)
    }
}

/// One retained lineage row: an open Workspace's or Tab's identity and the saved identity it
/// stands in for.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredLineage {
    reopened: Counter,
    saved: Counter,
}

/// Every open Workspace and Tab identity.
fn open_structure(snapshot: &Snapshot) -> BTreeSet<&str> {
    snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.as_str())
        .chain(
            snapshot
                .terminal_contexts
                .iter()
                .flat_map(|context| &context.tabs)
                .map(|tab| tab.tab_id.as_str()),
        )
        .collect()
}
