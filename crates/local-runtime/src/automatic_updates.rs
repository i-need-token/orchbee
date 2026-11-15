//! Whether this build runs automatic updates: the one switch that every automatic-update behavior a
//! release build would show or run consults, so that release builds turn them on together, and the
//! spelling of the helper the Session Server runs. It decides nothing else; each behavior keeps its
//! own rule for development builds, platforms and settings.
use crate::{EnvironmentKind, environment_kind};

/// The one argument with which a Session Server runs the store's active Client for an automatic
/// update. Its spelling never changes; a changed helper takes a new name.
pub const AUTOMATIC_UPDATE_HELPER: &str = "--automatic-update-v1";

/// Whether release builds run automatic updates. While it is false, a release build neither shows
/// nor runs any automatic-update behavior. The npm launcher keeps the same value as
/// `automaticUpdatesInReleaseBuilds`, and a change sets both.
const IN_RELEASE_BUILDS: bool = false;

/// Whether this build runs automatic-update behaviors. Test builds always do, so tests and
/// rehearsals drive them; release builds do only once `IN_RELEASE_BUILDS` is set. Development
/// builds answer true because this switch does not gate them: each behavior applies its own
/// development rule, such as never scheduling an update and never showing Update. It is a constant
/// expression, so a caller may decide at compile time.
pub const fn automatic_updates_live() -> bool {
    match environment_kind() {
        EnvironmentKind::Release => IN_RELEASE_BUILDS,
        EnvironmentKind::Development | EnvironmentKind::Test => true,
    }
}
