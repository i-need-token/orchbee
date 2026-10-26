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

/// A pure single-line text editor shared by settings, rename, workspace-setup drafts and
/// picker queries.
///
/// - `text` preserves the exact UTF-8 bytes supplied by prefill and edits; the
///   editor never normalizes, filters, or ANSI-parses input (character
///   admission is the typed-input boundary's job).
/// - `cursor` is a byte index that is always a valid extended-grapheme
///   boundary, possibly `text.len()`.
///
/// The editor holds no I/O, clock, or Client state. The horizontal window a
/// draft field shows is a pure consumer projection over this state measured
/// in the chrome-safe display space — the core does not own display layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LineEditor {
    text: String,
    cursor: usize,
    limit: LengthLimit,
}

impl LineEditor {
    /// Prefills a draft. `text` is kept byte-exact (including control scalars
    /// a protocol-legal label may carry); the cursor starts at the end.
    pub(crate) fn new(text: String, limit: LengthLimit) -> Self {
        Self {
            cursor: text.len(),
            text,
            limit,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
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
        let fits = match self.limit {
            LengthLimit::Scalars(maximum) => {
                self.text.chars().count() + inserted.chars().count() <= maximum
            }
            LengthLimit::Bytes(maximum) => self.text.len() + inserted.len() <= maximum,
        };
        if !fits {
            return Err(LengthLimitReached);
        }
        let intent = self.cursor + inserted.len();
        self.text.insert_str(self.cursor, inserted);
        self.cursor = boundary_at_or_after(&self.text, intent);
        Ok(())
    }

    /// Deletes the grapheme cluster before the cursor. Returns `false` at the
    /// start of the draft.
    pub(crate) fn backspace(&mut self) -> bool {
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
        let end = boundary_after(&self.text, self.cursor);
        if end == self.cursor {
            return false;
        }
        self.remove(self.cursor..end);
        true
    }

    /// Deletes the span from the last Unicode word boundary before the cursor
    /// to the cursor (`Ctrl+W`), keeping earlier whitespace runs. Returns
    /// `false` at the start of the draft.
    pub(crate) fn word_back(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let start = self.text[..self.cursor]
            .unicode_word_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
        self.remove(start..self.cursor);
        true
    }

    /// Clears the draft (`Ctrl+U`).
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Moves the cursor one grapheme cluster left/right. Returns `false` at
    /// the draft boundary.
    pub(crate) fn left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor = boundary_before(&self.text, self.cursor);
        true
    }

    pub(crate) fn right(&mut self) -> bool {
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
        if self.cursor == 0 {
            return false;
        }
        self.cursor = 0;
        true
    }

    pub(crate) fn end(&mut self) -> bool {
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
