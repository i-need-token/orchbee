//! The fallback record, the pointer's `extensions.fallback`: the versions nothing may make active
//! again, and the move that made an entry active, with its Client and Session windows. Every object
//! is closed and every value bounded, so the record stays under 2 KiB; a `fallback` member of any
//! other shape counts as absent. Its shape never changes: a later need adds another member of
//! `extensions`. This module owns the format only; the store's writers own the transitions.
use crate::{
    pointer::{Entry, Pointer},
    strict_json::closed_object,
    tokens::valid_version,
};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Each role's window: UTC wall-clock milliseconds from its first record after the move.
pub const FALLBACK_WINDOW_MILLISECONDS: u64 = 600_000;
/// The failures of one role inside its window that send the store back.
pub const FALLBACK_FAILURES: usize = 3;
/// The most versions `held` lists, and the most generations `starts` lists.
const HELD_LIMIT: usize = 8;
const STARTS_LIMIT: usize = 8;
/// The pointer's `extensions` member that holds the record.
const MEMBER: &str = "fallback";

/// The whole record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fallback {
    /// The versions nothing may make active, at most eight, in the order they were held. Writers
    /// keep only those above the active version.
    pub held: Vec<String>,
    /// The move that made `last_move.entry` active. It counts only while that entry is `active`.
    pub last_move: Move,
}

/// One move, written by the completion that made `entry` active or by a fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Move {
    /// The entry this move made active, exactly as in `active`.
    pub entry: Entry,
    /// Whether this move may fall back; settled when the move is written.
    pub back: bool,
    /// True only on the move a fallback writes: writers restore migrated records while it lasts.
    pub restore: bool,
    /// The Client window, absent until opened.
    pub client: Option<ClientWindow>,
    /// The Session window, absent until opened.
    pub session: Option<SessionWindow>,
}

impl Move {
    /// A move that has opened no window.
    pub fn fresh(entry: Entry, back: bool, restore: bool) -> Self {
        Self {
            entry,
            back,
            restore,
            client: None,
            session: None,
        }
    }
}

/// The Client window: when it opened, in Unix milliseconds UTC, and the Client failures since.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientWindow {
    pub since: u64,
    /// 0 to [`FALLBACK_FAILURES`].
    pub failures: u8,
}

/// The Session window: when it opened, the activation identifiers of the generations the moved
/// version started, at most eight, and those of them that failed, at most three.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionWindow {
    pub since: u64,
    pub starts: Vec<String>,
    pub failed: Vec<String>,
}

/// Whether `text` is an activation identifier: 32 lowercase hexadecimal digits.
pub fn valid_activation_id(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The helper that records a failed start or join of the Client at
/// `<root>/versions/<version>/<name>`, given the pointer read again after it failed:
/// `<root>/versions/<previous>/<name>` with `<version>`, when the pointer still makes `<version>`
/// active through a counting move whose `back` is true and names a `previous`. Any other pointer, or
/// none, records nothing. Either launcher, and a macOS or Linux TUI that no launcher started, run the helper
/// through the previous version, because the failed Client may be what broke and `back` proves that
/// the previous version records.
pub fn client_failure_helper(client: &Path, pointer: Option<&[u8]>) -> Option<(PathBuf, String)> {
    let name = client.file_name()?;
    let folder = client.parent()?;
    let version = folder.file_name()?.to_str()?;
    let pointer = Pointer::parse(pointer?)?;
    let previous = pointer.previous.as_ref()?;
    let counting = pointer.counting_move()?;
    if !counting.back || pointer.active.product != version {
        return None;
    }
    Some((
        folder.parent()?.join(&previous.product).join(name),
        version.to_owned(),
    ))
}

impl Pointer {
    /// The fallback record, or `None` when the pointer has none or one of any other shape.
    pub fn fallback(&self) -> Option<Fallback> {
        Fallback::parse(self.extension(MEMBER)?)
    }

    /// The record's move when it counts: its entry is the active one. Otherwise the pointer has no
    /// counting move.
    pub fn counting_move(&self) -> Option<Move> {
        self.fallback()
            .map(|fallback| fallback.last_move)
            .filter(|last_move| last_move.entry == self.active)
    }

    /// Replaces the fallback record, keeping every other member of `extensions`.
    pub fn set_fallback(&mut self, fallback: &Fallback) {
        self.set_extension(MEMBER, fallback.to_value());
    }
}

impl Fallback {
    fn parse(value: &Value) -> Option<Self> {
        let object = closed_object(value, &["held", "move"], &[])?;
        let held = strings(object.get("held")?, HELD_LIMIT, valid_version)?;
        Some(Self {
            held,
            last_move: Move::parse(object.get("move")?)?,
        })
    }

    fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("held".into(), list(&self.held));
        object.insert("move".into(), self.last_move.to_value());
        Value::Object(object)
    }
}

impl Move {
    fn parse(value: &Value) -> Option<Self> {
        let object = closed_object(value, &["entry", "back"], &["restore", "client", "session"])?;
        let restore = match object.get("restore") {
            None => false,
            Some(Value::Bool(true)) => true,
            Some(_) => return None,
        };
        let client = match object.get("client") {
            None => None,
            Some(value) => Some(ClientWindow::parse(value)?),
        };
        let session = match object.get("session") {
            None => None,
            Some(value) => Some(SessionWindow::parse(value)?),
        };
        Some(Self {
            entry: Entry::parse(object.get("entry")?)?,
            back: object.get("back")?.as_bool()?,
            restore,
            client,
            session,
        })
    }

    fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("entry".into(), self.entry.to_value());
        object.insert("back".into(), Value::Bool(self.back));
        if self.restore {
            object.insert("restore".into(), Value::Bool(true));
        }
        if let Some(client) = &self.client {
            let mut window = Map::new();
            window.insert("since".into(), client.since.into());
            window.insert("failures".into(), client.failures.into());
            object.insert("client".into(), Value::Object(window));
        }
        if let Some(session) = &self.session {
            let mut window = Map::new();
            window.insert("since".into(), session.since.into());
            window.insert("starts".into(), list(&session.starts));
            window.insert("failed".into(), list(&session.failed));
            object.insert("session".into(), Value::Object(window));
        }
        Value::Object(object)
    }
}

impl ClientWindow {
    fn parse(value: &Value) -> Option<Self> {
        let object = closed_object(value, &["since", "failures"], &[])?;
        let failures = object.get("failures")?.as_u64()?;
        (failures <= FALLBACK_FAILURES as u64).then_some(Self {
            since: object.get("since")?.as_u64()?,
            failures: u8::try_from(failures).ok()?,
        })
    }
}

impl SessionWindow {
    fn parse(value: &Value) -> Option<Self> {
        let object = closed_object(value, &["since", "starts", "failed"], &[])?;
        let starts = strings(object.get("starts")?, STARTS_LIMIT, valid_activation_id)?;
        let failed = strings(
            object.get("failed")?,
            FALLBACK_FAILURES,
            valid_activation_id,
        )?;
        failed
            .iter()
            .all(|failure| starts.contains(failure))
            .then_some(Self {
                since: object.get("since")?.as_u64()?,
                starts,
                failed,
            })
    }
}

/// An array of at most `limit` distinct strings that each pass `valid`.
fn strings(value: &Value, limit: usize, valid: fn(&str) -> bool) -> Option<Vec<String>> {
    let array = value.as_array()?;
    if array.len() > limit {
        return None;
    }
    let mut items: Vec<String> = Vec::with_capacity(array.len());
    for item in array {
        let text = item.as_str().filter(|text| valid(text))?;
        if items.iter().any(|seen| seen == text) {
            return None;
        }
        items.push(text.to_owned());
    }
    Some(items)
}

fn list(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::String).collect())
}
