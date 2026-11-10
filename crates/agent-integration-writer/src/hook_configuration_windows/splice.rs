//! Byte-preserving edits of a validated JSON document. Values a change does not name keep their
//! original bytes, including member order, whitespace and number spelling; new values follow the
//! indentation of their neighbours. When a change removes every member or element of a container
//! below the root, the container goes too, so a container an enable created leaves with disable.
use serde::Serialize;
use serde_json::{Serializer, Value, ser::PrettyFormatter};

/// One change to a JSON value.
pub(super) enum Change {
    /// Replaces the value; an equal value keeps its bytes.
    Replace(Value),
    /// Removes the member or element.
    Remove,
    /// Changes existing members by name and appends new members.
    Object {
        members: Vec<(&'static str, Change)>,
        append: Vec<(&'static str, Value)>,
    },
    /// Changes existing elements by index and appends new elements.
    Array {
        elements: Vec<(usize, Change)>,
        append: Vec<Value>,
    },
}

/// Applies `change` to the root value of `text`, which the caller already parsed as strict JSON.
/// Returns `None` when the document does not have the shape the change names.
pub(super) fn apply(text: &str, change: &Change) -> Option<String> {
    let bytes = text.as_bytes();
    let start = skip_space(bytes, 0);
    let end = value_end(bytes, start)?;
    let style = Style::of(text);
    match rewrite(text, start, change, &style, None, true)? {
        Rewrite::Same => Some(text.to_owned()),
        Rewrite::Removed => None,
        Rewrite::Text(edited) => Some(format!("{}{edited}{}", &text[..start], &text[end..])),
    }
}

enum Rewrite {
    Same,
    Removed,
    Text(String),
}

struct Style {
    newline: &'static str,
    unit: String,
    pretty: bool,
}

impl Style {
    fn of(text: &str) -> Self {
        let unit = text
            .split('\n')
            .skip(1)
            .map(|line| &line[..line.len() - line.trim_start_matches([' ', '\t']).len()])
            .find(|indent| !indent.is_empty())
            .unwrap_or("  ")
            .to_owned();
        Self {
            newline: if text.contains("\r\n") { "\r\n" } else { "\n" },
            unit,
            pretty: text.contains('\n'),
        }
    }

    /// `value` starting on a line indented by `indent`, or on one line when `indent` is `None`.
    fn write(&self, value: &Value, indent: Option<&str>) -> Option<String> {
        let Some(indent) = indent else {
            return serde_json::to_string(value).ok();
        };
        let mut bytes = Vec::new();
        let formatter = PrettyFormatter::with_indent(self.unit.as_bytes());
        value
            .serialize(&mut Serializer::with_formatter(&mut bytes, formatter))
            .ok()?;
        let text = String::from_utf8(bytes).ok()?;
        Some(text.replace('\n', &format!("{}{indent}", self.newline)))
    }
}

/// The indentation that whitespace before a value gives it, or `None` when it shares a line.
fn indent_of(lead: &str) -> Option<&str> {
    lead.rfind('\n').map(|at| &lead[at + 1..])
}

struct Child {
    key: Option<String>,
    /// Where the whitespace before the child starts: after the opening bracket or the comma.
    lead: usize,
    /// The key of a member, or the value of an element.
    start: usize,
    /// The bytes from a member's key to its value.
    separator: (usize, usize),
    value: (usize, usize),
}

enum Target<'a> {
    Key(&'a str),
    Index(usize),
}

fn rewrite(
    text: &str,
    start: usize,
    change: &Change,
    style: &Style,
    indent: Option<&str>,
    root: bool,
) -> Option<Rewrite> {
    let bytes = text.as_bytes();
    let (changes, append, object) = match change {
        Change::Remove => return Some(Rewrite::Removed),
        Change::Replace(value) => {
            let current: Value =
                serde_json::from_str(&text[start..value_end(bytes, start)?]).ok()?;
            if &current == value {
                return Some(Rewrite::Same);
            }
            return style.write(value, indent).map(Rewrite::Text);
        }
        Change::Object { members, append } => (
            members
                .iter()
                .map(|(key, change)| (Target::Key(key), change))
                .collect::<Vec<_>>(),
            append
                .iter()
                .map(|(key, value)| (Some(*key), value))
                .collect::<Vec<_>>(),
            true,
        ),
        Change::Array { elements, append } => (
            elements
                .iter()
                .map(|(index, change)| (Target::Index(*index), change))
                .collect(),
            append.iter().map(|value| (None, value)).collect(),
            false,
        ),
    };
    let (children, close) = container(bytes, start, object)?;
    for (target, change) in &changes {
        let present = match target {
            Target::Key(key) => children
                .iter()
                .any(|child| child.key.as_deref() == Some(*key)),
            Target::Index(index) => *index < children.len(),
        };
        // A change naming an absent member or element is not this document's shape.
        if !present && !matches!(change, Change::Remove) {
            return None;
        }
    }
    let mut survivors = Vec::with_capacity(children.len());
    let mut removed = false;
    let mut changed = false;
    for (index, child) in children.iter().enumerate() {
        let change = changes.iter().find_map(|(target, change)| {
            let matched = match target {
                Target::Key(key) => child.key.as_deref() == Some(*key),
                Target::Index(wanted) => *wanted == index,
            };
            matched.then_some(*change)
        });
        let lead = &text[child.lead..child.start];
        let value = match change {
            None => Rewrite::Same,
            Some(change) => rewrite(text, child.value.0, change, style, indent_of(lead), false)?,
        };
        let content = match value {
            Rewrite::Removed => {
                removed = true;
                continue;
            }
            Rewrite::Same => text[child.start..child.value.1].to_owned(),
            Rewrite::Text(value) => {
                changed = true;
                format!("{}{value}", &text[child.start..child.value.0])
            }
        };
        survivors.push((index, content));
    }
    if survivors.is_empty() && append.is_empty() && removed && !root {
        return Some(Rewrite::Removed);
    }
    if !removed && !changed && append.is_empty() {
        return Some(Rewrite::Same);
    }
    let separator = children
        .last()
        .map(|child| &text[child.separator.0..child.separator.1]);
    let write = |key: Option<&str>, value: &Value, lead: &str| -> Option<String> {
        let indent = indent_of(lead);
        let value = style.write(value, indent)?;
        Some(match key {
            None => value,
            Some(key) => {
                let separator = separator.unwrap_or(if indent.is_some() { ": " } else { ":" });
                format!("{}{separator}{value}", serde_json::to_string(key).ok()?)
            }
        })
    };
    // Only the root can end empty here; it keeps no whitespace of its removed members.
    let keeps_values = !survivors.is_empty() || !append.is_empty();
    let mut inner = String::new();
    if let (true, Some(first), Some(last)) = (keeps_values, children.first(), children.last()) {
        // Survivors keep the whitespace and comma before them; appended values follow the
        // whitespace before the last survivor. Removing values appended this way restores
        // exactly the bytes they were appended to.
        let head = &text[start + 1..first.start];
        inner.push_str(head);
        let mut lead = head;
        for (position, (index, content)) in survivors.iter().enumerate() {
            if position > 0 {
                let child = &children[*index];
                inner.push_str(&text[children[index - 1].value.1..child.start]);
                lead = &text[child.lead..child.start];
            }
            inner.push_str(content);
        }
        for (position, (key, value)) in append.iter().enumerate() {
            if !survivors.is_empty() || position > 0 {
                inner.push(',');
                inner.push_str(lead);
            }
            inner.push_str(&write(*key, value, lead)?);
        }
        inner.push_str(&text[last.value.1..close]);
    } else if !append.is_empty() {
        let line = line_indent(text, start);
        let lead = if style.pretty {
            format!("{}{line}{}", style.newline, style.unit)
        } else {
            String::new()
        };
        for (position, (key, value)) in append.iter().enumerate() {
            if position > 0 {
                inner.push(',');
            }
            inner.push_str(&lead);
            inner.push_str(&write(*key, value, &lead)?);
        }
        if style.pretty {
            inner.push_str(style.newline);
            inner.push_str(line);
        }
    }
    Some(Rewrite::Text(format!(
        "{}{inner}{}",
        &text[start..=start],
        &text[close..=close]
    )))
}

/// The leading whitespace of the line holding `at`.
fn line_indent(text: &str, at: usize) -> &str {
    let rest = &text[text[..at].rfind('\n').map_or(0, |found| found + 1)..];
    &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
}

/// Members or elements of the container opening at `open`, and its closing bracket.
fn container(bytes: &[u8], open: usize, object: bool) -> Option<(Vec<Child>, usize)> {
    let (opening, closing) = if object { (b'{', b'}') } else { (b'[', b']') };
    if bytes.get(open) != Some(&opening) {
        return None;
    }
    let mut children = Vec::new();
    let mut lead = open + 1;
    let mut at = skip_space(bytes, lead);
    if bytes.get(at) == Some(&closing) {
        return Some((children, at));
    }
    loop {
        let start = at;
        let (key, separator) = if object {
            let end = string_end(bytes, at)?;
            let key: String = serde_json::from_slice(&bytes[at..end]).ok()?;
            let colon = skip_space(bytes, end);
            if bytes.get(colon) != Some(&b':') {
                return None;
            }
            at = skip_space(bytes, colon + 1);
            (Some(key), (end, at))
        } else {
            (None, (at, at))
        };
        let end = value_end(bytes, at)?;
        children.push(Child {
            key,
            lead,
            start,
            separator,
            value: (at, end),
        });
        let next = skip_space(bytes, end);
        match bytes.get(next) {
            Some(b',') => {
                lead = next + 1;
                at = skip_space(bytes, lead);
            }
            Some(byte) if *byte == closing => return Some((children, next)),
            _ => return None,
        }
    }
}

fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    while matches!(bytes.get(at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        at += 1;
    }
    at
}

fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut at = start + 1;
    loop {
        match bytes.get(at)? {
            b'\\' => at += 2,
            b'"' => return Some(at + 1),
            _ => at += 1,
        }
    }
}

fn value_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'"' => string_end(bytes, start),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut at = start;
            loop {
                match bytes.get(at)? {
                    b'"' => {
                        at = string_end(bytes, at)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            return Some(at + 1);
                        }
                    }
                    _ => {}
                }
                at += 1;
            }
        }
        _ => {
            let mut at = start;
            while !matches!(
                bytes.get(at),
                None | Some(b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            ) {
                at += 1;
            }
            (at > start).then_some(at)
        }
    }
}

#[cfg(test)]
mod tests;
