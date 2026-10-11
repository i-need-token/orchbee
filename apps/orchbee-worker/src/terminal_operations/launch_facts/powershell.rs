//! Process-local cwd synchronization for recognized interactive Windows shells.

use orchbee_local_site_runtime::WINDOWS_DIRECTORY_PROMPT as PROMPT;

pub(super) fn interactive_arguments(executable: &str, mut arguments: Vec<String>) -> Vec<String> {
    let name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    if !["powershell", "powershell.exe", "pwsh", "pwsh.exe"]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
        || arguments.len() > 3
        || arguments.iter().any(|arg| {
            !["-NoLogo", "-NoProfile", "-NoExit"]
                .iter()
                .any(|flag| arg.eq_ignore_ascii_case(flag))
        })
    {
        return arguments;
    }
    if !arguments
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-NoExit"))
    {
        arguments.push("-NoExit".to_owned());
    }
    arguments.extend(["-Command".to_owned(), PROMPT.to_owned()]);
    arguments
}

#[cfg(test)]
mod tests;
