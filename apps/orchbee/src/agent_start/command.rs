//! Literal Windows launch text constructed before runtime admission.
use crate::agent_provider::executable;

pub(super) fn launch(provider: &str, args: &[String]) -> Option<String> {
    launch_program(executable(provider)?, args)
}

fn launch_program(program: &str, args: &[String]) -> Option<String> {
    let encoded = orchbee_local_site_runtime::windows_encoded_command(program, args)?;
    let text = format!("powershell.exe -NoLogo -NoProfile -EncodedCommand {encoded}");
    // This text is typed into an existing shell; retain cmd.exe's smaller limit.
    (text.len() <= 8000).then_some(text)
}

#[cfg(test)]
mod tests;
