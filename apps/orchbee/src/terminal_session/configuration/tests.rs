use super::*;
use crate::{
    client_view::{KeyCode, KeyEvent, Modifiers, PrefixInput, SessionActionDisposition},
    local_client::{LocalClient, tests::server::FakeServer},
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
        diagnostics: Default::default(),
        keys_profile: profile.into(),
        theme_name: orchbee_workbench_protocol::ConfigurationThemeName::TerminalNative,
        notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
        agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
        spaces_sidebar_profile: orchbee_local_site_runtime::WindowsSpaceSidebarSettings::default()
            .profile(),
        window_title: "{hostname}: {workspace}".into(),
        restart_required: Vec::new(),
        default_shell: String::new(),
        shell_mode: orchbee_workbench_protocol::ConfigurationShellMode::Auto,
        manifest_check: true,
    }
}

#[test]
fn native_reload_keeps_shared_appearance_while_updating_hints_and_sidebars() {
    use crate::{client_view::ClientAction, localization::Locale};
    use orchbee_workbench_protocol::{
        ApplicationAppearance, PreferencesLocale, UserPreferencesValues,
    };
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: None,
        applied_snapshot: None,
        notification_update: None,
    };
    let mut view = ClientView::new(120);
    let preferences = UserPreferencesValues {
        application_theme: ApplicationAppearance::Dark,
        locale: PreferencesLocale::ZhCn,
        ..Default::default()
    };
    view.apply_user_preferences(preferences.clone());
    view.settle_pointer_released(true);
    let theme = view.configured_theme();
    let mut published = snapshot(1, "[keys]\nprefix='ctrl+a'\nsettings='prefix+f6'");
    published.theme_name = orchbee_workbench_protocol::ConfigurationThemeName::OneLight;
    published.agent_sidebar_profile = r#"{"rows":[],"rows_by_agent":{},"row_gap":2}"#.into();
    published.spaces_sidebar_profile = r#"{"rows":[],"row_gap":3}"#.into();
    published.notification_profile = "[ui.toast]\ndelivery='orchbee'".into();
    watcher.shared.0.lock().unwrap().snapshot = Some(published);
    assert!(watcher.tick(&mut view));
    assert_eq!(view.configured_theme(), theme);
    assert_eq!(view.effective_locale(Locale::EnUs), Locale::ZhCn);
    assert_eq!(view.user_preferences.as_ref(), Some(&preferences));
    assert!(view.pointer_released());
    assert_eq!(view.machines.sidebar_settings().row_gap(), 2);
    assert_eq!(view.machines.space_settings().row_gap(), 3);
    assert!(view.notifications.card_delivery());
    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('a'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, SessionActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    assert_eq!(
        view.decide(PrefixInput::Key(KeyEvent::press(
            KeyCode::F(6),
            Modifiers::NONE
        )))
        .action(),
        Some(ClientAction::OpenSettings)
    );
    assert_eq!(
        view.bindings()
            .action_labels(crate::client_view::PrefixAction::Client(
                ClientAction::OpenSettings
            )),
        ["Ctrl+a F6"],
        "discovery uses the effective configured action"
    );
}

#[test]
fn bad_sidebar_publication_retains_keys_notifications_layout_and_revision_atomically() {
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: None,
        applied_snapshot: None,
        notification_update: None,
    };
    let mut view = ClientView::new(120);
    let before = view.clone();
    let mut candidate = snapshot(1, "[keys]\nprefix='ctrl+a'");
    candidate.theme_name = orchbee_workbench_protocol::ConfigurationThemeName::OneLight;
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
    assert_eq!(view.configured_theme().unwrap().name, "one-light");
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
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: None,
        applied_snapshot: None,
        notification_update: None,
    };
    let mut view = ClientView::new(120);
    view.configure_keys(&WindowsKeySettings::from_profile("[keys]\nprefix='ctrl+a'").unwrap());
    view.settle_pointer_released(true);
    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('a'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, SessionActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    let mut baseline = snapshot(0, "[keys]\nprefix='ctrl+x'");
    baseline.theme_name = orchbee_workbench_protocol::ConfigurationThemeName::OneDark;
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
    assert_eq!(view.configured_theme().unwrap().name, "one-dark");
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
        let mut rejected = snapshot(2, "[keys]\nhelp='ctrl+c'");
        rejected.theme_name = orchbee_workbench_protocol::ConfigurationThemeName::OneDark;
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
    let (_reader, wake) = LocalStream::pair().unwrap();
    let mut watcher = Watcher {
        shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
        wake,
        task: None,
        revision: None,
        applied_snapshot: None,
        notification_update: None,
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
    let server = FakeServer::start_connections(2, move |index, mut stream| {
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
                site_id: current.site_id,
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
                site_id: reload.site_id,
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
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SiteId::parse(&format!("{:032x}", 2)).unwrap(),
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
    assert!(crate::notifications::available(
        &view,
        Some(crate::terminal_session::TerminalSessionView::Native(
            crate::terminal_session::NativeSessionView::notification_test_view(),
        )),
    ));
    watcher.close();
    server.finish().unwrap();
}

#[test]
fn warning_reload_reports_restart_then_restores_warning_and_notification_availability() {
    use orchbee_workbench_protocol::{
        ConfigurationDiagnostic, ConfigurationDiagnosticDisposition, ConfigurationDiagnostics,
        ConfigurationRestart, ConfigurationResult, WorkbenchResponse, write_workbench_response,
    };
    let server = FakeServer::start_connections(2, move |index, mut stream| {
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
                site_id: current.site_id,
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
        warning.restart_required = vec![ConfigurationRestart::ResumeAgents];
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: reload.request_id,
                site_id: reload.site_id,
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
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SiteId::parse(&format!("{:032x}", 2)).unwrap(),
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
            "warning restart reload deadline"
        );
        watcher.tick(&mut view);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(view.notice(), Some(Notice::ConfigurationRestart));
    assert!(crate::notifications::available(
        &view,
        Some(crate::terminal_session::TerminalSessionView::Native(
            crate::terminal_session::NativeSessionView::notification_test_view(),
        )),
    ));

    let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('b'),
        Modifiers::CTRL,
    )));
    view.commit(prefix, SessionActionDisposition::KeepNavigator);
    let dismiss = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('c'),
        Modifiers::NONE,
    )));
    view.commit(dismiss, SessionActionDisposition::KeepNavigator);
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));
    assert!(crate::notifications::available(
        &view,
        Some(crate::terminal_session::TerminalSessionView::Native(
            crate::terminal_session::NativeSessionView::notification_test_view(),
        )),
    ));
    watcher.close();
    server.finish().unwrap();
}

#[test]
fn unchanged_reload_reports_restart_without_reapplying_the_existing_snapshot() {
    use orchbee_workbench_protocol::{
        ConfigurationRestart, ConfigurationResult, WorkbenchResponse, write_workbench_response,
    };
    let server = FakeServer::start_connections(2, move |index, mut stream| {
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
        let WorkbenchRequest::Configuration(current) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("configuration current");
        };
        assert_eq!(current.action, ConfigurationAction::Current);
        let mut baseline = snapshot(1, "[keys]\nprefix='ctrl+b'");
        baseline.restart_required = vec![ConfigurationRestart::ResumeAgents];
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Configuration(ConfigurationResult {
                protocol_version: ProtocolVersion,
                request_id: current.request_id,
                site_id: current.site_id,
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
                site_id: reload.site_id,
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
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let mut watcher = Watcher::spawn(
        client.origin(),
        client.operation_clock(),
        SiteId::parse(&format!("{:032x}", 2)).unwrap(),
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
    view.commit(prefix, SessionActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    assert!(watcher.reload());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while view.notice() != Some(Notice::ConfigurationRestart) {
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
    let server = FakeServer::start_connections(2, move |index, mut stream| {
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
                site_id: current.site_id,
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
                site_id: reload.site_id,
                outcome: ConfigurationOutcome::Reloaded {
                    snapshot: snapshot(2, "[keys]\nprefix='ctrl+x'"),
                    changed: false,
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0])?, 0);
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
    assert_eq!(
        watcher
            .take_notification_update()
            .map(|(revision, _)| revision),
        Some(2)
    );
    assert!(!watcher.tick(&mut view));
    assert!(watcher.take_notification_update().is_none());
    watcher.close();
    server.finish().unwrap();
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
    let (poll_tx, poll_rx) = mpsc::sync_channel(1);
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
            let WorkbenchRequest::Configuration(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("configuration poll");
            };
            assert_eq!(request.action, ConfigurationAction::Current);
            poll_tx.send(()).unwrap();
            // Hold the next poll pending until close cancels it. UI scheduling need not
            // beat the polling interval, and recovery must never resubmit the reload.
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
    poll_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut view = ClientView::new(120);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while watcher.revision != Some(2) {
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
        revision: None,
        applied_snapshot: None,
        notification_update: None,
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
    assert_eq!(watcher.revision, Some(1));
    assert_eq!(view.bindings(), &keys);
    assert!(view.notifications.card_delivery());
}
