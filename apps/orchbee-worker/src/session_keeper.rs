//! The Session's Keeper as the Worker owns it, on Windows and macOS: started once the Worker's first
//! Host is ready, or adopted from the Session record by a replacement Worker; each Host it starts
//! registered with it and told to attach it; replaced when it ended or stopped answering; and ended
//! at a stop once the Host is gone. On Windows it is also updated when it runs another
//! installation's program, and a Keeper an update replaced or rolled back is retired
//! (`retiring`). A failed Keeper step leaves the terminals running unheld and the Worker serving,
//! so nothing here ends a program or the Worker (`proposals/platform/host-handover.md`, The
//! Keeper's life from H2, and the H4 design).
//!
//! A Host exchange runs either inside the Server request it serves, which watches that request's
//! Server stream, or between requests, where the Server's next request waits for it.

use std::{
    error::Error,
    io,
    num::NonZeroU32,
    path::Path,
    time::{Duration, Instant},
};

use orchbee_connection_handshake::{Build, ConnectionKind, EnvironmentName, Offer, ProductVersion};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{
    DataRoot, ExactProcess, KeeperClient, KeeperProcess, KeeperRefusal, KeeperSlot, KeeperStatus,
    ModeCheckedLocalSessionRendezvous, RetainedChild,
};
use orchbee_terminal_protocol::{
    AttachKeeper, HandoverControl, HandoverProcess, HandoverRefusal, MessageRole,
};

use crate::host_connection::HostBrokerConnection;

#[cfg(windows)]
mod retiring;

#[cfg(windows)]
pub(crate) use retiring::RetiringKeeper;

/// How long a Keeper start or adoption may take, from its creation until it answered the Worker.
const CONNECT_BUDGET: Duration = Duration::from_secs(5);
/// How long ending a Keeper waits for it to exit.
const END_BOUND: Duration = Duration::from_secs(2);
/// How long a write of the Session record's Keeper slots may take.
const RECORD_BUDGET: Duration = Duration::from_secs(2);
/// How long a handover request between Server requests may take: a Host fails an `AttachKeeper`
/// that takes it more than two seconds, and the Server's next request waits meanwhile.
const BETWEEN_REQUESTS_BUDGET: Duration = Duration::from_secs(3);

/// Where a Host exchange runs: inside the Server request whose stream is named, which sends
/// nothing until it is answered, with the connection's whole exchange bound or within a budget;
/// or between requests.
#[derive(Clone, Copy)]
pub(crate) enum Serving<'a> {
    Request(&'a LocalStream),
    #[cfg(windows)]
    Within(&'a LocalStream, Duration),
    BetweenRequests,
}

/// The Keeper process this Worker answers for: the child it started, or the recorded Keeper it
/// adopted, held open on its exact identity.
enum Owner {
    Started(RetainedChild),
    Adopted(ExactProcess),
}

/// The Session's Keeper and this Worker's one connection to it.
pub(crate) struct SessionKeeper {
    owner: Owner,
    process: KeeperProcess,
    slot: KeeperSlot,
    endpoint_id: [u8; 16],
    client: KeeperClient,
    /// A live Keeper an adoption found in the other slot, which this Worker retires.
    #[cfg(windows)]
    other: Option<RetiringKeeper>,
}

/// A `TypedFailure` a Host sent for a handover request.
#[derive(Debug)]
pub(crate) struct HostRefusal(pub(crate) HandoverRefusal);

impl std::fmt::Display for HostRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0.reason {
            Some(reason) => write!(formatter, "the Host refused: {} ({reason})", self.0.code),
            None => write!(formatter, "the Host refused: {}", self.0.code),
        }
    }
}

impl Error for HostRefusal {}

#[cfg(windows)]
impl HostRefusal {
    /// Whether `error` is a Host's refusal of a step because the terminal is still moving: a
    /// quiesced terminal whose Keeper answers takes a drop only once its Host saw `claimed`.
    pub(crate) fn moving(error: &(dyn Error + 'static)) -> bool {
        error.downcast_ref::<Self>().is_some_and(|refusal| {
            refusal.0.code == "move-passing" && refusal.0.reason.as_deref() == Some("moving")
        })
    }
}

impl SessionKeeper {
    /// Starts the Session's Keeper in `slot` from `executable`, the Terminal Host's program, as
    /// this Worker's registered child, and connects to it.
    ///
    /// # Errors
    ///
    /// Returns an error when the start or the connection fails; a Keeper started but not
    /// connected is ended first.
    pub(crate) fn start(
        runtime: &Path,
        slot: KeeperSlot,
        executable: &Path,
    ) -> Result<Self, Box<dyn Error>> {
        let deadline = Instant::now() + CONNECT_BUDGET;
        let root = data_root()?;
        let (child, (pid, created)) =
            orchbee_local_runtime::start_keeper(&root, runtime, slot, executable, deadline)?;
        let mut owner = Owner::Started(child);
        match connect(runtime, slot, deadline) {
            Ok(client) => Ok(Self {
                owner,
                process: KeeperProcess { pid, created },
                slot,
                endpoint_id: endpoint_id(runtime, slot)?,
                client,
                #[cfg(windows)]
                other: None,
            }),
            Err(error) => {
                // A Keeper no Host attached holds nothing, so ending it ends no program.
                owner.end(Instant::now() + END_BOUND);
                Err(error)
            }
        }
    }

    /// Adopts a live Keeper the Session record names, which must be `expected` when the caller
    /// names one, and connects to it; `None` when the record names no live Keeper. Without
    /// `expected` it adopts the current Keeper, or the one live Keeper left in the other slot,
    /// which it then makes current. On Windows a live Keeper in the slot not adopted is kept to
    /// retire; only a Windows update starts a second Keeper.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be read or written, names no live Keeper that is
    /// `expected`, or the Keeper does not answer.
    pub(crate) fn adopt(
        runtime: &Path,
        expected: Option<(u32, u64)>,
    ) -> Result<Option<Self>, Box<dyn Error>> {
        let root = data_root()?;
        let recorded = orchbee_local_runtime::recorded_keepers(&root, runtime)?;
        let mut live = recorded.live;
        if live.is_empty() {
            return Ok(None);
        }
        let index = match expected {
            Some(expected) => live
                .iter()
                .position(|(_, process)| process.identity() == expected)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "the Session record names another Keeper",
                    )
                })?,
            None => live
                .iter()
                .position(|(slot, _)| *slot == recorded.current)
                .unwrap_or(0),
        };
        let (slot, process) = live.swap_remove(index);
        let (pid, created) = process.identity();
        let process = process.endable()?;
        let client = connect(runtime, slot, Instant::now() + CONNECT_BUDGET)?;
        if slot != recorded.current {
            orchbee_local_runtime::make_session_keeper_current(
                &root,
                runtime,
                slot,
                (pid, created),
                Instant::now() + RECORD_BUDGET,
            )?;
        }
        #[cfg(windows)]
        let other = live
            .pop()
            .map(|(slot, process)| RetiringKeeper::adopted(slot, process))
            .transpose()?;
        Ok(Some(Self {
            owner: Owner::Adopted(process),
            process: KeeperProcess { pid, created },
            slot,
            endpoint_id: endpoint_id(runtime, slot)?,
            client,
            #[cfg(windows)]
            other,
        }))
    }

    /// The slot of the Session record's current Keeper, where a first Keeper starts.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be read.
    pub(crate) fn current_slot(runtime: &Path) -> Result<KeeperSlot, Box<dyn Error>> {
        Ok(orchbee_local_runtime::recorded_keepers(&data_root()?, runtime)?.current)
    }

    /// The Keeper's slot in the Session record.
    pub(crate) const fn slot(&self) -> KeeperSlot {
        self.slot
    }

    /// Starts the Session's Keeper as [`SessionKeeper::start`] does and has the Host `process`,
    /// which `host` serves, attach it between Server requests.
    ///
    /// # Errors
    ///
    /// Returns an error when the start or the attachment fails; a Keeper no Host attached is ended
    /// first.
    pub(crate) fn start_attached(
        runtime: &Path,
        slot: KeeperSlot,
        executable: &Path,
        host: &HostBrokerConnection,
        process: (u32, u64),
    ) -> Result<Self, Box<dyn Error>> {
        let mut keeper = Self::start(runtime, slot, executable)?;
        if let Err(error) = keeper.attach(host, Serving::BetweenRequests, process) {
            // No terminal is held yet, so ending the Keeper ends no program.
            keeper.end(Instant::now() + END_BOUND);
            return Err(error);
        }
        Ok(keeper)
    }

    /// Registers the Host `process`, which `host` serves, and has it attach this Keeper, so it
    /// holds its running terminals here.
    ///
    /// # Errors
    ///
    /// Returns an error when the Keeper refuses the registration or the Host the attachment.
    pub(crate) fn attach(
        &mut self,
        host: &HostBrokerConnection,
        serving: Serving<'_>,
        process: (u32, u64),
    ) -> Result<(), Box<dyn Error>> {
        let (pid, created) = process;
        self.client
            .register_host(KeeperProcess { pid, created })?
            .map_err(refusal)?;
        let keeper = HandoverProcess {
            pid: NonZeroU32::new(self.process.pid).ok_or_else(unexpected)?,
            creation_time: self.process.created,
        };
        let attach = HandoverControl::AttachKeeper(AttachKeeper {
            keeper,
            endpoint_id: self.endpoint_id,
        });
        match request(host, serving, &attach)? {
            HandoverControl::KeeperAttached(attached) if attached == keeper => Ok(()),
            _ => Err(unexpected().into()),
        }
    }

    /// Every holding and registered Host the Keeper reports.
    ///
    /// # Errors
    ///
    /// Returns an error when the Keeper does not answer within its reply bound.
    pub(crate) fn status(&mut self) -> io::Result<KeeperStatus> {
        self.client.status()
    }

    /// Assigns the orphaned `holding` to the registered Host `target` under `mark`.
    #[cfg(windows)]
    ///
    /// # Errors
    ///
    /// Returns an error when the Keeper refuses or does not answer.
    pub(crate) fn assign(
        &mut self,
        holding: [u8; 16],
        target: KeeperProcess,
        mark: [u8; 16],
    ) -> Result<(), Box<dyn Error>> {
        self.client
            .assign(holding, target, mark)?
            .map_err(refusal)
            .map_err(Into::into)
    }

    /// Discards the orphaned `holding`, which ends its program.
    ///
    /// # Errors
    ///
    /// Returns an error when the Keeper refuses or does not answer.
    #[cfg(windows)]
    pub(crate) fn discard(&mut self, holding: [u8; 16]) -> Result<(), Box<dyn Error>> {
        self.client
            .discard(holding)?
            .map_err(refusal)
            .map_err(Into::into)
    }

    /// Ends the Keeper and waits until it exited or `deadline` passes. The caller ends it only once
    /// no Host holds a terminal in it, so ending it ends no program.
    pub(crate) fn end(&mut self, deadline: Instant) {
        self.owner.end(deadline);
    }
}

impl Owner {
    fn end(&mut self, deadline: Instant) {
        let ended = match self {
            Self::Started(child) => child.terminate_before(deadline).map(drop),
            Self::Adopted(process) => process.end_before(deadline),
        };
        if let Err(error) = ended {
            crate::diagnostic_events::keeper_failed("end", &error);
        }
    }
}

/// Sends `host` one handover request and reads its successful answer.
///
/// # Errors
///
/// Returns an error when the exchange fails or the answer is out of contract; a `TypedFailure`
/// is the error's [`HostRefusal`].
pub(crate) fn request(
    host: &HostBrokerConnection,
    serving: Serving<'_>,
    value: &HandoverControl,
) -> Result<HandoverControl, Box<dyn Error>> {
    use orchbee_terminal_protocol::{
        CorrelationId, decode_handover_control, decode_handover_refusal, decode_transport_envelope,
        encode_handover_control,
    };

    let encoded = encode_handover_control(value)?;
    let correlation = CorrelationId::from_bytes(crate::worker_identity::fresh_identity()?)?;
    let record = match serving {
        Serving::Request(server) => crate::host_connection::exchange(
            host,
            server,
            correlation,
            encoded.kind,
            &encoded.payload,
            encoded.payload_encoding,
        )?,
        #[cfg(windows)]
        Serving::Within(server, budget) => crate::host_connection::exchange_within(
            host,
            server,
            budget,
            correlation,
            encoded.kind,
            &encoded.payload,
            encoded.payload_encoding,
        )?,
        Serving::BetweenRequests => crate::host_connection::exchange_between_requests(
            host,
            correlation,
            encoded.kind,
            &encoded.payload,
            encoded.payload_encoding,
            BETWEEN_REQUESTS_BUDGET,
        )?,
    };
    let envelope = decode_transport_envelope(&record)?;
    if envelope.kind == encoded.answer_kind {
        return Ok(decode_handover_control(
            envelope.kind,
            MessageRole::Response,
            envelope.payload,
        )?);
    }
    let refusal = decode_handover_refusal(envelope.kind, envelope.role, envelope.payload)
        .map_err(|_error| unexpected())?;
    Err(Box::new(HostRefusal(refusal)))
}

fn unexpected() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "the Host answered a handover request with another message",
    )
}

fn connect(
    runtime: &Path,
    slot: KeeperSlot,
    deadline: Instant,
) -> Result<KeeperClient, Box<dyn Error>> {
    let endpoint = ModeCheckedLocalSessionRendezvous::keeper_endpoint_path(runtime, slot);
    let offer = Offer::local(
        ConnectionKind::Keeper,
        ProductVersion::new(env!("CARGO_PKG_VERSION"))?,
        Build {
            commit: crate::host_connection::source_commit(),
            cohort: None,
        },
        EnvironmentName::new(orchbee_local_runtime::environment_stem())?,
    );
    let (client, recovery) = KeeperClient::connect(&endpoint, &offer, deadline)?;
    if !recovery {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the Keeper does not offer recovery",
        )
        .into());
    }
    Ok(client)
}

/// The Keeper's endpoint id, the 32 hexadecimal digits of its pipe name, as its 16 bytes.
fn endpoint_id(runtime: &Path, slot: KeeperSlot) -> io::Result<[u8; 16]> {
    let text = ModeCheckedLocalSessionRendezvous::keeper_endpoint_id(runtime, slot);
    let digits = text
        .chars()
        .map(|digit| {
            digit
                .to_digit(16)
                .and_then(|value| u8::try_from(value).ok())
        })
        .collect::<Option<Vec<u8>>>()
        .filter(|digits| digits.len() == 32)
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
    let mut bytes = [0_u8; 16];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
        if let [high, low] = pair {
            *byte = (*high << 4) | *low;
        }
    }
    Ok(bytes)
}

fn data_root() -> io::Result<DataRoot> {
    DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))
}

fn refusal(refusal: KeeperRefusal) -> io::Error {
    match refusal {
        KeeperRefusal::Refused(reason) => io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("the Keeper refused: {reason}"),
        ),
        KeeperRefusal::Unsupported => io::Error::new(
            io::ErrorKind::Unsupported,
            "the Keeper does not know the request",
        ),
    }
}
