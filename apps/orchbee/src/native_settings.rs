//! A bounded settings draft. Storage, transport and live application remain separate owners.
use crate::terminal_text::line_editor::{LengthLimit, LineEditor};
use orchbee_workbench_protocol::{
    NativeSettingsOperation, NativeSettingsSource, PreferenceKey, UserPreferencesPatch,
    UserPreferencesSnapshot, UserPreferencesValues,
};
mod input;
mod rows;
#[cfg(test)]
mod tests;
mod values;
pub(crate) use input::key_edit;
pub(crate) use rows::{Page, Row};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Token(u64);
/// Painted settings controls retain the same draft authority as keyboard edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Control {
    Row(Row),
    Adjust { row: Row, forward: bool },
    Accept,
    Back,
    Close,
}
pub(crate) type Basis = UserPreferencesSnapshot;
#[derive(Clone, Copy, Debug)]
pub(crate) enum Edit {
    Next,
    Previous,
    #[cfg(test)]
    Select(Row),
    Adjust(bool),
    Activate,
    Cancel,
    Insert(char),
    Backspace,
    WordBack,
    Delete,
    Home,
    End,
    Clear,
}
pub(crate) struct Submission {
    pub(crate) token: Token,
    pub(crate) operation: NativeSettingsOperation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Confirmation {
    ResetAll,
    ResetProfiles,
    Reload,
    DeleteProfile,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Observation {
    Unchanged,
    Refreshed,
    Conflict,
}
#[derive(Clone, Copy)]
pub(crate) struct DraftView<'a> {
    pub(crate) token: Token,
    pub(crate) values: &'a UserPreferencesValues,
    pub(crate) selected: Row,
    pub(crate) dirty: bool,
    pub(crate) page: Page,
    pub(crate) profile: usize,
    pub(crate) argument: usize,
    pub(crate) text: Option<(&'a str, usize)>,
    pub(crate) invalid: bool,
    pub(crate) confirmation: Option<Confirmation>,
}
impl DraftView<'_> {
    pub(crate) fn enabled(self, row: Row) -> bool {
        use orchbee_workbench_protocol::UserLaunchCommand;
        let catalog = &self.values.launch_profiles;
        let profile = &catalog.profiles[self.profile];
        let arguments = match &profile.command {
            UserLaunchCommand::CustomExecutable { arguments, .. } => Some(arguments),
            _ => None,
        };
        match row {
            Row::Save => self.dirty,
            Row::Foreground | Row::Background => {
                self.values.terminal_display_theme
                    == orchbee_workbench_protocol::TerminalDisplayAppearance::Custom
            }
            Row::Profile => catalog.profiles.len() > 1,
            Row::Login => {
                !cfg!(windows) && matches!(profile.command, UserLaunchCommand::SystemShell { .. })
            }
            Row::Executable => arguments.is_some(),
            Row::Argument | Row::ArgumentValue | Row::DeleteArgument => {
                arguments.is_some_and(|args| self.argument < args.len())
            }
            Row::AddArgument => arguments.is_some_and(|args| args.len() < 128),
            Row::SetDefault => profile.id != catalog.default_profile_id,
            Row::AddProfile => catalog.profiles.len() < 64,
            Row::DeleteProfile => catalog.profiles.len() > 1,
            _ => true,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum View<'a> {
    Loading { token: Token },
    Draft(DraftView<'a>),
    Saving { token: Token },
}
impl View<'_> {
    pub(crate) const fn token(self) -> Token {
        match self {
            Self::Loading { token } | Self::Saving { token } => token,
            Self::Draft(draft) => draft.token,
        }
    }
}
struct Draft {
    token: Token,
    basis: Basis,
    values: UserPreferencesValues,
    row: Row,
    page: Page,
    profile: usize,
    argument: usize,
    text: Option<LineEditor>,
    invalid: bool,
    reset_all: bool,
    reset_profiles: bool,
    confirmation: Option<Confirmation>,
    drawn: bool,
}
impl Draft {
    fn patch(&self) -> UserPreferencesPatch {
        if self.reset_all {
            UserPreferencesPatch::reset(PreferenceKey::ALL).with_overrides(
                UserPreferencesPatch::between(&UserPreferencesValues::default(), &self.values),
            )
        } else if self.reset_profiles {
            let mut baseline = self.basis.values.clone();
            baseline.launch_profiles = Default::default();
            UserPreferencesPatch::reset([PreferenceKey::LaunchProfiles])
                .with_overrides(UserPreferencesPatch::between(&baseline, &self.values))
        } else {
            UserPreferencesPatch::between(&self.basis.values, &self.values)
        }
    }
    fn project(&self) -> DraftView<'_> {
        DraftView {
            token: self.token,
            values: &self.values,
            selected: self.row,
            dirty: self.reset_all || self.reset_profiles || self.values != self.basis.values,
            page: self.page,
            profile: self.profile,
            argument: self.argument,
            text: self.text.as_ref().map(|text| (text.text(), text.cursor())),
            invalid: self.invalid,
            confirmation: self.confirmation,
        }
    }
}
#[derive(Default)]
enum State {
    #[default]
    Closed,
    Loading {
        token: Token,
        visible: bool,
    },
    Draft(Box<Draft>),
    Saving {
        draft: Box<Draft>,
        visible: bool,
    },
}
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
        if !matches!(self.state, State::Loading { token: current, .. } if current == token) {
            return false;
        }
        if matches!(self.state, State::Loading { visible: false, .. }) {
            self.state = State::Closed;
            return false;
        }
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            return false;
        };
        self.state = State::Draft(Box::new(Draft {
            token,
            values: basis.values.clone(),
            basis,
            row: Row::Language,
            page: Page::General,
            profile: 0,
            argument: 0,
            text: None,
            invalid: false,
            reset_all: false,
            reset_profiles: false,
            confirmation: None,
            drawn: false,
        }));
        true
    }
    /// Clean views follow confirmed storage; active edits keep their original CAS basis.
    pub(crate) fn observe(&mut self, basis: &Basis) -> Observation {
        let State::Draft(draft) = &mut self.state else {
            return Observation::Unchanged;
        };
        if draft.basis.source == basis.source {
            return Observation::Unchanged;
        }
        if draft.project().dirty || draft.text.is_some() || draft.confirmation.is_some() {
            return Observation::Conflict;
        }
        let selected = &draft.values.launch_profiles.profiles[draft.profile].id;
        draft.profile = basis
            .values
            .launch_profiles
            .profiles
            .iter()
            .position(|profile| &profile.id == selected)
            .unwrap_or(0);
        draft.argument = 0;
        draft.values = basis.values.clone();
        draft.basis = basis.clone();
        draft.invalid = false;
        self.redraw();
        Observation::Refreshed
    }
    pub(crate) fn complete(&mut self, token: Token) -> bool {
        let matches = match &self.state {
            State::Loading { token: current, .. } => *current == token,
            State::Saving { draft, .. } => draft.token == token,
            _ => false,
        };
        if matches {
            self.state = State::Closed;
        }
        matches
    }
    /// Failed or unconfirmed saves retain the exact draft and original CAS basis.
    pub(crate) fn failed(&mut self, token: Token) -> bool {
        if matches!(&self.state, State::Saving { draft, .. } if draft.token == token)
            && let State::Saving { mut draft, visible } = std::mem::take(&mut self.state)
        {
            if visible {
                draft.drawn = false;
                self.state = State::Draft(draft);
                self.redraw();
            }
            return true;
        }
        self.complete(token)
    }
    pub(crate) fn visible(&self) -> bool {
        self.view().is_some()
    }
    pub(crate) fn saving(&self) -> bool {
        matches!(self.state, State::Saving { .. })
    }
    pub(crate) fn retire(&mut self) {
        self.state = State::Closed;
    }
    pub(crate) fn view(&self) -> Option<View<'_>> {
        match &self.state {
            State::Loading {
                token,
                visible: true,
            } => Some(View::Loading { token: *token }),
            State::Saving {
                draft,
                visible: true,
            } => Some(View::Saving { token: draft.token }),
            State::Draft(draft) => Some(View::Draft(draft.project())),
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
    pub(crate) fn not_presented(&mut self) {
        if let State::Draft(draft) = &mut self.state {
            draft.drawn = false;
        }
    }
    pub(crate) fn paste(&mut self, text: &str) {
        if let State::Draft(draft) = &mut self.state
            && let Some(editor) = &mut draft.text
        {
            draft.invalid = text.contains('\0') || editor.insert(text).is_err();
            self.redraw();
        }
    }
    pub(crate) fn reject_paste(&mut self) {
        if let State::Draft(draft) = &mut self.state
            && draft.text.is_some()
        {
            draft.invalid = true;
            self.redraw();
        }
    }
    pub(crate) fn remaining_text_bytes(&self) -> usize {
        match &self.state {
            State::Draft(draft) => draft.text.as_ref().map_or(0, |text| {
                draft
                    .row
                    .text_limit()
                    .unwrap_or(0)
                    .saturating_sub(text.text().len())
            }),
            _ => 0,
        }
    }
    fn redraw(&mut self) {
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            return;
        };
        if let State::Draft(draft) = &mut self.state {
            draft.token = token;
            draft.drawn = false;
        }
    }
    pub(crate) fn edit(&mut self, edit: Edit) -> Option<Submission> {
        if matches!(edit, Edit::Cancel) {
            match &mut self.state {
                State::Draft(draft) if draft.text.is_some() => {
                    draft.text = None;
                    draft.invalid = false;
                    self.redraw();
                }
                State::Draft(draft) if draft.confirmation.is_some() => {
                    draft.confirmation = None;
                    self.redraw();
                }
                State::Draft(draft) if draft.page == Page::Profiles => {
                    draft.page = Page::General;
                    draft.row = Row::LaunchProfiles;
                    self.redraw();
                }
                State::Loading { visible, .. } | State::Saving { visible, .. } => *visible = false,
                _ => self.state = State::Closed,
            }
            return None;
        }
        let State::Draft(draft) = &mut self.state else {
            return None;
        };
        if draft.text.is_some() {
            values::edit_text(draft, edit);
            self.redraw();
            return None;
        }
        if let Some(confirmation) = draft.confirmation {
            if !matches!(edit, Edit::Activate) || !draft.drawn {
                return None;
            }
            draft.confirmation = None;
            match confirmation {
                Confirmation::Reload => {
                    let token = draft.token;
                    self.state = State::Loading {
                        token,
                        visible: true,
                    };
                    return Some(Submission {
                        token,
                        operation: NativeSettingsOperation::Open {},
                    });
                }
                Confirmation::ResetAll => {
                    draft.values = UserPreferencesValues::default();
                    draft.reset_all = true;
                    draft.profile = 0;
                    draft.argument = 0;
                }
                Confirmation::ResetProfiles => {
                    draft.values.launch_profiles = Default::default();
                    draft.reset_profiles = true;
                    draft.profile = 0;
                    draft.argument = 0;
                }
                Confirmation::DeleteProfile => values::delete_profile(draft),
            }
            self.redraw();
            return None;
        }
        if matches!(edit, Edit::Activate | Edit::Adjust(_)) && !draft.project().enabled(draft.row) {
            return None;
        }
        if matches!(edit, Edit::Activate) && draft.row == Row::Save {
            let changes = draft.patch();
            if !draft.drawn || changes.is_empty() {
                return None;
            }
            if draft.values.validate().is_err() {
                draft.invalid = true;
                self.redraw();
                return None;
            }
            let source = NativeSettingsSource::parse(&draft.basis.source).ok()?;
            let token = draft.token;
            if let State::Draft(draft) = std::mem::take(&mut self.state) {
                self.state = State::Saving {
                    draft,
                    visible: true,
                };
            }
            return Some(Submission {
                token,
                operation: NativeSettingsOperation::Save { source, changes },
            });
        }
        let rows = draft.page.rows();
        let index = rows.iter().position(|row| *row == draft.row).unwrap_or(0);
        draft.invalid = false;
        match edit {
            Edit::Next => draft.row = rows[(index + 1) % rows.len()],
            Edit::Previous => draft.row = rows[(index + rows.len() - 1) % rows.len()],
            Edit::Home => draft.row = rows[0],
            Edit::End => draft.row = rows[rows.len() - 1],
            #[cfg(test)]
            Edit::Select(row) if rows.contains(&row) => draft.row = row,
            Edit::Adjust(forward) => values::adjust(draft, forward),
            Edit::Activate => {
                if let Some(limit) = draft.row.text_limit() {
                    if let Some(value) = values::text(draft) {
                        draft.text = Some(LineEditor::new(value, LengthLimit::Bytes(limit)));
                    }
                } else {
                    values::activate(draft);
                }
            }
            _ => {}
        }
        self.redraw();
        None
    }
    pub(crate) fn scroll(&mut self, token: Token, up: bool, lines: usize) -> Option<Token> {
        let State::Draft(draft) = &mut self.state else {
            return None;
        };
        if draft.token != token
            || !draft.drawn
            || draft.text.is_some()
            || draft.confirmation.is_some()
        {
            return None;
        }
        let rows = draft.page.rows();
        let index = rows.iter().position(|row| *row == draft.row)?;
        let next = if up {
            index.saturating_sub(lines)
        } else {
            index.saturating_add(lines).min(rows.len() - 1)
        };
        if next == index {
            return Some(token);
        }
        draft.row = rows[next];
        self.redraw();
        self.view().map(|view| view.token())
    }

    pub(crate) fn click(&mut self, token: Token, row: Row) -> Option<Submission> {
        if !matches!(&self.state, State::Draft(draft) if draft.token == token && draft.drawn && draft.text.is_none() && draft.confirmation.is_none())
        {
            return None;
        }
        if let State::Draft(draft) = &mut self.state {
            draft.row = row;
        }
        self.edit(Edit::Activate)
    }

    pub(crate) fn control(&mut self, token: Token, control: Control) -> Option<Submission> {
        if self.view().is_none_or(|view| view.token() != token) {
            return None;
        }
        if control == Control::Close {
            match &mut self.state {
                State::Loading { visible, .. } | State::Saving { visible, .. } => *visible = false,
                _ => self.state = State::Closed,
            }
            return None;
        }
        let State::Draft(draft) = &mut self.state else {
            return None;
        };
        if !draft.drawn {
            return None;
        }
        match control {
            Control::Row(row) => self.click(token, row),
            Control::Adjust { row, forward }
                if draft.text.is_none()
                    && draft.confirmation.is_none()
                    && draft.page.rows().contains(&row)
                    && draft.project().enabled(row) =>
            {
                draft.row = row;
                self.edit(Edit::Adjust(forward))
            }
            Control::Accept if draft.text.is_some() || draft.confirmation.is_some() => {
                self.edit(Edit::Activate)
            }
            Control::Back => self.edit(Edit::Cancel),
            _ => None,
        }
    }
}
