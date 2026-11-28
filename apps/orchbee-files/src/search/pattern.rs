//! The rewrites VS Code makes to a text search's pattern before rg reads it, at ef5ca45
//! (`ripgrepTextSearchEngine.ts` and `strings.ts`): `\u` escapes in PCRE2's spelling, each `\n`
//! made to match a CRLF line ending as well, and a whole-word search's expression as V8's
//! `RegExp.prototype.source` spells it.

use super::{
    js,
    regexp::{self, Alternative, Atom, Class, ClassItem, Element, Rejected},
};

/// `unicodeEscapesToPCRE2`: each `\uXXXX`, then each `\u{XXXX}`, whose backslash no backslash
/// escapes, becomes `\x{XXXX}`, until none is left.
pub(super) fn unicode_escapes(pattern: &str) -> String {
    let mut pattern = pattern.to_owned();
    for braces in [false, true] {
        while let Some(replaced) = replace_escapes(&pattern, braces) {
            pattern = replaced;
        }
    }
    pattern
}

/// One global replacement of `/((?:[^\\]|^)(?:\\\\)*)\\u([a-z0-9]{4})/gi` with `$1\x{$2}`, or
/// of its braced form; `None` when nothing matches.
fn replace_escapes(pattern: &str, braces: bool) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut output = String::with_capacity(pattern.len());
    let (mut at, mut found) = (0, false);
    while at < chars.len() {
        if let Some((end, kept, digits)) = escape_at(&chars, at, braces) {
            output.extend(&chars[at..kept]);
            output.push_str("\\x{");
            output.extend(digits);
            output.push('}');
            (at, found) = (end, true);
        } else {
            output.push(chars[at]);
            at += 1;
        }
    }
    found.then_some(output)
}

/// The match starting at `at`: its end, the end of the group kept before the escape, and the
/// four digits.
fn escape_at(chars: &[char], at: usize, braces: bool) -> Option<(usize, usize, &[char])> {
    // One character other than a backslash, else the pattern's start.
    let starts = [
        (chars[at] != '\\').then_some(at + 1),
        (at == 0).then_some(at),
    ];
    starts.into_iter().flatten().find_map(|start| {
        // Pairs of backslashes and then the escape's own: an odd run.
        let run = chars[start..].iter().take_while(|ch| **ch == '\\').count();
        if run % 2 == 0 {
            return None;
        }
        let escape = start + run - 1;
        let mut cursor = escape + 1;
        if !matches!(chars.get(cursor), Some('u' | 'U')) {
            return None;
        }
        cursor += 1;
        if braces {
            (chars.get(cursor) == Some(&'{')).then_some(())?;
            cursor += 1;
        }
        let digits = chars.get(cursor..cursor + 4)?;
        if !digits.iter().all(char::is_ascii_alphanumeric) {
            return None;
        }
        cursor += 4;
        if braces {
            (chars.get(cursor) == Some(&'}')).then_some(())?;
            cursor += 1;
        }
        Some((cursor, escape, digits))
    })
}

/// `fixRegexNewline`: each `\n` escape matches `\r?\n` instead, by its place in the expression;
/// one in a lookbehind or a class range stays. A pattern JavaScript would reject is unchanged.
pub(super) fn fix_regex_newline(pattern: &str) -> String {
    if !pattern.contains("\\n") {
        return pattern.to_owned();
    }
    let units = js::units(pattern);
    let Ok(alternatives) = regexp::parse(&units) else {
        return pattern.to_owned();
    };
    let mut rewrite = Rewrite {
        pattern: &units,
        output: Vec::with_capacity(units.len()),
        emitted: 0,
        context: Vec::new(),
        lookbehinds: 0,
    };
    rewrite.alternatives(&alternatives);
    let rest = units.get(rewrite.emitted..).unwrap_or_default();
    rewrite.output.extend_from_slice(rest);
    js::text(&rewrite.output)
}

/// The node `fixRegexNewline`'s visitor holds as context: quantifiers, class ranges and classes
/// at the front, lookbehinds at the back.
enum Frame<'a> {
    Quantifier,
    Range,
    Class { class: &'a Class, quantified: bool },
    Lookbehind(usize),
}

struct Rewrite<'a> {
    pattern: &'a [u16],
    output: Vec<u16>,
    /// Where the pattern's next unemitted unit is.
    emitted: usize,
    context: Vec<Frame<'a>>,
    lookbehinds: usize,
}

impl<'a> Rewrite<'a> {
    fn alternatives(&mut self, alternatives: &'a [Alternative]) {
        for alternative in alternatives {
            for element in alternative {
                self.element(element, false);
            }
        }
    }

    fn element(&mut self, element: &'a Element, quantified: bool) {
        match element {
            Element::Character(atom) => self.character(*atom),
            Element::Other => {}
            Element::Class(class) => {
                self.context.insert(0, Frame::Class { class, quantified });
                for item in &class.items {
                    match item {
                        ClassItem::Character(atom) => self.character(*atom),
                        ClassItem::Range(low, high) => {
                            self.context.insert(0, Frame::Range);
                            self.character(*low);
                            self.character(*high);
                            self.context.remove(0);
                        }
                        ClassItem::Set => {}
                    }
                }
                self.context.remove(0);
            }
            Element::Group(alternatives) => self.alternatives(alternatives),
            Element::Lookaround {
                behind,
                alternatives,
            } => {
                self.lookbehinds += 1;
                let id = self.lookbehinds;
                if *behind {
                    self.context.push(Frame::Lookbehind(id));
                }
                self.alternatives(alternatives);
                // VS Code leaves only a lookbehind it finds at the front, so one entered under a
                // quantifier or class stays for the rest of the pattern.
                if matches!(self.context.first(), Some(Frame::Lookbehind(first)) if *first == id) {
                    self.context.remove(0);
                }
            }
            Element::Quantifier(quantified) => {
                self.context.insert(0, Frame::Quantifier);
                self.element(quantified, true);
                self.context.remove(0);
            }
        }
    }

    fn character(&mut self, atom: Atom) {
        if !atom.newline {
            return;
        }
        if self
            .context
            .iter()
            .any(|frame| matches!(frame, Frame::Lookbehind(_)))
        {
            return;
        }
        match self.context.first() {
            None => self.replace(atom.start, atom.end, "\\r?\\n"),
            Some(Frame::Class { class, quantified }) => {
                let (class, quantified) = (*class, *quantified);
                let skip = if class.negated { 2 } else { 1 };
                let mut other = slice(self.pattern, class.start + skip, atom.start).to_vec();
                other.extend_from_slice(slice(self.pattern, atom.end, class.end - 1));
                let other = js::text(&other);
                let replacement = match (class.negated, quantified, other.is_empty()) {
                    (true, true, true) => ".".to_owned(),
                    (true, true, false) => format!("[^{other}]"),
                    (true, false, true) => "(?!\\r?\\n)".to_owned(),
                    (true, false, false) => format!("(?!\\r?\\n|[{other}])"),
                    (false, _, true) => "\\r?\\n".to_owned(),
                    (false, _, false) => format!("(?:[{other}]|\\r?\\n)"),
                };
                self.replace(class.start, class.end, &replacement);
            }
            Some(Frame::Quantifier) => self.replace(atom.start, atom.end, "(?:\\r?\\n)"),
            Some(Frame::Range | Frame::Lookbehind(_)) => {}
        }
    }

    /// VS Code's `replace`: the pattern from the last emitted unit to `start`, which is nothing
    /// when `start` comes first, then `text`.
    fn replace(&mut self, start: usize, end: usize, text: &str) {
        let before = slice(self.pattern, self.emitted, start);
        self.output.extend_from_slice(before);
        self.output.extend(text.encode_utf16());
        self.emitted = end;
    }
}

/// JavaScript's `slice`: empty when `start` is not before `end`.
fn slice(units: &[u16], start: usize, end: usize) -> &[u16] {
    let end = end.min(units.len());
    units.get(start..end).unwrap_or_default()
}

/// `fixNewline`: each line feed in the pattern becomes `\r?\n`.
pub(super) fn fix_newline(pattern: &str) -> String {
    pattern.replace('\n', "\\r?\\n")
}

/// The expression VS Code passes for a whole-word search: the pattern, escaped unless it is a
/// regular expression, after `\b` when its first unit is a word character and before `\b` when
/// its last is, as V8's `source` spells it, with `\/` turned back into `/`.
///
/// # Errors
///
/// [`Rejected`] when JavaScript rejects the expression, as `new RegExp` throws.
pub(super) fn whole_word(pattern: &str, regex: bool) -> Result<String, Rejected> {
    // `createRegExp` refuses an empty search string.
    if pattern.is_empty() {
        return Err(Rejected);
    }
    let mut expression = if regex {
        pattern.to_owned()
    } else {
        js::escape_regexp(pattern)
    };
    let is_word = |unit: Option<&u16>| {
        unit.is_some_and(|unit| {
            u8::try_from(*unit).is_ok_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
    };
    let units = js::units(&expression);
    let (starts, ends) = (is_word(units.first()), is_word(units.last()));
    if starts {
        expression.insert_str(0, "\\b");
    }
    if ends {
        expression.push_str("\\b");
    }
    regexp::parse(&js::units(&expression))?;
    Ok(source(&expression).replace("\\/", "/"))
}

/// V8's `RegExp.prototype.source`: a `/` outside a class escaped, line terminators written as
/// escapes, and a backslash before a line terminator dropped.
fn source(expression: &str) -> String {
    if expression.is_empty() {
        return "(?:)".to_owned();
    }
    let chars: Vec<char> = expression.chars().collect();
    let line_terminator = |ch: char| matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    let mut spelled = String::with_capacity(expression.len() + 8);
    let (mut at, mut in_class) = (0, false);
    while at < chars.len() {
        let ch = chars[at];
        match ch {
            '\\' => {
                if chars.get(at + 1).copied().is_some_and(line_terminator) {
                    at += 1;
                    continue;
                }
                spelled.push(ch);
                at += 1;
                if let Some(next) = chars.get(at) {
                    spelled.push(*next);
                    at += 1;
                }
                continue;
            }
            '/' if !in_class => spelled.push('\\'),
            '[' => in_class = true,
            ']' => in_class = false,
            '\n' => {
                spelled.push_str("\\n");
                at += 1;
                continue;
            }
            '\r' => {
                spelled.push_str("\\r");
                at += 1;
                continue;
            }
            '\u{2028}' => {
                spelled.push_str("\\u2028");
                at += 1;
                continue;
            }
            '\u{2029}' => {
                spelled.push_str("\\u2029");
                at += 1;
                continue;
            }
            _ => {}
        }
        spelled.push(ch);
        at += 1;
    }
    spelled
}
