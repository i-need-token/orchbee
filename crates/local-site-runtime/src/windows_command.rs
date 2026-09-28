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
