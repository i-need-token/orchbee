//! Literal launch text for the shell that types it, constructed before runtime admission.
use crate::agent_provider::{chooses_session, executable, launch_arguments, launch_session_option};
use orchbee_agent_observation_protocol::AgentRunToken;

/// The shell a start's line is typed into, as the Host's launch record names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shell {
    /// Managed PowerShell, which strips a run prefix and echoes its token.
    Powershell,
    /// A marked cmd.
    Cmd,
    /// Managed zsh or bash, which strip a run prefix and echo its token.
    Posix,
}

impl Shell {
    /// The family of line this platform's shells read, for the budget check before any effect:
    /// a Windows Host starts PowerShell or cmd, a macOS or Linux Host zsh or bash.
    pub(super) const NATIVE: Self = if cfg!(windows) {
        Self::Cmd
    } else {
        Self::Posix
    };
}

/// The provider's fixed launch arguments come first, then its launch session option with
/// `session`, when given and the provider has one; the caller's arguments follow them. Windows
/// shells take one literal-argv PowerShell launch; zsh and bash take the words themselves.
pub(super) fn launch(
    provider: &str,
    args: &[String],
    session: Option<&str>,
    shell: Shell,
) -> Option<String> {
    let program = executable(provider)?;
    let arguments = arguments(provider, args, session);
    match shell {
        Shell::Powershell | Shell::Cmd => launch_program(program, &arguments),
        Shell::Posix => orchbee_local_runtime::posix_command_line(program, &arguments)
            .filter(|text| text.len() <= TYPED_LIMIT),
    }
}

/// Launch text is typed into an existing shell; every shell keeps cmd.exe's smaller limit.
const TYPED_LIMIT: usize = 8000;

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

/// Whether this platform's launch text fits, with room for the id a start may add.
pub(super) fn fits(provider: &str, args: &[String]) -> bool {
    const SLOT: &str = "00000000-0000-4000-8000-000000000000";
    launch(
        provider,
        args,
        takes_session(provider, args).then_some(SLOT),
        Shell::NATIVE,
    )
    .is_some()
}

/// The Windows form without caller arguments. Resolve the fixed provider without aliases,
/// preserving the caller's console and avoiding a multi-page encoded wrapper; a provider's
/// fixed launch arguments are plain words that need no quoting. PowerShell single-quotes the
/// session option and its id, so neither is read as a parameter or a number; cmd takes them bare,
/// where both are inert. zsh and bash have no shorter form: their launch text is already the words.
pub(super) fn interactive(provider: &str, shell: Shell, session: Option<&str>) -> Option<String> {
    let powershell = match shell {
        Shell::Powershell => true,
        Shell::Cmd => false,
        Shell::Posix => return None,
    };
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
    (text.len() <= TYPED_LIMIT).then_some(text)
}

/// `text` as typed, marked with `run_token` for a shell that strips the mark: managed
/// PowerShell's `<#orchbee-run:<token>#> ` comment, or managed zsh's and bash's
/// `: orchbee-run:<token>; ` no-op (`orchbee.zsh`, `orchbee.bash`). Either shell runs the command
/// with that token, echoes it in the command's start and finish marks, and keeps neither form in
/// the person's history. A marked PowerShell without the managed wrapper reads its prefix as a
/// block comment, and zsh or bash without it run the no-op first; both then run the command
/// unchanged. cmd gets no mark.
pub(super) fn typed_line(text: &str, shell: Shell, run_token: Option<&AgentRunToken>) -> String {
    match (run_token, shell) {
        (Some(token), Shell::Powershell) => format!("<#orchbee-run:{token}#> {text}"),
        (Some(token), Shell::Posix) => format!(": orchbee-run:{token}; {text}"),
        (_, Shell::Powershell | Shell::Cmd | Shell::Posix) => text.to_owned(),
    }
}

#[cfg(test)]
mod tests;
