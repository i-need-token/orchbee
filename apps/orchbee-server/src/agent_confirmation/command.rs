//! The command check: whether a command an Agent asks to run is one that must wait for a person.
//!
//! It reads text an Agent controls, so it is a trust boundary: pure, bounded, and without a panic
//! path. It lists what must wait, never what may run, and it does not resolve quoting, escapes
//! or expansion, so it never has to agree with a shell about them. Whatever it cannot read waits.
use super::Wait;
use super::place::{self, Scope};

mod families;

use families::{FAMILIES, Family, Rule};

const COMMAND_BYTES: usize = 2048;
/// A dialog wraps long commands; each break may be a wrap or a new line, and every reading of
/// every break is checked, so the count is bounded.
const BREAKS: usize = 5;

/// Words that start a compound statement; the check reads simple commands only.
const KEYWORDS: &[&str] = &[
    "!", "[[", "case", "catch", "do", "done", "elif", "else", "esac", "fi", "finally", "for",
    "foreach", "function", "if", "select", "switch", "then", "trap", "try", "until", "while",
];

/// Programs that fetch from a network; one in a command that also feeds a shell waits as a
/// download run as code.
const DOWNLOADERS: &[&str] = &[
    "curl",
    "wget",
    "iwr",
    "irm",
    "invoke-webrequest",
    "invoke-restmethod",
    "fetch",
];

/// Statements that wait in any command, whatever program carries them.
const STATEMENTS: &[&str] = &["drop", "truncate", "flushall", "flushdb"];

/// How strongly a reason speaks; the strongest one found is the one a person is told.
fn rank(wait: Wait) -> u8 {
    match wait {
        Wait::Destructive => 4,
        Wait::ProtectedFile => 3,
        // A place found in a command the check could not read may not be a place at all.
        Wait::LeavesWorkspace => 1,
        _ => 2,
    }
}

#[derive(Default)]
struct Found(Option<Wait>);

impl Found {
    fn add(&mut self, wait: Wait) {
        if self.0.is_none_or(|held| rank(wait) > rank(held)) {
            self.0 = Some(wait);
        }
    }
}

fn readable(c: char) -> bool {
    matches!(c, ' '..='~') || c.is_alphanumeric()
}

/// Removes what a shell may drop from a word before a program sees it, so a quoted or escaped
/// spelling of a listed word compares equal to the plain one.
fn bare(word: &str) -> String {
    word.chars()
        .filter(|c| !matches!(c, '"' | '\'' | '^' | '\\'))
        .collect()
}

/// The program a command word names: its last path step, without case and without a Windows
/// program suffix.
fn program(word: &str) -> String {
    let lower = word.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    for suffix in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    name.to_owned()
}

fn bundle(argument: &str, letters: &str) -> bool {
    argument
        .strip_prefix('-')
        .filter(|rest| !rest.starts_with('-'))
        .is_some_and(|rest| rest.chars().any(|c| letters.contains(c)))
}

/// `host:path`, the form a remote copy takes; a drive letter is one character and is not one.
fn remote(argument: &str) -> bool {
    argument
        .split_once(':')
        .is_some_and(|(host, _)| host.len() >= 2 && !host.contains(['/', '\\']))
}

struct Words<'a> {
    /// Arguments with quotes and escapes removed, without case.
    lower: &'a [String],
    /// The same arguments as written.
    exact: &'a [String],
}

fn applies(rule: &Rule, words: &Words<'_>) -> bool {
    let has = |list: &[&str]| words.lower.iter().any(|word| list.contains(&word.as_str()));
    match rule {
        Rule::Always => true,
        Rule::Word(list) => has(list),
        Rule::Exact(list) => words.exact.iter().any(|word| list.contains(&word.as_str())),
        Rule::Prefix(list) => words
            .lower
            .iter()
            .any(|word| list.iter().any(|prefix| word.starts_with(prefix))),
        Rule::Starts(list) => words
            .exact
            .iter()
            .any(|word| list.iter().any(|prefix| word.starts_with(prefix))),
        Rule::Bundle(letters) => words.lower.iter().any(|word| bundle(word, letters)),
        Rule::Pair(first, second) => has(first) && has(second),
        Rule::Bare => !words.lower.iter().any(|word| !word.starts_with('-')),
        Rule::Setting => {
            words
                .exact
                .windows(2)
                .any(|pair| pair[0] == "-c" && pair[1].contains('='))
                || words
                    .exact
                    .iter()
                    .any(|word| word.starts_with("--config-env"))
        }
        Rule::Remote => words.exact.iter().any(|word| remote(word)),
        Rule::Glob => words
            .exact
            .iter()
            .any(|word| word.contains(['*', '?', '['])),
    }
}

fn family(name: &str) -> impl Iterator<Item = &'static Family> {
    FAMILIES.iter().filter(move |family| {
        family.names.contains(&name) || family.stems.iter().any(|stem| name.starts_with(stem))
    })
}

/// A variable in an argument: a home form names a place outside the Workspace, and any other
/// form names something the check cannot see.
fn variable(argument: &str) -> Option<Wait> {
    let lower = argument.to_lowercase();
    // `%NAME%`: a name between two percent signs, wherever the pair stands.
    let parts: Vec<&str> = lower.split('%').collect();
    let percent = parts.len() > 2
        && parts[1..parts.len() - 1].iter().any(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '(' | ')'))
        });
    if !lower.contains('$') && !percent {
        return None;
    }
    let home = [
        "$home",
        "${home}",
        "$env:userprofile",
        "$env:home",
        "%userprofile%",
        "%homepath%",
    ]
    .iter()
    .any(|form| lower.contains(form));
    Some(if home {
        Wait::LeavesWorkspace
    } else {
        Wait::NotUnderstood
    })
}

/// Places an argument names. The whole argument is judged, then what follows an `=`, then a
/// drive path that starts inside it, so `--out=C:\x` and `/out:C:\x` are placed too.
fn places(argument: &str, scope: Scope<'_>, found: &mut Found) {
    let mut judge = |text: &str| {
        if let Err(wait) = place::argument(text, scope) {
            found.add(wait);
        }
    };
    judge(argument);
    if let Some((_, value)) = argument.split_once('=') {
        judge(value);
    }
    let bytes = argument.as_bytes();
    for start in 1..bytes.len().saturating_sub(2) {
        if bytes[start].is_ascii_alphabetic()
            && bytes[start + 1] == b':'
            && matches!(bytes[start + 2], b'/' | b'\\')
            && !bytes[start - 1].is_ascii_alphanumeric()
        {
            judge(&argument[start..]);
        }
    }
}

#[derive(Default)]
struct Pipeline {
    downloads: bool,
    feeds_shell: bool,
}

/// Turns redirection operators into spaces, so that their targets are placed like any other
/// argument, and drops a redirection to the null device: the one place outside the Workspace a
/// command may write to.
fn without_redirections(command: &str) -> String {
    let mut plain = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(at) = rest.find(['<', '>']) {
        plain.push_str(&rest[..at]);
        plain.push(' ');
        let after = rest[at + 1..].trim_start_matches(['<', '>', ' ']);
        let end = after.find(' ').unwrap_or(after.len());
        let target = bare(&after[..end]).to_lowercase();
        rest = if matches!(target.as_str(), "/dev/null" | "nul" | "$null") {
            &after[end..]
        } else {
            &rest[at + 1..]
        };
    }
    plain.push_str(rest);
    plain
}

/// Whether a command word, without quotes and escapes, is spelled like a program name or a path
/// to one. A method call, an expansion and a history reference are not.
fn names_a_program(word: &str) -> bool {
    let shape = word
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | ':' | '+' | '-'));
    let start = match word.chars().next() {
        Some('.') => word == "." || word.contains('/'),
        Some(first) => first.is_alphanumeric() || first == '/',
        None => false,
    };
    word == "[" || (shape && start)
}

fn simple(command: &str, scope: Scope<'_>, pipeline: &mut Pipeline, found: &mut Found) {
    let command = without_redirections(command);
    let mut words = command.split_whitespace();
    let Some(first) = words.next() else {
        return;
    };
    let spelled = bare(first);
    if first.contains(['"', '\'', '^', '\\', '$', '%', '`', '=', '~']) || !names_a_program(&spelled)
    {
        found.add(Wait::NotUnderstood);
    }
    if spelled.split('/').any(|step| step == "..") {
        found.add(Wait::LeavesWorkspace);
    }
    let name = program(&spelled);
    if KEYWORDS.contains(&name.as_str()) {
        found.add(Wait::NotUnderstood);
    }
    // `cmd` reads `rd/s/q build` as `rd` with two switches: a listed program glued to what
    // follows it waits as that program would.
    if let Some((head, _)) = spelled.to_lowercase().split_once('/')
        && head != "."
    {
        for family in family(head) {
            found.add(family.wait);
        }
    }
    let written: Vec<&str> = words.collect();
    let exact: Vec<String> = written.iter().map(|word| bare(word)).collect();
    let lower: Vec<String> = exact.iter().map(|word| word.to_lowercase()).collect();
    let arguments = Words {
        lower: &lower,
        exact: &exact,
    };
    for family in family(&name) {
        if family.rules.iter().any(|rule| applies(rule, &arguments)) {
            found.add(family.wait);
            pipeline.feeds_shell |= family.shell;
        }
    }
    pipeline.downloads |= DOWNLOADERS.contains(&name.as_str());
    let statement = |word: &String| {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        STATEMENTS.contains(&word)
    };
    let deletes_rows = lower
        .windows(2)
        .any(|pair| pair[0].ends_with("delete") && pair[1] == "from");
    if lower.iter().any(statement) || STATEMENTS.contains(&name.as_str()) || deletes_rows {
        found.add(Wait::Destructive);
    }
    for word in &written {
        // Quotes and a caret never change which place a word names; a backslash is a Windows
        // path separator and stays.
        let argument: String = word
            .chars()
            .filter(|c| !matches!(c, '"' | '\'' | '^'))
            .collect();
        if let Some(wait) = variable(&argument) {
            found.add(wait);
        }
        places(&argument, scope, found);
    }
}

/// One reading of the command, with every line break resolved.
fn reading(text: &str, scope: Scope<'_>, found: &mut Found) {
    let quotes = |quote: char| text.chars().filter(|c| *c == quote).count() % 2 == 1;
    let continued = text.lines().any(|line| line.trim_end().ends_with('\\'));
    if text.contains("$(")
        || text.contains('`')
        || text.contains("<(")
        || text.contains(">(")
        || text.contains("<<")
        || quotes('"')
        || quotes('\'')
        || continued
    {
        found.add(Wait::NotUnderstood);
    }
    let mut pipeline = Pipeline::default();
    let mut any = false;
    for command in text.split([';', '&', '|', '\n', '(', ')', '{', '}']) {
        any |= !command.trim().is_empty();
        simple(command, scope, &mut pipeline, found);
    }
    if !any {
        found.add(Wait::NotUnderstood);
    }
    if pipeline.downloads && pipeline.feeds_shell {
        found.add(Wait::Destructive);
    }
}

/// Judges a command shown on `lines` screen rows, already without the dialog's own gutter.
/// `Ok` means no reading of it is one that must wait.
pub(crate) fn check(lines: &[String], scope: Scope<'_>) -> Result<(), Wait> {
    let bytes: usize = lines.iter().map(String::len).sum();
    if lines.is_empty() || bytes == 0 || bytes > COMMAND_BYTES || lines.len() > BREAKS + 1 {
        return Err(Wait::NotUnderstood);
    }
    if lines.iter().any(|line| line.contains('…')) {
        return Err(Wait::NotVisible);
    }
    if !lines.iter().all(|line| line.chars().all(readable)) {
        return Err(Wait::NotUnderstood);
    }
    let mut found = Found::default();
    if !place::settled(scope) {
        found.add(Wait::LeavesWorkspace);
    }
    let breaks = lines.len() - 1;
    let joints = ["", " ", "\n"];
    for mut choice in 0..joints.len().pow(u32::try_from(breaks).unwrap_or(0)) {
        let mut text = String::with_capacity(bytes + breaks);
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                text.push_str(joints[choice % joints.len()]);
                choice /= joints.len();
            }
            text.push_str(line);
        }
        reading(&text, scope, &mut found);
    }
    found.0.map_or(Ok(()), Err)
}
