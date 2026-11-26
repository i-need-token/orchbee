//! What a match becomes: the replacement text read once, with its groups, escapes and case
//! changes for a regular expression, and the case of the replaced text kept where asked.

/// A case change a `\u`, `\l`, `\U` or `\L` asks of the next group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    UpperFirst,
    LowerFirst,
    Upper,
    Lower,
}

/// A group a replacement inserts, by number or name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Group {
    Index(usize),
    Name(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Piece {
    Text(String),
    /// A group with the case changes before it, and the text it stands for when the regular
    /// expression has no such group. Two digits name a group only where that group exists;
    /// otherwise the first digit does and the second is text.
    Group {
        group: Group,
        cases: Vec<Case>,
        written: String,
    },
}

/// A replacement made ready to write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Replacement {
    pieces: Vec<Piece>,
}

impl Replacement {
    /// Reads `text`; without `regex` it is written as it stands.
    pub(super) fn parse(text: &str, regex: bool) -> Self {
        if !regex {
            return Self {
                pieces: vec![Piece::Text(text.to_owned())],
            };
        }
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut cases = Vec::new();
        let mut characters = text.char_indices().peekable();
        while let Some((index, character)) = characters.next() {
            match character {
                '\\' => {
                    let Some(next) = characters.next().map(|(_, next)| next) else {
                        literal.push('\\');
                        continue;
                    };
                    match next {
                        '\\' => literal.push('\\'),
                        'n' => literal.push('\n'),
                        't' => literal.push('\t'),
                        'u' => cases.push(Case::UpperFirst),
                        'l' => cases.push(Case::LowerFirst),
                        'U' => cases.push(Case::Upper),
                        'L' => cases.push(Case::Lower),
                        other => {
                            literal.push('\\');
                            literal.push(other);
                        }
                    }
                }
                '$' => {
                    let rest = &text[index + 1..];
                    match group_at(rest) {
                        Some((group, length)) => {
                            if !literal.is_empty() {
                                pieces.push(Piece::Text(std::mem::take(&mut literal)));
                            }
                            pieces.push(Piece::Group {
                                group,
                                cases: std::mem::take(&mut cases),
                                written: text[index..=index + length].to_owned(),
                            });
                            for _ in 0..rest[..length].chars().count() {
                                characters.next();
                            }
                        }
                        None if rest.starts_with('$') => {
                            literal.push('$');
                            characters.next();
                        }
                        None => literal.push('$'),
                    }
                }
                other => literal.push(other),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Text(literal));
        }
        Self { pieces }
    }

    /// Whether the replacement inserts no group, so it can keep the replaced text's case.
    fn fixed(&self) -> Option<String> {
        let mut text = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Text(part) => text.push_str(part),
                Piece::Group { .. } => return None,
            }
        }
        Some(text)
    }

    /// The text replacing `matched`; `group` finds a group's text, or none where the regular
    /// expression has no such group or it took part in no match.
    pub(super) fn expand<'a>(
        &self,
        matched: &str,
        preserve_case: bool,
        group: impl Fn(&Group) -> Option<Option<&'a str>>,
    ) -> String {
        if preserve_case && let Some(fixed) = self.fixed() {
            return with_case_of(matched, &fixed);
        }
        let mut text = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Text(part) => text.push_str(part),
                Piece::Group {
                    group: wanted,
                    cases,
                    written,
                } => match resolve(wanted, written, &group) {
                    Resolved::Group(value) => {
                        let mut value = value.unwrap_or_default().to_owned();
                        for case in cases {
                            value = changed(&value, *case);
                        }
                        text.push_str(&value);
                    }
                    Resolved::Text(value) => text.push_str(&value),
                },
            }
        }
        text
    }
}

/// What a group in a replacement stands for.
enum Resolved<'a> {
    Group(Option<&'a str>),
    Text(String),
}

/// A two-digit group that does not exist reads as its first digit's group and the second digit;
/// a group that does not exist at all reads as it was written.
fn resolve<'a>(
    wanted: &Group,
    written: &str,
    group: &impl Fn(&Group) -> Option<Option<&'a str>>,
) -> Resolved<'a> {
    if let Some(value) = group(wanted) {
        return Resolved::Group(value);
    }
    if let Group::Index(index) = wanted
        && *index >= 10
        && let Some(value) = group(&Group::Index(index / 10))
    {
        let mut text = value.unwrap_or_default().to_owned();
        text.push_str(&(index % 10).to_string());
        return Resolved::Text(text);
    }
    Resolved::Text(written.to_owned())
}

/// The group a `$` starts in `rest`, the text after it, and how many bytes of `rest` name it:
/// `0` and `&` the whole match, one or two digits a numbered group, and `{name}` or `<name>` a
/// named one.
fn group_at(rest: &str) -> Option<(Group, usize)> {
    let bytes = rest.as_bytes();
    match bytes.first()? {
        b'&' | b'0' => Some((Group::Index(0), 1)),
        first @ b'1'..=b'9' => {
            let first = usize::from(first - b'0');
            match bytes.get(1) {
                Some(second @ b'0'..=b'9') => {
                    Some((Group::Index(first * 10 + usize::from(second - b'0')), 2))
                }
                _ => Some((Group::Index(first), 1)),
            }
        }
        open @ (b'{' | b'<') => {
            let close = if *open == b'{' { '}' } else { '>' };
            let end = rest[1..].find(close)? + 1;
            let name = &rest[1..end];
            let valid = !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_alphanumeric() || character == '_');
            valid.then(|| (Group::Name(name.to_owned()), end + 1))
        }
        _ => None,
    }
}

fn changed(text: &str, case: Case) -> String {
    let mut characters = text.chars();
    match case {
        Case::Upper => text.to_uppercase(),
        Case::Lower => text.to_lowercase(),
        Case::UpperFirst => characters.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(characters).collect()
        }),
        Case::LowerFirst => characters.next().map_or_else(String::new, |first| {
            first.to_lowercase().chain(characters).collect()
        }),
    }
}

/// `replacement` in the case of `matched`: all upper or all lower where it is, a capital or small
/// first letter where it starts so, and part by part where both hold the same number of
/// hyphens, or of underscores, and not the other.
pub(super) fn with_case_of(matched: &str, replacement: &str) -> String {
    if matched.is_empty() {
        return replacement.to_owned();
    }
    let hyphens = same_parts(matched, replacement, '-');
    let underscores = same_parts(matched, replacement, '_');
    if hyphens != underscores {
        let separator = if hyphens { '-' } else { '_' };
        return matched
            .split(separator)
            .zip(replacement.split(separator))
            .map(|(part, wanted)| with_case_of(part, wanted))
            .collect::<Vec<_>>()
            .join(&separator.to_string());
    }
    if matched.to_uppercase() == matched {
        return replacement.to_uppercase();
    }
    if matched.to_lowercase() == matched {
        return replacement.to_lowercase();
    }
    let first = matched.chars().next().unwrap_or_default();
    if first.is_uppercase() {
        changed(replacement, Case::UpperFirst)
    } else if first.to_uppercase().ne(std::iter::once(first)) {
        changed(replacement, Case::LowerFirst)
    } else {
        replacement.to_owned()
    }
}

/// Whether both texts hold `separator` and as many parts around it.
fn same_parts(matched: &str, replacement: &str, separator: char) -> bool {
    matched.contains(separator)
        && replacement.contains(separator)
        && matched.split(separator).count() == replacement.split(separator).count()
}
