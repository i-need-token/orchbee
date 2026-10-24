//! Workspace-name filtering and its local editor; typing never becomes list shortcuts or effects.
use super::{
    Control, Frame, KeyCode, Locale, Mode, Modifiers, Picker, PickerAction, PickerSource, Rect,
    Row, Selection, Theme, agents::AgentAction, picker_interaction,
};
use crate::{
    terminal_session::NativeContextLocation,
    terminal_text::{display_width, sanitize_terminal_text, unique_identity_suffix},
    tui_pointer::{HitLayer, HitMapBuilder},
    tui_render::controls::{ActionKind, QueryField, render_query_field},
};
use orchbee_workbench_protocol::SiteId;

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

    /// Same-named Workspaces on one site carry the shortest unique ID suffix, as in
    /// the Sidebar, so a search result names its exact target before it opens.
    pub(super) fn workspace_identity_suffix(&self, row: &Row) -> String {
        let Some((site, label, id)) = workspace_identity(row) else {
            return String::new();
        };
        let peers = self
            .workspace_rows
            .iter()
            .filter(|peer| peer.choice == row.choice)
            .filter_map(workspace_identity)
            .filter(|(peer_site, peer_label, _)| *peer_site == site && *peer_label == label)
            .map(|(_, _, peer_id)| peer_id)
            .collect::<Vec<_>>();
        if peers.len() > 1 {
            unique_identity_suffix(id, &peers)
        } else {
            String::new()
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

    pub(super) fn render_workspace_query(
        &self,
        frame: &mut Frame<'_>,
        inner: &mut Rect,
        locale: Locale,
        theme: &Theme,
        source: &PickerSource,
        hits: &mut HitMapBuilder,
    ) {
        if self.mode != Mode::Workspaces || inner.height < 4 {
            return;
        }
        let mut field = Rect::new(inner.x, inner.y, inner.width, 1);
        let clear_label = locale.machine_picker_text("picker-clear-hint");
        let clear_width = u16::try_from(display_width(clear_label) + 4).unwrap_or(u16::MAX);
        if field.width > clear_width.saturating_add(16) {
            let area = Rect::new(field.right() - clear_width, field.y, clear_width, 1);
            if let Some(painted) = self.render_agent_action(
                frame,
                AgentAction {
                    area,
                    control: Control::Clear,
                    label: clear_label,
                    kind: ActionKind::Secondary,
                },
                theme,
                source,
                hits,
            ) {
                field.width = painted.x.saturating_sub(field.x).saturating_sub(1);
            }
        }
        // Typing in Workspaces mode always reaches this query, so it keeps its editing form.
        let _ = render_query_field(
            frame,
            field,
            QueryField::Editing {
                label: locale.machine_picker_text("workspace-search"),
                query: self.workspace_query.text(),
                cursor: self.workspace_query.cursor(),
            },
            theme,
        );
        if let Some((target, interaction)) =
            picker_interaction(source, PickerAction::Control(Control::Search))
        {
            hits.record_interaction(HitLayer::Overlay, field, field, target, interaction);
        }
        inner.y += 1;
        inner.height -= 1;
    }
}

fn workspace_identity(row: &Row) -> Option<(&SiteId, &str, &str)> {
    let context = row.context.as_ref()?;
    let NativeContextLocation::Workspace { workspace_id, .. } = &context.target.location else {
        return None;
    };
    Some((
        &context.target.site_id,
        context.workspace.as_deref()?,
        workspace_id.as_str(),
    ))
}
