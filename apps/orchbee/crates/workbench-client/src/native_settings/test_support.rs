//! Test-only presentation of the Settings editor's current draft, as the native painter reports it.

use super::{BrowseMode, Editor, SettingsDraw, State, Token};

impl Editor {
    pub(crate) fn draw(&mut self, token: Token) {
        let draw = match &self.state {
            State::Draft(draft) => match &draft.browse {
                BrowseMode::Picker(picker) => SettingsDraw::picker(
                    token,
                    if draft.follow_selection {
                        picker.cursor
                    } else {
                        picker.viewport
                    },
                    draft.visible_rows.max(1),
                ),
                _ => SettingsDraw {
                    token,
                    rows: Some((draft.viewport, draft.visible_rows.max(1))),
                    painted: if draft.follow_selection {
                        draft.row.mask()
                    } else {
                        0
                    },
                },
            },
            _ => SettingsDraw::surface(token),
        };
        self.presented(draw);
    }
}
