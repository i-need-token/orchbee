//! The successor entry of a Server restart that keeps terminals: `extensions.successor` in
//! `session.json`. The restart's requester writes it with the successor Server's identity and the
//! activation identifier that Server commits, the successor adds the identity of the Worker it
//! created, and the commit removes it. A live process the entry names stays named until it is
//! proven gone: whoever removes the entry first ends what it names by its recorded identity, the
//! Worker first, under the record's writer lock and before the publish, through `ExactProcess`,
//! which this module leaves to its callers since process observation already depends on the
//! record. An entry this build cannot read is no entry. The rules that write the entry change a
//! record their caller read and publishes under that lock.
use super::Record;
use crate::process_registration::{ProcessIdentity, ProcessRole};
use serde::{Deserialize, Serialize};

/// The `extensions` member that carries the entry.
pub(super) const SUCCESSOR: &str = "successor";

/// One restart's successor, as the Session record names it. Members a later release adds are
/// ignored.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Successor {
    ticket: [u8; 16],
    server: ProcessIdentity,
    #[serde(
        default,
        deserialize_with = "Option::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
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

/// Names `server`, the suspended successor Server of a restart, in `record` with `ticket` and the
/// `activation_id` it will commit. Only an Active record whose Server is identified and that
/// carries no entry takes one; a caller clears an earlier entry first.
pub(crate) fn reserve(
    record: &mut Record,
    ticket: [u8; 16],
    server: ProcessIdentity,
    activation_id: [u8; 16],
) -> std::io::Result<()> {
    let recorded = record
        .processes
        .registration(ProcessRole::Server)
        .identity();
    if record.lifecycle != super::Lifecycle::Active
        || recorded.is_none()
        || recorded == Some(server)
        || record.extensions.contains_key(SUCCESSOR)
        || ticket == [0; 16]
        || activation_id == [0; 16]
        || record.activation_id == Some(activation_id)
    {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    write(
        record,
        &Successor {
            ticket,
            server,
            worker: None,
            activation_id,
        },
    )
}

/// Adds `worker`, the suspended Worker the successor created, to the entry for `ticket` whose
/// Server is `server`. An entry that names another ticket or Server, or a Worker already, refuses.
pub(crate) fn record_worker(
    record: &mut Record,
    ticket: [u8; 16],
    server: ProcessIdentity,
    worker: ProcessIdentity,
) -> std::io::Result<()> {
    let entry = Successor::of(record)
        .filter(|entry| entry.ticket == ticket && entry.server == server && entry.worker.is_none())
        .ok_or(std::io::ErrorKind::InvalidData)?;
    if worker == server || named(record, worker) {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    write(
        record,
        &Successor {
            worker: Some(worker),
            ..entry
        },
    )
}

/// The commit of a restart: while the entry for `ticket` names `server` and `worker` and the
/// record still names `replaced` as its Server, the record becomes the successor's generation.
/// Its Server and Worker are the successor pair, its activation is the one the entry reserved, the
/// entry and the ended generation's `exited` member are gone, and the Host keeps its registration.
pub(crate) fn commit(
    record: &mut Record,
    ticket: [u8; 16],
    server: ProcessIdentity,
    worker: ProcessIdentity,
    replaced: ProcessIdentity,
) -> std::io::Result<()> {
    let entry = Successor::of(record)
        .filter(|entry| {
            entry.ticket == ticket && entry.server == server && entry.worker == Some(worker)
        })
        .ok_or(std::io::ErrorKind::InvalidData)?;
    if record.lifecycle != super::Lifecycle::Active
        || record
            .processes
            .registration(ProcessRole::Server)
            .identity()
            != Some(replaced)
    {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    record
        .processes
        .hand_over(server, worker)
        .map_err(|_| std::io::ErrorKind::InvalidData)?;
    record.activation_id = Some(entry.activation_id);
    record.extensions.remove(SUCCESSOR);
    record.extensions.remove(super::EXITED);
    Ok(())
}

/// Removes the entry for `ticket`; answers whether `record` carried it.
pub(crate) fn remove(record: &mut Record, ticket: [u8; 16]) -> bool {
    if Successor::of(record).is_some_and(|entry| entry.ticket == ticket) {
        record.extensions.remove(SUCCESSOR);
        return true;
    }
    false
}

/// Whether one of `record`'s registrations already names `identity`.
fn named(record: &Record, identity: ProcessIdentity) -> bool {
    [ProcessRole::Server, ProcessRole::Worker]
        .into_iter()
        .chain(crate::process_registration::HOST_ROLES)
        .any(|role| record.processes.registration(role).identity() == Some(identity))
}

fn write(record: &mut Record, entry: &Successor) -> std::io::Result<()> {
    let value = serde_json::to_value(entry).map_err(std::io::Error::other)?;
    record.extensions.insert(SUCCESSOR.to_owned(), value);
    Ok(())
}
