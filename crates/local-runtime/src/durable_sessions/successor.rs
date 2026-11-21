//! The successor entry of a Server restart that keeps terminals: `extensions.successor` in
//! `session.json`. The restart's requester writes it with the successor Server's identity and the
//! activation identifier that Server commits, the successor adds the identity of the Worker it
//! created, and the commit removes it. A live process the entry names stays named until it is
//! proven gone: whoever removes the entry first ends what it names by its recorded identity, the
//! Worker first, under the record's writer lock and before the publish, through `ExactProcess`,
//! which this module leaves to its callers since process observation already depends on the
//! record. An entry this build cannot read is no entry.
use super::Record;
use crate::process_registration::{ProcessIdentity, ProcessRole};
use serde::Deserialize;

/// The `extensions` member that carries the entry.
pub(super) const SUCCESSOR: &str = "successor";

/// One restart's successor, as the Session record names it. Members a later release adds are
/// ignored.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub struct Successor {
    ticket: [u8; 16],
    server: ProcessIdentity,
    #[serde(default, deserialize_with = "Option::deserialize")]
    worker: Option<ProcessIdentity>,
    activation_id: [u8; 16],
}

impl Successor {
    /// The entry `record` carries, if this build can read it.
    pub(crate) fn of(record: &Record) -> Option<Self> {
        let entry = record.extensions.get(SUCCESSOR)?;
        serde_json::from_value(entry.clone()).ok()
    }

    /// The ticket every `handover` step of this restart names.
    pub fn ticket(&self) -> [u8; 16] {
        self.ticket
    }

    /// Whether the successor recorded the Worker it created.
    pub fn worker_recorded(&self) -> bool {
        self.worker.is_some()
    }

    /// Whether `record` is the successor's committed generation: it names the successor's
    /// activation and Server.
    pub(crate) fn committed_in(&self, record: &Record) -> bool {
        record.activation_id == Some(self.activation_id)
            && record
                .processes
                .registration(ProcessRole::Server)
                .identity()
                == Some(self.server)
    }
}

#[cfg(windows)]
impl Successor {
    /// The successor Server's recorded identity.
    pub(crate) fn server(&self) -> ProcessIdentity {
        self.server
    }

    /// The recorded identity of the successor's Worker, once it is recorded.
    pub(crate) fn worker(&self) -> Option<ProcessIdentity> {
        self.worker
    }
}
