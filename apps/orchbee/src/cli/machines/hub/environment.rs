//! The hub's one view of its environment, taken at start. Everything the hub reads comes from the
//! view. `docs/machines.md#environment` owns the rule, including what a child of the hub will
//! receive once the slice that starts `ssh` brings the one path children start through.

use std::{ffi::OsString, time::Duration};

const PREFIX: &str = "ORCHBEE_";
/// Kept unchanged: they select the data root, the store and the settings file every client shares.
const SHARED: [&str; 3] = ["ORCHBEE_HOME", "ORCHBEE_INSTALL_DIR", "ORCHBEE_CONFIG_FILE"];
/// Kept when it applies to this build, as a Session route does: the hub reads its Sessions there.
const RUNTIME_BASE: &str = "ORCHBEE_RUNTIME_BASE";
/// The development and test expiry override, in milliseconds.
const EXPIRY_OVERRIDE: &str = "ORCHBEE_MACHINE_HUB_EXPIRY_MS";

pub(super) struct Environment {
    variables: Vec<(String, String)>,
}

impl Environment {
    /// The view the hub keeps of this process's environment: every variable outside Orchbee's,
    /// the three that select shared folders, the runtime base when it applies to this build, and
    /// the expiry override in a development or test build. Names compare without letter case; a
    /// variable that is not valid Unicode is left out.
    pub(super) fn take() -> Self {
        let variables = std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect::<Vec<_>>();
        let applies =
            orchbee_local_runtime::session_route_environment(|name| lookup(&variables, name))(
                RUNTIME_BASE,
            )
            .is_some();
        let release = orchbee_local_runtime::is_release();
        let variables = variables
            .into_iter()
            .filter(|(name, _)| {
                let name = name.to_ascii_uppercase();
                !name.starts_with(PREFIX)
                    || SHARED.contains(&name.as_str())
                    || (name == RUNTIME_BASE && applies)
                    || (!release && name == EXPIRY_OVERRIDE)
            })
            .collect();
        Self { variables }
    }

    /// A variable of the view, its name compared without letter case.
    pub(super) fn get(&self, name: &str) -> Option<OsString> {
        lookup(&self.variables, name)
    }

    /// The development or test expiry override, from 1,000 to 600,000 ms; a release has none.
    pub(super) fn expiry(&self) -> Option<Duration> {
        if orchbee_local_runtime::is_release() {
            return None;
        }
        self.get(EXPIRY_OVERRIDE)?
            .to_str()?
            .parse::<u64>()
            .ok()
            .filter(|milliseconds| (1_000..=600_000).contains(milliseconds))
            .map(Duration::from_millis)
    }
}

/// The value of `name` in `variables`, comparing names without letter case.
fn lookup(variables: &[(String, String)], name: &str) -> Option<OsString> {
    variables
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| OsString::from(value))
}
