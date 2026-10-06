//! A local three-field draft and consumed save intent; no storage, transport or live application.
use orchbee_workbench_protocol::{
    ConfigurationThemeName, NativeSettingsChanges, NativeSettingsOperation, NativeSettingsSource,
    WorkbenchRevision,
};
mod input;
#[cfg(test)]
mod tests;
pub(crate) use input::key_edit;

/// One successful frame or operation identity; old identities never authorize a replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Token(u64);
/// Fully admitted current selections, independent of the disk representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Values {
    pub(crate) theme: ConfigurationThemeName,
    pub(crate) reminders: bool,
    pub(crate) sound: bool,
}
/// One frozen observation; saving compares both source bytes and publication revision.
pub(crate) struct Basis {
    pub(crate) revision: WorkbenchRevision,
    pub(crate) source: NativeSettingsSource,
    pub(crate) values: Values,
}
/// Exact selectable rows in the explicit-save overlay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Row {
    Theme,
    Reminders,
    Sound,
    Save,
}
impl Row {
    pub(crate) const ALL: [Self; 4] = [Self::Theme, Self::Reminders, Self::Sound, Self::Save];
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Theme => 0,
            Self::Reminders => 1,
            Self::Sound => 2,
            Self::Save => 3,
        }
    }
}
/// Input owns this semantic intent before any effect can be dispatched.
#[derive(Clone, Copy)]
pub(crate) enum Edit {
    Next,
    Previous,
    Select(Row),
    Adjust(bool),
    Activate,
    Cancel,
}
/// One already consumed operation; transport may submit it once.
pub(crate) struct Submission {
    pub(crate) token: Token,
    pub(crate) operation: NativeSettingsOperation,
}
/// Pure projection; a returned token is armed only by successful draw feedback.
#[derive(Clone, Copy)]
pub(crate) enum View {
    Loading {
        token: Token,
    },
    Draft {
        token: Token,
        values: Values,
        selected: Row,
        dirty: bool,
    },
    Saving {
        token: Token,
    },
}
impl View {
    pub(crate) const fn token(self) -> Token {
        match self {
            Self::Loading { token } | Self::Draft { token, .. } | Self::Saving { token } => token,
        }
    }
}
struct Draft {
    token: Token,
    basis: Basis,
    values: Values,
    row: Row,
    drawn: bool,
}
#[derive(Default)]
enum State {
    #[default]
    Closed,
    Loading {
        token: Token,
        visible: bool,
    },
    Draft(Draft),
    Saving {
        token: Token,
        visible: bool,
    },
}
/// This owner keeps one bounded episode and never queues or replays saves.
#[derive(Default)]
pub(crate) struct Editor {
    next: u64,
    state: State,
}
impl Editor {
    fn allocate(&mut self) -> Option<Token> {
        self.next = self.next.checked_add(1)?;
        Some(Token(self.next))
    }
    pub(crate) fn open(&mut self) -> Option<Token> {
        if !matches!(self.state, State::Closed) {
            return None;
        }
        let token = self.allocate()?;
        self.state = State::Loading {
            token,
            visible: true,
        };
        Some(token)
    }
    pub(crate) fn loaded(&mut self, token: Token, basis: Basis) -> bool {
        let State::Loading {
            token: current,
            visible,
        } = self.state
        else {
            return false;
        };
        if current != token {
            return false;
        }
        if !visible {
            self.state = State::Closed;
            return false;
        }
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            return false;
        };
        let values = basis.values;
        self.state = State::Draft(Draft {
            token,
            basis,
            values,
            row: Row::Theme,
            drawn: false,
        });
        true
    }
    pub(crate) fn complete(&mut self, token: Token) -> bool {
        if matches!(self.state, State::Loading { token: current, .. } | State::Saving { token: current, .. } if current == token)
        {
            self.state = State::Closed;
            true
        } else {
            false
        }
    }
    pub(crate) fn visible(&self) -> bool {
        self.view().is_some()
    }
    pub(crate) fn view(&self) -> Option<View> {
        match &self.state {
            State::Loading {
                token,
                visible: true,
            } => Some(View::Loading { token: *token }),
            State::Saving {
                token,
                visible: true,
            } => Some(View::Saving { token: *token }),
            State::Draft(draft) => Some(View::Draft {
                token: draft.token,
                values: draft.values,
                selected: draft.row,
                dirty: draft.values != draft.basis.values,
            }),
            _ => None,
        }
    }
    pub(crate) fn draw(&mut self, token: Token) {
        if let State::Draft(draft) = &mut self.state
            && draft.token == token
        {
            draft.drawn = true;
        }
    }
    pub(crate) fn edit(&mut self, edit: Edit) -> Option<Submission> {
        if matches!(edit, Edit::Cancel) {
            match &mut self.state {
                State::Loading { visible, .. } | State::Saving { visible, .. } => *visible = false,
                _ => self.state = State::Closed,
            }
            return None;
        }
        let State::Draft(draft) = &mut self.state else {
            return None;
        };
        if matches!(edit, Edit::Activate) && draft.row == Row::Save {
            if !draft.drawn || draft.values == draft.basis.values {
                return None;
            }
            let changes = NativeSettingsChanges {
                theme_name: (draft.values.theme != draft.basis.values.theme)
                    .then_some(draft.values.theme),
                background_reminders: (draft.values.reminders != draft.basis.values.reminders)
                    .then_some(draft.values.reminders),
                sound_enabled: (draft.values.sound != draft.basis.values.sound)
                    .then_some(draft.values.sound),
            };
            let submission = Submission {
                token: draft.token,
                operation: NativeSettingsOperation::Save {
                    expected_revision: draft.basis.revision,
                    source: draft.basis.source.clone(),
                    changes,
                },
            };
            self.state = State::Saving {
                token: submission.token,
                visible: true,
            };
            return Some(submission);
        }
        match edit {
            Edit::Next => draft.row = Row::ALL[(draft.row.index() + 1) % 4],
            Edit::Previous => draft.row = Row::ALL[(draft.row.index() + 3) % 4],
            Edit::Select(row) => draft.row = row,
            Edit::Adjust(forward) => adjust(draft, forward),
            Edit::Activate => adjust(draft, true),
            Edit::Cancel => {}
        }
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            return None;
        };
        if let State::Draft(draft) = &mut self.state {
            draft.token = token;
            draft.drawn = false;
        }
        None
    }
    pub(crate) fn click(&mut self, token: Token, row: Row) -> Option<Submission> {
        if !matches!(&self.state, State::Draft(draft) if draft.token == token && draft.drawn) {
            return None;
        }
        if row == Row::Save {
            if let State::Draft(draft) = &mut self.state {
                draft.row = Row::Save;
            }
            self.edit(Edit::Activate)
        } else {
            self.edit(Edit::Select(row));
            self.edit(Edit::Activate)
        }
    }
}
fn adjust(draft: &mut Draft, forward: bool) {
    match draft.row {
        Row::Theme => {
            use ConfigurationThemeName::*;
            draft.values.theme = match (draft.values.theme, forward) {
                (TerminalNative, true) | (OneDark, false) => OneLight,
                (OneLight, true) | (TerminalNative, false) => OneDark,
                _ => TerminalNative,
            };
        }
        Row::Reminders => draft.values.reminders = !draft.values.reminders,
        Row::Sound => draft.values.sound = !draft.values.sound,
        Row::Save => {}
    }
}
