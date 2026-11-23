//! Starts one managed Terminal Host from this Worker's cohort and reaches it under one deadline:
//! a Host in a slot no Host holds, or a recovery Host in the slot of a Host that ended. It names
//! where a failed start stopped and whether a later start may pass; it never decides when a Host
//! starts or what it serves.

use crate::{
    agent_drain::AgentDrain,
    diagnostic_events::{ExitCause, Peer, Reason, Site},
    host_connection::{DialFailure, HostBrokerConnection},
};
use orchbee_local_runtime::{
    Cohort, CohortArtifact, ModeCheckedLocalSessionRendezvous, SessionScope,
};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::{
    cell::RefCell,
    error::Error,
    io,
    path::Path,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

/// Where a managed Host start failed, named in its diagnostic record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostStartStage {
    /// Preparing the Host's runtime folder.
    Directory,
    /// Checking the endpoint address, or an endpoint that existed before the start.
    Endpoint,
    /// Registering the Host in the Session record before creating it; only Windows and macOS
    /// register.
    #[cfg(any(windows, target_os = "macos"))]
    Registration,
    /// Creating the Host process.
    Spawn,
    /// Publishing the created Host's identity and letting it run.
    #[cfg(any(windows, target_os = "macos"))]
    Release,
    /// The Host exited, or could not be observed, before its endpoint appeared.
    Exited,
    /// Dialing the Host's endpoint, which never reached the Host.
    Connect,
    /// The `worker-host` offer or the protocol `Hello` with a reached Host.
    Handshake,
    /// The shared deadline passed before the endpoint appeared.
    Deadline,
}

impl HostStartStage {
    /// The closed token the diagnostic record names this stage by.
    fn token(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Endpoint => "endpoint",
            #[cfg(any(windows, target_os = "macos"))]
            Self::Registration => "registration",
            Self::Spawn => "spawn",
            #[cfg(any(windows, target_os = "macos"))]
            Self::Release => "release",
            Self::Exited => "exited",
            Self::Connect => "connect",
            Self::Handshake => "handshake",
            Self::Deadline => "deadline",
        }
    }
}

/// A failed managed Host start: its stage, the time since the start began, the Host's exit code
/// when it had exited, the failure itself, and whether this Worker may start a Host again.
#[derive(Debug)]
pub(super) struct HostStartFailure {
    stage: HostStartStage,
    elapsed: Duration,
    host_exit: Option<u32>,
    error: Box<dyn Error>,
    /// No Host process was registered or created, or the one created has exited and left no
    /// endpoint, so a later start cannot meet it; and the failure is not one every later start
    /// would repeat, such as an endpoint present before the start. A Windows Host's registration
    /// outlives its process, so there only a failure before registration leaves a retry.
    retryable: bool,
}

impl HostStartFailure {
    /// Writes the failure's diagnostic record.
    pub(super) fn record(&self) {
        crate::diagnostic_events::host_start_failed(
            self.stage.token(),
            self.error.as_ref(),
            self.host_exit,
            self.elapsed,
        );
    }

    /// The failure itself.
    pub(super) fn into_error(self) -> Box<dyn Error> {
        self.error
    }

    /// Whether a later start may pass where this one failed.
    pub(super) const fn retryable(&self) -> bool {
        self.retryable
    }

    /// The loop's end after a failure that leaves no retry, or the last of its starts: the Host
    /// never answered in time, ended before it was reached, or a local step failed.
    pub(super) fn into_exit_cause(self) -> Box<dyn Error> {
        let (reason, peer) = match self.stage {
            HostStartStage::Deadline => (Reason::Deadline, Peer::Host),
            HostStartStage::Exited | HostStartStage::Connect | HostStartStage::Handshake => {
                (Reason::HostClosed, Peer::Host)
            }
            _ => (Reason::Local, Peer::None),
        };
        Box::new(ExitCause {
            site: Site::HostStart,
            reason,
            peer,
            waited: None,
            source: self.error,
        })
    }
}

/// How long a failed dial waits for the Host to exit, so the record carries the exit code of a
/// Host that ended the connection by ending itself.
const HOST_EXIT_GRACE: Duration = Duration::from_millis(250);

/// Host starts a Worker makes. A start that fails in a way a later one could pass leaves a retry,
/// but only this many in a row: the last failure ends the Worker after its answer, as a permanent
/// one does.
pub(super) const HOST_START_ATTEMPTS: u32 = 3;

/// Which Host a start makes: a Host in a slot no Host holds, or a recovery Host in the slot of a
/// Host that ended while the Keeper kept its programs, serving that Host's epoch when the bindings
/// name one. A Linux Worker starts only the first slot's Host.
#[derive(Clone, Copy)]
pub(super) enum HostStart {
    Fresh(orchbee_local_runtime::HostSlot),
    #[cfg(any(windows, target_os = "macos"))]
    Recovery(
        orchbee_local_runtime::HostSlot,
        Option<orchbee_capability_node_protocol::HostEpoch>,
    ),
}

impl HostStart {
    pub(super) const fn slot(self) -> orchbee_local_runtime::HostSlot {
        match self {
            Self::Fresh(slot) => slot,
            #[cfg(any(windows, target_os = "macos"))]
            Self::Recovery(slot, _) => slot,
        }
    }
}

pub(super) fn start_host(
    cohort: &Cohort,
    runtime: &Path,
    drain: Rc<RefCell<AgentDrain>>,
    start: HostStart,
) -> Result<(HostBrokerConnection, orchbee_local_runtime::RetainedChild), HostStartFailure> {
    let began = Instant::now();
    let deadline = began + Duration::from_secs(5);
    let failed = |stage, error: Box<dyn Error>| HostStartFailure {
        stage,
        elapsed: began.elapsed(),
        host_exit: None,
        error,
        retryable: true,
    };
    // The Host binds `<id>-host.sock` relatively inside the socket folder while the Worker
    // connects through the derived absolute path; validate that address before the Host
    // directory or process can appear.
    let endpoint = ModeCheckedLocalSessionRendezvous::slot_endpoint_path(runtime, start.slot());
    // A failure no later start could pass leaves no retry: the address is the same every time.
    let permanent = |stage, error: Box<dyn Error>| HostStartFailure {
        retryable: false,
        ..failed(stage, error)
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    if start.slot() != orchbee_local_runtime::HostSlot::FIRST {
        return Err(permanent(
            HostStartStage::Endpoint,
            "a Linux Session runs its one Host in the first slot".into(),
        ));
    }
    #[cfg(unix)]
    nix::sys::socket::UnixAddr::new(&endpoint)
        .map_err(|error| permanent(HostStartStage::Endpoint, error.into()))?;
    let scope_name = ModeCheckedLocalSessionRendezvous::slot_scope_name(start.slot());
    let directory = SessionScope::parse(&scope_name)
        .map_err(Box::<dyn Error>::from)
        .and_then(|scope| {
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(runtime, scope)
                .map_err(Box::<dyn Error>::from)
        })
        .map_err(|error| failed(HostStartStage::Directory, error))?;
    // An endpoint already present belongs to a Host this Worker did not start, live or dead, and
    // stays until the Session cleans up: every later start would meet it too.
    let recovery_required = || -> Box<dyn Error> {
        io::Error::new(io::ErrorKind::AlreadyExists, "Host recovery is required").into()
    };
    #[cfg(unix)]
    match std::fs::symlink_metadata(&endpoint) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        // The lost Host's socket outlives it. Once no Host holds the slot's lock the socket names
        // no listener, so it goes before the recovery Host starts, which a dial would meet first.
        #[cfg(target_os = "macos")]
        Ok(_) if matches!(start, HostStart::Recovery(..)) => {
            match orchbee_local_runtime::host_lock_held(runtime, start.slot()) {
                Ok(false) => {}
                Ok(true) => return Err(permanent(HostStartStage::Endpoint, recovery_required())),
                Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
            }
            match std::fs::remove_file(&endpoint) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    return Err(failed(HostStartStage::Endpoint, error.into()));
                }
                _ => {}
            }
        }
        Ok(_) => return Err(permanent(HostStartStage::Endpoint, recovery_required())),
        Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
    }
    #[cfg(windows)]
    match orchbee_local_ipc::endpoint_exists(&endpoint) {
        Ok(false) => {}
        Ok(true) => return Err(permanent(HostStartStage::Endpoint, recovery_required())),
        Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
    }
    let host_endpoint_id =
        ModeCheckedLocalSessionRendezvous::slot_endpoint_id(runtime, start.slot());
    let executable = cohort.executable(CohortArtifact::TerminalHost);
    let arguments = [
        std::ffi::OsStr::new("serve"),
        std::ffi::OsStr::new("--runtime-directory"),
        directory.runtime_directory().as_os_str(),
        std::ffi::OsStr::new("--endpoint-id"),
        std::ffi::OsStr::new(&host_endpoint_id),
    ];
    // A recovery Host serves the lost Host's epoch, so each recovered terminal keeps its identity.
    #[cfg(any(windows, target_os = "macos"))]
    let epoch = match start {
        HostStart::Recovery(_, Some(epoch)) => Some(
            orchbee_terminal_protocol::HostEpoch::from_bytes(epoch.as_bytes())
                .map_err(|error| permanent(HostStartStage::Spawn, error.into()))?
                .to_text(),
        ),
        HostStart::Fresh(_) | HostStart::Recovery(_, None) => None,
    };
    #[cfg(any(windows, target_os = "macos"))]
    let mut arguments = arguments.to_vec();
    #[cfg(any(windows, target_os = "macos"))]
    if let Some(epoch) = &epoch {
        arguments.extend([
            std::ffi::OsStr::new("--host-epoch"),
            std::ffi::OsStr::new(epoch),
        ]);
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let HostStart::Fresh(_) = start;
    // A macOS or Linux Host binds its endpoint in the socket folder, which the Session admitted.
    #[cfg(unix)]
    let socket_root = ModeCheckedLocalSessionRendezvous::socket_root_path();
    #[cfg(unix)]
    let arguments = [
        &arguments[..],
        &[
            std::ffi::OsStr::new("--endpoint-directory"),
            socket_root.as_os_str(),
        ],
    ]
    .concat();
    #[cfg(any(windows, target_os = "macos"))]
    let mut child = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))
        .and_then(|root| {
            orchbee_local_runtime::spawn_session_child(
                &root,
                runtime,
                match start {
                    HostStart::Fresh(slot) => orchbee_local_runtime::SessionChild::Host(slot),
                    #[cfg(any(windows, target_os = "macos"))]
                    HostStart::Recovery(slot, _) => {
                        orchbee_local_runtime::SessionChild::RecoveryHost(slot)
                    }
                },
                &executable,
                &arguments,
                deadline,
            )
        })
        .map_err(|error| {
            use orchbee_local_runtime::SessionChildStep;
            let stage = match SessionChildStep::of(&error) {
                Some(SessionChildStep::Creation) => HostStartStage::Spawn,
                Some(SessionChildStep::Release) => HostStartStage::Release,
                Some(SessionChildStep::Registration) | None => HostStartStage::Registration,
            };
            HostStartFailure {
                retryable: false,
                ..failed(stage, error.into())
            }
        })?;
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut child = orchbee_local_runtime::spawn_retained(&executable, &arguments)
        .map_err(|error| failed(HostStartStage::Spawn, error.into()))?;
    let dialed = |child: &mut orchbee_local_runtime::RetainedChild,
                  dial: Result<HostBrokerConnection, DialFailure>| {
        let failure = match dial {
            Ok(host) => return Ok(host),
            Err(DialFailure::Connect(error)) => failed(HostStartStage::Connect, error.into()),
            #[cfg(windows)]
            Err(DialFailure::ServerProcess(error)) => failed(HostStartStage::Connect, error.into()),
            Err(DialFailure::Busy(error) | DialFailure::Handshake(error)) => {
                failed(HostStartStage::Handshake, error)
            }
        };
        // The elapsed time stays the dial's own; the wait only observes the Host's exit.
        Err(HostStartFailure {
            host_exit: exit_after_failure(child),
            ..failure
        })
    };
    let ready = (|| loop {
        if Instant::now() >= deadline {
            return Err(failed(
                HostStartStage::Deadline,
                io::Error::from(io::ErrorKind::TimedOut).into(),
            ));
        }
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                return Err(HostStartFailure {
                    host_exit: exit_code(status),
                    ..failed(
                        HostStartStage::Exited,
                        io::Error::other("Terminal Host exited before readiness").into(),
                    )
                });
            }
            Err(error) => return Err(failed(HostStartStage::Exited, error.into())),
        }
        #[cfg(windows)]
        {
            match orchbee_local_ipc::endpoint_exists(&endpoint) {
                Ok(true) => {
                    let dial = HostBrokerConnection::connect_before(
                        &endpoint,
                        deadline,
                        Rc::clone(&drain),
                    );
                    return dialed(&mut child, dial);
                }
                Ok(false) => thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(failed(HostStartStage::Connect, error.into())),
            }
        }
        #[cfg(unix)]
        match std::fs::symlink_metadata(&endpoint) {
            Ok(status)
                if status.file_type().is_socket()
                    && status.uid() == nix::unistd::geteuid().as_raw()
                    && status.mode() & 0o7777 == 0o600 =>
            {
                let dial =
                    HostBrokerConnection::connect_before(&endpoint, deadline, Rc::clone(&drain));
                return dialed(&mut child, dial);
            }
            Ok(status)
                if status.file_type().is_socket()
                    && status.uid() == nix::unistd::geteuid().as_raw() =>
            {
                // This endpoint did not exist before our one spawn. Wait for the Host's
                // final chmod; never connect to the intermediate socket mode.
                thread::sleep(Duration::from_millis(10));
            }
            Ok(_) => {
                return Err(failed(
                    HostStartStage::Endpoint,
                    io::Error::new(io::ErrorKind::PermissionDenied, "invalid Host endpoint").into(),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
        }
    })();
    match ready {
        Ok(host) => {
            // The Server pairs this Host with its terminals through the epoch the record keeps.
            #[cfg(any(windows, target_os = "macos"))]
            if let Ok(process) = child.exact_identity() {
                crate::host_adoption::record_epoch(
                    runtime,
                    start.slot(),
                    process,
                    *host.host_epoch().as_bytes(),
                );
            }
            Ok((host, child))
        }
        // A Host still running could later bind the endpoint a second start dials, one that bound
        // its endpoint before it ended leaves that endpoint behind, and a Windows or macOS
        // registration outlives the process; each leaves this Worker no further start.
        Err(failure) => Err(HostStartFailure {
            retryable: cfg!(not(any(windows, target_os = "macos")))
                && matches!(child.try_wait(), Ok(Some(_)))
                && endpoint_absent(&endpoint),
            ..failure
        }),
    }
}

/// Whether no endpoint, not even one a Host that ended left behind, is at the Host's address.
#[cfg(unix)]
fn endpoint_absent(endpoint: &Path) -> bool {
    matches!(std::fs::symlink_metadata(endpoint), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

#[cfg(windows)]
fn endpoint_absent(endpoint: &Path) -> bool {
    matches!(orchbee_local_ipc::endpoint_exists(endpoint), Ok(false))
}

/// The Host's exit code once it exits within [`HOST_EXIT_GRACE`]; a Host still running keeps none.
fn exit_after_failure(child: &mut orchbee_local_runtime::RetainedChild) -> Option<u32> {
    let until = Instant::now() + HOST_EXIT_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return exit_code(status),
            Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(10)),
            Ok(None) | Err(_) => return None,
        }
    }
}

/// A Windows exit code keeps all 32 bits, so a crash reads as its status value; a POSIX Host ended
/// by a signal has none.
fn exit_code(status: std::process::ExitStatus) -> Option<u32> {
    status
        .code()
        .map(|code| u32::from_ne_bytes(code.to_ne_bytes()))
}
