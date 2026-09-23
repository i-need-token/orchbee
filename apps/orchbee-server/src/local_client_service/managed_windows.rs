//! Retained Windows Client listener and handlers, closed only by the managed Site owner.
use super::{AgentRuntimeOwner, ConnectionSlots, bind_managed, handle_candidate_control};
use crate::site_shutdown::SiteShutdown;
use crate::{
    agent_service::AgentService,
    client_session_registry::ClientSessionRegistry,
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchPolicy, WorkbenchService},
};
use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    io,
    path::Path,
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
    streams: Mutex<Vec<(u64, UnixStream)>>,
}

/// Closing this owner wakes idle Clients; success proves the listener and handlers have joined.
pub(crate) struct ManagedClients {
    control: Arc<Control>,
    thread: Option<thread::JoinHandle<io::Result<()>>>,
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
            .retain(|(id, _)| *id != self.id);
    }
}

/// Starts the retained same-user/cohort listener; every accepted stream remains owned until join.
/// Returns listener or thread-creation errors without claiming readiness.
pub(crate) fn start(
    endpoint: &Path,
    cohort: &str,
    terminals: Arc<LocalTerminalService>,
    agents: Arc<AgentService>,
    shutdown: Arc<SiteShutdown>,
) -> io::Result<ManagedClients> {
    let listener = bind_managed(endpoint)?;
    listener.set_nonblocking(true)?;
    let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
    let slots = Arc::new(ConnectionSlots::new());
    let cohort: Arc<str> = Arc::from(cohort);
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
    let thread = thread::Builder::new()
        .name("orchbee-managed-clients".into())
        .spawn(move || {
            let _runtime_owner = AgentRuntimeOwner::new(Arc::clone(&agents));
            let mut handlers: Vec<thread::JoinHandle<()>> = vec![];
            let mut serial = 0_u64;
            let result = loop {
                let mut index = 0;
                while index < handlers.len() {
                    if handlers[index].is_finished() {
                        let _joined = handlers.swap_remove(index).join();
                    } else {
                        index += 1;
                    }
                }
                if owner.stop.load(Ordering::Acquire) {
                    break Ok(());
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        let Some(slot) = slots.acquire() else {
                            continue;
                        };
                        let Some(id) = serial.checked_add(1) else {
                            break Err(io::Error::other("Client connection identity exhausted"));
                        };
                        serial = id;
                        let cloned = match stream.try_clone() {
                            Ok(cloned) => cloned,
                            Err(error) => break Err(error),
                        };
                        owner
                            .streams
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push((id, cloned));
                        let registration = Registration {
                            control: Arc::clone(&owner),
                            id,
                        };
                        let registry = Arc::clone(&registry);
                        let cohort = Arc::clone(&cohort);
                        let terminals = Arc::clone(&terminals);
                        let workbench = Arc::clone(&workbench);
                        let agents = Arc::clone(&agents);
                        let shutdown = Arc::clone(&shutdown);
                        match thread::Builder::new()
                            .name("orchbee-local-client".into())
                            .spawn(move || {
                                let _registration = registration;
                                handle_candidate_control(
                                    stream,
                                    &registry,
                                    &cohort,
                                    slot,
                                    &terminals,
                                    &workbench,
                                    super::stop_control::StopControl {
                                        agents: &agents,
                                        shutdown: Some(&shutdown),
                                    },
                                );
                            }) {
                            Ok(handler) => handlers.push(handler),
                            Err(error) => break Err(error),
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => break Err(error),
                }
            };
            drop(listener);
            {
                let streams = owner.streams.lock().unwrap_or_else(|e| e.into_inner());
                for (_, stream) in &*streams {
                    let _closed = stream.shutdown(Shutdown::Both);
                }
            }
            for handler in handlers {
                let _joined = handler.join();
            }
            result
        })?;
    Ok(ManagedClients {
        control,
        thread: Some(thread),
    })
}

#[cfg(test)]
mod tests;
