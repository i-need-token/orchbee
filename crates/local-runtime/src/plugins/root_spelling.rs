//! Spells a canonical plugin root the way plugin processes read it. The catalog stores the
//! verbatim drive or UNC spelling that canonicalization returns; programs such as cmd.exe refuse
//! that spelling, so processes receive the ordinary Win32 one whenever it names the same path.
//! The rule matches the Terminal Host's `ordinaryPath`. It neither probes the filesystem nor
//! changes the stored registry value.

/// The ordinary spelling of a verbatim `path`, or `path` unchanged when it has no verbatim prefix
/// or dropping the prefix could change what it names: a device path, an empty, `.` or `..`
/// component, a forward slash, a component ending in a dot or space, or a DOS device name, all of
/// which the Win32 path parser rewrites.
pub(super) fn process_spelling(path: &str) -> String {
    ordinary(path).unwrap_or_else(|| path.to_owned())
}

fn ordinary(path: &str) -> Option<String> {
    let rest = path.strip_prefix(r"\\?\")?;
    let (unc, components) = if let Some(share) = strip_prefix_ignore_case(rest, r"UNC\") {
        (true, share)
    } else {
        let drive = rest.as_bytes();
        if !matches!(drive, [letter, b':', b'\\', ..] if letter.is_ascii_alphabetic()) {
            return None;
        }
        (false, &rest[3..])
    };
    if (unc && components.is_empty()) || rest.contains('/') {
        return None;
    }
    let mut parts = components.split('\\').peekable();
    while let Some(part) = parts.next() {
        // Only a trailing separator may leave the final component empty.
        if part.is_empty() {
            if parts.peek().is_none() {
                break;
            }
            return None;
        }
        if part.ends_with(['.', ' ']) || is_dos_device_name(part) {
            return None;
        }
    }
    Some(if unc {
        format!(r"\\{components}")
    } else {
        rest.to_owned()
    })
}

fn strip_prefix_ignore_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &value[prefix.len()..])
}

/// Win32 maps these names to devices in any directory, with or without an extension.
fn is_dos_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .trim_end_matches(' ');
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
        .iter()
        .any(|name| stem.eq_ignore_ascii_case(name))
    {
        return true;
    }
    let Some(suffix) = stem
        .get(..3)
        .filter(|head| head.eq_ignore_ascii_case("COM") || head.eq_ignore_ascii_case("LPT"))
        .map(|_| &stem[3..])
    else {
        return false;
    };
    matches!(suffix.as_bytes(), [digit] if digit.is_ascii_digit())
        || matches!(suffix, "\u{00B9}" | "\u{00B2}" | "\u{00B3}")
}
