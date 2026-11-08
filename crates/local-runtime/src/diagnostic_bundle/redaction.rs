//! Text redaction shared by every document a bundle carries.
use super::{CREDENTIAL_PLACEHOLDER, HOME_PLACEHOLDER, QUERY_PLACEHOLDER};
use serde_json::Value;
use std::ffi::OsString;

/// The folder variables, besides `ORCHBEE_*`, whose values can reach a carried document.
const FOLDER_VARIABLES: [&str; 4] = ["APPDATA", "LOCALAPPDATA", "TEMP", "TMP"];
/// A shorter value, such as a drive letter, would erase ordinary text.
const MINIMUM_VALUE: usize = 3;

/// Replaces the home folder and folder variable values with placeholders, and strips URL user
/// information and queries.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticRedactor {
    /// Values and their placeholders, longest value first, so a folder inside home keeps its own
    /// name.
    replacements: Vec<(String, String)>,
}

impl DiagnosticRedactor {
    /// `home` becomes `<home>`, and each named variable's value becomes `<env:NAME>`.
    pub fn new<'a>(
        home: Option<&str>,
        variables: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        let mut replacements: Vec<(String, String)> = variables
            .into_iter()
            .map(|(name, value)| (value.to_owned(), format!("<env:{name}>")))
            .chain(home.map(|home| (home.to_owned(), HOME_PLACEHOLDER.to_owned())))
            .map(|(value, placeholder)| {
                (value.trim_end_matches(['\\', '/']).to_owned(), placeholder)
            })
            .filter(|(value, _)| value.chars().count() >= MINIMUM_VALUE)
            .collect();
        replacements.sort_by_key(|(value, _)| std::cmp::Reverse(value.len()));
        replacements.dedup_by(|later, earlier| later.0.eq_ignore_ascii_case(&earlier.0));
        Self { replacements }
    }

    /// The home folder this process resolves its data root from, and every `ORCHBEE_*` and folder
    /// variable value.
    pub fn from_process_environment() -> Self {
        let home = std::env::var(crate::data_root::HOME_SOURCE).ok();
        let variables: Vec<(String, String)> = std::env::vars_os()
            .filter_map(|(name, value): (OsString, OsString)| {
                let name = name.into_string().ok()?;
                (name.starts_with("ORCHBEE_") || FOLDER_VARIABLES.contains(&name.as_str()))
                    .then_some((name, value.into_string().ok()?))
            })
            .collect();
        Self::new(
            home.as_deref(),
            variables
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )
    }

    /// One string with URL credentials, queries, folder values and the home folder replaced.
    /// Matching ignores ASCII case and accepts either path separator.
    pub fn scrub(&self, text: &str) -> String {
        let mut text = scrub_urls(text);
        for (value, placeholder) in &self.replacements {
            text = replace_ignoring_case(&text, value, placeholder);
            if value.contains('\\') {
                text = replace_ignoring_case(&text, &value.replace('\\', "/"), placeholder);
            }
        }
        text
    }

    /// Scrubs every string and object key of a document.
    pub fn scrub_json(&self, value: &mut Value) {
        match value {
            Value::String(text) => *text = self.scrub(text),
            Value::Array(items) => items.iter_mut().for_each(|item| self.scrub_json(item)),
            Value::Object(map) => {
                *map = std::mem::take(map)
                    .into_iter()
                    .map(|(key, mut value)| {
                        self.scrub_json(&mut value);
                        (self.scrub(&key), value)
                    })
                    .collect();
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
}

fn replace_ignoring_case(text: &str, needle: &str, replacement: &str) -> String {
    let haystack = text.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut output = String::with_capacity(text.len());
    let mut last = 0;
    for (index, _) in haystack.match_indices(&needle) {
        if index < last {
            continue;
        }
        output.push_str(&text[last..index]);
        output.push_str(replacement);
        last = index + needle.len();
    }
    output.push_str(&text[last..]);
    output
}

/// Replaces the user information of every `scheme://` URL with `<credential>` and its query with
/// `<query>`.
fn scrub_urls(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("://") {
        let (head, tail) = rest.split_at(index + 3);
        output.push_str(head);
        let end = tail
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '"' | '\'' | '<' | '>')
            })
            .unwrap_or(tail.len());
        let (url, after) = tail.split_at(end);
        let authority_end = url.find(['/', '?', '#']).unwrap_or(url.len());
        let (authority, path) = url.split_at(authority_end);
        match authority.rfind('@') {
            Some(at) => {
                output.push_str(CREDENTIAL_PLACEHOLDER);
                output.push_str(&authority[at..]);
            }
            None => output.push_str(authority),
        }
        match path.find('?') {
            Some(query) => {
                let fragment = path[query..]
                    .find('#')
                    .map_or(path.len(), |offset| query + offset);
                output.push_str(&path[..=query]);
                output.push_str(QUERY_PLACEHOLDER);
                output.push_str(&path[fragment..]);
            }
            None => output.push_str(path),
        }
        rest = after;
    }
    output.push_str(rest);
    output
}
