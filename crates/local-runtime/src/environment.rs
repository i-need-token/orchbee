//! This build's environment. The compiled artifact stem names the environment (ORC-382) and its
//! [data root](crate::DataRoot), `<home>/.<stem>`, which derives every state location; this owner
//! adds the routing rule that keeps one environment's terminals from reaching another's Session.
use std::ffi::OsString;

/// The variable through which a managed terminal names the environment whose Session it routes to.
pub const ENVIRONMENT_VARIABLE: &str = "ORCHBEE_ENVIRONMENT";
/// Inherited Session routing that only the naming environment may follow.
const ROUTE_VARIABLES: [&str; 2] = ["ORCHBEE_RUNTIME_BASE", "ORCHBEE_RUNTIME_SCOPE"];

const RELEASE_STEM: &str = "orchbee";

/// The stem compiled into this build on every platform. Staging sets `ORCHBEE_ARTIFACT_STEM` while
/// it compiles the cohort; a build without it is this checkout's test environment,
/// `ob-test-<checkout digest>`, so only a build that names `orchbee` is release. `build.rs`
/// resolves the stem.
const ENVIRONMENT_STEM: &str = env!("ORCHBEE_ENVIRONMENT_STEM");
// A malformed stem would name an environment no staging produces, so it fails the build instead.
const _: () = assert!(
    valid_stem(ENVIRONMENT_STEM.as_bytes()),
    "ORCHBEE_ARTIFACT_STEM must be orchbee, ob-dev-<16 lowercase hex>, or ob-test-<16 lowercase hex>"
);

/// This build's environment stem: `orchbee` for release, otherwise `ob-dev-<digest>` or
/// `ob-test-<digest>`. It is fixed at compile time; no runtime input changes it. Windows cohort
/// file names derive from it, while macOS and Linux cohorts keep the shipped `orchbee` names until
/// ORC-328, so a rule about file names asks [`CohortArtifact`](crate::CohortArtifact) instead.
pub const fn environment_stem() -> &'static str {
    ENVIRONMENT_STEM
}

/// This build's environment as the connection handshake compares it: the compiled stem, which every
/// negotiated connection requires to equal its peer's.
pub fn environment_name() -> orchbee_connection_handshake::EnvironmentName {
    orchbee_connection_handshake::EnvironmentName::new(ENVIRONMENT_STEM)
        .expect("a valid environment stem follows the handshake name grammar")
}

/// Whether this build is the release environment, the only one that may act on the release
/// installation or use the shipped state locations. It is a constant expression, so a caller can
/// choose compiled data by environment.
pub const fn is_release() -> bool {
    let stem = environment_stem().as_bytes();
    let release = RELEASE_STEM.as_bytes();
    if stem.len() != release.len() {
        return false;
    }
    let mut index = 0;
    while index < stem.len() {
        if stem[index] != release[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The three environments one machine runs side by side, as the TUI and Desktop mark them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentKind {
    /// `orchbee`, which shows no marker.
    Release,
    /// `ob-dev-<digest>`: `pnpm desktop` and default native staging.
    Development,
    /// `ob-test-<digest>`: the packaged test build and every build that names no stem.
    Test,
}

/// The kind of this build's environment, read from its compiled stem.
pub const fn environment_kind() -> EnvironmentKind {
    match ENVIRONMENT_STEM.as_bytes() {
        [b'o', b'b', b'-', b'd', b'e', b'v', b'-', ..] => EnvironmentKind::Development,
        [b'o', b'b', b'-', b't', b'e', b's', b't', b'-', ..] => EnvironmentKind::Test,
        _ => EnvironmentKind::Release,
    }
}

/// The checkout directory name `build.rs` compiled in, empty when the stem names no checkout.
const CHECKOUT_NAME: &str = env!("ORCHBEE_CHECKOUT_NAME");

/// The final directory name of the checkout this build came from, when its stem carries that
/// checkout's digest: a development build, or a test build that named no stem. Release and the
/// packaged test build, whose digest names a home, have none.
pub const fn checkout_name() -> Option<&'static str> {
    if CHECKOUT_NAME.is_empty() {
        None
    } else {
        Some(CHECKOUT_NAME)
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

/// Whether `stem` names an environment some build can compile: `orchbee`, `ob-dev-<16 lowercase
/// hex>`, or `ob-test-<16 lowercase hex>`. A rule about another installation's executable asks this
/// instead of restating the grammar.
pub const fn valid_environment_stem(stem: &str) -> bool {
    valid_stem(stem.as_bytes())
}

/// Mirrors `ARTIFACT_STEM_PATTERN` in `scripts/stage-native-windows.mjs`.
const fn valid_stem(stem: &[u8]) -> bool {
    let digest = match stem {
        b"orchbee" => return true,
        [b'o', b'b', b'-', b'd', b'e', b'v', b'-', digest @ ..]
        | [b'o', b'b', b'-', b't', b'e', b's', b't', b'-', digest @ ..] => digest,
        _ => return false,
    };
    if digest.len() != 16 {
        return false;
    }
    let mut index = 0;
    while index < digest.len() {
        if !matches!(digest[index], b'0'..=b'9' | b'a'..=b'f') {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests;
