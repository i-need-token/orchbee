use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

use super::line_editor::LineEditor;
use crate::terminal_text::{display_width, project_terminal_text};

/// The drawable window of a draft's single-line field inside a
/// `field_width`-cell area: the chrome-safe projection of the window's text
/// and the cursor's column inside that window. A window attests that what it
/// shows is drawable — it never claims the whole draft fits on screen.
pub(crate) struct FieldWindow<'a> {
    display: Cow<'a, str>,
    cursor_column: usize,
}

impl FieldWindow<'_> {
    /// Chrome-safe text of the visible window, whole grapheme clusters only.
    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    /// Cursor column inside the displayed window, in display cells.
    pub(crate) const fn cursor_column(&self) -> usize {
        self.cursor_column
    }
}

/// Pure projection over a draft's raw editor state. The raw text is projected
/// once through the chrome sanitizer — whole ANSI sequences stripped — and
/// the raw cursor lands inside the display text through the same parse's
/// byte map, so no raw middle fragment is ever re-parsed as if from ground
/// state. The window then packs whole display graphemes. A zero-width field
/// is not drawable and yields no window rather than a fake cursor position.
pub(super) fn field_window(editor: &LineEditor, field_width: usize) -> Option<FieldWindow<'_>> {
    if field_width == 0 {
        return None;
    }
    let projection = project_terminal_text(editor.text());
    let display: &str = projection.display();
    let visible_cursor = projection.cursor_boundary(editor.cursor());
    // The left edge is the earliest display boundary whose cursor prefix
    // already fits the field: the cursor lands inside the field's cells when
    // the draft overflows, showing the most context before it.
    let mut scroll = 0;
    while scroll < visible_cursor && display_width(&display[scroll..visible_cursor]) >= field_width
    {
        scroll = boundary_after(display, scroll);
    }
    // Pack whole display clusters from the left edge; a cluster wider than
    // the remaining space is never split.
    let mut used = 0;
    let mut end = scroll;
    for (offset, cluster) in display[scroll..].grapheme_indices(true) {
        let width = display_width(cluster);
        if used + width > field_width {
            break;
        }
        used += width;
        end = scroll + offset + cluster.len();
    }
    let window = match projection.display() {
        Cow::Borrowed(text) => Cow::Borrowed(&text[scroll..end]),
        Cow::Owned(text) => Cow::Owned(text[scroll..end].to_owned()),
    };
    Some(FieldWindow {
        display: window,
        cursor_column: display_width(&display[scroll..visible_cursor]),
    })
}

/// The smallest extended-grapheme boundary strictly after `index` — the next
/// cluster's start, or `text.len()`. `index` must be a grapheme boundary.
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
