//! The binding record: which running terminal each Presentation shows and what this Server issued
//! for it, kept in the Session folder so that a Server that did not create those terminals can adopt
//! them from their Host. The checkpoint saver writes it while the Server runs and exactly at every
//! final capture; the local runtime carries its bytes. It belongs to the class of `session.json`:
//! version 1, an `extensions` object written back unchanged, read by two installed versions at
//! once, never restored from a backup. A reader that cannot use a record treats it as absent.

use std::{
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroU64},
};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptBindingState, AdoptCloseInFlight, AdoptHostProcess, HostCreationId,
    HostEpoch, HostTerminalId, HostTerminalIdentity, LifecycleStepOrdinal, OperationId,
    TerminalFence, TerminalId, WorkerId,
};
use orchbee_client_server_protocol::{
    Geometry, LaunchProfileProvenance, LaunchSelection, RequestId,
};
use orchbee_local_runtime::MAXIMUM_BINDING_RECORD_BYTES;
use orchbee_workbench_protocol::{PresentationLaunchProgram, TerminalPresentationId};
use serde::{Deserialize, Serialize};

const VERSION: u8 = 1;
/// The most Hosts one record names; today a Session has one.
const MAXIMUM_HOSTS: usize = 8;
const MAXIMUM_TERMINALS: usize = crate::terminal_coordinator::MAXIMUM_TERMINAL_BINDINGS;
/// How far past the recorded next identity an adopting Server starts allocating: the writer may have
/// issued identities after its last write, and the Host may hold receipts naming them.
const ADOPTION_IDENTITY_MARGIN: u128 = 1 << 64;

/// One binding record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BindingRecord {
    /// The activation of the generation that wrote it: a diagnostic, never proof of validity, which
    /// only a match with the running Host gives.
    pub(crate) activation_id: [u8; 16],
    /// The Session Worker every recorded binding was issued to.
    pub(crate) worker_id: WorkerId,
    /// The identity the writer would have allocated next.
    pub(crate) next_identity: u128,
    pub(crate) hosts: Vec<RecordedHost>,
    pub(crate) terminals: Vec<RecordedTerminal>,
    /// Members a later release added, kept unread and written back unchanged.
    pub(crate) extensions: serde_json::Map<String, serde_json::Value>,
}

/// A Host the recorded terminals run in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecordedHost {
    /// The process the Session record registered; POSIX registers none.
    pub(crate) process: Option<AdoptHostProcess>,
    /// The epoch its terminals' identities carry; none before any terminal reported one.
    pub(crate) epoch: Option<HostEpoch>,
}

/// One recorded terminal: the fields an adoption sends for it, plus what a Server needs to serve it
/// as the Server that created it did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecordedTerminal {
    /// The product Terminal ID, its Host identity, its state, the highest fence the Server issued
    /// for it and the Close in flight, if any.
    pub(crate) binding: AdoptBinding,
    /// The geometry it started at, exactly when it started.
    pub(crate) geometry: Option<Geometry>,
    /// The creation request it answers, which a Client's start names.
    pub(crate) create_request: RequestId,
    pub(crate) presentation_id: TerminalPresentationId,
    pub(crate) launch_program: Option<PresentationLaunchProgram>,
    pub(crate) deferred: bool,
    pub(crate) interactive_shell: bool,
    pub(crate) profile: Option<LaunchProfileProvenance>,
    pub(crate) launch_selection: Option<LaunchSelection>,
}

impl RecordedTerminal {
    /// The settled binding a Server that adopted this terminal's Host enters for it in `worker`'s
    /// generation, as the coordinator that created it kept it, without the Close in flight.
    pub(crate) fn recorded_binding(
        &self,
        worker: crate::terminal_coordinator::WorkerPlacement,
    ) -> crate::terminal_coordinator::RecordedBinding {
        crate::terminal_coordinator::RecordedBinding {
            reference: self.create_request.clone(),
            binding: AdoptBinding {
                close: None,
                ..self.binding
            },
            worker,
            geometry: self.geometry,
            deferred: self.deferred,
            interactive_shell: self.interactive_shell,
            profile_provenance: self.profile.clone(),
            launch_selection: self.launch_selection.clone(),
        }
    }
}

/// A Host the binding writer observed running, as the Session record registers it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunningHost {
    /// Windows registers its process, POSIX none.
    pub(crate) process: Option<AdoptHostProcess>,
    /// The epoch its Worker recorded once connected; none before one did.
    pub(crate) epoch: Option<HostEpoch>,
}

/// The process a Session record registers as its ID and creation time, in protocol terms.
pub(crate) fn registered_process((pid, creation): (u32, u64)) -> Option<AdoptHostProcess> {
    Some(AdoptHostProcess {
        process_id: NonZeroU32::new(pid)?,
        creation_time: NonZeroU64::new(creation)?,
    })
}

/// What the coordinator settled, joined with the Workbench, in one look.
#[derive(Clone, Debug)]
pub(crate) struct Capture {
    pub(crate) worker_id: WorkerId,
    pub(crate) next_identity: Option<u128>,
    pub(crate) terminals: Vec<RecordedTerminal>,
}

impl BindingRecord {
    /// The record a generation writes from `capture` while `hosts` run. A terminal is recorded
    /// only with a running Host that serves its epoch: the epoch the Host recorded, or for the one
    /// Host that recorded none, the epoch every terminal no recorded epoch names shares. When they
    /// share none, or several Hosts recorded none, those terminals are left out and `true` comes
    /// back so the writer can report it.
    pub(crate) fn assemble(
        activation_id: [u8; 16],
        capture: Capture,
        hosts: &[RunningHost],
        extensions: serde_json::Map<String, serde_json::Value>,
    ) -> (Self, bool) {
        let Capture {
            worker_id,
            next_identity,
            mut terminals,
        } = capture;
        let known: Vec<HostEpoch> = hosts.iter().filter_map(|host| host.epoch).collect();
        let mut others = terminals
            .iter()
            .map(|terminal| terminal.binding.identity.host_epoch())
            .filter(|epoch| !known.contains(epoch));
        let other = others.next();
        let shared = others.all(|epoch| Some(epoch) == other);
        let unrecorded = hosts.iter().filter(|host| host.epoch.is_none()).count();
        let (lone, mixed) = match other {
            // Terminals of a Host that no longer runs cannot be adopted.
            Some(_) if unrecorded == 0 => (None, false),
            Some(epoch) if unrecorded == 1 && shared => (Some(epoch), false),
            Some(_) => (None, true),
            None => (None, false),
        };
        let hosts: Vec<RecordedHost> = hosts
            .iter()
            .map(|host| RecordedHost {
                process: host.process,
                epoch: host.epoch.or(lone),
            })
            .collect();
        terminals.retain(|terminal| {
            hosts
                .iter()
                .any(|host| host.epoch == Some(terminal.binding.identity.host_epoch()))
        });
        (
            Self {
                activation_id,
                worker_id,
                next_identity: next_identity.unwrap_or(u128::MAX),
                hosts,
                terminals,
                extensions,
            },
            mixed,
        )
    }

    /// The identity a Server that adopts this record's Host allocates from; none when none is left.
    /// Every recorded terminal's identity lies below the recorded next identity.
    pub(crate) fn identity_floor(&self) -> Option<u128> {
        self.next_identity.checked_add(ADOPTION_IDENTITY_MARGIN)
    }

    /// The same record without the explicit programs the terminals launched, which are the only
    /// large members; a record over the bound is written this way.
    pub(crate) fn without_launch_selections(&self) -> Self {
        let mut record = self.clone();
        for terminal in &mut record.terminals {
            terminal.launch_selection = None;
        }
        record
    }
}

/// The record's bytes, or `None` when they would exceed the bound.
pub(crate) fn encode(record: &BindingRecord) -> Option<Vec<u8>> {
    let bytes = serde_json::to_vec(&Stored::from(record)).ok()?;
    (bytes.len() <= MAXIMUM_BINDING_RECORD_BYTES).then_some(bytes)
}

/// The record `bytes` hold, or `None` when this build cannot use them: another version, an unknown
/// member outside `extensions`, or contents that contradict each other.
pub(crate) fn decode(bytes: &[u8]) -> Option<BindingRecord> {
    if bytes.len() > MAXIMUM_BINDING_RECORD_BYTES {
        return None;
    }
    serde_json::from_slice::<Stored>(bytes).ok()?.admit()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u8,
    /// Fields a later release added, kept unread; two installed versions read this record at once.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    extensions: serde_json::Map<String, serde_json::Value>,
    activation_id: String,
    worker_id: String,
    next_identity: String,
    hosts: Vec<StoredHost>,
    terminals: Vec<StoredTerminal>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredHost {
    #[serde(deserialize_with = "Option::deserialize")]
    process: Option<StoredProcess>,
    #[serde(deserialize_with = "Option::deserialize")]
    epoch: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProcess {
    pid: u32,
    creation_time: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTerminal {
    terminal_id: String,
    create_request: String,
    presentation_id: String,
    #[serde(deserialize_with = "Option::deserialize")]
    launch_program: Option<String>,
    host: StoredHostTerminal,
    phase: StoredPhase,
    #[serde(deserialize_with = "Option::deserialize")]
    geometry: Option<StoredGeometry>,
    fence: u64,
    deferred: bool,
    interactive_shell: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    profile: Option<StoredProfile>,
    #[serde(deserialize_with = "Option::deserialize")]
    launch_selection: Option<StoredSelection>,
    #[serde(deserialize_with = "Option::deserialize")]
    close: Option<StoredClose>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredHostTerminal {
    creation_id: String,
    epoch: String,
    terminal_id: String,
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StoredPhase {
    AwaitingStart,
    Started,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredGeometry {
    columns: u16,
    rows: u16,
    cell_width: u16,
    cell_height: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProfile {
    id: String,
    catalog_revision: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSelection {
    executable: String,
    arguments: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredClose {
    operation_id: String,
    attempt_generation: u64,
    step: u8,
    effect_unknown: bool,
}

impl From<&BindingRecord> for Stored {
    fn from(record: &BindingRecord) -> Self {
        Self {
            version: VERSION,
            extensions: record.extensions.clone(),
            activation_id: hex(record.activation_id),
            worker_id: hex(record.worker_id.as_bytes()),
            next_identity: hex(record.next_identity.to_be_bytes()),
            hosts: record
                .hosts
                .iter()
                .map(|host| StoredHost {
                    process: host.process.map(|process| StoredProcess {
                        pid: process.process_id.get(),
                        creation_time: process.creation_time.get(),
                    }),
                    epoch: host.epoch.map(|epoch| hex(epoch.as_bytes())),
                })
                .collect(),
            terminals: record.terminals.iter().map(StoredTerminal::from).collect(),
        }
    }
}

impl From<&RecordedTerminal> for StoredTerminal {
    fn from(terminal: &RecordedTerminal) -> Self {
        let identity = terminal.binding.identity;
        Self {
            terminal_id: hex(terminal.binding.terminal_id.as_bytes()),
            create_request: terminal.create_request.as_str().to_owned(),
            presentation_id: terminal.presentation_id.as_str().to_owned(),
            launch_program: terminal
                .launch_program
                .as_ref()
                .map(|program| program.as_str().to_owned()),
            host: StoredHostTerminal {
                creation_id: hex(identity.creation_id().as_bytes()),
                epoch: hex(identity.host_epoch().as_bytes()),
                terminal_id: hex(identity.terminal_id().as_bytes()),
            },
            phase: match terminal.binding.state {
                AdoptBindingState::AwaitingStart => StoredPhase::AwaitingStart,
                AdoptBindingState::Started => StoredPhase::Started,
            },
            geometry: terminal.geometry.map(|geometry| StoredGeometry {
                columns: geometry.columns(),
                rows: geometry.rows(),
                cell_width: geometry.cell_pixel_width(),
                cell_height: geometry.cell_pixel_height(),
            }),
            fence: terminal.binding.fence.get(),
            deferred: terminal.deferred,
            interactive_shell: terminal.interactive_shell,
            profile: terminal.profile.as_ref().map(|profile| StoredProfile {
                id: profile.profile_id().to_owned(),
                catalog_revision: profile.catalog_revision(),
            }),
            launch_selection: terminal
                .launch_selection
                .as_ref()
                .map(|selection| StoredSelection {
                    executable: selection.executable().to_owned(),
                    arguments: selection.arguments().to_vec(),
                }),
            close: terminal.binding.close.map(|close| StoredClose {
                operation_id: hex(close.close_operation_id.as_bytes()),
                attempt_generation: close.attempt_generation.get(),
                step: close.step_ordinal.get(),
                effect_unknown: close.step_effect_unknown,
            }),
        }
    }
}

impl Stored {
    fn admit(self) -> Option<BindingRecord> {
        if self.version != VERSION
            || self.hosts.len() > MAXIMUM_HOSTS
            || self.terminals.len() > MAXIMUM_TERMINALS
        {
            return None;
        }
        let activation_id = identity(&self.activation_id)?;
        let worker_id = WorkerId::from_bytes(identity(&self.worker_id)?).ok()?;
        let next_identity = u128::from_be_bytes(identity(&self.next_identity)?);
        let hosts = self
            .hosts
            .into_iter()
            .map(StoredHost::admit)
            .collect::<Option<Vec<_>>>()?;
        let epochs: Vec<HostEpoch> = hosts.iter().filter_map(|host| host.epoch).collect();
        if !distinct(epochs.iter()) {
            return None;
        }
        let terminals = self
            .terminals
            .into_iter()
            .map(StoredTerminal::admit)
            .collect::<Option<Vec<_>>>()?;
        let consistent = terminals.iter().all(|terminal| {
            epochs.contains(&terminal.binding.identity.host_epoch())
                && u128::from_be_bytes(terminal.binding.terminal_id.as_bytes()) < next_identity
        }) && distinct(
            terminals
                .iter()
                .map(|terminal| terminal.binding.terminal_id),
        ) && distinct(terminals.iter().map(|terminal| &terminal.create_request))
            && distinct(terminals.iter().map(|terminal| &terminal.presentation_id))
            && distinct(terminals.iter().map(|terminal| {
                let identity = terminal.binding.identity;
                (identity.host_epoch(), identity.terminal_id())
            }));
        consistent.then_some(BindingRecord {
            activation_id,
            worker_id,
            next_identity,
            hosts,
            terminals,
            extensions: self.extensions,
        })
    }
}

impl StoredHost {
    fn admit(self) -> Option<RecordedHost> {
        let process = match self.process {
            Some(process) => Some(AdoptHostProcess {
                process_id: NonZeroU32::new(process.pid)?,
                creation_time: NonZeroU64::new(process.creation_time)?,
            }),
            None => None,
        };
        let epoch = match self.epoch {
            Some(epoch) => Some(HostEpoch::from_bytes(identity(&epoch)?).ok()?),
            None => None,
        };
        Some(RecordedHost { process, epoch })
    }
}

impl StoredTerminal {
    fn admit(self) -> Option<RecordedTerminal> {
        let geometry = match self.geometry {
            Some(geometry) => Some(
                Geometry::new(
                    geometry.columns,
                    geometry.rows,
                    geometry.cell_width,
                    geometry.cell_height,
                )
                .ok()?,
            ),
            None => None,
        };
        let state = match (self.phase, geometry.is_some()) {
            (StoredPhase::AwaitingStart, false) => AdoptBindingState::AwaitingStart,
            (StoredPhase::Started, true) => AdoptBindingState::Started,
            _ => return None,
        };
        let close = match self.close {
            Some(close) => Some(AdoptCloseInFlight {
                close_operation_id: OperationId::from_bytes(identity(&close.operation_id)?).ok()?,
                attempt_generation: NonZeroU64::new(close.attempt_generation)?,
                step_ordinal: LifecycleStepOrdinal::new(close.step).ok()?,
                step_effect_unknown: close.effect_unknown,
            }),
            None => None,
        };
        let launch_program = match self.launch_program {
            Some(program) => Some(PresentationLaunchProgram::parse(&program).ok()?),
            None => None,
        };
        let profile = match self.profile {
            Some(profile) => {
                Some(LaunchProfileProvenance::new(profile.id, profile.catalog_revision).ok()?)
            }
            None => None,
        };
        let launch_selection = match self.launch_selection {
            Some(selection) => {
                Some(LaunchSelection::new(selection.executable, selection.arguments).ok()?)
            }
            None => None,
        };
        Some(RecordedTerminal {
            binding: AdoptBinding {
                terminal_id: TerminalId::from_bytes(identity(&self.terminal_id)?).ok()?,
                identity: HostTerminalIdentity::new(
                    HostCreationId::from_bytes(identity(&self.host.creation_id)?).ok()?,
                    HostEpoch::from_bytes(identity(&self.host.epoch)?).ok()?,
                    HostTerminalId::from_bytes(identity(&self.host.terminal_id)?).ok()?,
                ),
                state,
                fence: TerminalFence::new(NonZeroU64::new(self.fence)?),
                close,
            },
            geometry,
            create_request: RequestId::parse(&self.create_request).ok()?,
            presentation_id: TerminalPresentationId::parse(&self.presentation_id).ok()?,
            launch_program,
            deferred: self.deferred,
            interactive_shell: self.interactive_shell,
            profile,
            launch_selection,
        })
    }
}

fn hex(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}

/// A nonzero 16-byte identity written as exactly 32 lowercase hexadecimal digits.
fn identity(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let value = u128::from_str_radix(text, 16).ok()?;
    (value != 0).then_some(value.to_be_bytes())
}

fn distinct<T: Ord>(mut values: impl Iterator<Item = T>) -> bool {
    let mut seen = BTreeSet::new();
    values.all(|value| seen.insert(value))
}

#[cfg(test)]
mod tests;
