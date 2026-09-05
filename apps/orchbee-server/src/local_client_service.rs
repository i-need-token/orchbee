use std::{
    fs, io,
    os::unix::{fs::PermissionsExt, net::UnixListener, net::UnixStream},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use orchbee_local_client_protocol::{
    LocalClientMessage, ProtocolError, ReadMessageError, RejectionReason, read_message,
    write_message,
};
use orchbee_local_peer_credentials::require_same_effective_user;

use crate::client_session_registry::{ClientSessionAdmission, ClientSessionRegistry};

use crate::local_terminal_service::LocalTerminalService;

mod obcs_session;
mod record_stream;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
const MAXIMUM_CLIENT_CONNECTIONS: usize = 16;

pub(crate) fn start(
    endpoint: &Path,
    expected_cohort: &str,
    terminals: Arc<LocalTerminalService>,
) -> io::Result<thread::JoinHandle<io::Result<()>>> {
    let listener = UnixListener::bind(endpoint)?;
    fs::set_permissions(endpoint, fs::Permissions::from_mode(0o600))?;
    let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
    let connection_slots = Arc::new(ConnectionSlots::new());
    let expected_cohort: Arc<str> = Arc::from(expected_cohort);
    thread::Builder::new()
        .name("orchbee-local-client-listener".to_owned())
        .spawn(move || {
            serve(
                listener,
                registry,
                connection_slots,
                expected_cohort,
                terminals,
            )
        })
}

fn serve(
    listener: UnixListener,
    registry: Arc<Mutex<ClientSessionRegistry>>,
    connection_slots: Arc<ConnectionSlots>,
    expected_cohort: Arc<str>,
    terminals: Arc<LocalTerminalService>,
) -> io::Result<()> {
    loop {
        match listener.accept() {
            Ok((stream, _address)) => {
                let Some(connection_slot) = connection_slots.acquire() else {
                    continue;
                };
                let candidate_registry = Arc::clone(&registry);
                let candidate_cohort = Arc::clone(&expected_cohort);
                let candidate_terminals = Arc::clone(&terminals);
                let _candidate = thread::Builder::new()
                    .name("orchbee-local-client".to_owned())
                    .spawn(move || {
                        handle_candidate(
                            stream,
                            &candidate_registry,
                            &candidate_cohort,
                            connection_slot,
                            &candidate_terminals,
                        );
                    });
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn handle_candidate(
    mut stream: UnixStream,
    registry: &Arc<Mutex<ClientSessionRegistry>>,
    expected_cohort: &str,
    _connection_slot: ConnectionSlot,
    terminals: &LocalTerminalService,
) {
    if stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || require_same_effective_user(&stream).is_err()
    {
        return;
    }

    let message = match read_message(&mut stream) {
        Ok(message) => message,
        Err(ReadMessageError::Protocol(ProtocolError::IncompatibleVersion { .. })) => {
            send_rejection(&mut stream, RejectionReason::IncompatibleProtocol);
            return;
        }
        Err(ReadMessageError::Io(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            send_rejection(&mut stream, RejectionReason::HandshakeTimeout);
            return;
        }
        Err(ReadMessageError::Io(_) | ReadMessageError::Protocol(_)) => {
            send_rejection(&mut stream, RejectionReason::MalformedFrame);
            return;
        }
    };
    let LocalClientMessage::ClientHello(hello) = message else {
        send_rejection(&mut stream, RejectionReason::WrongMessage);
        return;
    };
    if hello.cohort() != expected_cohort {
        send_rejection(&mut stream, RejectionReason::IncompatibleCohort);
        return;
    }

    let admission = match registry.lock() {
        Ok(mut registry) => registry.admit(),
        Err(_poisoned) => return,
    };
    let generation = match admission {
        ClientSessionAdmission::Accepted { generation } => generation,
        ClientSessionAdmission::CapacityExceeded => {
            send_rejection(&mut stream, RejectionReason::CapacityExceeded);
            return;
        }
        ClientSessionAdmission::GenerationExhausted => return,
    };
    let _session_registration = ClientSessionRegistration::new(Arc::clone(registry), generation);
    if write_message(
        &mut stream,
        &LocalClientMessage::ClientAccepted { generation },
    )
    .is_err()
    {
        return;
    }
    if stream.set_read_timeout(None).is_err() || stream.set_write_timeout(None).is_err() {
        return;
    }

    let Ok(mut records) = record_stream::ClientRecordStream::new(&stream) else {
        return;
    };
    let mut session = terminals.session();
    let _session_result = obcs_session::run(&mut records, |request| session.execute(request));
}

fn send_rejection(stream: &mut UnixStream, reason: RejectionReason) {
    let _result = write_message(stream, &LocalClientMessage::Rejected { reason });
}

struct ClientSessionRegistration {
    registry: Arc<Mutex<ClientSessionRegistry>>,
    generation: std::num::NonZeroU64,
}

impl ClientSessionRegistration {
    const fn new(
        registry: Arc<Mutex<ClientSessionRegistry>>,
        generation: std::num::NonZeroU64,
    ) -> Self {
        Self {
            registry,
            generation,
        }
    }
}

impl Drop for ClientSessionRegistration {
    fn drop(&mut self) {
        let mut registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(poisoned) => poisoned.into_inner(),
        };
        let _was_active = registry.disconnect(self.generation);
    }
}

struct ConnectionSlots {
    in_use: Mutex<usize>,
}

impl ConnectionSlots {
    const fn new() -> Self {
        Self {
            in_use: Mutex::new(0),
        }
    }

    fn acquire(self: &Arc<Self>) -> Option<ConnectionSlot> {
        let mut in_use = self.in_use.lock().ok()?;
        if *in_use >= MAXIMUM_CLIENT_CONNECTIONS {
            return None;
        }
        *in_use += 1;
        Some(ConnectionSlot(Arc::clone(self)))
    }

    fn release(&self) {
        let mut in_use = match self.in_use.lock() {
            Ok(in_use) => in_use,
            Err(poisoned) => poisoned.into_inner(),
        };
        *in_use = in_use.saturating_sub(1);
    }
}

struct ConnectionSlot(Arc<ConnectionSlots>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[cfg(test)]
mod tests;
