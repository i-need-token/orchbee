use super::*;
use crate::{
    terminal_authorization::{AuthorizedHostOperationKernel, HostOperationAdmission},
    terminal_host_launch_adapter::map_launch_dispatch,
};
use orchbee_capability_node_protocol::*;
use orchbee_local_site_runtime::WindowsShellSettings;
use std::num::NonZeroU64;

#[test]
fn configured_and_explicit_shells_reach_real_host_launch_preparation() -> Result<(), Box<dyn Error>>
{
    let powershell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let cwd = AbsoluteCwd::new(std::env::temp_dir().to_str().unwrap())?;
    let configured = WindowsShellSettings::new(powershell.to_str().unwrap(), "login")?;
    for (explicit, shared) in [(false, false), (true, false), (false, true), (true, true)] {
        let defaults = shared.then(|| orchbee_local_site_runtime::UserTerminalLaunchDefaults {
            launch: Some(orchbee_local_site_runtime::DefaultInteractiveLaunch {
                executable: "C:/shared/default.exe".into(),
                arguments: vec!["literal && shared value".into()],
                environment: default_launch_environment(),
            }),
            scrollback_bytes: 4321,
            colors: Some(orchbee_local_site_runtime::UserTerminalColors::DARK),
        });
        let selection = explicit.then(|| {
            LaunchSelection::new(
                "C:/explicit/program.exe".into(),
                vec!["literal & value".into()],
            )
            .unwrap()
        });
        let snapshot = if explicit {
            WindowsShellSettings::new("definitely-missing-shell.exe", "login")?
        } else {
            configured.clone()
        };
        let environment =
            LaunchEnvironment::new(vec![("USER_MARKER".into(), "literal $ value".into())])?;
        let plugin = explicit.then(|| {
            PluginTerminalEnvironment::new(
                PLUGIN_TERMINAL_ENVIRONMENT_NAMES[..7]
                    .iter()
                    .map(|name| ((*name).into(), "plugin value".into()))
                    .collect(),
            )
            .unwrap()
        });
        let facts = resolve(
            Some(&cwd),
            selection.as_ref(),
            &environment,
            plugin.as_ref(),
            defaults.as_ref(),
            orchbee_local_site_runtime::UserTerminalColors::LIGHT,
            WindowsShellLaunch {
                editor_launch: false,
                settings: &snapshot,
            },
        )?;
        let n = NonZeroU64::MIN;
        let authorization = TerminalOperationAuthorization::new(
            TerminalId::from_bytes([1; 16])?,
            OperationId::from_bytes([2; 16])?,
            WorkerId::from_bytes([3; 16])?,
            WorkerGeneration::new(n),
            TerminalFence::new(n),
            TerminalOperationLease::new(
                LeaseId::from_bytes([4; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(200).unwrap()),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        );
        let mut kernel = AuthorizedHostOperationKernel::new(
            orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
            n,
        )?;
        let request = AuthorizedTerminalLaunch::new(
            authorization,
            Some(cwd.clone()),
            TerminalLaunchMode::Deferred,
            selection,
        )
        .with_environment(environment)
        .with_plugin_environment(plugin)?;
        let HostOperationAdmission::Dispatch(dispatch) = kernel.admit_launch(request, 100)? else {
            panic!("dispatch")
        };
        let host = map_launch_dispatch(&dispatch, facts, None)?;
        assert_eq!(
            host.launch.terminal_profile.default_foreground_rgb,
            if shared { 0xd4d4d4 } else { 0x141414 }
        );
        assert_eq!(
            host.launch.terminal_profile.default_background_rgb,
            if shared { 0x1e1e1e } else { 0xfcfcfc }
        );
        assert_eq!(
            host.launch.terminal_profile.color_scheme,
            if shared {
                ColorScheme::Dark
            } else {
                ColorScheme::Light
            }
        );
        assert_eq!(
            host.launch
                .configuration
                .as_ref()
                .map(|value| value.scrollback_bytes),
            shared.then_some(4321)
        );
        assert_eq!(host.launch.current_directory.as_deref(), Some(cwd.as_str()));
        assert_eq!(
            host.launch
                .environment
                .iter()
                .find(|entry| entry.name == "ORCHBEE_PLUGIN_ID")
                .unwrap()
                .value
                .as_deref(),
            explicit.then_some("plugin value")
        );
        assert!(
            host.launch
                .environment
                .iter()
                .any(|entry| entry.name == "ORCHBEE_WORKSPACE_ID" && entry.value.is_none())
        );
        assert!(
            host.environment_overlay
                .iter()
                .any(|entry| entry.name == "ORCHBEE_TERMINAL_ID" && entry.value == "01".repeat(16))
        );
        assert!(
            host.environment_overlay
                .iter()
                .any(|entry| entry.name == "ORCHBEE_WORKER_ID" && entry.value == "03".repeat(16))
        );

        assert!(
            host.launch
                .environment
                .iter()
                .any(|entry| entry.name == "USER_MARKER"
                    && entry.value.as_deref() == Some("literal $ value"))
        );
        if explicit {
            assert_eq!(host.launch.executable, "C:/explicit/program.exe");
            assert_eq!(host.launch.arguments, ["literal & value"]);
        } else if shared {
            assert_eq!(host.launch.executable, "C:/shared/default.exe");
            assert_eq!(host.launch.arguments, ["literal && shared value"]);
        } else {
            assert_eq!(std::path::Path::new(&host.launch.executable), powershell);
            assert!(
                host.launch
                    .arguments
                    .iter()
                    .any(|argument| argument == "-Command")
            );
            assert!(
                !host
                    .launch
                    .arguments
                    .iter()
                    .any(|argument| argument == "-l")
            );
        }
    }
    Ok(())
}

#[test]
fn resolved_launch_colors_reach_the_profile_without_changing_the_ansi_palette() {
    use orchbee_local_site_runtime::UserTerminalColors;
    let baseline = terminal_profile(UserTerminalColors::LIGHT);
    for (colors, pair, scheme) in [
        (
            UserTerminalColors::DARK,
            (0xd4d4d4, 0x1e1e1e),
            ColorScheme::Dark,
        ),
        (
            UserTerminalColors::LIGHT,
            (0x141414, 0xfcfcfc),
            ColorScheme::Light,
        ),
        (
            UserTerminalColors::from_rgb(0xcccccc, 0x0c0c0c),
            (0xcccccc, 0x0c0c0c),
            ColorScheme::Dark,
        ),
    ] {
        let profile = terminal_profile(colors);
        assert_eq!(
            (
                profile.default_foreground_rgb,
                profile.default_background_rgb
            ),
            pair
        );
        assert_eq!(profile.color_scheme, scheme);
        assert_eq!(profile.palette, baseline.palette);
        assert_eq!(profile.default_cursor_rgb, baseline.default_cursor_rgb);
        assert_eq!(profile.device_attributes, baseline.device_attributes);
    }
}

#[test]
fn plugin_launch_environment_removes_inherited_display_keys_and_keeps_routing_owned() {
    let ordinary = LaunchEnvironment::new(vec![("MARKER".into(), "user".into())]).unwrap();
    let plugin = PluginTerminalEnvironment::new(
        PLUGIN_TERMINAL_ENVIRONMENT_NAMES[..7]
            .iter()
            .map(|name| ((*name).into(), "literal 中文".into()))
            .collect(),
    )
    .unwrap();
    for selected in [None, Some(&plugin)] {
        let result = launch_environment(&ordinary, selected);
        for name in [
            "ORCHBEE_PLUGIN_ACTION_ID",
            "ORCHBEE_PLUGIN_CLICKED_URL",
            "ORCHBEE_PLUGIN_LINK_HANDLER_ID",
        ] {
            assert!(
                result
                    .iter()
                    .any(|entry| entry.name == name && entry.value.is_none())
            );
        }
        for name in PLUGIN_TERMINAL_ENVIRONMENT_NAMES {
            let entries: Vec<_> = result.iter().filter(|entry| entry.name == name).collect();
            assert_eq!(entries.len(), 1);
            assert_eq!(
                entries[0].value.as_deref(),
                selected
                    .and_then(|plugin| plugin.entries().iter().find(|(key, _)| key == name))
                    .map(|(_, value)| value.as_str())
            );
        }
        assert!(
            result
                .iter()
                .any(|entry| entry.name == "MARKER" && entry.value.as_deref() == Some("user"))
        );
        assert!(!result.iter().any(|entry| matches!(
            entry.name.as_str(),
            "ORCHBEE_RUNTIME_BASE"
                | "ORCHBEE_RUNTIME_SCOPE"
                | "ORCHBEE_WORKER_ID"
                | "ORCHBEE_TERMINAL_ID"
        )));
    }
}
