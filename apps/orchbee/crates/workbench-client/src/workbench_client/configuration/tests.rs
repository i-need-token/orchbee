use super::*;
use crate::{
    client_view::{ClientActionDisposition, KeyCode, KeyEvent, Modifiers, PrefixInput},
    local_client::{LocalClient, test_support::server::FakeServer},
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{WorkbenchRequest, WorkbenchRevision, read_workbench_request};
use std::{
    io::{self, Read},
    num::NonZeroU64,
    sync::mpsc,
    thread,
};

fn snapshot(revision: u64, profile: &str) -> ConfigurationSnapshot {
    ConfigurationSnapshot {
        revision: WorkbenchRevision::new(revision),
        diagnostics: Default::default(),
        keys_profile: profile.into(),
        notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
        agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
        spaces_sidebar_profile: orchbee_local_runtime::SpaceSidebarSettings::default().profile(),
        window_title: "{hostname}: {workspace}".into(),
        manifest_check: true,
        focus_new_panel: true,
        worktrees_directory: None,
        new_cwd: "follow".into(),
    }
}

#[test]
fn native_reload_keeps_shared_appearance_while_updating_hints_and_sidebars() {
    use crate::{client_view::ClientAction, localization::Locale};
    use orchbee_workbench_protocol::{ApplicationAppearance, SettingsLocale, UserSettingsValues};
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        connection: Mutex::new(None),
        revision: None,
        applied_snapshot: None,
        notification_update: None,
        creation_update: None,
    };
    let mut view = ClientView::new(120);
    let preferences = UserSettingsValues {
        application_theme: ApplicationAppearance::Dark,
        locale: SettingsLocale::ZhCn,
        ..Default::default()
    };
    view.apply_user_preferences(preferences.clone());
    view.settle_pointer_released(true);
    let theme = view.configured_theme();
    let mut published = snapshot(1, "[keys]\nprefix='ctrl+a'\nreload_config='prefix+f6'");
    published.agent_sidebar_profile = r#"{"rows":[],"rows_by_agent":{},"row_gap":2}"#.into();
    published.spaces_sidebar_profile = r#"{"rows":[],"row_gap":3}"#.into();
    published.notification_profile = "[ui.toast]\ndelivery='orchbee'".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(published);
    assert!(watcher.tick(&mut view));
    assert_eq!(view.configured_theme(), theme);
    assert_eq!(view.effective_locale(Locale::EnUs), Locale::ZhCn);
    assert_eq!(view.user_preferences(), Some(&preferences));
    assert!(view.pointer_released());
    assert_eq!(view.machines.sidebar_settings().row_gap(), 2);
    assert_eq!(view.machines.space_settings().row_gap(), 3);
    assert!(view.notifications.card_delivery());
    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('a'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, ClientActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    assert_eq!(
        view.decide(PrefixInput::Key(KeyEvent::press(
            KeyCode::F(6),
            Modifiers::NONE
        )))
        .action(),
        Some(ClientAction::ReloadConfiguration)
    );
    assert_eq!(
        view.bindings().action_labels_with_glyphs(
            crate::client_view::PrefixAction::Client(ClientAction::ReloadConfiguration),
            crate::client_view::KeyLabelGlyphs::Unicode,
        ),
        [crate::terminal_events::keys::tests::host_label("Ctrl+a F6")],
        "discovery uses the effective configured action"
    );
}

#[test]
fn bad_sidebar_publication_retains_keys_notifications_layout_and_revision_atomically() {
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        connection: Mutex::new(None),
        revision: None,
        applied_snapshot: None,
        notification_update: None,
        creation_update: None,
    };
    let mut view = ClientView::new(120);
    let before = view.clone();
    let mut candidate = snapshot(1, "[keys]\nprefix='ctrl+a'");
    candidate.notification_profile = "[ui.toast]\ndelivery='orchbee'".into();
    candidate.agent_sidebar_profile = "{}".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(candidate.clone());
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, None);
    assert_eq!(view.configured_theme(), before.configured_theme());
    assert_eq!(view.bindings(), before.bindings());
    assert_eq!(view.notifications, before.notifications);
    assert_eq!(view.machines, before.machines);
    assert_eq!(view.notice(), Some(Notice::ConfigurationFailed));
    candidate.agent_sidebar_profile = r#"{"rows":[],"rows_by_agent":{},"row_gap":2}"#.into();
    candidate.spaces_sidebar_profile = "{}".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(candidate.clone());
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, None);
    assert_eq!(view.bindings(), before.bindings());
    assert_eq!(view.notifications, before.notifications);
    assert_eq!(view.machines, before.machines);
    candidate.spaces_sidebar_profile = r#"{"rows":[],"row_gap":3}"#.into();
    watcher.shared.0.lock().unwrap().snapshot = Some(candidate);
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.configured_theme(), before.configured_theme());
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    assert_ne!(view.notifications, before.notifications);
    assert_eq!(view.machines.sidebar_settings().row_gap(), 2);
    assert_eq!(view.machines.space_settings().row_gap(), 3);
    assert!(
        view.machines
            .sidebar_settings()
            .rows_for_agent("codex", None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn authoritative_server_baseline_applies_once_then_revision_order_is_strict() {
    use orchbee_workbench_protocol::{
        ConfigurationDiagnostic, ConfigurationDiagnosticDisposition, ConfigurationDiagnostics,
    };
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        connection: Mutex::new(None),
        revision: None,
        applied_snapshot: None,
        notification_update: None,
        creation_update: None,
    };
    let mut view = ClientView::new(120);
    view.configure_keys(&KeySettings::from_profile("[keys]\nprefix='ctrl+a'").unwrap());
    view.settle_pointer_released(true);
    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('a'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, ClientActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    let mut baseline = snapshot(0, "[keys]\nprefix='ctrl+x'");
    baseline.diagnostics = ConfigurationDiagnostics {
        items: vec![ConfigurationDiagnostic {
            field: "keys.help".into(),
            reason: "unsafe-binding".into(),
            disposition: ConfigurationDiagnosticDisposition::Disabled,
        }],
        omitted_count: 0,
    };
    watcher.shared.0.lock().unwrap().snapshot = Some(baseline);
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(0));
    assert!(!view.prefix_active());
    assert!(view.pointer_released());
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('x'), Modifiers::CTRL))
    );
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));
    let retained = view.clone();
    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(0, "[keys]\nprefix='ctrl+b'"));
    assert!(!watcher.tick(&mut view));
    assert_eq!(retained, view);
    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(1, "[keys]\nprefix='ctrl+a'"));
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    let retained = view.clone();
    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(0, "[keys]\nprefix='ctrl+x'"));
    assert!(!watcher.tick(&mut view));
    assert_eq!(retained, view);
    {
        let mut state = watcher.shared.0.lock().unwrap();
        let rejected = snapshot(2, "[keys]\nhelp='ctrl+c'");
        state.snapshot = Some(rejected);
        state.notice = Some(PendingNotice {
            revision: Some(2),
            needs_application: true,
            notice: Notice::ConfigurationPublished,
        });
    }
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.bindings(), retained.bindings());
    assert_eq!(view.configured_theme(), retained.configured_theme());
    assert_eq!(view.notice(), Some(Notice::ConfigurationFailed));
}

#[test]
fn diagnostics_follow_only_a_complete_newer_snapshot_and_clean_publication_clears_warning() {
    use orchbee_workbench_protocol::{
        ConfigurationDiagnostic, ConfigurationDiagnosticDisposition, ConfigurationDiagnostics,
    };
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        connection: Mutex::new(None),
        revision: None,
        applied_snapshot: None,
        notification_update: None,
        creation_update: None,
    };
    let mut view = ClientView::new(120);
    let mut warning = snapshot(1, "[keys]\nprefix='ctrl+a'");
    warning.diagnostics = ConfigurationDiagnostics {
        items: vec![ConfigurationDiagnostic {
            field: "keys.help".into(),
            reason: "unsafe-binding".into(),
            disposition: ConfigurationDiagnosticDisposition::Disabled,
        }],
        omitted_count: 0,
    };
    watcher.shared.0.lock().unwrap().snapshot = Some(warning);
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));

    {
        let mut state = watcher.shared.0.lock().unwrap();
        state.snapshot = Some(snapshot(1, "[keys]\nprefix='ctrl+x'"));
        state.notice = Some(PendingNotice {
            revision: Some(1),
            needs_application: true,
            notice: Notice::ConfigurationPublished,
        });
    }
    assert!(!watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));

    {
        let mut state = watcher.shared.0.lock().unwrap();
        state.snapshot = Some(snapshot(1, "[keys]\nprefix='ctrl+x'"));
        state.notice = Some(PendingNotice {
            revision: Some(1),
            needs_application: false,
            notice: Notice::ConfigurationPublished,
        });
    }
    assert!(!watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));

    watcher.shared.0.lock().unwrap().snapshot = Some(snapshot(2, "[keys]\nprefix='ctrl+x'"));
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(2));
    assert_eq!(view.notice(), None);
}

#[test]
fn warning_reload_keeps_the_agent_notification_surface_available() {
    use orchbee_workbench_protocol::{
        ConfigurationDiagnostic, ConfigurationDiagnosticDisposition, ConfigurationDiagnostics,
        ConfigurationResult, WorkbenchResponse, write_workbench_response,
    };
    let server = FakeServer::start_connections(1, move |index, stream| {
        let mut stream = crate::local_client::test_support::multiplex_peer::Peer::new(stream);
        crate::local_client::test_support::handshake::answer_offer(&mut stream)?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Configuration(current) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration current");
        };
        assert_eq!(current.action, ConfigurationAction::Current);
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: current.request_id,
                session_id: current.session_id,
                outcome: ConfigurationOutcome::Current {
                    snapshot: snapshot(0, "[keys]\nprefix='ctrl+b'"),
                },
            }),
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Configuration(reload) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration reload");
        };
        assert_eq!(reload.action, ConfigurationAction::Reload);
        let mut warning = snapshot(1, "[keys]\nprefix='ctrl+b'");
        warning.diagnostics = ConfigurationDiagnostics {
            items: vec![ConfigurationDiagnostic {
                field: "keys.help".into(),
                reason: "unsafe-binding".into(),
                disposition: ConfigurationDiagnosticDisposition::Disabled,
            }],
            omitted_count: 0,
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: reload.request_id,
                session_id: reload.session_id,
                outcome: ConfigurationOutcome::Reloaded {
                    snapshot: warning,
                    changed: true,
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    drop(client);
    let mut view = ClientView::new(120);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != Some(0) {
        assert!(
            std::time::Instant::now() < deadline,
            "initial snapshot deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(watcher.reload());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != Some(1) {
        assert!(
            std::time::Instant::now() < deadline,
            "warning reload deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));
    assert!(crate::workbench_client::notification_available(
        &view,
        Some(crate::workbench_client::WorkbenchClientView::notification_test_view(),),
    ));
    watcher.close();
    server.finish().unwrap();
}

#[test]
fn unchanged_reload_reports_publication_without_reapplying_the_existing_snapshot() {
    use orchbee_workbench_protocol::{
        ConfigurationResult, WorkbenchResponse, write_workbench_response,
    };
    let server = FakeServer::start_connections(1, move |index, stream| {
        let mut stream = crate::local_client::test_support::multiplex_peer::Peer::new(stream);
        crate::local_client::test_support::handshake::answer_offer(&mut stream)?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Configuration(current) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration current");
        };
        assert_eq!(current.action, ConfigurationAction::Current);
        let baseline = snapshot(1, "[keys]\nprefix='ctrl+b'");
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: current.request_id,
                session_id: current.session_id,
                outcome: ConfigurationOutcome::Current {
                    snapshot: baseline.clone(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Configuration(reload) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration reload");
        };
        assert_eq!(reload.action, ConfigurationAction::Reload);
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: reload.request_id,
                session_id: reload.session_id,
                outcome: ConfigurationOutcome::Reloaded {
                    snapshot: baseline,
                    changed: false,
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    drop(client);
    let mut view = ClientView::new(120);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != Some(1) {
        assert!(
            std::time::Instant::now() < deadline,
            "initial snapshot deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('b'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, ClientActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    assert!(watcher.reload());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while view.notice() != Some(Notice::ConfigurationPublished) {
        assert!(
            std::time::Instant::now() < deadline,
            "unchanged reload receipt deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(watcher.revision, Some(1));
    assert!(
        view.prefix_active(),
        "unchanged snapshot must not be replayed"
    );
    assert!(watcher.take_notification_update().is_none());
    watcher.close();
    server.finish().unwrap();
}

#[test]
fn unchanged_reload_catches_up_a_lagging_client_once_before_reporting_success() {
    use orchbee_workbench_protocol::{
        ConfigurationResult, WorkbenchResponse, write_workbench_response,
    };
    let server = FakeServer::start_connections(1, move |index, stream| {
        let mut stream = crate::local_client::test_support::multiplex_peer::Peer::new(stream);
        crate::local_client::test_support::handshake::answer_offer(&mut stream)?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Configuration(current) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration current");
        };
        assert_eq!(current.action, ConfigurationAction::Current);
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: current.request_id,
                session_id: current.session_id,
                outcome: ConfigurationOutcome::Current {
                    snapshot: snapshot(1, "[keys]\nprefix='ctrl+b'"),
                },
            }),
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Configuration(reload) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration reload");
        };
        assert_eq!(reload.action, ConfigurationAction::Reload);
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: reload.request_id,
                session_id: reload.session_id,
                outcome: ConfigurationOutcome::Reloaded {
                    snapshot: ConfigurationSnapshot {
                        focus_new_panel: false,
                        worktrees_directory: Some("C:/trees".into()),
                        ..snapshot(2, "[keys]\nprefix='ctrl+x'")
                    },
                    changed: false,
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    drop(client);
    let mut view = ClientView::new(120);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != Some(1) {
        assert!(
            std::time::Instant::now() < deadline,
            "initial snapshot deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(watcher.reload());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while view.notice() != Some(Notice::ConfigurationPublished) {
        assert!(
            std::time::Instant::now() < deadline,
            "lagging reload receipt deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(watcher.revision, Some(2));
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('x'), Modifiers::CTRL))
    );
    assert_eq!(watcher.take_notification_update(), Some(2));
    let defaults = watcher.take_creation_update().unwrap();
    assert!(!defaults.focus_new_panel);
    assert_eq!(defaults.worktrees_directory.as_deref(), Some("C:/trees"));
    assert!(!watcher.tick(&mut view));
    assert!(watcher.take_notification_update().is_none());
    assert!(watcher.take_creation_update().is_none());
    watcher.close();
    server.finish().unwrap();
}

#[test]
fn closing_watcher_cancels_only_its_pending_stream() {
    use crate::local_client::test_support::multiplex_peer::{admit, read_open};
    use orchbee_workbench_protocol::{MultiplexMessage, read_multiplex_message};
    let (opened, ready) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream, 1)?;
        let open = read_open(&mut stream)?;
        opened.send(()).unwrap();
        loop {
            if let MultiplexMessage::StreamClose { stream_id, .. } =
                read_multiplex_message(&mut stream)?
            {
                assert_eq!(stream_id, open.id);
                break;
            }
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    watcher.close();
    assert!(watcher.connection.lock().unwrap().is_none());
    assert!(!watcher.reload());
    watcher.close();
    // Keep the carrier alive until the peer observes this stream's cancellation.
    server.finish().unwrap();
    drop(client);
}

#[test]
fn submitted_reload_loss_recovers_by_subscription_without_replaying_the_write() {
    use crate::local_client::test_support::multiplex_peer::{admit, read_open, respond};
    use orchbee_workbench_protocol::{ConfigurationResult, WorkbenchResponse};
    let (lost, loss) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        admit(&mut stream, u64::try_from(index + 1).unwrap())?;
        let open = read_open(&mut stream)?;
        let WorkbenchRequest::Configuration(current) =
            read_workbench_request(&mut io::Cursor::new(&open.request)).unwrap()
        else {
            panic!("configuration subscription");
        };
        assert_eq!(
            current.action,
            ConfigurationAction::Current,
            "reconnect replays only the subscription"
        );
        respond(
            &mut stream,
            &open,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: current.request_id,
                session_id: current.session_id,
                outcome: ConfigurationOutcome::Current {
                    snapshot: snapshot(if index == 0 { 0 } else { 2 }, "[keys]\nprefix='ctrl+a'"),
                },
            }),
            1,
        )?;
        if index == 0 {
            let open = read_open(&mut stream)?;
            let WorkbenchRequest::Configuration(reload) =
                read_workbench_request(&mut io::Cursor::new(open.request)).unwrap()
            else {
                panic!("reload");
            };
            assert_eq!(reload.action, ConfigurationAction::Reload);
            // Commit may have happened, but its acknowledgement is lost with the carrier.
            lost.send(()).unwrap();
        } else {
            finish.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    assert!(watcher.reload());
    loss.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut view = ClientView::new(120);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != Some(2) {
        assert!(
            std::time::Instant::now() < deadline,
            "publication recovery deadline"
        );
        watcher.tick(&mut view);
        thread::yield_now();
    }
    assert_eq!(view.notice(), Some(Notice::ConfigurationUnknown));
    assert!(
        view.bindings()
            .prefix
            .matches(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    release.send(()).unwrap();
    watcher.close();
    drop(client);
    server.finish().unwrap();
}

#[test]
fn notification_policy_and_keys_are_applied_as_one_valid_publication() {
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        connection: Mutex::new(None),
        revision: None,
        applied_snapshot: None,
        notification_update: None,
        creation_update: None,
    };
    let mut view = ClientView::new(100);
    let mut next = snapshot(1, "[keys]\nprefix='ctrl+a'");
    next.notification_profile = "[ui.toast]\ndelivery='orchbee'".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(next);
    assert!(watcher.tick(&mut view));
    assert!(view.notifications.card_delivery());
    let keys = view.bindings().clone();
    let mut invalid = snapshot(2, "[keys]\nprefix='ctrl+x'");
    invalid.notification_profile = "[ui.toast]\ndelivery='bad'".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(invalid);
    assert!(watcher.tick(&mut view));
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.bindings(), &keys);
    assert!(view.notifications.card_delivery());
}
