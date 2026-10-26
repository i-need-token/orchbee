//! Adapts the existing default-shell capability without introducing another launch policy.
use orchbee_local_runtime::{DefaultInteractiveLaunch, DefaultInteractiveLaunchError};

pub(super) fn resolve() -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    #[cfg(windows)]
    {
        orchbee_local_runtime::resolve_default_interactive_launch(
            &orchbee_local_runtime::ShellSettings::default(),
        )
    }
    #[cfg(not(windows))]
    {
        orchbee_local_runtime::resolve_default_interactive_launch()
    }
}
