//! Retained Client listener and handlers, closed only by the managed Session owner.
use super::{
    AgentRuntimeOwner, ConnectionSlots, ManagedEndpoint, ServerIdentity, bind_managed,
    handle_candidate_control,
};
use crate::session_shutdown::SessionShutdown;
use crate::update_job::UpdateJob;
use crate::{
    agent_service::AgentService,
    client_connection_registry::ClientConnectionRegistry,
    fault_records::ListenerRecovery,
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchPolicy, WorkbenchService},
};
use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use std::{
    net::Shutdown,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

#[derive(Default)]
struct Control {
    stop: AtomicBool,
    streams: Mutex<Streams>,
}

/// Every Client connection the listener holds, admitted or still in its handshake.
#[derive(Default)]
struct Streams {
    open: Vec<(u64, UnixStream)>,
}

/// Closing this owner wakes idle Clients; success proves the listener and handlers have joined.
pub(crate) struct ManagedClients {
    control: Arc<Control>,
    thread: Option<thread::JoinHandle<io::Result<()>>>,
    /// The admitted Client connections and what each Client offered, which a restart weighs.
    registry: Arc<Mutex<ClientConnectionRegistry>>,
}

impl ManagedClients {
    /// Requests listener shutdown and proves all handlers settled before the caller's deadline.
    /// A timeout retains the join owner; dropping it still joins rather than detaching work.
    pub(crate) fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.control.stop.store(true, Ordering::Release);
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("Client listener panicked"))??;
        }
        Ok(())
    }

    /// Whether every admitted Client connection copes with this Server being replaced: its Client
    /// offered `reconnect`.
    pub(crate) fn rejoin(&self) -> bool {
        self.registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .all_offer(orchbee_connection_handshake::RECONNECT)
    }
}

impl Drop for ManagedClients {
    fn drop(&mut self) {
        self.control.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _joined = thread.join();
        }
    }
}

struct Registration {
    control: Arc<Control>,
    id: u64,
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.control
            .streams
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .open
            .retain(|(id, _)| *id != self.id);
    }
}

/// A handler the listener starts for one accepted candidate.
type Handler = Box<dyn FnOnce() + Send>;
/// Accepts the next candidate, or reports why none was accepted.
type AcceptStep = Box<dyn FnMut(&UnixListener) -> io::Result<UnixStream> + Send>;
/// Starts one candidate's handler thread.
type SpawnStep = Box<dyn FnMut(Handler) -> io::Result<thread::JoinHandle<()>> + Send>;

/// The two steps of the listener loop that can fail for one candidate: accepting it and starting
/// its handler thread. Tests substitute a step that fails once.
struct Steps {
    accept: AcceptStep,
    spawn: SpawnStep,
}

impl Steps {
    fn system() -> Self {
        Self {
            // The listener never blocks; a macOS socket it accepts inherits that mode, and every
            // handler reads and writes in blocking mode.
            accept: Box::new(|listener| {
                let (stream, _address) = listener.accept()?;
                #[cfg(unix)]
                stream.set_nonblocking(false)?;
                Ok(stream)
            }),
            spawn: Box::new(|handler| {
                thread::Builder::new()
                    .name("orchbee-local-client".into())
                    .spawn(handler)
            }),
        }
    }
}

/// Sleeps for `pause` in short steps, returning early once the owner asks the listener to stop.
fn pause_unless_stopped(control: &Control, pause: Duration) {
    let until = Instant::now() + pause;
    while !control.stop.load(Ordering::Acquire) {
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

/// Starts the retained same-user handshake listener; every accepted stream remains owned until
/// join.
/// Returns listener or thread-creation errors without claiming readiness.
pub(crate) fn start(
    endpoint: ManagedEndpoint<'_>,
    identity: ServerIdentity,
    terminals: Arc<LocalTerminalService>,
    agents: Arc<AgentService>,
    shutdown: Arc<SessionShutdown>,
    updates: Arc<UpdateJob>,
) -> io::Result<ManagedClients> {
    start_with(
        endpoint,
        identity,
        terminals,
        agents,
        shutdown,
        updates,
        Steps::system(),
    )
}

/// One failed accept or handler start drops only that candidate, records it, and pauses the
/// listener before it accepts again; admitted Clients keep their connections until Session stop.
fn start_with(
    endpoint: ManagedEndpoint<'_>,
    identity: ServerIdentity,
    terminals: Arc<LocalTerminalService>,
    agents: Arc<AgentService>,
    shutdown: Arc<SessionShutdown>,
    updates: Arc<UpdateJob>,
    mut steps: Steps,
) -> io::Result<ManagedClients> {
    terminals.bind_agent_names(&agents)?;
    let listener = bind_managed(endpoint)?;
    listener.set_nonblocking(true)?;
    let registry = Arc::new(Mutex::new(ClientConnectionRegistry::new()));
    let slots = Arc::new(ConnectionSlots::new());
    let identity = Arc::new(identity);
    let workbench = Arc::new(WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: true,
            create_terminal: true,
            manage_terminal_lifecycle: true,
        },
        Arc::clone(&terminals),
    ));
    let control = Arc::new(Control::default());
    let owner = Arc::clone(&control);
    let admitted = Arc::clone(&registry);
    let thread = thread::Builder::new()
        .name("orchbee-managed-clients".into())
        .spawn(move || {
            let _runtime_owner = AgentRuntimeOwner::new(Arc::clone(&agents));
            let mut handlers: Vec<thread::JoinHandle<()>> = vec![];
            let mut serial = 0_u64;
            let mut recovery = ListenerRecovery::new("client");
            loop {
                let mut index = 0;
                while index < handlers.len() {
                    if handlers[index].is_finished() {
                        let _joined = handlers.swap_remove(index).join();
                    } else {
                        index += 1;
                    }
                }
                if owner.stop.load(Ordering::Acquire) {
                    break;
                }
                match (steps.accept)(&listener) {
                    Ok(stream) => {
                        let Some(slot) = slots.acquire() else {
                            continue;
                        };
                        let Some(id) = serial.checked_add(1) else {
                            let pause = recovery.fault("identity-exhausted", None);
                            pause_unless_stopped(&owner, pause);
                            continue;
                        };
                        serial = id;
                        let cloned = match stream.try_clone() {
                            Ok(cloned) => cloned,
                            Err(error) => {
                                let pause = recovery.fault("clone-stream", Some(error.kind()));
                                pause_unless_stopped(&owner, pause);
                                continue;
                            }
                        };
                        {
                            let mut streams =
                                owner.streams.lock().unwrap_or_else(|e| e.into_inner());
                            streams.open.push((id, cloned));
                        }
                        let registration = Registration {
                            control: Arc::clone(&owner),
                            id,
                        };
                        let registry = Arc::clone(&registry);
                        let identity = Arc::clone(&identity);
                        let terminals = Arc::clone(&terminals);
                        let workbench = Arc::clone(&workbench);
                        let agents = Arc::clone(&agents);
                        let shutdown = Arc::clone(&shutdown);
                        let updates = Arc::clone(&updates);
                        let handler: Handler = Box::new(move || {
                            let _registration = registration;
                            handle_candidate_control(
                                stream,
                                &registry,
                                &identity,
                                slot,
                                &terminals,
                                &workbench,
                                super::stop_control::StopControl {
                                    agents: &agents,
                                    shutdown: Some(&shutdown),
                                    updates: Some(&updates),
                                },
                            );
                        });
                        match (steps.spawn)(handler) {
                            Ok(handler) => handlers.push(handler),
                            // The unstarted handler dropped its stream, slot and registration.
                            Err(error) => {
                                let pause = recovery.fault("spawn-handler", Some(error.kind()));
                                pause_unless_stopped(&owner, pause);
                            }
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    // A failed accept drops at most the candidate it was admitting; the listener
                    // keeps its pipe and accepts the next one.
                    Err(error) => {
                        let pause = recovery.fault("accept", Some(error.kind()));
                        pause_unless_stopped(&owner, pause);
                    }
                }
            }
            drop(listener);
            {
                let streams = owner.streams.lock().unwrap_or_else(|e| e.into_inner());
                for (_, stream) in &streams.open {
                    let _closed = stream.shutdown(Shutdown::Both);
                }
            }
            for handler in handlers {
                let _joined = handler.join();
            }
            Ok(())
        })?;
    Ok(ManagedClients {
        control,
        thread: Some(thread),
        registry: admitted,
    })
}

#[cfg(all(test, windows))]
mod tests;
