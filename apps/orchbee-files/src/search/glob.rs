//! VS Code's glob matching at ef5ca45 (`base/common/glob.ts`), which decides whether a file rg
//! reported is one a request's includes and excludes admit, and which paths an exclude exception
//! re-includes. A pattern is trimmed, then matched by the shortcut VS Code takes for its shape or
//! by an automaton equivalent to the regular expression `parseRegExp` writes, over UTF-16 code
//! units as JavaScript compares them; a pattern whose expression JavaScript would reject matches
//! nothing. Requests carry no sibling (`when`) clause.

use super::js::{self, BACKSLASH, SEPARATOR, SLASH};

/// The folders an exception never looks into unless it names them.
const VERSION_CONTROL_FOLDERS: [&str; 4] = [".git", ".svn", ".hg", ".jj"];

/// One pattern as VS Code parses it.
enum Pattern {
    /// `**/*.ext`: the path ends with `.ext`.
    EndsWith(Vec<u16>),
    /// `**/name`: the basename is `name`.
    Basename(Vec<u16>),
    /// `**/a/b` (`ends`) or `a/b`: the path is the target or, for `ends`, ends with it after a
    /// separator, in the platform's form or, on Windows, with slashes.
    Path {
        native: Vec<u16>,
        posix: Vec<u16>,
        ends: bool,
    },
    /// `{**/*.a,**/b}`: any of these.
    Any(Vec<Pattern>),
    /// Any other pattern.
    Expression(Automaton),
}

/// A set of patterns, as `parse` reads an expression whose keys are all enabled: a path matches
/// when any pattern does.
pub(super) struct Globs {
    patterns: Vec<Pattern>,
    ignore_case: bool,
}

impl Globs {
    /// The patterns `keys`; a key VS Code cannot parse is left out.
    pub(super) fn new<'a>(keys: impl IntoIterator<Item = &'a str>, ignore_case: bool) -> Self {
        Self {
            patterns: keys
                .into_iter()
                .filter_map(|key| parse(key, ignore_case))
                .collect(),
            ignore_case,
        }
    }

    /// Whether any pattern matches `path`, whose last segment is `basename` when given.
    pub(super) fn matches(&self, path: &[u16], basename: Option<&[u16]>) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(path, basename, self.ignore_case))
    }
}

/// An exclude expression whose `!` keys are exceptions, as `parseExcludeExpression` reads it.
pub(super) struct Excludes {
    plain: Globs,
    exceptions: Vec<String>,
    excepted: Globs,
    skipped: Vec<&'static str>,
}

impl Excludes {
    pub(super) fn new(keys: &[String], ignore_case: bool) -> Self {
        let plain = keys
            .iter()
            .map(String::as_str)
            .filter(|key| !is_exception(key));
        let exceptions = exceptions(keys);
        let excepted = exceptions
            .iter()
            .flat_map(|exception| [exception.clone(), format!("{exception}/**")])
            .collect::<Vec<_>>();
        Self {
            plain: Globs::new(plain, ignore_case),
            skipped: skipped_folders(&exceptions),
            excepted: Globs::new(excepted.iter().map(String::as_str), ignore_case),
            exceptions,
        }
    }

    /// Whether `path` is excluded: a plain pattern matches it and no exception covers it.
    pub(super) fn matches(&self, path: &[u16], basename: Option<&[u16]>) -> bool {
        self.plain.matches(path, basename) && !self.is_excepted(path, basename)
    }

    /// Whether an exception matches `path` or a folder above it, outside version control
    /// folders the exceptions do not name.
    pub(super) fn is_excepted(&self, path: &[u16], basename: Option<&[u16]>) -> bool {
        if self.exceptions.is_empty() || !self.excepted.matches(path, basename) {
            return false;
        }
        let mut end = path.len();
        while end > 0 && path[end - 1] == SLASH {
            end -= 1;
        }
        while end > 0 && path[end - 1] == BACKSLASH {
            end -= 1;
        }
        let trimmed = &path[..end];
        trimmed.is_empty()
            || !trimmed
                .split(|unit| *unit == SLASH || *unit == BACKSLASH)
                .any(|segment| {
                    self.skipped
                        .iter()
                        .any(|folder| js::units(folder) == segment)
                })
    }
}

/// Whether an exclude key is an exception: it starts with `!` after leading whitespace.
pub(super) fn is_exception(key: &str) -> bool {
    key.trim_start_matches(js::is_space_char).starts_with('!')
}

/// VS Code's `getExcludeExceptions`: each exception's pattern, without the `!`, surrounding
/// whitespace and trailing slashes; an empty or absolute one is dropped.
pub(super) fn exceptions(keys: &[String]) -> Vec<String> {
    let mut exceptions = Vec::new();
    for key in keys.iter().filter(|key| is_exception(key)) {
        let start = key.trim_start_matches(js::is_space_char);
        let pattern = js::trim(&start[1..]).trim_end_matches('/');
        if !pattern.is_empty()
            && !is_absolute(pattern)
            && !exceptions.iter().any(|seen| seen == pattern)
        {
            exceptions.push(pattern.to_owned());
        }
    }
    exceptions
}

/// Node's `path.isAbsolute` on this platform.
fn is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    if cfg!(windows) {
        matches!(bytes.first(), Some(b'/' | b'\\'))
            || (bytes.len() > 2
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'/' | b'\\'))
    } else {
        bytes.first() == Some(&b'/')
    }
}

/// VS Code's `getFoldersSkippedByExcludeExceptions`.
pub(super) fn skipped_folders(exceptions: &[String]) -> Vec<&'static str> {
    let named: Vec<String> = exceptions
        .iter()
        .flat_map(|exception| expand_braces(exception))
        .flat_map(|pattern| split_glob_aware(&pattern, '/'))
        .collect();
    VERSION_CONTROL_FOLDERS
        .into_iter()
        .filter(|folder| !named.iter().any(|segment| segment == folder))
        .collect()
}

/// `glob.ts`'s `expandBraces`: one brace group at a time, without nesting or escapes.
fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(start) = pattern.find('{') else {
        return vec![pattern.to_owned()];
    };
    let Some(end) = pattern[start..].find('}').map(|end| start + end) else {
        return vec![pattern.to_owned()];
    };
    let prefix = &pattern[..start];
    let mut alternatives = split_glob_aware(&pattern[start + 1..end], ',');
    if alternatives.is_empty() {
        alternatives.push(String::new());
    }
    let suffixes = expand_braces(&pattern[end + 1..]);
    alternatives
        .iter()
        .flat_map(|alternative| {
            suffixes
                .iter()
                .map(move |suffix| format!("{prefix}{alternative}{suffix}"))
        })
        .collect()
}

/// VS Code's `splitGlobAware`: splits on `split` outside braces and brackets, dropping an empty
/// tail.
pub(super) fn split_glob_aware(pattern: &str, split: char) -> Vec<String> {
    let mut segments = Vec::new();
    let (mut in_braces, mut in_brackets) = (false, false);
    let mut current = String::new();
    for ch in pattern.chars() {
        if ch == split && !in_braces && !in_brackets {
            segments.push(std::mem::take(&mut current));
            continue;
        }
        match ch {
            '{' => in_braces = true,
            '}' => in_braces = false,
            '[' => in_brackets = true,
            ']' => in_brackets = false,
            _ => {}
        }
        current.push(ch);
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

/// `parsePattern`: `None` is VS Code's `NULL`, which matches nothing.
fn parse(key: &str, ignore_case: bool) -> Option<Pattern> {
    if key.is_empty() {
        return None;
    }
    let pattern = js::trim(key);
    if let Some(extension) = trivial_extension(pattern) {
        return Some(Pattern::EndsWith(js::units(extension)));
    }
    if let Some(name) = trivial_basename(pattern) {
        return Some(Pattern::Basename(js::units(name)));
    }
    if let Some(alternatives) = trivial_alternatives(pattern) {
        let mut parsed: Vec<Pattern> = alternatives
            .split(',')
            .filter_map(|alternative| parse(alternative, ignore_case))
            .collect();
        return match parsed.len() {
            0 => None,
            1 => parsed.pop(),
            _ => Some(Pattern::Any(parsed)),
        };
    }
    if let Some(target) = trivial_path_end(pattern) {
        return Some(path_pattern(target, true));
    }
    if let Some(target) = trivial_path(pattern) {
        return Some(path_pattern(target, false));
    }
    regexp(pattern)
        .ok()
        .map(|node| Pattern::Expression(Automaton::new(&node, ignore_case)))
}

fn path_pattern(target: &str, ends: bool) -> Pattern {
    let posix = js::units(target);
    let native = posix
        .iter()
        .map(|unit| if *unit == SLASH { SEPARATOR } else { *unit })
        .collect();
    Pattern::Path {
        native,
        posix,
        ends,
    }
}

/// `[\w.-]`.
fn is_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

fn is_words(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_word)
}

/// T1, `**/*.something`: the extension with its dot.
fn trivial_extension(pattern: &str) -> Option<&str> {
    let rest = pattern.strip_prefix("**/*.")?;
    is_words(rest).then(|| &pattern[4..])
}

/// T2, `**/something` with an optional trailing slash: the name.
fn trivial_basename(pattern: &str) -> Option<&str> {
    let rest = pattern.strip_prefix("**/")?;
    let name = rest.strip_suffix('/').unwrap_or(rest);
    is_words(name).then_some(name)
}

/// T3, `{**/*.something,**/else}`: the text between the braces.
fn trivial_alternatives(pattern: &str) -> Option<&str> {
    let inner = pattern.strip_prefix('{')?.strip_suffix('}')?;
    inner
        .split(',')
        .all(|alternative| {
            let Some(rest) = alternative.strip_prefix("**/") else {
                return false;
            };
            let rest = rest.strip_prefix('*').unwrap_or(rest);
            is_words(rest.strip_suffix('/').unwrap_or(rest))
        })
        .then_some(inner)
}

/// T4, `**/something/else`: the target after the first slash.
fn trivial_path_end(pattern: &str) -> Option<&str> {
    let rest = pattern.strip_prefix("**")?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let target = rest.strip_prefix('/')?;
    target.split('/').all(is_words).then_some(target)
}

/// T5, `something/else`.
fn trivial_path(pattern: &str) -> Option<&str> {
    let target = pattern.strip_suffix('/').unwrap_or(pattern);
    target.split('/').all(is_words).then_some(target)
}

impl Pattern {
    fn matches(&self, path: &[u16], basename: Option<&[u16]>, ignore_case: bool) -> bool {
        let equals = |a: &[u16], b: &[u16]| {
            if ignore_case {
                js::equals_ignore_case(a, b)
            } else {
                a == b
            }
        };
        let ends_with = |text: &[u16], candidate: &[u16]| {
            if ignore_case {
                js::ends_with_ignore_case(text, candidate)
            } else {
                text.ends_with(candidate)
            }
        };
        match self {
            Self::EndsWith(extension) => ends_with(path, extension),
            Self::Basename(name) => match basename.filter(|basename| !basename.is_empty()) {
                Some(basename) => equals(basename, name),
                None => {
                    equals(path, name)
                        || ends_with(path, &[&[SLASH], name.as_slice()].concat())
                        || ends_with(path, &[&[BACKSLASH], name.as_slice()].concat())
                }
            },
            Self::Path {
                native,
                posix,
                ends,
            } => {
                let windows = SEPARATOR != SLASH;
                let found = |target: &[u16], separator: u16| {
                    equals(path, target)
                        || (*ends && ends_with(path, &[&[separator], target].concat()))
                };
                found(native, SEPARATOR) || (windows && found(posix, SLASH))
            }
            Self::Any(patterns) => patterns
                .iter()
                .any(|pattern| pattern.matches(path, basename, ignore_case)),
            Self::Expression(automaton) => automaton.matches(path),
        }
    }
}

/// The regular expression `parseRegExp` writes, as a tree.
enum Node {
    Unit(u16),
    Class(Class),
    /// `[^/\\]`.
    NotSeparator,
    /// `[/\\]`.
    Separator,
    /// `.`.
    NotLineEnd,
    Sequence(Vec<Node>),
    Either(Vec<Node>),
    /// Zero or more.
    Repeat(Box<Node>),
}

/// A character class as JavaScript reads the text `parseRegExp` writes between brackets.
#[derive(Clone)]
struct Class {
    negated: bool,
    ranges: Vec<(u16, u16)>,
}

/// JavaScript would reject the expression.
struct Invalid;

/// `parseRegExp`.
fn regexp(pattern: &str) -> Result<Node, Invalid> {
    if pattern.is_empty() {
        return Ok(Node::Sequence(Vec::new()));
    }
    let segments = split_glob_aware(pattern, '/');
    if segments.iter().all(|segment| segment == "**") {
        return Ok(Node::Repeat(Box::new(Node::NotLineEnd)));
    }
    let mut nodes = Vec::new();
    let mut previous_globstar = false;
    for (index, segment) in segments.iter().enumerate() {
        if segment == "**" {
            if !previous_globstar {
                nodes.push(globstar(index == segments.len() - 1));
            }
            previous_globstar = true;
            continue;
        }
        let (mut in_braces, mut brace) = (false, String::new());
        let (mut in_brackets, mut bracket) = (false, String::new());
        for ch in segment.chars() {
            if ch != '}' && in_braces {
                brace.push(ch);
                continue;
            }
            if in_brackets && (ch != ']' || bracket.is_empty()) {
                match ch {
                    '-' => bracket.push('-'),
                    '^' | '!' if bracket.is_empty() => bracket.push('^'),
                    '/' => {}
                    _ => bracket.push_str(&js::escape_regexp(ch.encode_utf8(&mut [0; 4]))),
                }
                continue;
            }
            match ch {
                '{' => in_braces = true,
                '[' => in_brackets = true,
                '}' => {
                    let choices = split_glob_aware(&brace, ',')
                        .iter()
                        .map(|choice| regexp(choice))
                        .collect::<Result<Vec<_>, _>>()?;
                    nodes.push(if choices.is_empty() {
                        Node::Sequence(Vec::new())
                    } else {
                        Node::Either(choices)
                    });
                    in_braces = false;
                    brace.clear();
                }
                ']' => {
                    nodes.push(Node::Class(class(&bracket)?));
                    in_brackets = false;
                    bracket.clear();
                }
                '?' => nodes.push(Node::NotSeparator),
                '*' => nodes.push(Node::Repeat(Box::new(Node::NotSeparator))),
                _ => nodes.extend(
                    ch.encode_utf16(&mut [0; 2])
                        .iter()
                        .map(|unit| Node::Unit(*unit)),
                ),
            }
        }
        if index + 1 < segments.len() && (segments[index + 1] != "**" || index + 2 < segments.len())
        {
            nodes.push(Node::Separator);
        }
        previous_globstar = false;
    }
    Ok(Node::Sequence(nodes))
}

/// `**`: separators, or names followed by separators, and when last also a separator followed
/// by a name.
fn globstar(last: bool) -> Node {
    let name = || {
        Node::Sequence(vec![
            Node::NotSeparator,
            Node::Repeat(Box::new(Node::NotSeparator)),
        ])
    };
    let mut alternatives = vec![
        Node::Separator,
        Node::Sequence(vec![name(), Node::Separator]),
    ];
    if last {
        alternatives.push(Node::Sequence(vec![Node::Separator, name()]));
    }
    Node::Repeat(Box::new(Node::Either(alternatives)))
}

/// The class JavaScript reads from `text`, the inside of a bracket expression without the `u`
/// flag: an escaped unit stands for itself, and a range whose start exceeds its end is an error.
fn class(text: &str) -> Result<Class, Invalid> {
    let units = js::units(text);
    let negated = units.first() == Some(&u16::from(b'^'));
    let mut index = usize::from(negated);
    let atom = |index: &mut usize| {
        let mut unit = units[*index];
        if unit == BACKSLASH && *index + 1 < units.len() {
            *index += 1;
            unit = units[*index];
        }
        *index += 1;
        unit
    };
    let mut ranges = Vec::new();
    while index < units.len() {
        let start = atom(&mut index);
        if index + 1 < units.len() && units[index] == u16::from(b'-') {
            index += 1;
            let end = atom(&mut index);
            if start > end {
                return Err(Invalid);
            }
            ranges.push((start, end));
        } else {
            ranges.push((start, start));
        }
    }
    Ok(Class { negated, ranges })
}

impl Class {
    /// Whether the class holds `unit`; under `i`, whether it holds a unit with the same
    /// canonical form, which a wide range is asked for by the unit's own case forms.
    fn contains(&self, unit: u16, ignore_case: bool) -> bool {
        let canonical = js::canonicalize(unit);
        let found = self.ranges.iter().any(|&(start, end)| {
            let range = start..=end;
            if range.contains(&unit) {
                return true;
            }
            if !ignore_case {
                return false;
            }
            if end - start < 0x100 {
                return range
                    .clone()
                    .any(|member| js::canonicalize(member) == canonical);
            }
            let lower = js::to_lower(&[unit]);
            range.contains(&canonical) || (lower.len() == 1 && range.contains(&lower[0]))
        });
        found != self.negated
    }
}

/// A Thompson automaton for one anchored expression, simulated over every state at once, so a
/// pattern of many `**` stays linear in the path's length.
struct Automaton {
    states: Vec<State>,
    start: usize,
    ignore_case: bool,
}

enum State {
    Accept,
    Consume(Test, usize),
    Split(usize, usize),
}

enum Test {
    Unit(u16),
    Class(Class),
    NotSeparator,
    Separator,
    NotLineEnd,
}

impl Automaton {
    fn new(node: &Node, ignore_case: bool) -> Self {
        let mut states = vec![State::Accept];
        let start = compile(node, 0, &mut states);
        Self {
            states,
            start,
            ignore_case,
        }
    }

    fn matches(&self, path: &[u16]) -> bool {
        let mut seen = vec![usize::MAX; self.states.len()];
        let mut current = Vec::new();
        self.close(self.start, 0, &mut seen, &mut current);
        for (position, unit) in path.iter().enumerate() {
            let mut next = Vec::new();
            for state in &current {
                if let State::Consume(test, to) = &self.states[*state]
                    && self.passes(test, *unit)
                {
                    self.close(*to, position + 1, &mut seen, &mut next);
                }
            }
            if next.is_empty() {
                return false;
            }
            current = next;
        }
        current.contains(&0)
    }

    fn close(&self, state: usize, step: usize, seen: &mut [usize], into: &mut Vec<usize>) {
        let mut pending = vec![state];
        while let Some(state) = pending.pop() {
            if seen[state] == step {
                continue;
            }
            seen[state] = step;
            match self.states[state] {
                State::Split(first, second) => {
                    pending.push(second);
                    pending.push(first);
                }
                State::Accept | State::Consume(..) => into.push(state),
            }
        }
    }

    fn passes(&self, test: &Test, unit: u16) -> bool {
        match test {
            Test::Unit(expected) => {
                *expected == unit
                    || (self.ignore_case && js::canonicalize(*expected) == js::canonicalize(unit))
            }
            Test::Class(class) => class.contains(unit, self.ignore_case),
            Test::NotSeparator => unit != SLASH && unit != BACKSLASH,
            Test::Separator => unit == SLASH || unit == BACKSLASH,
            Test::NotLineEnd => !matches!(unit, 0x0A | 0x0D | 0x2028 | 0x2029),
        }
    }
}

/// Compiles `node` to enter before `next` and returns its first state.
fn compile(node: &Node, next: usize, states: &mut Vec<State>) -> usize {
    let consume = |test: Test, states: &mut Vec<State>| {
        states.push(State::Consume(test, next));
        states.len() - 1
    };
    match node {
        Node::Unit(unit) => consume(Test::Unit(*unit), states),
        Node::Class(class) => consume(Test::Class(class.clone()), states),
        Node::NotSeparator => consume(Test::NotSeparator, states),
        Node::Separator => consume(Test::Separator, states),
        Node::NotLineEnd => consume(Test::NotLineEnd, states),
        Node::Sequence(nodes) => nodes
            .iter()
            .rev()
            .fold(next, |entry, node| compile(node, entry, states)),
        Node::Either(alternatives) => {
            let entries: Vec<usize> = alternatives
                .iter()
                .map(|alternative| compile(alternative, next, states))
                .collect();
            entries
                .into_iter()
                .rev()
                .reduce(|rest, entry| {
                    states.push(State::Split(entry, rest));
                    states.len() - 1
                })
                .unwrap_or(next)
        }
        Node::Repeat(body) => {
            states.push(State::Split(next, next));
            let split = states.len() - 1;
            let entry = compile(body, split, states);
            states[split] = State::Split(entry, next);
            split
        }
    }
}
