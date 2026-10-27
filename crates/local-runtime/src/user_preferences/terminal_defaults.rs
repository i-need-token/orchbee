//! Capture the next terminal's launch intent from one confirmed snapshot. The device that executes
//! the launch resolves the System shell; this capture never discovers a program.
use super::{UserPreferencesStore, UserTerminalColors};
use orchbee_workbench_protocol::{PreferencesFailure, UserLaunchCommand};

pub struct UserTerminalLaunchIntent {
    /// The default profile's command; absent when the invocation already owns an explicit
    /// executable and arguments.
    pub program: Option<UserLaunchCommand>,
    pub scrollback_bytes: u32,
    /// Frozen with the launch facts; absent only for Host with System appearance.
    pub colors: Option<UserTerminalColors>,
}

impl UserPreferencesStore {
    /// A read failure refuses this creation; no default candidate is substituted.
    pub fn terminal_launch_intent(
        &self,
        select_default: bool,
    ) -> Result<UserTerminalLaunchIntent, PreferencesFailure> {
        let values = self.read()?.values;
        let program = if select_default {
            let catalog = &values.launch_profiles;
            let profile = catalog
                .profiles
                .iter()
                .find(|profile| profile.id == catalog.default_profile_id)
                .ok_or(PreferencesFailure::InvalidSource)?;
            Some(profile.command.clone())
        } else {
            None
        };
        Ok(UserTerminalLaunchIntent {
            program,
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
