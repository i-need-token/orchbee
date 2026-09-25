//! Process-local cwd synchronization for recognized interactive Windows shells.

// Invoke the original prompt first: preceding statements would overwrite `$?`
// on Windows PowerShell 5.1. Single-quoted constant text also avoids that shell's
// command-line quoting differences. No launch argument is interpolated here.
const PROMPT: &str = r"if ($null -eq $global:__OrchbeeDirectoryPrompt) { $global:__OrchbeeDirectoryPrompt = $function:prompt; function global:prompt { $out = @(& $global:__OrchbeeDirectoryPrompt) -join ' '; $loc = $ExecutionContext.SessionState.Path.CurrentLocation; if ($loc.Provider.Name -eq 'FileSystem') { try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {} }; $out } }";

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
mod tests {
    use super::*;
    use std::{os::windows::process::CommandExt, path::PathBuf, process::Command};

    #[test]
    fn recognizes_only_plain_interactive_powershell_and_preserves_input_flags() {
        for executable in [
            r"C:\Windows\PowerShell.EXE",
            r"C:/Program Files/PowerShell/7/pwsh.exe",
            "pwsh",
        ] {
            let args = vec!["-NoProfile".to_owned(), "-nOeXiT".to_owned()];
            let actual = interactive_arguments(executable, args.clone());
            assert_eq!(&actual[..2], &args);
            assert_eq!(&actual[2..], &["-Command", PROMPT]);
            assert_eq!(
                interactive_arguments(executable, vec![]),
                ["-NoExit", "-Command", PROMPT]
            );
        }
        for args in [
            vec!["-Command", "literal"],
            vec!["-File", "script.ps1"],
            vec!["-NonInteractive"],
            vec!["-nop"],
            vec!["-NoExit:1"],
            vec!["-NoLogo"; 4],
        ] {
            let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
            assert_eq!(interactive_arguments("pwsh.exe", args.clone()), args);
        }
        for exe in ["cmd.exe", "powershell.exe.bak", "python.exe"] {
            assert!(interactive_arguments(exe, vec![]).is_empty());
        }
    }

    #[test]
    fn actual_windows_powershell_prompts_preserve_status_and_publish_provider_directory() {
        let legacy = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let modern =
            PathBuf::from(std::env::var_os("ProgramFiles").unwrap()).join("PowerShell/7/pwsh.exe");
        for shell in [legacy, modern] {
            if !shell.is_file() {
                continue;
            }
            // No files/profile are written. The command owns and reaps its child.
            let script = format!(
                r"function global:prompt {{ if ($?) {{ 'ok> ' }} else {{ 'failed> ' }} }}; {PROMPT}; {PROMPT}; $null = Get-Item -LiteralPath 'Z:\orchbee-cwd-absent' -ErrorAction SilentlyContinue; $answer = prompt; if ($answer -ne 'failed> ') {{ throw ('lost status: ' + $answer) }}; Set-Location -LiteralPath $env:SystemRoot; $answer = prompt; if ($answer -ne 'ok> ') {{ throw 'changed prompt' }}; if ([Environment]::CurrentDirectory -ne $pwd.ProviderPath) {{ throw 'stale process cwd' }}; $last = [Environment]::CurrentDirectory; Set-Location HKCU:\; $null = prompt; if ([Environment]::CurrentDirectory -ne $last) {{ throw 'non-filesystem cwd' }}; 'directory-prompt-ok'"
            );
            let output = Command::new(&shell)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &script,
                ])
                .creation_flags(0x0800_0000)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}: {}",
                shell.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("directory-prompt-ok"));
        }
    }
}
