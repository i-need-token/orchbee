//! Keeps the binding record in step with the Session: the saver looks at the bindings on every poll
//! and writes the record once a change has waited [`DEBOUNCE`], each final capture writes it at
//! once, and a stop that ended the Host removes it. It runs on the saver thread or the stopping
//! thread, never on a Client request or under the Session gate. A record that cannot be written is
//! removed, so no reader adopts from one that no longer describes the Session.

use crate::{
    binding_record::{self, BindingRecord, RunningHost},
    local_terminal_service::LocalTerminalService,
    session_coordinator::BindingObservation,
};
use orchbee_capability_node_protocol::HostEpoch;
use orchbee_local_runtime::{HostPresence, RetainedSessionRecord, SessionStore};
use orchbee_workbench_protocol::WorkerId;
use std::{
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// How long the first change waits before the record is written; later changes ride along.
const DEBOUNCE: Duration = Duration::from_millis(500);
const RETRY: Duration = Duration::from_secs(1);

pub(in crate::local_worker_service::managed) struct Writer {
    store: Arc<Mutex<SessionStore>>,
    /// The Session record, read for the running Host and for the generation's activation.
    session: RetainedSessionRecord,
    /// Members a later release added to an adopted record, written back unchanged; a generation
    /// that adopted nothing has none.
    extensions: serde_json::Map<String, serde_json::Value>,
    /// The bytes last published, so an unchanged record is not written again.
    written: Option<Vec<u8>>,
    /// What the last look or write saw.
    observed: Option<BindingObservation>,
    due: Option<Instant>,
    /// The failure last reported, so one that repeats is reported once.
    reported: Option<(&'static str, String)>,
    /// Whether a record over the bound was already reported as written without launch selections.
    reported_bound: bool,
}

/// What one write did.
enum Wrote {
    Published,
    Unchanged,
    /// The Host is starting or could not be observed; the write waits for it.
    Deferred,
}

impl Writer {
    pub(super) fn new(store: Arc<Mutex<SessionStore>>, session: RetainedSessionRecord) -> Self {
        Self {
            store,
            session,
            extensions: serde_json::Map::new(),
            written: None,
            observed: None,
            due: None,
            reported: None,
            reported_bound: false,
        }
    }

    /// Writes `extensions`, the members a later release added to the record this generation
    /// adopted its Host from, back unchanged.
    pub(super) fn keep_extensions(
        &mut self,
        extensions: serde_json::Map<String, serde_json::Value>,
    ) {
        self.extensions = extensions;
    }

    /// One look from the saver: notes a change and writes once the first unwritten change waited
    /// [`DEBOUNCE`].
    pub(super) fn look(&mut self, terminals: &LocalTerminalService, worker: &WorkerId) {
        let now = Instant::now();
        match terminals.binding_observation() {
            Ok(observation) if self.observed.as_ref() != Some(&observation) => {
                self.observed = Some(observation);
                self.due.get_or_insert(now + DEBOUNCE);
            }
            Ok(_) => {}
            Err(error) => {
                self.report("capture", &error);
                self.due = Some(now + RETRY);
                return;
            }
        }
        if self.due.is_some_and(|due| now >= due) {
            self.due = match self.write(terminals, worker) {
                Wrote::Published | Wrote::Unchanged => None,
                Wrote::Deferred => Some(now + RETRY),
            };
        }
    }

    /// The write a final capture makes, whatever the schedule.
    pub(super) fn write_final(&mut self, terminals: &LocalTerminalService, worker: &WorkerId) {
        self.due = None;
        let _ = self.write(terminals, worker);
    }

    /// The write a restart's exact capture makes: the record on disk must describe the Session as
    /// captured, so a write that is deferred or fails fails the capture.
    pub(super) fn write_exact(
        &mut self,
        terminals: &LocalTerminalService,
        worker: &WorkerId,
    ) -> io::Result<()> {
        self.due = None;
        match self.write(terminals, worker) {
            Wrote::Published | Wrote::Unchanged => Ok(()),
            Wrote::Deferred => Err(io::Error::other("binding record not written")),
        }
    }

    /// Reads `session`, the generation a restart's successor committed, from now on, and publishes
    /// at the next look: the record on disk names the replaced generation's activation.
    pub(super) fn renew(&mut self, session: RetainedSessionRecord) {
        self.session = session;
        self.written = None;
        self.observed = None;
        self.due = None;
    }

    /// Forgets what was last published, so the next write publishes even an unchanged record: a
    /// restart's successor may have replaced it while it held the store.
    pub(super) fn forget_written(&mut self) {
        self.written = None;
        self.observed = None;
    }

    /// Removes the record once no running terminal is left to adopt.
    pub(super) fn remove(&mut self) {
        let removed = self
            .store
            .lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))
            .and_then(|mut store| store.remove_bindings());
        match removed {
            Ok(()) => self.written = None,
            Err(error) => self.report("remove", &error),
        }
    }

    fn write(&mut self, terminals: &LocalTerminalService, worker: &WorkerId) -> Wrote {
        let slots = match self.session.hosts() {
            Ok(slots) => slots,
            Err(error) => {
                self.report("host", &error);
                return Wrote::Deferred;
            }
        };
        let mut hosts = Vec::new();
        for slot in slots {
            match slot.presence {
                HostPresence::NotStarted | HostPresence::Gone => {}
                HostPresence::Running(process) => hosts.push(RunningHost {
                    process: process.and_then(binding_record::registered_process),
                    epoch: slot
                        .epoch
                        .and_then(|epoch| HostEpoch::from_bytes(epoch).ok()),
                }),
                HostPresence::Spawning | HostPresence::Unknown => return Wrote::Deferred,
            }
        }
        let activation_id = match self.session.current_activation_id() {
            Ok(activation_id) => activation_id,
            Err(error) => {
                self.report("activation", &error);
                return Wrote::Deferred;
            }
        };
        let (observation, capture) = match terminals.binding_capture(worker) {
            Ok(captured) => captured,
            Err(error) => return self.unwritable("capture", &error),
        };
        self.observed = Some(observation);
        let (record, mixed) =
            BindingRecord::assemble(activation_id, capture, &hosts, self.extensions.clone());
        if mixed {
            // A Host whose Worker recorded no epoch is paired with its terminals only when they share
            // one; the record leaves those terminals out.
            eprintln!(
                "orchbee-server diagnostic component=checkpoint operation=bindings stage=hosts cause=mixed-epochs"
            );
        }
        let bytes = match binding_record::encode(&record) {
            Some(bytes) => bytes,
            None => {
                if !self.reported_bound {
                    self.reported_bound = true;
                    eprintln!(
                        "orchbee-server diagnostic component=checkpoint operation=bindings stage=bound cause=launch-selections-dropped"
                    );
                }
                match binding_record::encode(&record.without_launch_selections()) {
                    Some(bytes) => bytes,
                    None => {
                        return self.unwritable("bound", &io::ErrorKind::FileTooLarge.into());
                    }
                }
            }
        };
        if self.written.as_deref() == Some(bytes.as_slice()) {
            return Wrote::Unchanged;
        }
        let published = self
            .store
            .lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))
            .and_then(|mut store| {
                store.publish_bindings(&bytes, |bytes| binding_record::decode(bytes).is_some())
            });
        match published {
            Ok(()) => {
                self.written = Some(bytes);
                self.reported = None;
                Wrote::Published
            }
            Err(error) => self.unwritable("save", &error),
        }
    }

    /// A record that cannot be written no longer describes the Session, so the earlier one goes;
    /// the next look tries again.
    fn unwritable(&mut self, stage: &'static str, error: &io::Error) -> Wrote {
        self.report(stage, error);
        self.remove();
        Wrote::Deferred
    }

    fn report(&mut self, stage: &'static str, error: &io::Error) {
        let cause = format!("io:{:?}", error.kind());
        if self
            .reported
            .as_ref()
            .is_some_and(|(reported, previous)| *reported == stage && *previous == cause)
        {
            return;
        }
        eprintln!(
            "orchbee-server diagnostic component=checkpoint operation=bindings stage={stage} cause={cause}"
        );
        self.reported = Some((stage, cause));
    }
}
