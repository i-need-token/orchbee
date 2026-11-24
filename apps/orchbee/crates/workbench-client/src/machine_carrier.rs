//! The local endpoint a remote Client connection runs on: every connection it accepts is carried
//! over one hub `session` channel to the Machine's Session, which admits this build's own offer
//! through the handshake. The carrier judges no byte. It builds wherever the hub does, and starts
//! only where the `SavedMachines` capability holds; `apps/orchbee/docs/machines.md#session-carrier`
//! owns the rules.

// module-layer: workflow

mod carry;

pub(crate) use carry::Hub;

use crate::{
    local_client::{LocalClient, RetainedCarrier, cancellation::Cancellation},
    machine_refusal::Refusal,
    machines::ProfileId,
    tui_capabilities::Capability,
};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    thread::JoinHandle,
    time::Instant,
};

/// Whether the hub reads the Machine `online` now; the endpoint closes new connections otherwise.
pub(crate) type Gate = Arc<dyn Fn() -> bool + Send + Sync>;

/// Native creation transfers these resources directly into the retained Client guard.
struct StartedCarrier {
    endpoint: PathBuf,
    stop: Arc<AtomicBool>,
    task: JoinHandle<io::Result<()>>,
}

/// Admits one Client connection to `machine`'s Session over `hub`, within `deadline`. The endpoint
/// lives as long as the returned Client or any origin derived from it.
pub(crate) fn connect(
    hub: &Hub,
    machine: &ProfileId,
    online: Gate,
    deadline: Instant,
    cancelled: Cancellation<'_>,
) -> io::Result<LocalClient> {
    // An unavailable host refuses before inspecting the Machine or cancellation source.
    Capability::SavedMachines.require()?;
    let StartedCarrier {
        endpoint,
        stop,
        task,
    } = carry::start(hub, machine, online)?;
    let guard = Arc::new(RetainedCarrier::new(stop, task));
    // The stream carries an ordinary Client connection: the remote Session admits this build's
    // own offer through the handshake. The invocation-owned wake is observed throughout.
    let offer = crate::local_client::client_offer(None);
    let client =
        LocalClient::connect_before_cancellable(&endpoint, &offer, deadline, cancelled.wake())
            .map_err(admission_error)?;
    if cancelled.is_cancelled()? {
        return Err(io::ErrorKind::Interrupted.into());
    }
    Ok(client.with_retained_carrier(guard))
}

/// One admitted Client connection to a Machine's Session for a byte relay. The carrier it runs over
/// lives as long as this value.
pub struct RelayedSession {
    pub stream: orchbee_local_ipc::LocalStream,
    /// Every capability the Session's handshake answer listed.
    pub capabilities: Vec<String>,
    _carrier: Arc<RetainedCarrier>,
}

/// Admits one Client connection to `machine`'s Session over the hub of this data root, which it
/// attaches to only when it already runs, within `deadline`, for the desktop window's relay. The
/// error is the refusal's wire reason.
///
/// # Errors
///
/// `not-found` for a malformed id, `hub-unavailable` when no hub could be joined, a handshake
/// refusal by its name, `transient` for a Session at capacity, stopping or out of time, `protocol`
/// for an answer outside its contract, and `link-lost` when the channel ended before admission.
pub fn relay_session(machine: &str, deadline: Instant) -> Result<RelayedSession, &'static str> {
    use crate::machine_hub::{Attachment, HubClient, OpenCancel, Opening};
    Capability::SavedMachines
        .require()
        .map_err(|_| "hub-unavailable")?;
    let machine = ProfileId::parse(machine).ok_or("not-found")?;
    // A relay holds no hub and is offered no login: the machines process keeps the hub.
    let attachment = Attachment {
        presence: false,
        asks: false,
        watch: false,
    };
    let (hub, _) = HubClient::open_with(
        attachment,
        deadline,
        Opening::Present,
        &OpenCancel::default(),
    )
    .map_err(|_| "hub-unavailable")?;
    let hub = Arc::new(hub);
    let StartedCarrier {
        endpoint,
        stop,
        task,
    } = carry::start(&hub, &machine, Arc::new(|| true)).map_err(|_| "hub-unavailable")?;
    let carrier = Arc::new(RetainedCarrier::new(stop, task));
    let offer = crate::local_client::client_offer(None);
    let client = LocalClient::connect_before(&endpoint, &offer, deadline).map_err(|error| {
        let error = admission_error(error);
        match (Refusal::of(&error), error.kind()) {
            (Some(Refusal::OtherEnvironment), _) => "other-environment",
            (Some(Refusal::ClientTooOld), _) => "client-too-old",
            (Some(Refusal::SessionTooOld), _) => "session-too-old",
            (Some(Refusal::DevelopmentBuildMismatch), _) => "development-build-mismatch",
            (None, io::ErrorKind::ConnectionRefused | io::ErrorKind::TimedOut) => "transient",
            (None, io::ErrorKind::InvalidData | io::ErrorKind::PermissionDenied) => "protocol",
            (None, _) => "link-lost",
        }
    })?;
    let capabilities = client
        .server_capabilities()
        .iter()
        .map(|name| name.as_str().to_owned())
        .collect();
    let stream = client.into_admitted_stream().map_err(|_| "protocol")?;
    Ok(RelayedSession {
        stream,
        capabilities,
        _carrier: carrier,
    })
}

/// The connection failure discovery classifies. A handshake refusal that needs the user to change
/// one side carries its [`Refusal`]; capacity, stopping, and timeout stay transient.
pub(crate) fn admission_error(error: crate::local_client::LocalClientConnectError) -> io::Error {
    use crate::local_client::LocalClientConnectError;
    use orchbee_local_client_protocol::ConnectClientError;
    match error {
        LocalClientConnectError::RequestIoConfiguration(error)
        | LocalClientConnectError::Admission(
            ConnectClientError::ConnectFailed(error)
            | ConnectClientError::Io(error)
            | ConnectClientError::CancellationSourceFault(error),
        ) => error,
        LocalClientConnectError::Admission(ConnectClientError::CancellationReady) => {
            io::ErrorKind::Interrupted.into()
        }
        LocalClientConnectError::Admission(ConnectClientError::Rejected(
            orchbee_local_client_protocol::RejectionReason::CapacityExceeded
            | orchbee_local_client_protocol::RejectionReason::HandshakeTimeout
            | orchbee_local_client_protocol::RejectionReason::Stopping,
        )) => io::ErrorKind::ConnectionRefused.into(),
        LocalClientConnectError::Admission(ConnectClientError::Refused(rejection)) => {
            use orchbee_connection_handshake::RejectionReason as Reason;
            match rejection.reason {
                Reason::Capacity | Reason::Stopping | Reason::Timeout => {
                    io::ErrorKind::ConnectionRefused.into()
                }
                Reason::EnvironmentMismatch => Refusal::OtherEnvironment.into(),
                Reason::InitiatorTooOld => Refusal::ClientTooOld.into(),
                Reason::InitiatorTooNew => Refusal::SessionTooOld.into(),
                Reason::FingerprintMismatch => Refusal::DevelopmentBuildMismatch.into(),
                Reason::Malformed | Reason::ConnectionUnsupported | Reason::Unknown => {
                    io::ErrorKind::PermissionDenied.into()
                }
            }
        }
        // A remote Session that predates the handshake is the older side.
        LocalClientConnectError::Admission(ConnectClientError::PredatesHandshake) => {
            Refusal::SessionTooOld.into()
        }
        LocalClientConnectError::Admission(ConnectClientError::Rejected(_)) => {
            io::ErrorKind::PermissionDenied.into()
        }
        LocalClientConnectError::Admission(
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse,
        ) => io::ErrorKind::InvalidData.into(),
    }
}
