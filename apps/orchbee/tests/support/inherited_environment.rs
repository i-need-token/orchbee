//! Keeps the shell that runs Cargo out of every Orchbee process a test starts. An inherited
//! `ORCHBEE_*` variable, such as `ORCHBEE_HOME` or `ORCHBEE_RUNTIME_BASE`, would move that
//! process's state or Session away from the fixture, so a fixture starts from none of them and sets
//! only what its test means to.

use std::{ffi::OsStr, process::Command};

/// `program`, an Orchbee executable or a shell that starts one, without any `ORCHBEE_*` variable
/// this test process inherited. Names compare without letter case, as Windows compares them.
pub fn orchbee_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    for (name, _) in std::env::vars_os() {
        if name
            .to_str()
            .and_then(|name| name.get(.."ORCHBEE_".len()))
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("ORCHBEE_"))
        {
            command.env_remove(name);
        }
    }
    command
}
