//! Platform capability admission is shared by both UI adapters before persistence. The rule itself
//! belongs to `config.toml` launch-profile admission, which judges this platform the same way.
use crate::user_settings::{current_platform, launch_command_admitted};
use orchbee_workbench_protocol::{PreferencesFailure, UserLaunchProfiles};

pub(super) fn validate(catalog: &UserLaunchProfiles) -> Result<(), PreferencesFailure> {
    if catalog
        .profiles
        .iter()
        .all(|profile| launch_command_admitted(&profile.command, current_platform()))
    {
        Ok(())
    } else {
        Err(PreferencesFailure::InvalidChanges)
    }
}
