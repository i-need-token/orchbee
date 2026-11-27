//! VS Code's `coalesceEvents`, which merges one delivery window's raw events. Per path, an add
//! followed by a delete cancels out, a delete followed by an add becomes a change, an add stays an
//! add through changes, and otherwise the latest kind wins; where names ignore case, a path that
//! returns spelled in another case keeps both its delete and its add, so a rename that changes only
//! case shows. Deletes come first, shortest path first, without those below another deleted path,
//! then adds and changes in arrival order. Paths compare as VS Code compares them: keys by
//! lower-casing, and prefixes by folding ASCII letters and lower-casing from the first other
//! character, with lengths in UTF-16 code units.

use std::collections::HashMap;

use orchbee_files_protocol::ChangeKind;

use crate::scope::path::NativePath;

/// One raw or coalesced event of the item at `path`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Event<P> {
    pub(crate) kind: ChangeKind,
    /// The item's native path.
    pub(crate) path: P,
    /// The item is or was a directory as far as the watcher knows; a merged event keeps it when
    /// any of its events did.
    pub(crate) directory: bool,
}

/// A native path as text, which coalescing compares.
pub(crate) trait Spelling {
    fn spelling(&self) -> &str;
}

impl Spelling for String {
    fn spelling(&self) -> &str {
        self
    }
}

impl Spelling for NativePath {
    fn spelling(&self) -> &str {
        self.as_str()
    }
}

/// How the machine's paths compare.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Paths {
    pub(crate) case_sensitive: bool,
    pub(crate) separator: char,
}

/// The coalesced form of `events`, which arrived in this order.
pub(crate) fn coalesce<P: Spelling>(
    events: impl IntoIterator<Item = Event<P>>,
    paths: Paths,
) -> Vec<Event<P>> {
    // Kept events in arrival order; a cancelled one leaves `None` behind.
    let mut kept: Vec<Option<Event<P>>> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for event in events {
        let key = paths.key(event.path.spelling());
        let existing = by_key.get(&key).copied().and_then(|index| {
            let existing = kept.get_mut(index).and_then(Option::as_mut)?;
            Some((index, existing))
        });
        let keep = match existing {
            None => true,
            Some((_, existing))
                if existing.path.spelling() != event.path.spelling()
                    && matches!(event.kind, ChangeKind::Deleted | ChangeKind::Added) =>
            {
                true
            }
            Some((index, existing)) => {
                match (existing.kind, event.kind) {
                    (ChangeKind::Added, ChangeKind::Deleted) => {
                        kept[index] = None;
                        by_key.remove(&key);
                    }
                    (ChangeKind::Deleted, ChangeKind::Added) => {
                        existing.kind = ChangeKind::Changed;
                        existing.directory |= event.directory;
                    }
                    (ChangeKind::Added, ChangeKind::Changed) => {
                        existing.directory |= event.directory;
                    }
                    (_, kind) => {
                        existing.kind = kind;
                        existing.directory |= event.directory;
                    }
                }
                false
            }
        };
        if keep {
            by_key.insert(key, kept.len());
            kept.push(Some(event));
        }
    }
    let (mut deletes, others): (Vec<Event<P>>, Vec<Event<P>>) = kept
        .into_iter()
        .flatten()
        .partition(|event| event.kind == ChangeKind::Deleted);
    // A stable sort, as JavaScript's is.
    deletes.sort_by_key(|event| utf16_length(event.path.spelling()));
    let mut coalesced: Vec<Event<P>> = Vec::new();
    for event in deletes {
        let below_deleted = coalesced
            .iter()
            .any(|deleted| paths.is_parent(event.path.spelling(), deleted.path.spelling()));
        if !below_deleted {
            coalesced.push(event);
        }
    }
    coalesced.extend(others);
    coalesced
}

impl Paths {
    fn key(self, path: &str) -> String {
        if self.case_sensitive {
            path.to_owned()
        } else {
            path.to_lowercase()
        }
    }

    /// Whether `candidate` is a strict parent of `path`, as VS Code's `isParent` decides.
    fn is_parent(self, path: &str, candidate: &str) -> bool {
        if path.is_empty() || candidate.is_empty() || path == candidate {
            return false;
        }
        if utf16_length(candidate) > utf16_length(path) {
            return false;
        }
        let mut prefix = candidate.to_owned();
        if !prefix.ends_with(self.separator) {
            prefix.push(self.separator);
        }
        if self.case_sensitive {
            path.starts_with(&prefix)
        } else {
            starts_with_ignoring_case(path, &prefix)
        }
    }
}

fn utf16_length(text: &str) -> usize {
    text.encode_utf16().count()
}

/// VS Code's `startsWithIgnoreCase`: ASCII letters fold to upper case, and from the first other
/// code unit that differs, the rest of both compare lower-cased.
fn starts_with_ignoring_case(text: &str, prefix: &str) -> bool {
    let text: Vec<u16> = text.encode_utf16().collect();
    let prefix: Vec<u16> = prefix.encode_utf16().collect();
    let Some(head) = text.get(..prefix.len()) else {
        return false;
    };
    let upper = |unit: u16| {
        if (u16::from(b'a')..=u16::from(b'z')).contains(&unit) {
            unit - 32
        } else {
            unit
        }
    };
    for (index, (&ours, &theirs)) in head.iter().zip(&prefix).enumerate() {
        if ours == theirs {
            continue;
        }
        if ours >= 128 || theirs >= 128 {
            let rest = |units: &[u16]| String::from_utf16_lossy(&units[index..]).to_lowercase();
            return rest(head) == rest(&prefix);
        }
        if upper(ours) != upper(theirs) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
