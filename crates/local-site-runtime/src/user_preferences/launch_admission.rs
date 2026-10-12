//! Platform capability admission is shared by both UI adapters before persistence.
use orchbee_workbench_protocol::{PreferencesFailure, UserLaunchCommand, UserLaunchProfiles};
use std::path::Path;

pub(super) fn validate(catalog: &UserLaunchProfiles) -> Result<(), PreferencesFailure> {
    for profile in &catalog.profiles {
        match &profile.command {
            UserLaunchCommand::CustomExecutable { executable, .. } => {
                if !Path::new(executable).is_absolute() {
                    return Err(PreferencesFailure::InvalidChanges);
                }
            }
            UserLaunchCommand::SystemShell { login } => {
                if !login_supported(*login) {
                    return Err(PreferencesFailure::InvalidChanges);
                }
            }
        }
    }
    Ok(())
}
#[cfg(windows)]
fn login_supported(requested: bool) -> bool {
    !requested
}
#[cfg(unix)]
fn login_supported(_requested: bool) -> bool {
    true
}
