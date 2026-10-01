//! Pure literal argv encoding. Applications retain program selection and effect ownership.

/// Builds UTF-16LE Base64 for Windows PowerShell's EncodedCommand.
/// The caller owns shell selection, interactive flags and any stricter transport budget.
/// Program discovery excludes aliases/functions; references and arguments are never script source.
#[must_use]
pub fn windows_encoded_command(program: &str, args: &[String]) -> Option<String> {
    if program.is_empty()
        || program.chars().any(char::is_control)
        || args.len() > 128
        || args.iter().any(|arg| arg.chars().any(char::is_control))
        || program.len() + args.iter().map(String::len).sum::<usize>() > 30 * 1024
    {
        return None;
    }
    let values = args
        .iter()
        .map(|arg| literal(arg))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        "{}; & {{ $ErrorActionPreference='Stop';$c=Get-Command {} -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1;$a=@({values});{} }}",
        crate::WINDOWS_DIRECTORY_PROMPT,
        literal(program),
        include_str!("windows_command.ps1")
    );
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64(&bytes);
    // Leave room for the absolute shell and switches below CreateProcessW's 32,767 UTF-16 limit.
    (encoded.len() <= 30_000).then_some(encoded)
}

fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Encodes a literal cmd command for an explicit PowerShell PTY launcher.
/// ProcessStartInfo's raw Arguments preserve cmd quoting and operators; no
/// caller text becomes PowerShell syntax. The caller owns executable discovery.
#[must_use]
pub fn windows_raw_command_encoded(
    shell: &str,
    command: &str,
    environment: &[(String, String)],
) -> Option<String> {
    if shell.is_empty()
        || shell.len() > 4096
        || shell.contains('\0')
        || command.trim().is_empty()
        || command.len() > 4096
        || command.contains('\0')
    {
        return None;
    }
    if environment.len() > 16
        || environment.iter().any(|(name, value)| {
            !name.starts_with("ORCHBEE_")
                || !name.bytes().all(|c| c.is_ascii_uppercase() || c == b'_')
                || value.len() > 4096
                || value.contains('\0')
        })
    {
        return None;
    }
    let assignments = environment
        .iter()
        .map(|(name, value)| {
            format!(
                "$s.EnvironmentVariables[{}]={};",
                literal(name),
                literal(value)
            )
        })
        .collect::<String>();
    let script = format!(
        "$ErrorActionPreference='Stop'; $s=[System.Diagnostics.ProcessStartInfo]::new(); $s.FileName={}; $s.Arguments={}; $s.UseShellExecute=$false; $s.WorkingDirectory=[Environment]::CurrentDirectory; {assignments} $p=[System.Diagnostics.Process]::Start($s); $p.WaitForExit(); exit $p.ExitCode",
        literal(&shell.replace('/', "\\")),
        literal(&format!("/d /c {command}"))
    );
    let encoded = base64(
        &script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    (encoded.len() <= 28 * 1024).then_some(encoded)
}

// RFC 4648 standard alphabet, used only for the PowerShell UTF-16LE boundary.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for block in bytes.chunks(3) {
        let a = block[0];
        let b = block.get(1).copied().unwrap_or(0);
        let c = block.get(2).copied().unwrap_or(0);
        encoded.push(char::from(ALPHABET[usize::from(a >> 2)]));
        encoded.push(char::from(ALPHABET[usize::from(((a & 3) << 4) | (b >> 4))]));
        encoded.push(if block.len() > 1 {
            char::from(ALPHABET[usize::from(((b & 15) << 2) | (c >> 6))])
        } else {
            '='
        });
        encoded.push(if block.len() > 2 {
            char::from(ALPHABET[usize::from(c & 63)])
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn raw_command_launch_preserves_environment_and_exit_without_script_interpolation() {
        let root = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        let shell = root.join("System32/cmd.exe");
        let encoded = windows_raw_command_encoded(
            shell.to_str().unwrap(),
            "if \"%ORCHBEE_TEST%\"==\"a'$b\" (exit /b 7) else (exit /b 9)",
            &[("ORCHBEE_TEST".into(), "a'$b".into())],
        )
        .unwrap();
        let status =
            std::process::Command::new(root.join("System32/WindowsPowerShell/v1.0/powershell.exe"))
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-EncodedCommand",
                    &encoded,
                ])
                .status()
                .unwrap();
        assert_eq!(status.code(), Some(7));
        assert!(windows_raw_command_encoded("cmd.exe", "x\0y", &[]).is_none());
        assert!(
            windows_raw_command_encoded(
                "cmd.exe",
                "echo hello",
                &[("PATH".into(), "untrusted".into())]
            )
            .is_none()
        );
    }
    #[test]
    fn encoding_has_independent_vectors_and_bounded_input() {
        for (plain, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), expected);
        }
        assert_eq!(literal("a'$b"), "'a''$b'");
        assert!(windows_encoded_command("codex", &["a\nb".into()]).is_none());
        assert!(windows_encoded_command("", &[]).is_none());
        assert!(windows_encoded_command("pi", &["'".repeat(4096)]).is_some());
        assert!(windows_encoded_command("pi", &["x".repeat(31 * 1024)]).is_none());
        assert!(windows_encoded_command("pi", &vec![String::new(); 129]).is_none());
    }
}
