//! Starts a Windows Session generation that kept the Terminal Host its predecessor left running:
//! its Session record carries that Host's registration and the binding record names it, so the
//! Worker this Server starts adopts it in the replacement adopt mode before any job is served, and
//! each Presentation the record names shows its running terminal again. A start that cannot adopt
//! the Host fails and leaves the Host and its terminals running for the next activation. It records
//! `host-adopted`.

use std::{
    io,
    num::{NonZeroU32, NonZeroU64},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptHostProcess, AdoptHostTarget, TerminalId,
};
use orchbee_local_runtime::{HostPresence, LogField, LogLevel, LogValue, RetainedSessionRecord};

use super::{
    checkpoint::Kept,
    replacement::{self, Adoption},
};
use crate::{
    binding_record::BindingRecord,
    local_terminal_service::{CheckpointRestore, LocalTerminalService},
    worker_obcn_carrier::WorkerRequestHandle,
    worker_registry::WorkerRegistry,
};

const HOST_ADOPTED: &str = "host-adopted";

/// The running Host a starting generation adopts, as its Session record registered it, and the
/// binding record that names it.
pub(super) struct Adopting {
    record: BindingRecord,
    target: AdoptHostTarget,
    started: Instant,
}

impl Adopting {
    /// The Host this generation adopts: none when its Session record registered no Host, as every
    /// generation that kept none starts. A kept Host that no longer runs or cannot be observed, or
    /// one the binding record does not name, fails the start.
    pub(super) fn find(
        session: &RetainedSessionRecord,
        kept: Option<&Kept>,
    ) -> io::Result<Option<Self>> {
        let started = Instant::now();
        let process = match session.host_presence()? {
            HostPresence::NotStarted => return Ok(None),
            HostPresence::Running(Some((pid, creation))) => NonZeroU32::new(pid)
                .zip(NonZeroU64::new(creation))
                .map(|(process_id, creation_time)| AdoptHostProcess {
                    process_id,
                    creation_time,
                }),
            HostPresence::Running(None)
            | HostPresence::Spawning
            | HostPresence::Gone
            | HostPresence::Unknown => None,
        };
        let Some(process) = process else {
            failed_record("host", started.elapsed());
            return Err(io::Error::other("the kept Terminal Host is unavailable"));
        };
        let record = match kept.map(Kept::adopted_record) {
            Some(Ok(record)) => record,
            Some(Err(error)) => {
                failed_record("record", started.elapsed());
                return Err(error);
            }
            None => {
                failed_record("record", started.elapsed());
                return Err(io::Error::other("this Session keeps no binding record"));
            }
        };
        let epoch = match record.hosts.as_slice() {
            [host] if host.process == Some(process) => host.epoch,
            _ => {
                failed_record("record", started.elapsed());
                return Err(io::Error::other(
                    "the binding record names another Terminal Host",
                ));
            }
        };
        Ok(Some(Self {
            record,
            target: AdoptHostTarget::Host {
                process: Some(process),
                epoch,
            },
            started,
        }))
    }

    /// The Session Worker id the adopting Worker keeps, as its admission expects it.
    pub(super) fn worker_id(&self) -> orchbee_local_worker_protocol::WorkerId {
        orchbee_local_worker_protocol::WorkerId::from_bytes(self.record.worker_id.as_bytes())
    }

    /// The same id as the Worker's `--worker-id` argument.
    pub(super) fn worker_argument(&self) -> String {
        format!(
            "{:032x}",
            u128::from_be_bytes(self.record.worker_id.as_bytes())
        )
    }

    /// The Session's terminal service once the admitted Worker adopted the Host: every Presentation
    /// the record names shows the terminal the Host kept, every other one restores as at any start,
    /// and only then does the Worker serve the Session's jobs.
    pub(super) fn restore(
        &self,
        registry: &Arc<Mutex<WorkerRegistry>>,
        checkpoint: Option<orchbee_server::WorkbenchCheckpoint>,
        configuration: &orchbee_local_runtime::UserSettingsLoad,
    ) -> io::Result<(WorkerRequestHandle, LocalTerminalService, CheckpointRestore)> {
        let handle = registry
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .adopting_handle()
            .ok_or_else(|| io::Error::other("Worker carrier disappeared"))?;
        let (terminals, restore) = LocalTerminalService::adopting_checkpoint(
            Arc::clone(registry),
            checkpoint,
            configuration,
            &self.record,
            |bindings| self.adopt(&handle, bindings),
        )?;
        let serving = registry
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .serve_adopted(handle.generation());
        if !serving {
            return Err(io::Error::other("Worker carrier disappeared"));
        }
        Ok((handle, terminals, restore))
    }

    /// Sends the adopting Worker `handle` the `AdoptHost` pages for `bindings` and answers the
    /// offered terminals the Host's inventory lacked; any failure fails the start. One attempt: a
    /// failed start leaves the Host to the next activation.
    fn adopt(
        &self,
        handle: &WorkerRequestHandle,
        bindings: Vec<AdoptBinding>,
    ) -> io::Result<Vec<TerminalId>> {
        match replacement::send_pages(handle, self.target, bindings, || false) {
            Adoption::Adopted { outcome, .. } => {
                let fields: [LogField; 5] = [
                    ("outcome", LogValue::Token("adopted")),
                    ("imported", LogValue::Count(u64::from(outcome.imported))),
                    (
                        "quarantined",
                        LogValue::Count(u64::from(outcome.quarantined)),
                    ),
                    ("missing", LogValue::Count(outcome.missing.len() as u64)),
                    ("elapsedMs", LogValue::Count(milliseconds(self.started))),
                ];
                orchbee_local_runtime::log_event(LogLevel::Info, HOST_ADOPTED, &fields);
                Ok(outcome.missing)
            }
            Adoption::Failed(failure, _) => {
                failed_record(failure, self.started.elapsed());
                Err(io::Error::other("the Terminal Host was not adopted"))
            }
            Adoption::Ended => {
                failed_record("unavailable", self.started.elapsed());
                Err(io::Error::other("the Terminal Host was not adopted"))
            }
        }
    }
}

fn failed_record(failure: &'static str, elapsed: Duration) {
    let fields: [LogField; 3] = [
        ("outcome", LogValue::Token("failed")),
        ("failure", LogValue::Token(failure)),
        (
            "elapsedMs",
            LogValue::Count(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
        ),
    ];
    orchbee_local_runtime::log_event(LogLevel::Error, HOST_ADOPTED, &fields);
}

fn milliseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
