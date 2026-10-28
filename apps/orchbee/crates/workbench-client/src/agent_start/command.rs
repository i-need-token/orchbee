//! Literal Windows launch text constructed before runtime admission.
use crate::agent_provider::executable;

pub(super) fn launch(provider: &str, args: &[String]) -> Option<String> {
    launch_program(executable(provider)?, args)
}

/// The native form has no arguments. Resolve the fixed provider without aliases,
/// preserving the caller's console and avoiding a multi-page encoded wrapper.
pub(super) fn interactive(provider: &str, powershell: bool) -> Option<String> {
    let program = executable(provider)?;
    Some(if powershell {
        format!(
            "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1)"
        )
    } else {
        program.to_owned()
    })
}

fn launch_program(program: &str, args: &[String]) -> Option<String> {
    let encoded = orchbee_local_runtime::windows_encoded_command(program, args)?;
    let text = format!("powershell.exe -NoLogo -NoProfile -EncodedCommand {encoded}");
    // This text is typed into an existing shell; retain cmd.exe's smaller limit.
    (text.len() <= 8000).then_some(text)
}

#[cfg(test)]
mod tests;
