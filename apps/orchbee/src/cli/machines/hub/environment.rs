//! The hub's one view of its environment, taken at start, and the one path every child of the hub
//! starts through. Everything the hub reads comes from the view; a child receives the view without
//! any Orchbee variable, plus only that child's own variables. `docs/machines.md#environment` owns
//! the rule.

use std::{
    ffi::{OsStr, OsString},
    io,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

const PREFIX: &str = "ORCHBEE_";
/// Kept unchanged: they select the data root, the store and the settings file every client shares.
const SHARED: [&str; 3] = ["ORCHBEE_HOME", "ORCHBEE_INSTALL_DIR", "ORCHBEE_CONFIG_FILE"];
/// Kept when it applies to this build, as a Session route does: the hub reads its Sessions there.
const RUNTIME_BASE: &str = "ORCHBEE_RUNTIME_BASE";
/// The development and test expiry override, in milliseconds.
const EXPIRY_OVERRIDE: &str = "ORCHBEE_MACHINE_HUB_EXPIRY_MS";
/// In a development or test build, the absolute program the hub starts once after it binds.
const TEST_CHILD: &str = "ORCHBEE_MACHINE_HUB_TEST_CHILD";
/// A child runs without a console of its own, in its own process group.
const DETACHED: u32 = 0x0000_0008 | 0x0000_0200;

pub(super) struct Environment {
    variables: Vec<(String, String)>,
}

impl Environment {
    /// The view the hub keeps of this process's environment: every variable outside Orchbee's,
    /// the three that select shared folders, the runtime base when it applies to this build, and
    /// the hub's two test variables in a development or test build. Names compare without letter
    /// case; a variable that is not valid Unicode is left out.
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
                    || (!release && (name == EXPIRY_OVERRIDE || name == TEST_CHILD))
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

    /// Starts `program` with `arguments` and the view without any Orchbee variable, with
    /// `variables` added; a variable the child sets replaces one of the same name. The child has no console, null
    /// standard streams and the system folder as working directory. The hub holds no inheritable
    /// handle: its starter passed none, and every handle the hub opens is not inheritable.
    pub(super) fn start(
        &self,
        program: &Path,
        arguments: &[&OsStr],
        variables: &[(String, String)],
    ) -> io::Result<Child> {
        let directory = winsafe::GetSystemDirectory().map_err(io::Error::other)?;
        Command::new(program)
            .args(arguments)
            .env_clear()
            .envs(
                self.variables
                    .iter()
                    .filter(|(name, _)| {
                        !name.to_ascii_uppercase().starts_with(PREFIX)
                            && !variables
                                .iter()
                                .any(|(own, _)| own.eq_ignore_ascii_case(name))
                    })
                    .chain(variables)
                    .cloned(),
            )
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(DETACHED)
            .spawn()
    }

    /// Starts the program `ORCHBEE_MACHINE_HUB_TEST_CHILD` names, with no argument, so a test can
    /// read what a child of the hub receives. A release never reads the variable; the hub neither
    /// waits for the child nor ends it.
    pub(super) fn start_test_child(&self) {
        if orchbee_local_runtime::is_release() {
            return;
        }
        let Some(program) = self
            .get(TEST_CHILD)
            .map(PathBuf::from)
            .filter(|program| program.is_absolute())
        else {
            return;
        };
        let _child = self.start(&program, &[], &[]);
    }
}

/// The value of `name` in `variables`, comparing names without letter case.
fn lookup(variables: &[(String, String)], name: &str) -> Option<OsString> {
    variables
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| OsString::from(value))
}
