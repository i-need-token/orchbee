//! Verifies the Host a replacement Worker adopts, dials it and reads its complete inventory. On
//! Windows the recorded process must still have its creation time and serve the endpoint; on macOS
//! and Linux the Host lock must be held and the endpoint must be this user's private socket, and a
//! recorded process is never trusted. It imports nothing and answers no Server request; the
//! Worker's adoption phase does both.

use std::{
    cell::RefCell,
    error::Error,
    io,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AdoptHostFailure, AdoptHostProcess, HostCreationId, HostEpoch, HostTerminalId,
    HostTerminalIdentity,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use orchbee_terminal_protocol::{
    ConnectionControl, CorrelationId, InventoryControl, ListTerminals,
    MAXIMUM_INVENTORY_PAGE_TERMINALS, PayloadEncoding, TerminalInventoryCursor, TerminalList,
    TerminalSummary, decode_connection_control, decode_inventory_control,
    decode_transport_envelope, encode_inventory_control,
};

use crate::{
    agent_drain::AgentDrain,
    diagnostic_events::Site,
    host_connection::{DialBounds, DialFailure, HostBrokerConnection, HostExchange},
    terminal_authorization::InventoryTerminal,
};

/// How long a Host that still serves the lost Worker's session is dialed again.
const BUSY_RETRY: Duration = Duration::from_secs(5);
/// How many times the inventory is read again from its start after it changed under the read.
const MAXIMUM_INVENTORY_RESTARTS: u32 = 8;
/// The most Host terminals an adoption reads.
const MAXIMUM_INVENTORY_TERMINALS: usize = 4_096;
const PAGE_TERMINALS: u16 = 128;
const _: () = assert!(PAGE_TERMINALS as usize == MAXIMUM_INVENTORY_PAGE_TERMINALS);
/// The Host's retryable refusal of a cursor from an inventory that has since changed.
const STALE_INVENTORY_CURSOR: &str = "stale-inventory-cursor";

/// The adopted Host's process, whose exit completes a managed stop: its handle on Windows, and its
/// Session runtime, whose Host lock it holds, on macOS and Linux.
pub(crate) struct AdoptedHost {
    #[cfg(windows)]
    process: winsafe::guard::CloseHandleGuard<winsafe::HPROCESS>,
    #[cfg(unix)]
    runtime: std::path::PathBuf,
}

impl AdoptedHost {
    pub(crate) fn exited(&self) -> io::Result<bool> {
        #[cfg(windows)]
        {
            use winsafe::co;
            match self.process.WaitForSingleObject(Some(0)) {
                Ok(co::WAIT::OBJECT_0) => Ok(true),
                Ok(co::WAIT::TIMEOUT) => Ok(false),
                Ok(_) => Err(io::Error::other("the Host process wait failed")),
                Err(error) => Err(io::Error::other(error)),
            }
        }
        #[cfg(unix)]
        {
            orchbee_local_runtime::host_lock_held(&self.runtime).map(|held| !held)
        }
    }

    /// The adopted Host's process ID and creation time.
    #[cfg(windows)]
    pub(crate) fn identity(&self) -> io::Result<(u32, u64)> {
        let pid = self.process.GetProcessId().map_err(io::Error::other)?;
        let (creation, _, _, _) = self.process.GetProcessTimes().map_err(io::Error::other)?;
        Ok((pid, u64::from(creation)))
    }

    /// Whether the adopted Host runs `executable`, the Terminal Host of this Worker's cohort. A Host
    /// whose image cannot be read counts as running it, so it is never drained by mistake.
    #[cfg(windows)]
    pub(crate) fn runs(&self, executable: &std::path::Path) -> bool {
        let Ok(image) = self
            .process
            .QueryFullProcessImageName(winsafe::co::PROCESS_NAME::WIN32)
        else {
            return true;
        };
        match (
            std::fs::canonicalize(image),
            std::fs::canonicalize(executable),
        ) {
            (Ok(image), Ok(executable)) => image
                .as_os_str()
                .eq_ignore_ascii_case(executable.as_os_str()),
            _ => true,
        }
    }

    /// Ends the adopted Host and waits until it exited or `deadline` passes; one that already
    /// exited counts as ended.
    #[cfg(windows)]
    pub(crate) fn end_before(&self, deadline: Instant) -> io::Result<()> {
        use winsafe::co;
        if self.exited()? {
            return Ok(());
        }
        self.process.TerminateProcess(1).or_else(|error| {
            if self.exited()? {
                Ok(())
            } else {
                Err(io::Error::other(error))
            }
        })?;
        let wait = deadline.saturating_duration_since(Instant::now());
        let milliseconds = u32::try_from(wait.as_millis()).unwrap_or(u32::MAX);
        match self.process.WaitForSingleObject(Some(milliseconds)) {
            Ok(co::WAIT::OBJECT_0) => Ok(()),
            Ok(_) => Err(io::Error::from(io::ErrorKind::TimedOut)),
            Err(error) => Err(io::Error::other(error)),
        }
    }
}

/// A verified Host with its connection, its owner and its complete inventory.
pub(crate) struct Found {
    pub(crate) host: HostBrokerConnection,
    pub(crate) owner: AdoptedHost,
    pub(crate) inventory: Vec<InventoryTerminal>,
    pub(crate) restarts: u32,
}

/// Why no Host was adopted, with the error behind it and the inventory restarts made.
#[derive(Debug)]
pub(crate) struct Failed {
    pub(crate) failure: AdoptHostFailure,
    pub(crate) restarts: u32,
    pub(crate) error: Box<dyn Error>,
}

/// What an adoption reads its Host through.
pub(crate) struct Channel<'a> {
    pub(crate) server: &'a LocalStream,
    pub(crate) correlation: CorrelationId,
    pub(crate) drain: Rc<RefCell<AgentDrain>>,
    pub(crate) deadline: Instant,
}

fn failed(failure: AdoptHostFailure, error: impl Into<Box<dyn Error>>) -> Failed {
    Failed {
        failure,
        restarts: 0,
        error: error.into(),
    }
}

/// Verifies, dials and reads the Host in `slot` of the Session `runtime` within `channel`'s
/// deadline. A recorded `epoch` must be the Host's.
pub(crate) fn adopt(
    runtime: &Path,
    slot: orchbee_local_runtime::HostSlot,
    process: Option<AdoptHostProcess>,
    epoch: Option<HostEpoch>,
    channel: Channel<'_>,
) -> Result<Found, Failed> {
    #[cfg(windows)]
    let endpoint = ModeCheckedLocalSessionRendezvous::slot_endpoint_path(runtime, slot);
    // A POSIX Session runs its one Host in the first slot, which the caller checked.
    #[cfg(unix)]
    let endpoint = {
        let _ = slot;
        ModeCheckedLocalSessionRendezvous::host_endpoint_path(runtime)
    };
    let found = adopt_endpoint(runtime, &endpoint, process, epoch, channel)?;
    #[cfg(windows)]
    if let Some(process) = process {
        record_epoch(
            runtime,
            slot,
            (process.process_id.get(), process.creation_time.get()),
            *found.host.host_epoch().as_bytes(),
        );
    }
    Ok(found)
}

/// Writes `epoch`, the one the Host in `slot` answered with, into the Session record while the slot
/// still names `process`, so the Server can pair that Host with the terminals that carry it. A
/// failed write is reported and changes nothing else: a Server that finds no epoch recorded takes a
/// lone Host's from its terminals.
#[cfg(windows)]
pub(crate) fn record_epoch(
    runtime: &Path,
    slot: orchbee_local_runtime::HostSlot,
    process: (u32, u64),
    epoch: [u8; 16],
) {
    let recorded = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))
        .and_then(|root| {
            orchbee_local_runtime::record_session_host_epoch(
                &root,
                runtime,
                slot,
                process,
                epoch,
                Instant::now() + EPOCH_RECORD_BUDGET,
            )
        });
    if let Err(error) = recorded {
        crate::diagnostic_events::host_epoch_unrecorded(&error);
    }
}

/// Frees `slot` in the Session record while it still names `process`, a Host this Worker drained
/// and saw end; `false` when the record could not be written, which leaves the slot to the next
/// activation.
#[cfg(windows)]
pub(crate) fn release_slot(
    runtime: &Path,
    slot: orchbee_local_runtime::HostSlot,
    process: (u32, u64),
) -> bool {
    orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))
        .and_then(|root| {
            orchbee_local_runtime::release_session_host(
                &root,
                runtime,
                slot,
                process,
                Instant::now() + EPOCH_RECORD_BUDGET,
            )
        })
        .is_ok()
}

/// Whether the Host process `process` names ended, or ends before `deadline`. One that can no
/// longer be opened, or whose ID names a later process, ended.
#[cfg(windows)]
pub(crate) fn ended_before(process: AdoptHostProcess, deadline: Instant) -> bool {
    use winsafe::{HPROCESS, co};
    let handle = match HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
        false,
        process.process_id.get(),
    ) {
        Ok(handle) => handle,
        Err(error) => return error == co::ERROR::INVALID_PARAMETER,
    };
    if handle
        .GetProcessTimes()
        .is_ok_and(|(creation, ..)| u64::from(creation) != process.creation_time.get())
    {
        return true;
    }
    let wait = deadline.saturating_duration_since(Instant::now());
    let milliseconds = u32::try_from(wait.as_millis()).unwrap_or(u32::MAX);
    matches!(
        handle.WaitForSingleObject(Some(milliseconds)),
        Ok(co::WAIT::OBJECT_0)
    )
}

/// How long writing a Host's epoch into the Session record may wait for the record's writer lock.
#[cfg(windows)]
const EPOCH_RECORD_BUDGET: Duration = Duration::from_secs(1);

/// [`adopt`] at a known endpoint.
pub(crate) fn adopt_endpoint(
    runtime: &Path,
    endpoint: &Path,
    process: Option<AdoptHostProcess>,
    epoch: Option<HostEpoch>,
    channel: Channel<'_>,
) -> Result<Found, Failed> {
    let Channel {
        server,
        correlation,
        drain,
        deadline,
    } = channel;
    let owner = verify(runtime, process, endpoint)?;
    let bounds = DialBounds {
        #[cfg(windows)]
        server_process: process.map(|process| process.process_id),
        busy_until: Instant::now() + BUSY_RETRY,
        deadline,
    };
    let host = HostBrokerConnection::dial(endpoint, bounds, drain).map_err(|failure| {
        use AdoptHostFailure::{HostRefused, HostUnreachable, HostUnverified};
        match failure {
            DialFailure::Connect(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                failed(HostUnverified, error)
            }
            DialFailure::Connect(error) => failed(HostUnreachable, error),
            #[cfg(windows)]
            DialFailure::ServerProcess(error) => failed(HostUnverified, error),
            DialFailure::Busy(error) => failed(HostUnreachable, error),
            // A refusal reads as invalid data; any other transport error never reached an answer.
            DialFailure::Handshake(error) => {
                let unreachable = error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() != io::ErrorKind::InvalidData);
                failed(
                    if unreachable {
                        HostUnreachable
                    } else {
                        HostRefused
                    },
                    error,
                )
            }
        }
    })?;
    if let Some(expected) = epoch
        && *host.host_epoch().as_bytes() != expected.as_bytes()
    {
        return Err(failed(
            AdoptHostFailure::HostMismatch,
            io::Error::new(io::ErrorKind::InvalidData, "the Host epoch differs"),
        ));
    }
    host.pacing.set_site(Site::Adopt);
    let (inventory, restarts) = read_inventory(&host, server, correlation, deadline)?;
    Ok(Found {
        host,
        owner,
        inventory,
        restarts,
    })
}

#[cfg(windows)]
fn verify(
    _runtime: &Path,
    process: Option<AdoptHostProcess>,
    _endpoint: &Path,
) -> Result<AdoptedHost, Failed> {
    use winsafe::{HPROCESS, co};
    let unverified = |text: &str| {
        failed(
            AdoptHostFailure::HostUnverified,
            io::Error::other(text.to_owned()),
        )
    };
    let Some(process) = process else {
        return Err(unverified("no Host process was recorded"));
    };
    // Windows' protected system IDs never name a Host.
    if process.process_id.get() <= 4 {
        return Err(unverified("the recorded Host process is a system process"));
    }
    // The right to end it serves the hang check, which ends a hung Host whose terminals the
    // Keeper holds.
    let handle = match HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE | co::PROCESS::TERMINATE,
        false,
        process.process_id.get(),
    ) {
        Ok(handle) => handle,
        Err(error) if error == co::ERROR::INVALID_PARAMETER => {
            return Err(failed(AdoptHostFailure::HostGone, io::Error::other(error)));
        }
        Err(error) => {
            return Err(failed(
                AdoptHostFailure::HostUnverified,
                io::Error::other(error),
            ));
        }
    };
    match handle.WaitForSingleObject(Some(0)) {
        Ok(co::WAIT::TIMEOUT) => {}
        Ok(co::WAIT::OBJECT_0) => {
            return Err(failed(
                AdoptHostFailure::HostGone,
                io::Error::other("the recorded Host process has exited"),
            ));
        }
        Ok(_) | Err(_) => return Err(unverified("the recorded Host process cannot be observed")),
    }
    let (creation, _, _, _) = handle
        .GetProcessTimes()
        .map_err(|error| failed(AdoptHostFailure::HostUnverified, io::Error::other(error)))?;
    if u64::from(creation) != process.creation_time.get() {
        return Err(unverified(
            "the process with the recorded ID was created at another time",
        ));
    }
    Ok(AdoptedHost { process: handle })
}

#[cfg(unix)]
fn verify(
    runtime: &Path,
    _process: Option<AdoptHostProcess>,
    endpoint: &Path,
) -> Result<AdoptedHost, Failed> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    // Only the held Host lock proves the Host lives; a socket file outlives its listener, and a
    // recorded process ID may name another process by now.
    match orchbee_local_runtime::host_lock_held(runtime) {
        Ok(true) => {}
        Ok(false) => {
            return Err(failed(
                AdoptHostFailure::HostGone,
                io::Error::new(io::ErrorKind::NotFound, "no Host holds the Host lock"),
            ));
        }
        Err(error) => return Err(failed(AdoptHostFailure::HostUnverified, error)),
    }
    nix::sys::socket::UnixAddr::new(endpoint)
        .map_err(|error| failed(AdoptHostFailure::HostUnreachable, io::Error::from(error)))?;
    match std::fs::symlink_metadata(endpoint) {
        Ok(status)
            if status.file_type().is_socket()
                && status.uid() == nix::unistd::geteuid().as_raw()
                && status.mode() & 0o7777 == 0o600 =>
        {
            Ok(AdoptedHost {
                runtime: runtime.to_path_buf(),
            })
        }
        Ok(_) => Err(failed(
            AdoptHostFailure::HostUnverified,
            io::Error::new(io::ErrorKind::PermissionDenied, "invalid Host endpoint"),
        )),
        Err(error) => Err(failed(AdoptHostFailure::HostUnreachable, error)),
    }
}

/// Reads every page of one inventory generation, from its start again when it changed, at most
/// [`MAXIMUM_INVENTORY_RESTARTS`] times.
pub(crate) fn read_inventory(
    host: &HostBrokerConnection,
    server: &LocalStream,
    correlation: CorrelationId,
    deadline: Instant,
) -> Result<(Vec<InventoryTerminal>, u32), Failed> {
    let mut restarts = 0;
    let inventory_failed = |restarts, error: Box<dyn Error>| Failed {
        failure: AdoptHostFailure::InventoryFailed,
        restarts,
        error,
    };
    'inventory: loop {
        let mut cursor = None;
        let mut generation = None;
        let mut terminals = Vec::new();
        loop {
            let page = match list(host, server, correlation, deadline, cursor) {
                Ok(Some(page))
                    if generation.is_none_or(|current| current == page.inventory_generation) =>
                {
                    page
                }
                // The inventory changed under the read.
                Ok(_) => {
                    restarts += 1;
                    if restarts > MAXIMUM_INVENTORY_RESTARTS {
                        return Err(inventory_failed(
                            restarts,
                            io::Error::other("the Host inventory never stayed the same").into(),
                        ));
                    }
                    continue 'inventory;
                }
                Err(error) => return Err(inventory_failed(restarts, error)),
            };
            generation = Some(page.inventory_generation);
            if page.terminals.is_empty() && page.next_cursor.is_some() {
                return Err(inventory_failed(
                    restarts,
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "an empty inventory page continued",
                    )
                    .into(),
                ));
            }
            for summary in page.terminals {
                let terminal = inventory_terminal(host, summary)
                    .map_err(|error| inventory_failed(restarts, error))?;
                terminals.push(terminal);
            }
            if terminals.len() > MAXIMUM_INVENTORY_TERMINALS {
                return Err(inventory_failed(
                    restarts,
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "the Host inventory is too large",
                    )
                    .into(),
                ));
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok((terminals, restarts)),
            }
        }
    }
}

/// One inventory page, or `None` when the Host refused the cursor because the inventory changed.
fn list(
    host: &HostBrokerConnection,
    server: &LocalStream,
    correlation: CorrelationId,
    deadline: Instant,
    cursor: Option<TerminalInventoryCursor>,
) -> Result<Option<TerminalList>, Box<dyn Error>> {
    let control = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
        cursor,
        maximum_terminals: PAGE_TERMINALS,
    }))?;
    let mut exchange = HostExchange::before(host, server, deadline);
    exchange.prepare().map_err(|class| exchange.error(class))?;
    let record = exchange.request(
        correlation,
        control.kind,
        &control.payload,
        PayloadEncoding::ControlJson,
    )?;
    let envelope = decode_transport_envelope(&record)?;
    if let Ok(ConnectionControl::TypedFailure(refusal)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        if refusal.code == STALE_INVENTORY_CURSOR {
            return Ok(None);
        }
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "the Host refused its inventory").into(),
        );
    }
    match decode_inventory_control(envelope.kind, envelope.role, envelope.payload)? {
        InventoryControl::TerminalList(page) => Ok(Some(page)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the Host answered another record",
        )
        .into()),
    }
}

fn inventory_terminal(
    host: &HostBrokerConnection,
    summary: TerminalSummary,
) -> Result<InventoryTerminal, Box<dyn Error>> {
    let (core, started) = match summary {
        TerminalSummary::AwaitingStart { core } => (core, false),
        TerminalSummary::Running { core, .. }
        | TerminalSummary::Finishing { core, .. }
        | TerminalSummary::ContainingFailure { core, .. }
        | TerminalSummary::Exited { core, .. }
        | TerminalSummary::Failed { core, .. } => (core, true),
    };
    let identity = core.identity;
    let handover = core.handover;
    if identity.host_epoch != host.host_epoch() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a terminal of another Host epoch",
        )
        .into());
    }
    Ok(InventoryTerminal {
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes(*identity.creation_id.as_bytes())?,
            HostEpoch::from_bytes(*identity.host_epoch.as_bytes())?,
            HostTerminalId::from_bytes(*identity.terminal_id.as_bytes())?,
        ),
        started,
        handover,
    })
}
