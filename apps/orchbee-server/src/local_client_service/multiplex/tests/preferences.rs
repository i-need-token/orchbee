//! External preference writers have no Server commit signal; retain their sampled visibility.
use super::*;
use crate::{
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    SettingsOperation, SettingsOutcome, SettingsRequest, UserPreferencesRequest,
    UserPreferencesResponse, WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response,
    write_workbench_response,
};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct PreferenceDirectory(PathBuf);
impl Drop for PreferenceDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn external_preferences_keep_their_deadline_during_traffic_and_idle() {
    let directory = std::env::temp_dir().join(format!(
        "orchbee-multiplex-preferences-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let directory = PreferenceDirectory(directory);
    let scope = UserPreferencesScope::for_configuration(&directory.0.join("config.toml")).unwrap();
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    terminals.initialize_preferences(scope.clone()).unwrap();
    let service = Arc::new(WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        terminals,
    ));
    let WorkbenchExecution::Response(WorkbenchResponse::Snapshot(snapshot)) =
        service.execute(WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        }))
    else {
        panic!("snapshot");
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = snapshot.outcome else {
        panic!("available");
    };
    let mut request = Vec::new();
    write_workbench_request(
        &mut request,
        &WorkbenchRequest::Settings(SettingsRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
            session_id: snapshot.session_id,
            operation: SettingsOperation::Open {},
        }),
    )
    .unwrap();
    let first = MultiplexMessage::StreamOpen {
        stream_id: MultiplexStreamId::new(1).unwrap(),
        lane_id: MultiplexLaneId::ROOT,
        mode: StreamMode::State,
        request,
    };
    let signal = Arc::new(SessionChangeSignal::default());
    let observed = Arc::clone(&signal);
    let file_reads = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&file_reads);
    let (mut peer, server) = bounded_pair();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let task = thread::spawn(move || {
        let connection = service.client_connection(NonZeroU64::MIN);
        connection.watch_changes(&observed);
        run(
            &server,
            first,
            observed,
            |_, request| {
                if matches!(request.workbench(), Some(WorkbenchRequest::Settings(_))) {
                    captured.lock().unwrap().push(Instant::now());
                }
                let WorkbenchExecution::Response(response) =
                    connection.execute(request.workbench().unwrap().clone())
                else {
                    panic!("read outcome");
                };
                let mut bytes = Vec::new();
                write_workbench_response(&mut bytes, &response)
                    .map_err(ApplicationConnectionError::Workbench)?;
                Ok(bytes)
            },
            |_| {},
            |_| {},
            || connection.maintain_subscription_deadline(),
        )
    });
    // Join the actual carrier and release the store fixture even when a deadline assertion fails.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let settings_snapshot = |message, expected_kind| {
            let MultiplexMessage::StreamFrame {
                stream_id,
                kind,
                record,
                ..
            } = message
            else {
                panic!("settings frame");
            };
            assert_eq!(stream_id.value(), 1);
            assert_eq!(kind, expected_kind);
            let WorkbenchResponse::Settings(reply) =
                read_workbench_response(&mut Cursor::new(record)).unwrap()
            else {
                panic!("settings reply");
            };
            let SettingsOutcome::Opened { snapshot } = reply.outcome else {
                panic!("opened preferences");
            };
            snapshot
        };
        let initial = settings_snapshot(
            read_multiplex_message(&mut peer).unwrap(),
            StreamFrameKind::Snapshot,
        );
        let external = UserPreferencesStore::new(scope);
        let save = |source: &str, theme: &str| {
            let request: UserPreferencesRequest = serde_json::from_value(serde_json::json!({
                "kind": "update", "source": source, "changes": { "applicationTheme": theme }
            }))
            .unwrap();
            let UserPreferencesResponse::Snapshot {
                snapshot,
                changed: true,
            } = external.execute(&request)
            else {
                panic!("external writer");
            };
            snapshot
        };
        let revision = signal.revision();
        let changed = save(&initial.source, "dark");
        assert_eq!(signal.revision(), revision);
        let began = Instant::now();
        let mut next = 2;
        let mut pushed = None;
        // Real unrelated request frames wake the dispatcher faster than Host sampling. They
        // must neither read this file early nor keep pushing its absolute deadline forward.
        while pushed.is_none() && began.elapsed() < Duration::from_secs(3) {
            write_multiplex_message(&mut peer, &open(next, 1, StreamMode::Request)).unwrap();
            loop {
                let message = read_multiplex_message(&mut peer).unwrap();
                if matches!(&message, MultiplexMessage::StreamFrame { stream_id, kind: StreamFrameKind::Result, .. } if stream_id.value() == next)
                {
                    break;
                }
                pushed = Some(settings_snapshot(message, StreamFrameKind::Update));
            }
            next += 1;
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(pushed, Some(changed.clone()));
        assert_eq!(signal.revision(), revision);
        let reads = file_reads.lock().unwrap().clone();
        assert!(reads.len() >= 2);
        assert!(
            reads
                .windows(2)
                .all(|pair| pair[1].duration_since(pair[0]) >= Duration::from_secs(1))
        );
        let reads_before_idle = reads.len();

        let final_value = save(&changed.source, "light");
        assert_eq!(signal.revision(), revision);
        // There is no peer request or semantic notification between this external write and
        // delivery. An idle connection must wake for its independent file-observation deadline.
        assert_eq!(
            settings_snapshot(
                read_multiplex_message(&mut peer).unwrap(),
                StreamFrameKind::Update,
            ),
            final_value
        );
        let reads = file_reads.lock().unwrap();
        assert!(reads.len() > reads_before_idle);
        assert!(
            reads
                .windows(2)
                .all(|pair| pair[1].duration_since(pair[0]) >= Duration::from_secs(1))
        );
    }));
    let shutdown = peer.shutdown(std::net::Shutdown::Both);
    drop(peer);
    let joined = task.join();
    shutdown.unwrap();
    let _closed = joined.unwrap();
    if let Err(failure) = result {
        std::panic::resume_unwind(failure);
    }
}
