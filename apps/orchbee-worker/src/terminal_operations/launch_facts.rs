//! Resolves literal per-launch cwd, environment, program, and terminal profile facts for
//! the Host adapter on the current platform. It does not issue authorization, start a Host, or
//! submit a terminal creation request.

use std::{error::Error, io};

#[cfg(all(test, windows))]
mod tests;

#[cfg(windows)]
mod powershell;

#[cfg(unix)]
use nix::unistd::{Uid, User};
use orchbee_capability_node_protocol::{
    AbsoluteCwd, AuthorizedTerminalLaunch, DefaultLaunchProgram, TerminalLaunchPreferences,
};
use orchbee_terminal_protocol::{
    CreationId, CursorShape, DeviceAttributes, LaunchEnvironmentEntry, TerminalProfile,
    TerminalTheme,
};

use crate::{
    default_interactive_launch::{SystemShell, default_launch_environment},
    terminal_host_launch_adapter::ResolvedTerminalLaunchFacts,
    worker_identity::fresh_identity,
};

pub(super) fn resolve(
    launch: &AuthorizedTerminalLaunch,
    preferences: Option<&TerminalLaunchPreferences>,
    system_shell: &SystemShell,
    host_colors: orchbee_local_runtime::UserTerminalColors,
) -> Result<ResolvedTerminalLaunchFacts, Box<dyn Error>> {
    let selection = launch.launch();
    let plugin_environment = launch.plugin_environment();
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
        // The Session supplies only the selected profile; this Worker generation's single System
        // shell resolution supplies the shell.
        None => match preferences.and_then(TerminalLaunchPreferences::program) {
            Some(DefaultLaunchProgram::Custom(program)) => (
                program.executable().to_owned(),
                program.arguments().to_vec(),
            ),
            Some(&DefaultLaunchProgram::SystemShell { login }) => {
                let mut launch = system_shell.launch()?;
                if login {
                    launch.arguments.push("-l".into());
                }
                (launch.executable, launch.arguments)
            }
            None => {
                let launch = system_shell.launch()?;
                (launch.executable, launch.arguments)
            }
        },
    };
    // Windows editor launches keep their literal argument list.
    #[cfg(windows)]
    let arguments = if launch.editor_source().is_some() {
        arguments
    } else {
        powershell::interactive_arguments(&executable, arguments)
    };
    // An explicit authorized path requires no home-directory lookup or translation.
    let cwd = match launch.cwd() {
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
        launch_environment(launch.environment(), plugin_environment),
        cwd,
        terminal_profile(
            preferences
                .and_then(TerminalLaunchPreferences::colors)
                .map_or(host_colors, |colors| {
                    orchbee_local_runtime::UserTerminalColors::with_scheme(
                        colors.foreground,
                        colors.background,
                        colors.dark,
                    )
                }),
        ),
        preferences.map(
            |preferences| orchbee_terminal_protocol::TerminalConfiguration {
                scrollback_bytes: preferences.scrollback_bytes(),
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

// The effective default colors choose the scheme, cursor, and palette through the terminal theme;
// the cursor style, device attributes, and query responses are the same for every launch.
fn terminal_profile(colors: orchbee_local_runtime::UserTerminalColors) -> TerminalProfile {
    let TerminalTheme {
        color_scheme,
        default_background_rgb,
        default_cursor_rgb,
        default_foreground_rgb,
        palette,
    } = crate::terminal_theme::effective(colors);
    TerminalProfile {
        color_scheme,
        default_background_rgb,
        default_cursor_blinking: false,
        default_cursor_rgb,
        default_cursor_shape: CursorShape::Block,
        default_foreground_rgb,
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
