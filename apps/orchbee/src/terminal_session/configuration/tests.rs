use super::*;
use crate::{
    client_view::{KeyCode, KeyEvent, Modifiers, PrefixInput, SessionActionDisposition},
    local_client::{LocalClient, test_server::FakeServer},
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{WorkbenchRequest, WorkbenchRevision, read_workbench_request};
use std::{
    io::{self, Read},
    num::NonZeroU64,
    sync::mpsc,
};

fn snapshot(revision: u64, profile: &str) -> ConfigurationSnapshot {
    ConfigurationSnapshot {
        revision: WorkbenchRevision::new(revision),
        keys_profile: profile.into(),
        notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
        agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
        window_title: "{hostname}: {workspace}".into(),
        restart_required: Vec::new(),
        default_shell: String::new(),
        shell_mode: orchbee_workbench_protocol::ConfigurationShellMode::Auto,
        manifest_check: true,
    }
}

#[test]
fn bad_sidebar_publication_retains_keys_notifications_layout_and_revision_atomically() {
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: 0,
    };
    let mut view = ClientView::new(120);
    let before = view.clone();
    let mut candidate = snapshot(1, "[keys]\nprefix='ctrl+a'");
    candidate.notification_profile = "[ui.toast]\ndelivery='orchbee'".into();
    candidate.agent_sidebar_profile = "{}".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(candidate.clone());
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, 0);
    assert_eq!(view.bindings(), before.bindings());
    assert_eq!(view.notifications, before.notifications);
    assert_eq!(view.machines, before.machines);
    assert_eq!(view.notice(), Some(Notice::ConfigurationFailed));
    candidate.agent_sidebar_profile = r#"{"rows":[],"rows_by_agent":{},"row_gap":2}"#.into();
    watcher.shared.0.lock().unwrap().snapshot = Some(candidate);
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, 1);
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    assert_ne!(view.notifications, before.notifications);
    assert_eq!(view.machines.sidebar_settings().row_gap(), 2);
    assert!(
        view.machines
            .sidebar_settings()
            .rows_for_agent("codex", None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn only_valid_newer_publications_replace_keys_and_clear_prefix_without_changing_pointer_ownership()
{
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: 0,
    };
    let mut view = ClientView::new(120);
    view.settle_pointer_released(true);
    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('b'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, SessionActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(0, "[keys]\nprefix='ctrl+x'"));
    assert!(
        !watcher.tick(&mut view),
        "startup baseline is not a publication"
    );
    assert!(view.prefix_active());
    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(1, "[keys]\nprefix='ctrl+a'"));
    assert!(watcher.tick(&mut view));
    assert!(!view.prefix_active());
    assert!(view.pointer_released());
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    let retained = view.clone();
    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(1, "[keys]\nprefix='ctrl+x'"));
    assert!(!watcher.tick(&mut view));
    assert_eq!(retained, view);
    {
        let mut state = watcher.shared.0.lock().unwrap();
        state.snapshot = Some(snapshot(2, "[keys]\nhelp='ctrl+c'"));
        state.notice = Some(Notice::ConfigurationPublished);
    }
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, 1);
    assert_eq!(view.bindings(), retained.bindings());
    assert_eq!(view.notice(), Some(Notice::ConfigurationFailed));
}

#[test]
fn closing_watcher_wakes_and_joins_blocked_admission_or_configuration_read() {
    for during_admission in [false, true] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if index == 0 || !during_admission {
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
                    },
                )
                .map_err(io::Error::other)?;
            }
            if index == 1 {
                if !during_admission {
                    let WorkbenchRequest::Configuration(request) =
                        read_workbench_request(&mut stream).map_err(io::Error::other)?
                    else {
                        panic!("configuration read");
                    };
                    assert_eq!(request.action, ConfigurationAction::Current);
                }
                reached_tx.send(()).unwrap();
                assert_eq!(stream.read(&mut [0])?, 0);
            }
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let mut watcher = Watcher::spawn(
            client.origin(),
            client.operation_clock(),
            SiteId::parse(&format!("{:032x}", 2)).unwrap(),
        )
        .unwrap();
        drop(client);
        reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        watcher.close();
        assert!(watcher.task.is_none());
        assert!(!watcher.reload());
        watcher.close();
        server.finish().unwrap();
    }
}

#[test]
fn submitted_reload_loss_recovers_by_read_only_and_installs_the_observed_publication() {
    use orchbee_workbench_protocol::{
        ConfigurationResult, WorkbenchResponse, write_workbench_response,
    };
    let (first_tx, first_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(3, move |index, mut stream| {
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        if index == 0 {
            return Ok(());
        }
        let WorkbenchRequest::Configuration(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration");
        };
        assert_eq!(
            request.action,
            ConfigurationAction::Current,
            "reconnect must not retry a submitted reload"
        );
        if index == 1 {
            first_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: ConfigurationOutcome::Current {
                    snapshot: snapshot(if index == 1 { 0 } else { 2 }, "[keys]\nprefix='ctrl+a'"),
                },
            }),
        )
        .map_err(io::Error::other)?;
        if index == 1 {
            let WorkbenchRequest::Configuration(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("reload");
            };
            assert_eq!(request.action, ConfigurationAction::Reload);
            // Apply may have succeeded. Drop the response; the next connection only reads.
        } else {
            assert_eq!(stream.read(&mut [0])?, 0);
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SiteId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    drop(client);
    first_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(watcher.reload());
    release_tx.send(()).unwrap();
    let mut view = ClientView::new(120);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "publication recovery deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(view.notice(), Some(Notice::ConfigurationUnknown));
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    watcher.close();
    server.finish().unwrap();
}

#[test]
fn notification_policy_and_keys_are_applied_as_one_valid_publication() {
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: 0,
    };
    let mut view = ClientView::new(100);
    let mut next = snapshot(1, "[keys]\nprefix='ctrl+a'");
    next.notification_profile = "[ui.toast]\ndelivery='herdr'".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(next);
    assert!(watcher.tick(&mut view));
    assert!(view.notifications.card_delivery());
    let keys = view.bindings().clone();
    let mut invalid = snapshot(2, "[keys]\nprefix='ctrl+x'");
    invalid.notification_profile = "[ui.toast]\ndelivery='bad'".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(invalid);
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, 1);
    assert_eq!(view.bindings(), &keys);
    assert!(view.notifications.card_delivery());
}
