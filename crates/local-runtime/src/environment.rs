//! This build's environment and the state locations it owns. The compiled artifact stem names the
//! environment (ORC-382): release keeps the shipped locations, and a development or test
//! environment keeps all of its state under one data root, `<home>/.<stem>`. Callers derive every
//! Orchbee state location from here instead of spelling one.
use std::{ffi::OsString, path::PathBuf};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{create_state_root, is_state_root, open_state_root};
#[cfg(windows)]
pub use windows::{
    local_state_root, prepare_default_runtime_base, roaming_state_root, roaming_state_variable,
};

/// The variable through which a managed terminal names the environment whose Session it routes to.
pub const ENVIRONMENT_VARIABLE: &str = "ORCHBEE_ENVIRONMENT";
/// Inherited Session routing that only the naming environment may follow.
const ROUTE_VARIABLES: [&str; 2] = ["ORCHBEE_RUNTIME_BASE", "ORCHBEE_RUNTIME_SCOPE"];

const RELEASE_STEM: &str = "orchbee";

/// This build's environment stem: `orchbee` for release, otherwise `ob-dev-<digest>` or
/// `ob-test-<digest>`. It is fixed at compile time; no runtime input changes it, and cohort file
/// names derive from it.
pub fn environment_stem() -> &'static str {
    #[cfg(windows)]
    {
        windows::ARTIFACT_STEM
    }
    // POSIX cohorts keep the shipped names until ORC-328.
    #[cfg(not(windows))]
    {
        RELEASE_STEM
    }
}

/// Whether this build is the release environment, the only one that may act on the release
/// installation or use the shipped state locations.
#[cfg(windows)]
pub(crate) fn is_release() -> bool {
    environment_stem() == RELEASE_STEM
}

/// The runtime base this environment uses when `ORCHBEE_RUNTIME_BASE` does not name one. Absent
/// only when a non-release environment cannot resolve its home directory.
pub fn default_runtime_base(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows::default_runtime_base(&env)
    }
    #[cfg(not(windows))]
    {
        let _ = env;
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = PathBuf::from("/tmp");
        Some(base)
    }
}

/// The environment a Session route reader consults. When `ORCHBEE_ENVIRONMENT` names another
/// environment, inherited `ORCHBEE_RUNTIME_BASE` and `ORCHBEE_RUNTIME_SCOPE` read as unset, so a
/// build started in another environment's terminal reaches its own Session. Every other variable,
/// including `ORCHBEE_SESSION`, passes through, and without the tag nothing changes.
pub fn session_route_environment(
    env: impl Fn(&str) -> Option<OsString>,
) -> impl Fn(&str) -> Option<OsString> {
    let foreign = env(ENVIRONMENT_VARIABLE).is_some_and(|tag| tag != environment_stem());
    move |name| {
        if foreign && ROUTE_VARIABLES.contains(&name) {
            None
        } else {
            env(name)
        }
    }
}

#[cfg(test)]
mod tests;
