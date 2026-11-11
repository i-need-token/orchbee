//! Capture the next terminal's launch intent from one confirmed set of setting values. The
//! device that executes the launch resolves the System shell; this capture never discovers a
//! program.
use super::{UnusableTerminalDefaults, UserColorThemes, UserTerminalColors};
use orchbee_workbench_protocol::{UserLaunchCommand, UserSettingsValues};

pub struct UserTerminalLaunchIntent {
    /// The default profile's command; absent when the invocation already owns an explicit
    /// executable and arguments.
    pub program: Option<UserLaunchCommand>,
    pub scrollback_bytes: u32,
    /// Frozen with the launch facts; absent when Theme or Host follows the System appearance.
    pub colors: Option<UserTerminalColors>,
    /// The slots whose theme supplies the cursor and ANSI colors for the polarity the terminal's
    /// default colors take, including colors that follow the Host.
    pub themes: UserColorThemes,
}

impl UserTerminalLaunchIntent {
    /// The intent `values` give. `select_default` takes the default profile's command; a catalog
    /// without that profile refuses the creation, and no default candidate is substituted.
    pub fn capture(
        values: &UserSettingsValues,
        select_default: bool,
    ) -> Result<Self, UnusableTerminalDefaults> {
        let program = if select_default {
            let catalog = &values.launch_profiles;
            let profile = catalog
                .profiles
                .iter()
                .find(|profile| profile.id == catalog.default_profile_id)
                .ok_or(UnusableTerminalDefaults)?;
            Some(profile.command.clone())
        } else {
            None
        };
        let themes = UserColorThemes::from_values(values);
        Ok(Self {
            program,
            scrollback_bytes: values.scrollback_bytes,
            colors: UserTerminalColors::resolve(
                values.application_theme,
                values.terminal_display_theme,
                &themes,
                &values.custom_foreground,
                &values.custom_background,
            )?,
            themes,
        })
    }
}
