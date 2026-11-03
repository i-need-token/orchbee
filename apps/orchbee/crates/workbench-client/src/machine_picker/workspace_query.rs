//! Workspace-name filtering and its local editor; typing never becomes list shortcuts or effects.
use super::{KeyCode, Mode, Modifiers, Picker, Selection};
use crate::terminal_text::sanitize_terminal_text;

impl Picker {
    pub(super) fn filter_workspaces(&mut self, selected: Option<Selection>) {
        let query = self.workspace_query.text().to_lowercase();
        self.workspace_matches = self
            .workspace_rows
            .iter()
            .filter(|row| {
                query.is_empty()
                    || row.workspace_label().is_some_and(|label| {
                        sanitize_terminal_text(label)
                            .to_lowercase()
                            .contains(&query)
                    })
            })
            .cloned()
            .collect();
        if self.mode == Mode::Workspaces {
            self.selected = selected
                .and_then(|selection| {
                    self.workspace_matches
                        .iter()
                        .position(|row| row.selection() == selection)
                })
                .unwrap_or_else(|| {
                    self.selected
                        .min(self.workspace_matches.len().saturating_sub(1))
                });
            self.top = self.top.min(self.workspace_matches.len().saturating_sub(1));
        }
    }

    fn insert_workspace_query(&mut self, text: &str) {
        if sanitize_terminal_text(text) != text {
            self.error = Some("workspace-query-invalid");
            return;
        }
        let selected = self.selection();
        if self.workspace_query.insert(text).is_err() {
            self.error = Some("workspace-query-limit");
            return;
        }
        self.error = None;
        self.filter_workspaces(selected);
    }

    /// A complete paste is one bounded edit, consumed by the picker's capture owner.
    pub(crate) fn paste_workspace_query(&mut self, text: &str) {
        if self.open && self.mode == Mode::Workspaces {
            self.insert_workspace_query(text);
        }
    }

    pub(super) fn workspace_query_key(&mut self, code: KeyCode, mods: Modifiers) -> bool {
        if self.mode != Mode::Workspaces {
            return false;
        }
        let selected = self.selection();
        match (code, mods) {
            (KeyCode::Char(character), mods)
                if mods == Modifiers::NONE || mods == Modifiers::NONE.with_shift() =>
            {
                let character = if mods.shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                };
                self.insert_workspace_query(&character.to_string());
                return true;
            }
            (KeyCode::Backspace, Modifiers::NONE) => {
                self.workspace_query.backspace();
            }
            (KeyCode::Delete, Modifiers::NONE) => {
                self.workspace_query.delete();
            }
            (KeyCode::Left, Modifiers::NONE) => {
                self.workspace_query.left();
            }
            (KeyCode::Right, Modifiers::NONE) => {
                self.workspace_query.right();
            }
            (KeyCode::Home, Modifiers::NONE) | (KeyCode::Char('a'), Modifiers::CTRL) => {
                self.workspace_query.home();
            }
            (KeyCode::End, Modifiers::NONE) | (KeyCode::Char('e'), Modifiers::CTRL) => {
                self.workspace_query.end();
            }
            (KeyCode::Char('w'), Modifiers::CTRL) => {
                self.workspace_query.word_back();
            }
            (KeyCode::Char('u'), Modifiers::CTRL) => self.workspace_query.clear(),
            _ => return false,
        }
        self.error = None;
        self.filter_workspaces(selected);
        true
    }
}
