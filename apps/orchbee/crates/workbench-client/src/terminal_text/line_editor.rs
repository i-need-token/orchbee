//! Owns grapheme-safe single-line editing, including the readline-style kill, yank and word keys,
//! while enforcing each field's scalar or UTF-8-byte budget against the complete resulting value.
//! It does not admit domain syntax, persist a value, touch the system clipboard, or decide when a
//! draft is committed.

use crate::terminal_events::keys::{KeyCode, KeyEvent, KeyKind, Modifiers};
use unicode_segmentation::UnicodeSegmentation;

/// The protocol length budget for one editable field: a scalar count for
/// workbench labels or a UTF-8 byte count for filesystem paths. The budget is
/// checked against the exact resulting text, never against grapheme clusters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LengthLimit {
    Scalars(usize),
    Bytes(usize),
}

/// An insertion or paste exceeded the field's [`LengthLimit`]. The rejection
/// is atomic: text and cursor are both unchanged.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LengthLimitReached;

/// The readline-style editing keys every single-line field shares on top of its plain
/// navigation keys. They are fixed editing conventions, not configurable `[keys]` actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineKey {
    /// `Ctrl+U`: kill from the start of the draft to the cursor.
    KillToStart,
    /// `Ctrl+K`: kill from the cursor to the end of the draft.
    KillToEnd,
    /// `Ctrl+W`: kill from the previous Unicode word start to the cursor.
    KillWordBack,
    /// `Alt+D`: kill from the cursor to the next Unicode word end.
    KillWordForward,
    /// `Alt+B`: move to the previous Unicode word start.
    WordLeft,
    /// `Alt+F`: move to the next Unicode word end.
    WordRight,
    /// `Ctrl+Y`: insert the most recent kill at the cursor.
    Yank,
}

impl LineKey {
    /// The shared key for a decoded host key, matched on the exact normalized modifier set so
    /// `Ctrl+Shift+K` or `Ctrl+Alt+B` never alias a field key.
    pub(crate) fn for_key(event: KeyEvent) -> Option<Self> {
        if event.kind == KeyKind::Release {
            return None;
        }
        let (KeyCode::Char(character), mods) = event.normalized() else {
            return None;
        };
        if mods == Modifiers::CTRL {
            Some(match character {
                'u' => Self::KillToStart,
                'k' => Self::KillToEnd,
                'w' => Self::KillWordBack,
                'y' => Self::Yank,
                _ => return None,
            })
        } else if mods == Modifiers::NONE.with_alt() {
            Some(match character {
                'b' => Self::WordLeft,
                'f' => Self::WordRight,
                'd' => Self::KillWordForward,
                _ => return None,
            })
        } else {
            None
        }
    }

    /// Whether the key can change the draft's text (every key but word movement).
    pub(crate) const fn edits_text(self) -> bool {
        !matches!(self, Self::WordLeft | Self::WordRight)
    }

    /// Whether the key is an explicit deletion, which clears a draft's input fault the way
    /// Backspace does; a yank is an insertion and never clears one.
    pub(crate) const fn deletes_text(self) -> bool {
        !matches!(self, Self::WordLeft | Self::WordRight | Self::Yank)
    }
}

/// A pure single-line text editor shared by settings, rename, workspace-setup drafts and
/// picker queries.
///
/// - `text` preserves the exact UTF-8 bytes supplied by prefill and edits; the
///   editor never normalizes, filters, or ANSI-parses input (character
///   admission is the typed-input boundary's job).
/// - `cursor` is a byte index that is always a valid extended-grapheme
///   boundary, possibly `text.len()`.
///
/// - `kill` holds the most recent kill in memory for `Ctrl+Y`. Consecutive kills extend it
///   (backward kills prepend, forward kills append), as readline does; any other edit or
///   movement ends the chain. It never reaches the system clipboard.
/// - `selected` marks the whole prefilled value as selected, as a name field opens: an insertion
///   (typing, paste or yank) replaces it, a deletion or kill clears it, and a movement ends it
///   with the cursor at the start (left, Home, word left) or the end (right, End, word right).
///   Only [`Self::select_all`] sets it, so a field that never calls it never selects.
///
/// The editor holds no I/O, clock, or Client state. The horizontal window a
/// draft field shows is a pure consumer projection over this state measured
/// in the chrome-safe display space — the core does not own display layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LineEditor {
    text: String,
    cursor: usize,
    limit: LengthLimit,
    kill: String,
    kill_chain: bool,
    selected: bool,
}

impl LineEditor {
    /// Prefills a draft. `text` is kept byte-exact (including control scalars
    /// a protocol-legal label may carry); the cursor starts at the end.
    pub(crate) fn new(text: String, limit: LengthLimit) -> Self {
        Self {
            cursor: text.len(),
            text,
            limit,
            kill: String::new(),
            kill_chain: false,
            selected: false,
        }
    }

    /// Selects the whole draft with the cursor at its end, as a prefilled name opens; an empty
    /// draft has nothing to select.
    #[must_use]
    pub(crate) fn select_all(mut self) -> Self {
        self.cursor = self.text.len();
        self.selected = !self.text.is_empty();
        self
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Whether the whole draft is selected.
    pub(crate) const fn selected(&self) -> bool {
        self.selected
    }

    /// Byte index of the cursor; always an extended-grapheme boundary.
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    /// Inserts `inserted` at the cursor; typed input and paste share this one
    /// atomic path. Rejects without touching any state when the resulting text
    /// would exceed the length budget. The intended cursor lands at the end
    /// of the inserted span; when the edit merges it into a new grapheme (a
    /// ZWJ joining the neighbors, say) it snaps forward to that cluster's end.
    pub(crate) fn insert(&mut self, inserted: &str) -> Result<(), LengthLimitReached> {
        self.kill_chain = false;
        // A selected draft is replaced, so only the insertion counts against the budget.
        let kept = if self.selected {
            ""
        } else {
            self.text.as_str()
        };
        let fits = match self.limit {
            LengthLimit::Scalars(maximum) => {
                kept.chars().count() + inserted.chars().count() <= maximum
            }
            LengthLimit::Bytes(maximum) => kept.len() + inserted.len() <= maximum,
        };
        if !fits {
            return Err(LengthLimitReached);
        }
        if std::mem::take(&mut self.selected) {
            self.text.clear();
            self.cursor = 0;
        }
        let intent = self.cursor + inserted.len();
        self.text.insert_str(self.cursor, inserted);
        self.cursor = boundary_at_or_after(&self.text, intent);
        Ok(())
    }

    /// Deletes the grapheme cluster before the cursor. Returns `false` at the
    /// start of the draft.
    pub(crate) fn backspace(&mut self) -> bool {
        self.kill_chain = false;
        if self.clear_selection() {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        let start = boundary_before(&self.text, self.cursor);
        self.remove(start..self.cursor);
        true
    }

    /// Deletes the grapheme cluster after the cursor. Returns `false` at the
    /// end of the draft.
    pub(crate) fn delete(&mut self) -> bool {
        self.kill_chain = false;
        if self.clear_selection() {
            return true;
        }
        let end = boundary_after(&self.text, self.cursor);
        if end == self.cursor {
            return false;
        }
        self.remove(self.cursor..end);
        true
    }

    /// Clears the draft programmatically (a reset, not the `Ctrl+U` kill); the kill buffer is kept.
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.kill_chain = false;
        self.selected = false;
    }

    /// Clears a selected draft, as a deletion does; returns whether there was a selection.
    fn clear_selection(&mut self) -> bool {
        if !std::mem::take(&mut self.selected) {
            return false;
        }
        self.text.clear();
        self.cursor = 0;
        true
    }

    /// Ends a selection with the cursor at the draft's start or end; returns whether there was
    /// a selection.
    fn collapse_selection(&mut self, to_end: bool) -> bool {
        if !std::mem::take(&mut self.selected) {
            return false;
        }
        self.cursor = if to_end { self.text.len() } else { 0 };
        true
    }

    /// Applies one shared readline key. Returns whether text or cursor changed; only `Yank` can
    /// be rejected, atomically, by the length budget.
    pub(crate) fn apply(&mut self, key: LineKey) -> Result<bool, LengthLimitReached> {
        if self.selected {
            match key {
                // A kill takes the whole selected draft, so a yank can bring it back.
                LineKey::KillToStart
                | LineKey::KillToEnd
                | LineKey::KillWordBack
                | LineKey::KillWordForward => {
                    self.selected = false;
                    self.kill_chain = false;
                    return Ok(self.kill(0..self.text.len(), Direction::Forward));
                }
                LineKey::WordLeft | LineKey::WordRight => {
                    self.kill_chain = false;
                    return Ok(self.collapse_selection(key == LineKey::WordRight));
                }
                LineKey::Yank => {}
            }
        }
        Ok(match key {
            LineKey::KillToStart => self.kill(0..self.cursor, Direction::Backward),
            LineKey::KillToEnd => self.kill(self.cursor..self.text.len(), Direction::Forward),
            LineKey::KillWordBack => self.word_back(),
            LineKey::KillWordForward => {
                self.kill(self.cursor..self.next_word_end(), Direction::Forward)
            }
            LineKey::WordLeft => self.move_to(self.previous_word_start()),
            LineKey::WordRight => self.move_to(self.next_word_end()),
            LineKey::Yank => return self.yank(),
        })
    }

    /// Kills the span from the last Unicode word boundary before the cursor
    /// to the cursor (`Ctrl+W`), keeping earlier whitespace runs. Returns
    /// `false` at the start of the draft.
    fn word_back(&mut self) -> bool {
        self.kill(self.previous_word_start()..self.cursor, Direction::Backward)
    }

    /// Inserts the most recent kill at the cursor through the one atomic insert path.
    fn yank(&mut self) -> Result<bool, LengthLimitReached> {
        if self.kill.is_empty() {
            self.kill_chain = false;
            return Ok(false);
        }
        let kill = std::mem::take(&mut self.kill);
        let inserted = self.insert(&kill);
        self.kill = kill;
        inserted.map(|()| true)
    }

    /// The start of the Unicode word at or before the cursor, snapped to a grapheme boundary.
    fn previous_word_start(&self) -> usize {
        let start = self.text[..self.cursor]
            .unicode_word_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
        boundary_at_or_before(&self.text, start)
    }

    /// The end of the first Unicode word ending after the cursor, snapped to a grapheme boundary.
    fn next_word_end(&self) -> usize {
        let end = self.text[self.cursor..]
            .unicode_word_indices()
            .next()
            .map_or(self.text.len(), |(index, word)| {
                self.cursor + index + word.len()
            });
        boundary_at_or_after(&self.text, end)
    }

    fn move_to(&mut self, target: usize) -> bool {
        self.kill_chain = false;
        if target == self.cursor {
            return false;
        }
        self.cursor = target;
        true
    }

    /// Removes `range` into the kill buffer, extending it when the previous edit was also a kill.
    /// An empty range kills nothing and leaves the buffer and chain untouched.
    fn kill(&mut self, range: std::ops::Range<usize>, direction: Direction) -> bool {
        if range.is_empty() {
            return false;
        }
        if !self.kill_chain {
            self.kill.clear();
        }
        let removed = &self.text[range.clone()];
        match direction {
            Direction::Backward => self.kill.insert_str(0, removed),
            Direction::Forward => self.kill.push_str(removed),
        }
        self.kill_chain = true;
        self.remove(range);
        true
    }

    /// Moves the cursor one grapheme cluster left/right. Returns `false` at
    /// the draft boundary.
    pub(crate) fn left(&mut self) -> bool {
        self.kill_chain = false;
        if self.collapse_selection(false) {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        self.cursor = boundary_before(&self.text, self.cursor);
        true
    }

    pub(crate) fn right(&mut self) -> bool {
        self.kill_chain = false;
        if self.collapse_selection(true) {
            return true;
        }
        let next = boundary_after(&self.text, self.cursor);
        if next == self.cursor {
            return false;
        }
        self.cursor = next;
        true
    }

    /// Moves the cursor to the start/end of the draft (`Home`/`Ctrl+A`,
    /// `End`/`Ctrl+E`). Returns `false` when already there.
    pub(crate) fn home(&mut self) -> bool {
        self.kill_chain = false;
        if self.collapse_selection(false) {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        self.cursor = 0;
        true
    }

    pub(crate) fn end(&mut self) -> bool {
        self.kill_chain = false;
        if self.collapse_selection(true) {
            return true;
        }
        if self.cursor == self.text.len() {
            return false;
        }
        self.cursor = self.text.len();
        true
    }

    /// Removes `range` (grapheme boundaries) with the deletion intent anchored
    /// at the span start; when the removal merges neighbors into one cluster
    /// containing that intent, the cursor snaps forward to the cluster's end.
    fn remove(&mut self, range: std::ops::Range<usize>) {
        let start = range.start;
        self.text.replace_range(range, "");
        self.cursor = boundary_at_or_after(&self.text, start);
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Backward,
    Forward,
}

/// The largest extended-grapheme boundary at or before `index`.
fn boundary_at_or_before(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    text.grapheme_indices(true)
        .map(|(offset, _)| offset)
        .take_while(|offset| *offset <= index)
        .last()
        .unwrap_or(0)
}

/// The smallest extended-grapheme boundary at or after `index` — snaps an
/// intent position inside a merged cluster forward to the cluster's end.
fn boundary_at_or_after(text: &str, index: usize) -> usize {
    if index == 0 || index >= text.len() {
        return index.min(text.len());
    }
    text.grapheme_indices(true)
        .find_map(|(offset, cluster)| {
            (offset < index && index < offset + cluster.len()).then_some(offset + cluster.len())
        })
        .unwrap_or(index)
}

/// The largest grapheme boundary strictly before `index` — the previous
/// cluster's start, or 0. `index` must be a grapheme boundary.
fn boundary_before(text: &str, index: usize) -> usize {
    text[..index]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(offset, _)| offset)
}

/// The smallest grapheme boundary strictly after `index` — the next cluster's
/// start, or `text.len()`. `index` must be a grapheme boundary.
fn boundary_after(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    text[index..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(text.len(), |(offset, _)| index + offset)
}

#[cfg(test)]
mod tests;
