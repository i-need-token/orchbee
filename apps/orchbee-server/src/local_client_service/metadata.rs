//! Read process-owned diagnostics before allocating any Client Session state.
use orchbee_local_client_protocol::{
    ClientHello, LocalClientMessage, SERVER_FEATURE_AGENT_OBSERVATION, SERVER_FEATURE_CLIENT_VIEW,
    SERVER_FEATURE_RENDER_ROWS, ServerMetadata, ServerMode, write_message,
};
use orchbee_local_ipc::LocalStream;
use std::num::{NonZeroU8, NonZeroU32};

pub(super) fn respond(stream: &mut LocalStream, cohort: &str, mode: ServerMode) {
    let Ok(cohort) = ClientHello::new(cohort) else {
        return;
    };
    let metadata = ServerMetadata::new(
        env!("CARGO_PKG_VERSION"),
        cohort,
        NonZeroU32::new(std::process::id()).expect("Windows process ID is nonzero"),
        NonZeroU8::new(orchbee_client_server_protocol::PROTOCOL_VERSION)
            .expect("OBCS version is nonzero"),
        mode,
        SERVER_FEATURE_RENDER_ROWS | SERVER_FEATURE_CLIENT_VIEW | SERVER_FEATURE_AGENT_OBSERVATION,
    );
    if let Ok(metadata) = metadata {
        let _delivery = write_message(stream, &LocalClientMessage::ServerMetadata(metadata));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agent_service::{AgentPolicy, AgentService},
        client_session_registry::{
            ClientSessionAdmission, ClientSessionRegistry, MAXIMUM_ACTIVE_CLIENT_SESSIONS,
        },
        local_client_service::{
            ConnectionSlots, handle_candidate_control, stop_control::StopControl,
        },
        local_terminal_service::LocalTerminalService,
        site_shutdown::SiteShutdown,
        workbench_service::{WorkbenchPolicy, WorkbenchService},
        worker_registry::WorkerRegistry,
    };
    use orchbee_local_client_protocol::{SiteStopRequest, StopAction, StopId, read_message};
    use std::{
        io::Read,
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    #[test]
    fn metadata_bypasses_full_session_admission_without_allocating_a_generation_or_stop_lease() {
        for mode in [
            ServerMode::Unmanaged,
            ServerMode::ManagedReady,
            ServerMode::ManagedStopping,
        ] {
            let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
            for _ in 0..MAXIMUM_ACTIVE_CLIENT_SESSIONS {
                assert!(matches!(
                    registry.lock().unwrap().admit(),
                    ClientSessionAdmission::Accepted { .. }
                ));
            }
            let before = registry.lock().unwrap().clone();
            let server_registry = Arc::clone(&registry);
            let (mut client, stream) = LocalStream::pair().unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let handler = thread::spawn(move || {
                let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
                    WorkerRegistry::new(),
                ))));
                let agents = AgentService::open(AgentPolicy {
                    read: true,
                    manage: true,
                })
                .unwrap();
                let workbench = WorkbenchService::new(
                    WorkbenchPolicy {
                        read: true,
                        manage_navigation: true,
                        associate_directory: true,
                        create_terminal: true,
                        manage_terminal_lifecycle: true,
                    },
                    Arc::clone(&terminals),
                );
                let shutdown = SiteShutdown::default();
                if mode == ServerMode::ManagedStopping {
                    shutdown.request(&SiteStopRequest {
                        hello: ClientHello::new("server-cohort").unwrap(),
                        id: StopId::new([1; 16]).unwrap(),
                        action: StopAction::Start,
                    });
                }
                let slots = Arc::new(ConnectionSlots::new());
                handle_candidate_control(
                    stream,
                    &server_registry,
                    "server-cohort",
                    slots.acquire().unwrap(),
                    &terminals,
                    &workbench,
                    StopControl {
                        agents: &agents,
                        shutdown: (mode != ServerMode::Unmanaged).then_some(&shutdown),
                    },
                );
                assert_eq!(shutdown.stopping(), mode == ServerMode::ManagedStopping);
                assert!(shutdown.settle_records(std::time::Instant::now()));
            });
            write_message(&mut client, &LocalClientMessage::ServerMetadataRequest).unwrap();
            let LocalClientMessage::ServerMetadata(metadata) = read_message(&mut client).unwrap()
            else {
                panic!("metadata response")
            };
            assert_eq!(metadata.cohort(), "server-cohort");
            assert_eq!(metadata.version(), env!("CARGO_PKG_VERSION"));
            assert_eq!(metadata.process_id().get(), std::process::id());
            assert_eq!(
                metadata.terminal_protocol().get(),
                orchbee_client_server_protocol::PROTOCOL_VERSION
            );
            assert_eq!(metadata.mode(), mode);
            assert_eq!(metadata.features(), 7);
            assert_eq!(client.read(&mut [0]).unwrap(), 0);
            handler.join().unwrap();
            assert_eq!(*registry.lock().unwrap(), before);
        }
    }
}
