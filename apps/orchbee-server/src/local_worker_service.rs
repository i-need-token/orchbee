//! Owns explicit local Worker endpoint startup and same-user, cohort-checked Worker admission
//! before transferring each accepted wire to its registered owner. It does not execute
//! terminal requests or infer peer identity from an endpoint pathname.

use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use orchbee_capability_node_protocol::WorkerId as CapabilityWorkerId;
use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_worker_protocol::{
    LocalWorkerMessage, ProtocolError, ReadMessageError, RejectionReason, read_message,
    write_message,
};
use orchbee_workbench_protocol::PresentationLaunchProgram;

use crate::{
    agent_service::AgentService,
    local_client_service,
    local_terminal_service::LocalTerminalService,
    worker_obcn_carrier::create_worker_carrier,
    worker_registry::{WorkerAdmission, WorkerRegistry},
};

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod managed;

/// The managed Session-owned private observation endpoint for RICH hook
/// envelopes — bound and started by the managed bootstrap before the
/// Worker exists. `unix` because the transport is an AF_UNIX socket over
/// `nix::poll`.
#[cfg(any(unix, windows))]
mod agent_observation_endpoint;

#[cfg(windows)]
mod configuration_windows;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn run(
    arguments: impl Iterator<Item = OsString>,
    expected_cohort: &str,
) -> io::Result<()> {
    let mut arguments = arguments.peekable();
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    if arguments
        .peek()
        .is_some_and(|argument| argument == "--runtime-directory")
    {
        arguments.next();
        let runtime = arguments.next().map(PathBuf::from).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "missing Session runtime")
        })?;
        #[cfg(windows)]
        let configuration = configuration_windows::resolve(
            arguments,
            || orchbee_local_runtime::read_user_settings(|name| std::env::var_os(name)),
            |path, explicit| {
                orchbee_local_runtime::read_user_settings_at(path, explicit, |name| {
                    std::env::var_os(name)
                })
            },
        )?;
        #[cfg(not(windows))]
        if arguments.next().is_some() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        return managed::run(
            &runtime,
            #[cfg(windows)]
            configuration,
        );
    }
    let endpoints = parse_endpoints(arguments)?;
    let listener = UnixListener::bind(&endpoints.worker)?;
    #[cfg(unix)]
    fs::set_permissions(&endpoints.worker, fs::Permissions::from_mode(0o600))?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let terminals = Arc::new(LocalTerminalService::new(Arc::clone(&registry)));
    #[cfg(not(windows))]
    terminals.initialize_preferences(
        orchbee_local_runtime::UserPreferencesScope::from_environment(|name| {
            std::env::var_os(name)
        })
        .map_err(|_| io::Error::other("Settings scope is unavailable"))?,
    )?;
    terminals.start_close_maintenance()?;
    #[cfg(windows)]
    {
        let configuration =
            orchbee_local_runtime::read_user_settings(|name| std::env::var_os(name))
                .map_err(io::Error::other)?;
        terminals
            .initialize_preferences(configuration_windows::preferences_scope(&configuration)?)?;
        terminals.start_process_observer_with_configuration(configuration, false)?;
    }
    let _client_service = endpoints
        .client
        .as_deref()
        .map(|endpoint| {
            local_client_service::start(endpoint, expected_cohort, Arc::clone(&terminals))
        })
        .transpose()?;
    write_ready_marker()?;

    loop {
        match listener.accept() {
            Ok((stream, _address)) => handle_candidate(stream, &registry, expected_cohort, None),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

struct LocalEndpoints {
    client: Option<PathBuf>,
    worker: PathBuf,
}

fn parse_endpoints(mut arguments: impl Iterator<Item = OsString>) -> io::Result<LocalEndpoints> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--local-endpoint")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private Server invocation",
        ));
    }
    let worker = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing endpoint path"))?;
    validate_endpoint(&worker)?;

    let client = match arguments.next() {
        None => None,
        Some(flag) if flag == "--client-endpoint" => {
            let endpoint = arguments.next().map(PathBuf::from).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "missing Client endpoint path")
            })?;
            validate_endpoint(&endpoint)?;
            if endpoint == worker {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Worker and Client endpoints must differ",
                ));
            }
            Some(endpoint)
        }
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid private Server invocation",
            ));
        }
    };
    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private Server invocation",
        ));
    }
    Ok(LocalEndpoints { client, worker })
}

fn validate_endpoint(endpoint: &Path) -> io::Result<()> {
    if endpoint.is_absolute() && endpoint.file_name().is_some() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid endpoint path",
    ))
}

fn write_ready_marker() -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"ready\n")?;
    stdout.flush()
}

fn handle_candidate(
    stream: UnixStream,
    registry: &Arc<Mutex<WorkerRegistry>>,
    expected_cohort: &str,
    agents: Option<&Arc<AgentService>>,
) {
    let _owner = admit_candidate(stream, registry, expected_cohort, agents);
}

fn admit_candidate(
    mut stream: UnixStream,
    registry: &Arc<Mutex<WorkerRegistry>>,
    expected_cohort: &str,
    agents: Option<&Arc<AgentService>>,
) -> Option<thread::JoinHandle<()>> {
    if stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || require_same_effective_user(&stream).is_err()
    {
        return None;
    }

    let message = match read_message(&mut stream) {
        Ok(message) => message,
        Err(ReadMessageError::Protocol(ProtocolError::IncompatibleVersion { .. })) => {
            send_rejection(&mut stream, RejectionReason::IncompatibleProtocol);
            return None;
        }
        Err(ReadMessageError::Io(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            send_rejection(&mut stream, RejectionReason::HandshakeTimeout);
            return None;
        }
        Err(ReadMessageError::Io(_) | ReadMessageError::Protocol(_)) => {
            send_rejection(&mut stream, RejectionReason::MalformedFrame);
            return None;
        }
    };
    let LocalWorkerMessage::WorkerHello(hello) = message else {
        send_rejection(&mut stream, RejectionReason::WrongMessage);
        return None;
    };
    if hello.cohort() != expected_cohort {
        send_rejection(&mut stream, RejectionReason::IncompatibleCohort);
        return None;
    }
    if CapabilityWorkerId::from_bytes(hello.worker_id().as_bytes()).is_err() {
        send_rejection(&mut stream, RejectionReason::MalformedFrame);
        return None;
    }

    // OBLW bounds a reported name exactly as the published launched program is bounded.
    let system_shell = hello
        .system_shell()
        .and_then(|name| PresentationLaunchProgram::parse(name.as_str()).ok());
    let admission = match registry.lock() {
        Ok(mut registry) => registry.admit(hello.worker_id(), system_shell),
        Err(_poisoned) => return None,
    };
    let generation = match admission {
        WorkerAdmission::Accepted { generation } => generation,
        WorkerAdmission::Duplicate { .. } => {
            send_rejection(&mut stream, RejectionReason::DuplicateWorker);
            return None;
        }
        WorkerAdmission::GenerationExhausted => return None,
    };
    if write_message(
        &mut stream,
        &LocalWorkerMessage::WorkerAccepted { generation },
    )
    .is_err()
    {
        cancel_pending_admission(registry, generation);
        return None;
    }

    let carrier = create_worker_carrier(hello.worker_id(), generation);
    // A managed Session admits the canonical Agent leaf inside the registry
    // guard before the Live record installs; the non-managed listener has
    // no AgentService, so it keeps the plain registration unchanged.
    let Some(registered_owner) = (match agents {
        Some(agents) => {
            WorkerRegistry::register_carrier_with_agents(registry, generation, carrier, agents)
        }
        None => WorkerRegistry::register_carrier(registry, generation, carrier),
    }) else {
        cancel_pending_admission(registry, generation);
        return None;
    };
    thread::Builder::new()
        .name("orchbee-local-worker".to_owned())
        .spawn(move || {
            let _exit = registered_owner.run(stream);
        })
        .ok()
}

fn send_rejection(stream: &mut UnixStream, reason: RejectionReason) {
    let _result = write_message(stream, &LocalWorkerMessage::Rejected { reason });
}

fn cancel_pending_admission(
    registry: &Arc<Mutex<WorkerRegistry>>,
    generation: std::num::NonZeroU64,
) {
    if let Ok(mut registry) = registry.lock() {
        let _cancelled = registry.cancel_pending_admission(generation);
    }
}

#[cfg(test)]
mod tests;
