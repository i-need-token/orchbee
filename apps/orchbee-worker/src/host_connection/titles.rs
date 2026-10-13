//! Connection-owned projection of exact physical OSC titles. Never Agent semantic state.
use orchbee_capability_node_protocol::HostTerminalIdentity;
#[cfg(any(windows, test))]
use orchbee_capability_node_protocol::TerminalTitleObservation;
use orchbee_terminal_protocol::TerminalMetadataChanged;
use std::{collections::BTreeMap, io, num::NonZeroU64};
const MAXIMUM_ROWS: usize = crate::terminal_authorization::MAXIMUM_RETAINED_TERMINAL_BINDINGS;
const MAXIMUM_BYTES: usize = 64 * 1024 * 1024;
#[derive(Default)]
pub(super) struct Titles {
    rows: BTreeMap<[u8; 16], (HostTerminalIdentity, NonZeroU64, Option<String>)>,
    bytes: usize,
}
impl Titles {
    pub(super) fn observe(&mut self, event: TerminalMetadataChanged) -> io::Result<()> {
        let identity = crate::terminal_host_launch_adapter::capability_identity(event.identity)
            .map_err(io::Error::other)?;
        let key = identity.terminal_id().as_bytes();
        let previous = self.rows.get(&key);
        if let Some((prior, revision, title)) = previous {
            if *prior != identity
                || event.revision < *revision
                || (event.revision == *revision && event.title != *title)
            {
                return Err(io::Error::other(
                    "invalid physical terminal title revision or identity",
                ));
            }
            if event.revision == *revision {
                return Ok(());
            }
        }
        let n = event.title.as_ref().map_or(0, String::len);
        if n > orchbee_capability_node_protocol::MAXIMUM_TERMINAL_TITLE_BYTES {
            return Err(io::Error::other("physical terminal title is too large"));
        }
        let old = previous
            .and_then(|(_, _, t)| t.as_ref())
            .map_or(0, String::len);
        let bytes = self
            .bytes
            .checked_sub(old)
            .and_then(|v| v.checked_add(n))
            .filter(|v| *v <= MAXIMUM_BYTES);
        if (previous.is_none() && self.rows.len() >= MAXIMUM_ROWS) || bytes.is_none() {
            return Err(io::Error::other(
                "physical terminal title capacity exhausted",
            ));
        }
        self.rows
            .insert(key, (identity, event.revision, event.title));
        self.bytes = bytes.expect("checked title bound");
        Ok(())
    }
    #[cfg(any(windows, test))]
    pub(super) fn read(&self, identity: HostTerminalIdentity) -> TerminalTitleObservation {
        match self
            .rows
            .get(&identity.terminal_id().as_bytes())
            .filter(|(i, _, _)| *i == identity)
        {
            Some((_, revision, title)) => TerminalTitleObservation::Observed {
                revision: *revision,
                title: title.clone(),
            },
            None => TerminalTitleObservation::Unavailable,
        }
    }
    pub(super) fn remove(&mut self, identity: HostTerminalIdentity) {
        let key = identity.terminal_id().as_bytes();
        if self.rows.get(&key).is_some_and(|(i, _, _)| *i == identity)
            && let Some((_, _, title)) = self.rows.remove(&key)
        {
            self.bytes -= title.as_ref().map_or(0, String::len);
        }
    }
}

#[cfg(test)]
mod tests;
