//! This build's environment. The compiled artifact stem names the environment (ORC-382) and its
//! [data root](crate::DataRoot), `<home>/.<stem>`, which derives every state location; this owner
//! adds the routing rule that keeps one environment's terminals from reaching another's Session.
use std::ffi::OsString;

#[cfg(windows)]
mod windows;

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
/// installation or use the shipped state locations. POSIX cohorts keep the release stem until
/// ORC-328, so this is true there; a rule that applies only on Windows gates on the platform too.
pub fn is_release() -> bool {
    environment_stem() == RELEASE_STEM
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
