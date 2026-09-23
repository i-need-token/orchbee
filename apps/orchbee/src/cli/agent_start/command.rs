//! Literal Windows launch text, constructed before runtime admission. The
//! encoded script keeps cmd.exe's expansion away from provider arguments.

use crate::agent_provider::executable;

pub(super) fn launch(provider: &str, args: &[String]) -> Option<String> {
    let program = executable(provider)?;
    launch_program(program, args)
}

fn launch_program(program: &str, args: &[String]) -> Option<String> {
    if args.iter().any(|arg| arg.chars().any(char::is_control)) {
        return None;
    }
    let values = args
        .iter()
        .map(|arg| powershell_literal(arg))
        .collect::<Vec<_>>()
        .join(",");
    let native = args
        .iter()
        .map(|arg| windows_argument(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let batch = args
        .iter()
        .map(|arg| quoted_argument(arg, true))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        "$ErrorActionPreference='Stop';$c=Get-Command {} -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1;$a=@({values});if($c.CommandType -eq 'ExternalScript'){{& $c.Path @a}}elseif([IO.Path]::GetExtension($c.Path) -in '.cmd','.bat'){{$line='/e:ON /v:OFF /d /c \"\"'+$c.Path.Replace('%','%%cd:~,%')+'\" '+{}+'\"';Start-Process -FilePath ($env:SystemRoot+'\\System32\\cmd.exe') -ArgumentList $line -NoNewWindow -Wait}}else{{Start-Process -FilePath $c.Path -ArgumentList {} -NoNewWindow -Wait}}",
        powershell_literal(program),
        powershell_literal(&batch),
        powershell_literal(&native)
    );
    // Empty ArgumentList is rejected by Start-Process; omit that parameter.
    let script = if args.is_empty() {
        script.replace(" -ArgumentList ''", "")
    } else {
        script
    };
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let text = format!(
        "powershell.exe -NoLogo -NoProfile -EncodedCommand {}",
        base64(&bytes)
    );
    // cmd.exe is the smallest supported command-line owner. Refuse before
    // reserving or typing instead of relying on its silent truncation.
    (text.len() <= 8000).then_some(text)
}

fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn windows_argument(value: &str) -> String {
    quoted_argument(value, false)
}

fn quoted_argument(value: &str, batch: bool) -> String {
    let mut answer = String::from("\"");
    let mut slashes = 0;
    for c in value.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        answer.extend(std::iter::repeat_n(
            '\\',
            slashes * if c == '"' { 2 } else { 1 },
        ));
        slashes = 0;
        if c == '"' {
            answer.push(if batch { '"' } else { '\\' });
        }
        // Match Rust std's Windows batch boundary: command extensions turn
        // the empty CD substring into nothing, interrupting %VAR% expansion.
        // Delayed expansion is disabled by the explicit cmd.exe invocation.
        if batch && c == '%' {
            answer.push_str("%%cd:~,");
        }
        answer.push(c);
    }
    answer.extend(std::iter::repeat_n('\\', slashes * 2));
    answer.push('"');
    answer
}

// RFC 4648's padded standard alphabet, used only for PowerShell UTF-16LE text.
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
    #[path = "windows_argv.rs"]
    mod windows_argv;
    #[test]
    fn windows_quoting_and_encoding_have_independent_known_vectors() {
        for (plain, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), expected);
        }
        assert_eq!(windows_argument(""), "\"\"");
        assert_eq!(windows_argument("a b"), "\"a b\"");
        assert_eq!(windows_argument("a\"b"), "\"a\\\"b\"");
        assert_eq!(windows_argument("a\\"), "\"a\\\\\"");
        assert_eq!(powershell_literal("a'$b"), "'a''$b'");
        assert!(launch("codex", &["a\nb".into()]).is_none());
        assert!(launch("codex", &["x".repeat(10000)]).is_none());
        assert!(launch("unknown", &[]).is_none());
    }
}
