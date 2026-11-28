//! The JavaScript string semantics VS Code's search code relies on, over UTF-16 code units: case
//! mapping, whitespace, ASCII-first case-insensitive comparison and regular expression escaping.
//! The ported modules index text as JavaScript does, so offsets and comparisons match VS Code's.

/// The platform's path separator, as Node's `path.sep`.
pub(super) const SEPARATOR: u16 = if cfg!(windows) { 0x5C } else { 0x2F };

pub(super) const SLASH: u16 = 0x2F;
pub(super) const BACKSLASH: u16 = 0x5C;

/// `text` as JavaScript holds it.
pub(super) fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// `units` as Rust text, a lone surrogate replaced.
pub(super) fn text(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// Whether `unit` is in JavaScript's `\s`, which `trim` also removes.
pub(super) fn is_space(unit: u16) -> bool {
    matches!(
        unit,
        0x09..=0x0D
            | 0x20
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

/// Whether `ch` is in JavaScript's `\s`.
pub(super) fn is_space_char(ch: char) -> bool {
    u16::try_from(u32::from(ch)).is_ok_and(is_space)
}

/// `String.prototype.trim`.
pub(super) fn trim(text: &str) -> &str {
    text.trim_matches(is_space_char)
}

/// `String.prototype.toLowerCase`, which keeps a lone surrogate.
pub(super) fn to_lower(text: &[u16]) -> Vec<u16> {
    if text.iter().all(|unit| *unit < 0x80) {
        return text.iter().map(|unit| ascii_lower(*unit)).collect();
    }
    let mut lowered = Vec::with_capacity(text.len());
    let mut run = String::new();
    for decoded in char::decode_utf16(text.iter().copied()) {
        match decoded {
            Ok(ch) => run.push(ch),
            Err(lone) => {
                lowered.extend(run.to_lowercase().encode_utf16());
                run.clear();
                lowered.push(lone.unpaired_surrogate());
            }
        }
    }
    lowered.extend(run.to_lowercase().encode_utf16());
    lowered
}

pub(super) fn ascii_lower(unit: u16) -> u16 {
    if is_ascii_upper(unit) {
        unit + 0x20
    } else {
        unit
    }
}

/// `A` to `Z`.
pub(super) fn is_ascii_upper(unit: u16) -> bool {
    (0x41..=0x5A).contains(&unit)
}

/// `a` to `z`.
pub(super) fn is_ascii_lower(unit: u16) -> bool {
    (0x61..=0x7A).contains(&unit)
}

/// The ECMAScript `Canonicalize` of a pattern without the `u` flag, under which `i` compares
/// code units: the unit's uppercase when that is one unit and does not move a non-ASCII unit
/// into ASCII.
pub(super) fn canonicalize(unit: u16) -> u16 {
    let Some(ch) = char::from_u32(u32::from(unit)) else {
        return unit;
    };
    let mut upper = ch.to_uppercase();
    let (Some(first), None) = (upper.next(), upper.next()) else {
        return unit;
    };
    match u16::try_from(u32::from(first)) {
        Ok(code) if !(unit >= 0x80 && code < 0x80) => code,
        _ => unit,
    }
}

/// VS Code's `compareSubstringIgnoreCase`: ASCII letters compare without case, and from the first
/// differing non-ASCII unit on the rest of both ranges compares lowercased instead.
pub(super) fn compare_ignore_case(
    a: &[u16],
    b: &[u16],
    (mut a_start, a_end): (usize, usize),
    (mut b_start, b_end): (usize, usize),
) -> i64 {
    while a_start < a_end && b_start < b_end {
        let (mut code_a, mut code_b) = (a[a_start], b[b_start]);
        if code_a != code_b {
            if code_a >= 0x80 || code_b >= 0x80 {
                let rest_a = to_lower(&a[a_start..a_end]);
                let rest_b = to_lower(&b[b_start..b_end]);
                return order(rest_a.cmp(&rest_b));
            }
            if is_ascii_lower(code_a) {
                code_a -= 0x20;
            }
            if is_ascii_lower(code_b) {
                code_b -= 0x20;
            }
            let difference = i64::from(code_a) - i64::from(code_b);
            if difference != 0 {
                return difference;
            }
        }
        a_start += 1;
        b_start += 1;
    }
    length_order(a_end - a_start, b_end - b_start)
}

fn length_order(a: usize, b: usize) -> i64 {
    order(a.cmp(&b))
}

/// An order as VS Code's comparisons return it.
fn order(order: std::cmp::Ordering) -> i64 {
    match order {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// VS Code's `equalsIgnoreCase`.
pub(super) fn equals_ignore_case(a: &[u16], b: &[u16]) -> bool {
    a.len() == b.len() && compare_ignore_case(a, b, (0, a.len()), (0, b.len())) == 0
}

/// VS Code's `startsWithIgnoreCase`.
pub(super) fn starts_with_ignore_case(text: &[u16], candidate: &[u16]) -> bool {
    candidate.len() <= text.len()
        && compare_ignore_case(text, candidate, (0, candidate.len()), (0, candidate.len())) == 0
}

/// VS Code's `endsWithIgnoreCase`.
pub(super) fn ends_with_ignore_case(text: &[u16], candidate: &[u16]) -> bool {
    text.len() >= candidate.len()
        && compare_ignore_case(
            text,
            candidate,
            (text.len() - candidate.len(), text.len()),
            (0, candidate.len()),
        ) == 0
}

/// VS Code's `escapeRegExpCharacters`.
pub(super) fn escape_regexp(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if "\\{}*+?|^$.[]()".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}
