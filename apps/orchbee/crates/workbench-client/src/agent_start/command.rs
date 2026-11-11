//! Literal Windows launch text constructed before runtime admission.
use crate::agent_provider::{chooses_session, executable, launch_arguments, launch_session_option};

/// The provider's fixed launch arguments come first, then its launch session option with
/// `session`, when given and the provider has one; the caller's arguments follow them.
pub(super) fn launch(provider: &str, args: &[String], session: Option<&str>) -> Option<String> {
    let program = executable(provider)?;
    launch_program(program, &arguments(provider, args, session))
}

fn arguments(provider: &str, args: &[String], session: Option<&str>) -> Vec<String> {
    let session = session.and_then(|id| Some([launch_session_option(provider)?, id]));
    launch_arguments(provider)
        .iter()
        .chain(session.iter().flatten())
        .map(|arg| (*arg).to_owned())
        .chain(args.iter().cloned())
        .collect()
}

/// A fresh conversation id for one start of `provider` with the caller's `args`: a random
/// version-4 UUID in lowercase hyphenated form when the provider takes one at launch and `args`
/// choose no conversation, else none. Nothing when entropy is unavailable, which starts without
/// one rather than with a predictable id.
pub(super) fn launch_session(provider: &str, args: &[String]) -> Option<String> {
    if !takes_session(provider, args) {
        return None;
    }
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = format!("{:032x}", u128::from_be_bytes(bytes));
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

fn takes_session(provider: &str, args: &[String]) -> bool {
    launch_session_option(provider).is_some() && !chooses_session(provider, args)
}

/// Whether the launch text fits, with room for the id a start may add.
pub(super) fn fits(provider: &str, args: &[String]) -> bool {
    const SLOT: &str = "00000000-0000-4000-8000-000000000000";
    launch(
        provider,
        args,
        takes_session(provider, args).then_some(SLOT),
    )
    .is_some()
}

/// The native form has no caller arguments. Resolve the fixed provider without aliases,
/// preserving the caller's console and avoiding a multi-page encoded wrapper; a provider's
/// fixed launch arguments are plain words that need no quoting. PowerShell single-quotes the
/// session option and its id, so neither is read as a parameter or a number; cmd takes them bare,
/// where both are inert.
pub(super) fn interactive(
    provider: &str,
    powershell: bool,
    session: Option<&str>,
) -> Option<String> {
    let program = executable(provider)?;
    let command = if powershell {
        orchbee_local_runtime::windows_powershell_application(program)
    } else {
        program.to_owned()
    };
    let session = session
        .and_then(|id| Some([launch_session_option(provider)?, id]))
        .into_iter()
        .flatten()
        .map(|token| {
            if powershell {
                format!("'{token}'")
            } else {
                token.to_owned()
            }
        });
    Some(
        std::iter::once(command)
            .chain(
                launch_arguments(provider)
                    .iter()
                    .map(|arg| (*arg).to_owned()),
            )
            .chain(session)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn launch_program(program: &str, args: &[String]) -> Option<String> {
    let encoded = orchbee_local_runtime::windows_encoded_command(program, args)?;
    let text = format!("powershell.exe -NoLogo -NoProfile -EncodedCommand {encoded}");
    // This text is typed into an existing shell; retain cmd.exe's smaller limit.
    (text.len() <= 8000).then_some(text)
}

#[cfg(test)]
mod tests;
