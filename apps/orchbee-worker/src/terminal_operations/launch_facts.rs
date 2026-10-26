//! Resolves literal per-launch cwd, environment, shell policy, and terminal profile facts for
//! the Host adapter on the current platform. It does not issue authorization, start a Host, or
//! submit a terminal creation request.

use std::{error::Error, io};

#[cfg(all(test, windows))]
mod tests;

#[cfg(windows)]
mod powershell;

#[cfg(unix)]
use nix::unistd::{Uid, User};
use orchbee_capability_node_protocol::AbsoluteCwd;
use orchbee_terminal_protocol::{
    ColorScheme, CreationId, CursorShape, DeviceAttributes, LaunchEnvironmentEntry, TerminalProfile,
};

use crate::{
    default_interactive_launch::{default_launch_environment, resolve_default_interactive_launch},
    terminal_host_launch_adapter::ResolvedTerminalLaunchFacts,
    worker_identity::fresh_identity,
};

/// Captured Windows shell policy; editor launches keep their literal argument list.
#[cfg(windows)]
pub(super) struct ShellLaunchPolicy<'a> {
    pub(super) editor_launch: bool,
    pub(super) settings: &'a orchbee_local_runtime::ShellSettings,
}

pub(super) fn resolve(
    explicit_cwd: Option<&AbsoluteCwd>,
    selection: Option<&orchbee_capability_node_protocol::LaunchSelection>,
    environment: &orchbee_capability_node_protocol::LaunchEnvironment,
    plugin_environment: Option<&orchbee_capability_node_protocol::PluginTerminalEnvironment>,
    defaults: Option<&orchbee_local_runtime::UserTerminalLaunchDefaults>,
    host_colors: orchbee_local_runtime::UserTerminalColors,
    #[cfg(windows)] shell: ShellLaunchPolicy<'_>,
) -> Result<ResolvedTerminalLaunchFacts, Box<dyn Error>> {
    if plugin_environment.is_some() && (!cfg!(windows) || selection.is_none()) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "plugin terminal environment requires an explicit Windows launch",
        )
        .into());
    }
    // An authorized selection keeps its executable: no PATH search, no shell
    // resolution, and no silent fallback to the default launch. The default
    // environment entries are protocol-fixed, so an explicit selection does
    // not depend on a usable shell existing on this Worker.
    let (executable, arguments) = match selection {
        Some(selection) => (
            selection.executable().to_owned(),
            selection.arguments().to_vec(),
        ),
        None => {
            let launch = match defaults.and_then(|defaults| defaults.launch.as_ref()) {
                Some(launch) => launch.clone(),
                None => resolve_default_interactive_launch(
                    #[cfg(windows)]
                    shell.settings,
                )?,
            };
            (launch.executable, launch.arguments)
        }
    };
    #[cfg(windows)]
    let arguments = if shell.editor_launch {
        arguments
    } else {
        powershell::interactive_arguments(&executable, arguments)
    };
    // An explicit authorized path requires no home-directory lookup or translation.
    let cwd = match explicit_cwd {
        Some(cwd) => cwd.clone(),
        None => {
            let home_path = default_home()?;
            let home = home_path.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Worker home is not UTF-8")
            })?;
            AbsoluteCwd::new(home)?
        }
    };
    Ok(ResolvedTerminalLaunchFacts::new(
        CreationId::from_bytes(fresh_identity()?)?,
        executable,
        arguments,
        launch_environment(environment, plugin_environment),
        cwd,
        terminal_profile(
            defaults
                .and_then(|defaults| defaults.colors)
                .unwrap_or(host_colors),
        ),
        defaults.map(
            |defaults| orchbee_terminal_protocol::TerminalConfiguration {
                scrollback_bytes: defaults.scrollback_bytes,
            },
        ),
    ))
}

fn launch_environment(
    environment: &orchbee_capability_node_protocol::LaunchEnvironment,
    plugin: Option<&orchbee_capability_node_protocol::PluginTerminalEnvironment>,
) -> Vec<LaunchEnvironmentEntry> {
    let mut entries: Vec<_> = default_launch_environment()
        .into_iter()
        .map(|entry| LaunchEnvironmentEntry {
            name: entry.name,
            value: Some(entry.value),
        })
        .chain(
            environment
                .entries()
                .iter()
                .map(|(name, value)| LaunchEnvironmentEntry {
                    name: name.clone(),
                    value: Some(value.clone()),
                }),
        )
        .collect();
    if cfg!(windows) {
        for name in orchbee_capability_node_protocol::PLUGIN_TERMINAL_ENVIRONMENT_NAMES {
            let value = plugin
                .and_then(|plugin| plugin.entries().iter().find(|(key, _)| key == name))
                .map(|(_, value)| value.clone());
            entries.push(LaunchEnvironmentEntry {
                name: name.into(),
                value,
            });
        }
        // Action/link invocations can be ancestors of a newly started Session too.
        for name in [
            "ORCHBEE_PLUGIN_ACTION_ID",
            "ORCHBEE_PLUGIN_CLICKED_URL",
            "ORCHBEE_PLUGIN_LINK_HANDLER_ID",
        ] {
            entries.push(LaunchEnvironmentEntry {
                name: name.into(),
                value: None,
            });
        }
    }
    entries
}

// Only the default colors and their color scheme follow device preferences or Host colors.
// The ANSI palette remains the protocol profile's authority, independent of Client rendering.
fn terminal_profile(colors: orchbee_local_runtime::UserTerminalColors) -> TerminalProfile {
    let (foreground, background) = colors.rgb();
    let mut palette = vec![
        0x00_00_00, 0xcd_31_31, 0x10_7c_10, 0x94_98_00, 0x04_51_a5, 0xbc_05_bc, 0x05_98_bc,
        0x55_55_55, 0x66_66_66, 0xcd_31_31, 0x14_ce_14, 0xb5_ba_00, 0x04_51_a5, 0xbc_05_bc,
        0x05_98_bc, 0xa5_a5_a5,
    ];
    for red in [0, 95, 135, 175, 215, 255] {
        for green in [0, 95, 135, 175, 215, 255] {
            for blue in [0, 95, 135, 175, 215, 255] {
                palette.push((red << 16) | (green << 8) | blue);
            }
        }
    }
    for index in 0..24 {
        let value = 8 + index * 10;
        palette.push((value << 16) | (value << 8) | value);
    }
    TerminalProfile {
        color_scheme: if colors.dark() {
            ColorScheme::Dark
        } else {
            ColorScheme::Light
        },
        default_background_rgb: background,
        default_cursor_blinking: false,
        default_cursor_rgb: Some(0x57_be_f8),
        default_cursor_shape: CursorShape::Block,
        default_foreground_rgb: foreground,
        device_attributes: DeviceAttributes {
            primary_conformance_level: 62,
            primary_features: vec![22],
            secondary_device_type: 1,
            secondary_firmware_version: 0,
            secondary_rom_cartridge: 0,
            tertiary_unit_id: 0,
        },
        enquiry_response: String::new(),
        palette,
        version_response: "Orchbee".to_owned(),
    }
}

#[cfg(unix)]
fn default_home() -> io::Result<std::path::PathBuf> {
    Ok(User::from_uid(Uid::effective())?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Worker account is unavailable"))?
        .dir)
}
#[cfg(windows)]
fn default_home() -> io::Result<std::path::PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .ok_or(io::ErrorKind::NotFound)?;
    if !home.is_absolute() || !home.is_dir() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(home)
}
