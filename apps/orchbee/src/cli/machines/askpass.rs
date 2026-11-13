//! The askpass helper `ssh` starts for a login question, and its private pairing with the machine
//! hub: the variable that selects it, the pairing that variable carries, and the one record the
//! helper hands the hub. The answer itself never takes this way; it reaches the helper over the
//! question's own endpoint. `docs/machines.md#askpass-helper` owns the rules.

#[cfg(windows)]
mod helper;
#[cfg(windows)]
mod pairing;
pub(crate) use orchbee_workbench_client::application::machine_question as question;

#[cfg(windows)]
pub(crate) use pairing::{
    Asked, Pairing, RAISE_HEADER_BYTES, Raise, TAKEN, TICKET_BYTES, VARIABLE, hex, unhex,
};

use std::ffi::OsString;

#[cfg(test)]
mod tests;

/// The one start a development or test stand-in makes with the variable set that is not a
/// question: the far end it runs as the link helper.
const STAND_IN_ENTRY: &str = "--machine-link-v1";

/// What a start with the variable set is.
#[derive(Debug, Eq, PartialEq)]
enum Start {
    /// The askpass helper, asking this prompt.
    Helper(OsString),
    /// A development or test stand-in's far end, which parses its command line.
    StandIn,
}

/// Runs the askpass helper when the variable is set, whatever the arguments are, so text `ssh`
/// passes never reaches the command-line parser. `None` lets every other start parse its command
/// line: one without the variable, and a non-release stand-in's far end.
#[cfg(windows)]
pub(crate) fn run_if_selected() -> Option<std::process::ExitCode> {
    let pairing = std::env::var_os(VARIABLE)?;
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    match start(&arguments, orchbee_local_runtime::is_release()) {
        Start::Helper(prompt) => Some(helper::run(&prompt, &pairing)),
        Start::StandIn => None,
    }
}

/// The start `arguments` make with the variable set. The prompt is every argument joined by single
/// spaces, since `ssh` passes one and how a launcher splits it is not proven; only a build that is
/// not a release lets the stand-in's own entry through, alone.
fn start(arguments: &[OsString], release: bool) -> Start {
    if !release && matches!(arguments, [only] if only == STAND_IN_ENTRY) {
        return Start::StandIn;
    }
    let mut prompt = OsString::new();
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            prompt.push(" ");
        }
        prompt.push(argument);
    }
    Start::Helper(prompt)
}
