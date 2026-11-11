//! Literal Windows launch text constructed before runtime admission.
use crate::agent_provider::{executable, launch_arguments};

/// The provider's fixed launch arguments come first; the caller's arguments follow them.
pub(super) fn launch(provider: &str, args: &[String]) -> Option<String> {
    let program = executable(provider)?;
    let args: Vec<String> = launch_arguments(provider)
        .iter()
        .map(|arg| (*arg).to_owned())
        .chain(args.iter().cloned())
        .collect();
    launch_program(program, &args)
}

/// The native form has no caller arguments. Resolve the fixed provider without aliases,
/// preserving the caller's console and avoiding a multi-page encoded wrapper; a provider's
/// fixed launch arguments are plain words that need no quoting.
pub(super) fn interactive(provider: &str, powershell: bool) -> Option<String> {
    let program = executable(provider)?;
    let command = if powershell {
        format!(
            "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1)"
        )
    } else {
        program.to_owned()
    };
    Some(
        std::iter::once(command.as_str())
            .chain(launch_arguments(provider).iter().copied())
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
