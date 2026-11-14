//! The hub's one view of its environment, taken at start. Everything the hub reads comes from the
//! view, and every carrier child receives the view less Orchbee's and the terminal's variables.
//! `docs/machines.md#environment` owns the rule.

use orchbee_workbench_client::application::machine_ssh::STAND_IN;
use std::{ffi::OsString, path::PathBuf, time::Duration};

const PREFIX: &str = "ORCHBEE_";
/// Kept unchanged: they select the data root, the store and the settings file every client shares.
const SHARED: [&str; 3] = ["ORCHBEE_HOME", "ORCHBEE_INSTALL_DIR", "ORCHBEE_CONFIG_FILE"];
/// Kept when it applies to this build, as a Session route does: the hub reads its Sessions there.
const RUNTIME_BASE: &str = "ORCHBEE_RUNTIME_BASE";
/// The development and test expiry override, in milliseconds.
const EXPIRY_OVERRIDE: &str = "ORCHBEE_MACHINE_HUB_EXPIRY_MS";
/// The development and test override of a question's bound, in milliseconds.
const QUESTION_OVERRIDE: &str = "ORCHBEE_MACHINE_HUB_QUESTION_MS";
/// Describe the terminal the client ran in, which a carrier child does not run in.
const TERMINAL: [&str; 6] = [
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "WT_SESSION",
    "WT_PROFILE_ID",
];

pub(super) struct Environment {
    variables: Vec<(String, String)>,
}

/// What replaces `ssh` in a development or test build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StandIn {
    /// None is named: carriers run the system `ssh`.
    None,
    /// An absolute program, run with `--machine-link-v1` alone.
    Program(PathBuf),
    /// A value that is not an absolute path: every attempt fails rather than run `ssh`.
    Unusable,
}

impl Environment {
    /// The view the hub keeps of this process's environment: every variable outside Orchbee's,
    /// the three that select shared folders, the runtime base when it applies to this build, and
    /// the expiry and question overrides and the `ssh` stand-in in a development or test build. Names compare
    /// without letter case; a variable that is not valid Unicode is left out.
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
                    || (!release
                        && [EXPIRY_OVERRIDE, QUESTION_OVERRIDE, STAND_IN].contains(&name.as_str()))
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

    /// The development or test bound of a question, from 1,000 to 120,000 ms; a release has none.
    pub(super) fn question(&self) -> Option<Duration> {
        if orchbee_local_runtime::is_release() {
            return None;
        }
        self.get(QUESTION_OVERRIDE)?
            .to_str()?
            .parse::<u64>()
            .ok()
            .filter(|milliseconds| (1_000..=120_000).contains(milliseconds))
            .map(Duration::from_millis)
    }

    /// The development or test `ssh` stand-in; a release has none.
    pub(super) fn stand_in(&self) -> StandIn {
        if orchbee_local_runtime::is_release() {
            return StandIn::None;
        }
        match self.get(STAND_IN).map(PathBuf::from) {
            None => StandIn::None,
            Some(path) if path.is_absolute() => StandIn::Program(path),
            Some(_) => StandIn::Unusable,
        }
    }

    /// What every carrier child receives: the view without any `ORCHBEE_*` variable and without
    /// the terminal's.
    pub(super) fn child(&self) -> Vec<(String, String)> {
        self.variables
            .iter()
            .filter(|(name, _)| {
                let name = name.to_ascii_uppercase();
                !name.starts_with(PREFIX) && !TERMINAL.contains(&name.as_str())
            })
            .cloned()
            .collect()
    }
}

/// The value of `name` in `variables`, comparing names without letter case.
fn lookup(variables: &[(String, String)], name: &str) -> Option<OsString> {
    variables
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| OsString::from(value))
}
