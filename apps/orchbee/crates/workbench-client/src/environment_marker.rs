//! The build environment the sidebar marks (ORC-382): development or test, with the checkout a
//! checkout build came from. Release shows no marker. The Shell samples it once from the compiled
//! stem; pure UI traces carry none unless they set one.

/// A non-release environment, the only kind with a marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MarkedEnvironment {
    Development,
    Test,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EnvironmentMarker {
    pub(crate) environment: MarkedEnvironment,
    /// The checkout directory's name, for a build whose stem carries that checkout's digest.
    pub(crate) checkout: Option<&'static str>,
}

impl EnvironmentMarker {
    /// This build's marker; `None` for release.
    pub(crate) const fn for_build() -> Option<Self> {
        let environment = match orchbee_local_runtime::environment_kind() {
            orchbee_local_runtime::EnvironmentKind::Release => return None,
            orchbee_local_runtime::EnvironmentKind::Development => MarkedEnvironment::Development,
            orchbee_local_runtime::EnvironmentKind::Test => MarkedEnvironment::Test,
        };
        Some(Self {
            environment,
            checkout: orchbee_local_runtime::checkout_name(),
        })
    }
}
