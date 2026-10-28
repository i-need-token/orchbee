//! Adapts the existing default-shell capability without introducing another launch policy.
use orchbee_local_runtime::{DefaultInteractiveLaunch, DefaultInteractiveLaunchError};

pub(super) fn resolve() -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    orchbee_local_runtime::resolve_default_interactive_launch()
}
