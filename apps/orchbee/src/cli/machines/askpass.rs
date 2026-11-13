//! The askpass helper `ssh` starts for a login question, and its private pairing with the machine
//! hub: the variable that selects it, the pairing that variable carries, and the one record the
//! helper hands the hub. The answer itself never takes this way; it reaches the helper over the
//! question's own endpoint. `docs/machines.md#askpass-helper` owns the rules.

#[cfg(windows)]
mod helper;
#[cfg(windows)]
mod pairing;
pub(crate) mod question;

#[cfg(windows)]
pub(crate) use pairing::{
    Pairing, RAISE_HEADER_BYTES, Raise, TAKEN, TICKET_BYTES, VARIABLE, hex, unhex,
};

#[cfg(test)]
mod tests;

/// Runs the askpass helper when this start is one: the variable is set and the one argument, the
/// prompt, does not begin with `--`. `None` lets every other start parse its command line.
#[cfg(windows)]
pub(crate) fn run_if_selected() -> Option<std::process::ExitCode> {
    let pairing = std::env::var_os(VARIABLE)?;
    let prompt = selected_prompt(std::env::args_os().skip(1).collect())?;
    Some(helper::run(&prompt, &pairing))
}

/// The prompt of an askpass start: exactly one argument, never one of this program's options, so
/// a far end the stand-in starts with `--machine-link-v1` stays the link helper.
#[cfg(windows)]
fn selected_prompt(mut arguments: Vec<std::ffi::OsString>) -> Option<std::ffi::OsString> {
    let prompt = arguments.pop()?;
    let option = prompt.to_str().is_some_and(|text| text.starts_with("--"));
    (arguments.is_empty() && !option).then_some(prompt)
}
