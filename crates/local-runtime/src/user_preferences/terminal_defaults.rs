//! Resolve the next terminal on the device that will execute it, from one confirmed snapshot.
use super::{UserPreferencesStore, UserTerminalColors};
use crate::default_interactive_launch::{
    DefaultInteractiveLaunch, default_launch_environment, resolve_default_interactive_launch,
};
use orchbee_workbench_protocol::{PreferencesFailure, UserLaunchCommand};

pub struct UserTerminalLaunchDefaults {
    /// Absent when the invocation already owns an explicit executable and arguments.
    pub launch: Option<DefaultInteractiveLaunch>,
    pub scrollback_bytes: u32,
    /// Frozen with the launch facts; absent only for Host with System appearance.
    pub colors: Option<UserTerminalColors>,
}

impl UserPreferencesStore {
    /// A read or resolution failure refuses this creation; no default candidate is substituted.
    pub fn terminal_launch_defaults(
        &self,
        select_default: bool,
    ) -> Result<UserTerminalLaunchDefaults, PreferencesFailure> {
        let values = self.read()?.values;
        let launch = if select_default {
            let catalog = &values.launch_profiles;
            let profile = catalog
                .profiles
                .iter()
                .find(|profile| profile.id == catalog.default_profile_id)
                .ok_or(PreferencesFailure::InvalidSource)?;
            Some(match &profile.command {
                UserLaunchCommand::CustomExecutable {
                    executable,
                    arguments,
                } => DefaultInteractiveLaunch {
                    executable: executable.clone(),
                    arguments: arguments.clone(),
                    environment: default_launch_environment(),
                },
                UserLaunchCommand::SystemShell { login } => {
                    let mut launch = resolve_default_interactive_launch(
                        #[cfg(windows)]
                        &crate::ShellSettings::default(),
                    )
                    .map_err(|_| PreferencesFailure::Unavailable)?;
                    if *login {
                        launch.arguments.push("-l".into());
                    }
                    launch
                }
            })
        } else {
            None
        };
        Ok(UserTerminalLaunchDefaults {
            launch,
            scrollback_bytes: values.scrollback_bytes,
            colors: UserTerminalColors::resolve(
                values.application_theme,
                values.terminal_display_theme,
                &values.custom_foreground,
                &values.custom_background,
            )?,
        })
    }
}
