use super::fixture::{Effect, Fixture};
use orchbee_workbench_protocol::*;
use std::sync::atomic::Ordering;

pub(super) fn enter_isolated_catalog() -> bool {
    const KEY: &str = "ORCHBEE_OVERLAY_SERVER_FIXTURE";
    if let Some(root) = std::env::var_os(KEY) {
        orchbee_local_runtime::PluginRegistry::from_environment(|name| std::env::var_os(name))
            .unwrap()
            .mutate(orchbee_local_runtime::PluginRegistryMutation::Link {
                path: std::path::PathBuf::from(root).join("orchbee-plugin.toml"),
                enabled: true,
            })
            .unwrap();
        return true;
    }
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let path = self.0.canonicalize().unwrap();
            assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("orchbee-overlay-observe-")
            );
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = Root(std::env::temp_dir().join(format!(
        "orchbee-overlay-observe-{:032x}",
        u128::from_be_bytes(nonce)
    )));
    std::fs::create_dir(&root.0).unwrap();
    std::fs::write(root.0.join("orchbee-plugin.toml"), "id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\ncommand=['pane.exe']\n").unwrap();
    std::fs::write(
        root.0.join("pane.exe"),
        b"admission fixture; never executed",
    )
    .unwrap();
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture"])
        .env(KEY, &root.0)
        .env("APPDATA", root.0.join("appdata"))
        .env("USERPROFILE", root.0.join("appdata"))
        .env_remove("ORCHBEE_CONFIG_FILE");
    let result = orchbee_local_runtime::run_windows_command(
        command,
        Some(std::time::Instant::now() + std::time::Duration::from_secs(30)),
        || Ok(false),
    );
    if result.failure == Some(orchbee_local_runtime::CommandFailure::PhysicalUnknown) {
        eprintln!(
            "Codex owns unresolved overlay child fixture: {}",
            root.0.display()
        );
        std::mem::forget(root);
    }
    assert_eq!(
        result.failure,
        None,
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    false
}

fn observation(fixture: &Fixture) -> PluginRequest {
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let pane = super::fixture::read_snapshot(&fixture.service).terminal_contexts[0]
        .tabs
        .iter()
        .find_map(|tab| match &tab.root {
            PanelNode::Panel {
                primary_presentation,
                panel_id,
                ..
            } if primary_presentation.binding.as_ref() == Some(&target.binding) => {
                Some(panel_id.clone())
            }
            _ => None,
        })
        .unwrap();
    let PluginOutcome::PaneFound { pane, .. } = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .plugin_pane(&pane)
        .unwrap()
    else {
        panic!("plugin membership");
    };
    PluginRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 90_001)).unwrap(),
        session_id: target.session_id,
        action: PluginAction::PaneObserve { pane },
    }
}

#[test]
fn plugin_overlay_observation_releases_the_exact_temporary_viewer_before_settlement() {
    if !enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::with_plugin_panes(true);
    let request = observation(&fixture);
    let result = fixture.service.execute_plugin(request.clone());
    assert!(result.answers(&request));
    assert!(matches!(
        result.outcome,
        PluginOutcome::PaneObserved { settled: true, .. }
    ));
    let effects = fixture.barrier().unwrap();
    let [
        Effect::Attach(a, _),
        Effect::Lifecycle(b),
        Effect::Detach(c),
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn plugin_overlay_uncertain_cleanup_blocks_settlement_and_is_released_before_replacement() {
    if !enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::with_plugin_panes(true);
    let request = observation(&fixture);
    fixture.detach_refusals.store(1, Ordering::Release);
    assert!(matches!(
        fixture.service.execute_plugin(request.clone()).outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::PhysicalUnknown
        }
    ));
    let result = fixture.service.execute_plugin(request);
    assert!(matches!(
        result.outcome,
        PluginOutcome::PaneObserved { settled: true, .. }
    ));
    let effects = fixture.barrier().unwrap();
    let [
        Effect::Attach(a, _),
        Effect::Lifecycle(b),
        Effect::DetachRefused(c),
        Effect::Detach(d),
        Effect::Attach(e, _),
        Effect::Lifecycle(f),
        Effect::Detach(g),
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(c, d);
    assert_ne!(d, e);
    assert_eq!(e, f);
    assert_eq!(f, g);
}

#[test]
fn plugin_overlay_stale_membership_and_session_are_refused_before_any_worker_effect() {
    if !enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::with_plugin_panes(true);
    let mut request = observation(&fixture);
    let PluginAction::PaneObserve { pane } = &mut request.action else {
        unreachable!()
    };
    pane.presentation_id = TerminalPresentationId::parse(&format!("{:032x}", 99_001)).unwrap();
    assert!(matches!(
        fixture.service.execute_plugin(request.clone()).outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::Conflict
        }
    ));
    request.session_id = SessionId::parse(&format!("{:032x}", 99_002)).unwrap();
    assert!(matches!(
        fixture.service.execute_plugin(request).outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::SessionMismatch
        }
    ));
    assert!(fixture.barrier().unwrap().is_empty());
}
