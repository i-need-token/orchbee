//! Whether this build runs automatic updates: the one switch that every automatic-update behavior a
//! release build would show or run consults, so that release builds turn them on together, and the
//! spelling of the helper the Session Server runs. It decides nothing else; each behavior keeps its
//! own rule for development builds, platforms and settings. It also spells the helpers other
//! processes run a Client with, each in both spellings a release may read.
use crate::{EnvironmentKind, environment_kind};
use orchbee_release_records::CrossBuildName;

/// The one argument with which a Session Server runs the store's active Client for an automatic
/// update, in the spelling of the active version. Neither spelling ever changes; a changed helper
/// takes a new name.
pub const AUTOMATIC_UPDATE_HELPER: CrossBuildName = CrossBuildName {
    current: "--automatic-update",
    earlier: "--automatic-update-v1",
};

/// The argument with which a launcher, after a Client of the moved version failed, runs the
/// previous version's Client to record that failure, in the previous version's spelling and
/// followed by the failed version. Neither spelling ever changes; a changed helper takes a new
/// name.
pub const CLIENT_FAILURE_HELPER: CrossBuildName = CrossBuildName {
    current: "--record-client-failure",
    earlier: "--record-client-failure-v1",
};

/// The one argument with which Setup or install.sh runs the Client bundled in a window directory
/// to install that window version into the store. Neither spelling ever changes; a changed helper
/// takes a new name.
pub const WINDOW_INSTALLER_HELPER: CrossBuildName = CrossBuildName {
    current: "--installer-window",
    earlier: "--installer-window-v1",
};

/// The variable through which a launcher announces the relaunch protocol to the Client it starts:
/// the launcher's own process id in decimal, set in both spellings. A Client uses the protocol's
/// statuses only when either names its parent, so a value another process inherits does nothing,
/// and a process that passes its environment on removes both. Neither spelling ever changes.
pub const RELAUNCH_PROTOCOL_VARIABLE: CrossBuildName = CrossBuildName {
    current: "ORCHBEE_RELAUNCH",
    earlier: "ORCHBEE_RELAUNCH_V1",
};

/// Whether release builds run automatic updates, true from the automatic-updates proposal's slice 1
/// on. While it is false, a release build neither shows nor runs any automatic-update behavior. The
/// npm launcher keeps the same value as `automaticUpdatesInReleaseBuilds`, and a change sets both.
const IN_RELEASE_BUILDS: bool = true;

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

/// Whether release builds install window versions, true from the window self-update release on.
/// While it is false, a release build's `update` and automatic helper leave the window store alone,
/// and its Client does not know `--installer-window`.
const WINDOW_IN_RELEASE_BUILDS: bool = true;

/// Whether this build installs window versions: only where [`automatic_updates_live`] holds, and
/// in a release build only once `WINDOW_IN_RELEASE_BUILDS` is set. Test builds always do, and
/// development builds answer true as that switch does, since the window part keeps its own
/// development rule. It is a constant expression, so a caller may decide at compile time.
pub const fn window_updates_live() -> bool {
    automatic_updates_live()
        && match environment_kind() {
            EnvironmentKind::Release => WINDOW_IN_RELEASE_BUILDS,
            EnvironmentKind::Development | EnvironmentKind::Test => true,
        }
}
