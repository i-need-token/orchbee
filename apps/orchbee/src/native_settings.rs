//! A bounded settings draft. Storage, transport and live application remain separate owners.
use crate::terminal_text::line_editor::{LengthLimit, LineEditor};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use orchbee_workbench_protocol::{
    NativeSettingsOperation, NativeSettingsSource, PreferenceKey, UserLaunchField,
    UserPreferencesPatch, UserPreferencesSnapshot, UserPreferencesValues,
};
mod appearance;
pub(crate) use appearance::AppearancePreview;
mod browse;
pub(crate) use browse::Entry;
mod choices;
pub(crate) use choices::{Choice, ChoicePicker};
mod input;
mod metadata;
mod rows;
#[cfg(test)]
mod tests;
mod values;
pub(crate) use input::{SettingsHintLabel, footer_bindings, key_edit, key_edit_for_view};
#[cfg(test)]
pub(crate) use metadata::SETTINGS;
#[cfg(test)]
use metadata::SettingEffect;
pub(crate) use metadata::{
    SettingAvailability, SettingCategory, SettingControl, SettingUnit, setting, setting_for_row,
};
pub(crate) use rows::{Page, Row};
pub(crate) use values::scrollback_megabytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Token(u64);
/// The Settings row that presents a launch-profile field, for Save-failure focus.
fn invalid_row(field: UserLaunchField) -> Row {
    match field {
        UserLaunchField::Name => Row::ProfileName,
        UserLaunchField::Executable => Row::Executable,
        UserLaunchField::Argument(_) => Row::ArgumentValue,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsInputReadiness {
    Recovery,
    Interactive,
}
/// Current Settings capture intent projected by the feature owner.
///
/// Pending and suspended surfaces retain recovery capture without claiming
/// ordinary controls. Interactive capture additionally requires proof that
/// the current whole surface completed a paint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingsInputContext {
    pub(crate) token: Token,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: SettingsInputReadiness,
    pub(crate) remaining_text_bytes: usize,
}
/// Geometry evidence returned by the Settings renderer for one completed frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingsDraw {
    token: Token,
    rows: Option<(usize, usize)>,
    painted: u32,
}
impl SettingsDraw {
    /// Records the painted slice of a finite-choice child, independently of parent rows.
    pub(crate) const fn picker(token: Token, first: usize, visible: usize) -> Self {
        Self {
            token,
            rows: Some((first, visible)),
            painted: 0,
        }
    }
    pub(crate) const fn surface(token: Token) -> Self {
        Self {
            token,
            rows: None,
            painted: 0,
        }
    }
    pub(crate) const fn token(self) -> Token {
        self.token
    }
    pub(crate) const fn first_visible(self) -> Option<usize> {
        match self.rows {
            Some((first, _)) => Some(first),
            None => None,
        }
    }
    pub(crate) const fn visible_rows(self) -> Option<usize> {
        match self.rows {
            Some((_, visible)) => Some(visible),
            None => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Adjustment {
    pub(crate) backward: bool,
    pub(crate) forward: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdjustmentStep {
    Small,
    Large,
}
/// Painted settings controls retain the same draft authority as keyboard edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Control {
    Adjust { row: Row, forward: bool },
    Accept,
    Back,
    Close,
    Search,
    Choose(Choice),
}
pub(crate) type Basis = UserPreferencesSnapshot;
#[derive(Clone, Copy, Debug)]
pub(crate) enum Edit {
    Next,
    Previous,
    #[cfg(test)]
    Select(Row),
    Adjust(bool),
    AdjustLarge(bool),
    Activate,
    Cancel,
    Insert(char),
    Backspace,
    WordBack,
    Delete,
    Home,
    End,
    Clear,
    Search,
    Save,
    Page(bool),
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reconciliation {
    SourceChanged,
    OutcomeUnknown,
}
#[derive(Clone, Copy)]
pub(crate) struct DraftView<'a> {
    pub(crate) token: Token,
    pub(crate) values: &'a UserPreferencesValues,
    pub(crate) basis_values: &'a UserPreferencesValues,
    #[cfg(test)]
    pub(crate) basis_source: &'a str,
    #[cfg(test)]
    pub(crate) effective_values: &'a UserPreferencesValues,
    #[cfg(test)]
    pub(crate) effective_source: &'a str,
    pub(crate) selected: Row,
    pub(crate) dirty: bool,
    /// Projected from the draft owner's external-update and dismissal guard.
    pub(crate) active_task: bool,
    pub(crate) conflict: bool,
    pub(crate) reconciliation: Option<Reconciliation>,
    pub(crate) page: Page,
    /// Requested first row. Renderers clamp it for their concrete visible height.
    pub(crate) viewport: usize,
    /// Keyboard navigation reveals the anchor; manual scrolling preserves the requested viewport.
    pub(crate) follow_selection: bool,
    pub(crate) profile: usize,
    pub(crate) argument: usize,
    pub(crate) text: Option<(&'a str, usize)>,
    pub(crate) invalid: bool,
    /// The profile and field of the first invalid value found by the last Save
    /// attempt; `None` keeps the generic invalid-value copy.
    pub(crate) invalid_field: Option<(usize, UserLaunchField)>,
    pub(crate) confirmation: Option<Confirmation>,
    pub(crate) query: &'a str,
    pub(crate) search_cursor: usize,
    pub(crate) searching: bool,
    pub(crate) picker: Option<ChoicePicker>,
    reset_all: bool,
    reset_profiles: bool,
}
impl DraftView<'_> {
    pub(crate) fn entries(self) -> Vec<Entry> {
        browse::entries(self.page, self.query)
    }

    pub(crate) fn content_rows(self) -> Vec<Row> {
        self.entries()
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Setting(row) => Some(row),
                Entry::Heading(_) => None,
            })
            .collect()
    }

    fn navigation_rows(self) -> Vec<Row> {
        self.content_rows()
            .into_iter()
            .chain(self.page.footer_rows().iter().copied())
            .collect()
    }

    pub(crate) fn first_visible(self, visible: usize) -> usize {
        let entries = self.entries();
        let visible = visible.max(1);
        let maximum = entries.len().saturating_sub(visible);
        let requested = self.viewport.min(maximum);
        if !self.follow_selection {
            return requested;
        }
        let Some(selected) = entries
            .iter()
            .position(|entry| *entry == Entry::Setting(self.selected))
        else {
            return requested;
        };
        // Keep a section title beside its first focused row whenever both fit.
        let reveal =
            if selected > 0 && matches!(entries[selected - 1], Entry::Heading(_)) && visible > 1 {
                selected - 1
            } else {
                selected
            };
        if selected < requested {
            reveal
        } else if selected >= requested.saturating_add(visible) {
            selected.saturating_add(1).saturating_sub(visible)
        } else {
            requested
        }
    }

    pub(crate) fn presentation(self, visible: usize) -> SettingsDraw {
        let entries = self.entries();
        let visible = visible.max(1).min(entries.len());
        let first = self.first_visible(visible);
        let painted = entries
            .iter()
            .skip(first)
            .take(visible)
            .filter_map(|entry| match entry {
                Entry::Setting(row) => Some(*row),
                Entry::Heading(_) => None,
            })
            .chain(
                if self.searching {
                    &[][..]
                } else {
                    self.page.footer_rows()
                }
                .iter()
                .copied(),
            )
            .fold(0, |mask, row| mask | row.mask());
        SettingsDraw {
            token: self.token,
            rows: Some((first, visible)),
            painted,
        }
    }

    pub(crate) fn adjustment(self, row: Row) -> Option<Adjustment> {
        let available = |index: usize, len: usize| Adjustment {
            backward: len > 1 && index < len,
            forward: len > 1 && index < len,
        };
        match row {
            Row::CompletionThreshold => {
                let SettingControl::BoundedInteger {
                    minimum, maximum, ..
                } = setting(PreferenceKey::CompletionReminderMinimumSeconds).control
                else {
                    unreachable!("integer preference metadata")
                };
                let current = u64::from(self.values.completion_reminder_minimum_seconds);
                Some(Adjustment {
                    backward: current > minimum,
                    forward: current < maximum,
                })
            }
            Row::Scrollback => {
                let SettingControl::BoundedInteger {
                    minimum, maximum, ..
                } = setting(PreferenceKey::ScrollbackBytes).control
                else {
                    unreachable!("integer preference metadata")
                };
                let current = u64::from(self.values.scrollback_bytes);
                Some(Adjustment {
                    backward: current > minimum,
                    forward: current < maximum,
                })
            }
            Row::Profile => Some(available(
                self.profile,
                self.values.launch_profiles.profiles.len(),
            )),
            Row::Argument => {
                let arguments = match &self.values.launch_profiles.profiles[self.profile].command {
                    orchbee_workbench_protocol::UserLaunchCommand::CustomExecutable {
                        arguments,
                        ..
                    } => arguments,
                    orchbee_workbench_protocol::UserLaunchCommand::SystemShell { .. } => {
                        return Some(Adjustment {
                            backward: false,
                            forward: false,
                        });
                    }
                };
                Some(available(self.argument, arguments.len()))
            }
            _ => None,
        }
    }

    pub(crate) fn checked(self, row: Row) -> Option<bool> {
        match row {
            Row::CopyOnSelect => Some(self.values.copy_on_select),
            Row::TrimAgents => Some(self.values.trim_trailing_blank_rows_for_agents),
            Row::Sound => Some(self.values.sound_enabled),
            Row::Login => match &self.values.launch_profiles.profiles[self.profile].command {
                orchbee_workbench_protocol::UserLaunchCommand::SystemShell { login } => {
                    Some(*login)
                }
                orchbee_workbench_protocol::UserLaunchCommand::CustomExecutable { .. } => None,
            },
            _ => None,
        }
    }

    pub(crate) fn changed(self, row: Row) -> bool {
        let key = setting_for_row(row)
            .map(|metadata| metadata.key)
            .or_else(|| {
                matches!(
                    row,
                    Row::Profile
                        | Row::ProfileName
                        | Row::CommandType
                        | Row::Login
                        | Row::Executable
                        | Row::Argument
                        | Row::ArgumentValue
                        | Row::AddArgument
                        | Row::DeleteArgument
                        | Row::SetDefault
                        | Row::AddProfile
                        | Row::DeleteProfile
                )
                .then_some(PreferenceKey::LaunchProfiles)
            });
        let Some(key) = key else {
            return false;
        };
        if self.reset_all || self.reset_profiles && key == PreferenceKey::LaunchProfiles {
            return true;
        }
        match key {
            PreferenceKey::Locale => self.values.locale != self.basis_values.locale,
            PreferenceKey::ApplicationTheme => {
                self.values.application_theme != self.basis_values.application_theme
            }
            PreferenceKey::TerminalDisplayTheme => {
                self.values.terminal_display_theme != self.basis_values.terminal_display_theme
            }
            PreferenceKey::CustomForeground => {
                self.values.custom_foreground != self.basis_values.custom_foreground
            }
            PreferenceKey::CustomBackground => {
                self.values.custom_background != self.basis_values.custom_background
            }
            PreferenceKey::CopyOnSelect => {
                self.values.copy_on_select != self.basis_values.copy_on_select
            }
            PreferenceKey::TrimTrailingBlankRowsForAgents => {
                self.values.trim_trailing_blank_rows_for_agents
                    != self.basis_values.trim_trailing_blank_rows_for_agents
            }
            PreferenceKey::AgentNotificationMode => {
                self.values.agent_notification_mode != self.basis_values.agent_notification_mode
            }
            PreferenceKey::CompletionReminderMode => {
                self.values.completion_reminder_mode != self.basis_values.completion_reminder_mode
            }
            PreferenceKey::CompletionReminderMinimumSeconds => {
                self.values.completion_reminder_minimum_seconds
                    != self.basis_values.completion_reminder_minimum_seconds
            }
            PreferenceKey::SoundEnabled => {
                self.values.sound_enabled != self.basis_values.sound_enabled
            }
            PreferenceKey::ScrollbackBytes => {
                self.values.scrollback_bytes != self.basis_values.scrollback_bytes
            }
            PreferenceKey::LaunchProfiles => {
                self.values.launch_profiles != self.basis_values.launch_profiles
            }
        }
    }

    pub(crate) fn enabled(self, row: Row) -> bool {
        use orchbee_workbench_protocol::UserLaunchCommand;
        let catalog = &self.values.launch_profiles;
        let profile = &catalog.profiles[self.profile];
        let arguments = match &profile.command {
            UserLaunchCommand::CustomExecutable { arguments, .. } => Some(arguments),
            _ => None,
        };
        match row {
            Row::Save => self.dirty && !self.conflict && self.reconciliation.is_none(),
            row if setting_for_row(row).is_some_and(|metadata| {
                metadata.availability == SettingAvailability::TerminalDisplayThemeCustom
            }) =>
            {
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
    /// Only an idle root draft may expose its painted Close action to the backdrop.
    pub(crate) fn allows_backdrop_close(self) -> bool {
        matches!(self, Self::Draft(draft) if !draft.active_task)
    }

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
    effective: Basis,
    values: UserPreferencesValues,
    row: Row,
    page: Page,
    viewport: usize,
    visible_rows: usize,
    follow_selection: bool,
    activation_ready: bool,
    parent: Option<Navigation>,
    profile: usize,
    argument: usize,
    text: Option<LineEditor>,
    invalid: bool,
    invalid_field: Option<(usize, UserLaunchField)>,
    reset_all: bool,
    reset_profiles: bool,
    confirmation: Option<Confirmation>,
    reconciliation: Option<Reconciliation>,
    drawn: bool,
    filter: LineEditor,
    browse: BrowseMode,
}

#[derive(Clone, Copy, Default)]
enum BrowseMode {
    #[default]
    Browse,
    Search,
    Picker(ChoicePicker),
}
#[derive(Clone, Copy)]
struct Navigation {
    row: Row,
    viewport: usize,
    follow_selection: bool,
}
impl Draft {
    fn reconcile_search(&mut self) {
        let rows = self.project().content_rows();
        if !rows.contains(&self.row) {
            self.row = rows.first().copied().unwrap_or(Row::Save);
        }
        self.viewport = 0;
        self.follow_selection = true;
    }

    fn dirty(&self) -> bool {
        self.reset_all || self.reset_profiles || self.values != self.basis.values
    }

    fn has_active_task(&self) -> bool {
        self.dirty()
            || self.text.is_some()
            || self.confirmation.is_some()
            || self.page == Page::Profiles
            || self.reconciliation.is_some()
            || !matches!(self.browse, BrowseMode::Browse)
            || !self.filter.text().is_empty()
    }

    fn enter_profiles(&mut self) {
        self.parent = Some(Navigation {
            row: self.row,
            viewport: self.viewport,
            follow_selection: self.follow_selection,
        });
        self.page = Page::Profiles;
        self.row = Row::Profile;
        self.viewport = 0;
        self.visible_rows = 0;
        self.follow_selection = true;
        self.activation_ready = false;
    }

    fn return_from_profiles(&mut self) {
        let navigation = self.parent.take().unwrap_or(Navigation {
            row: Row::LaunchProfiles,
            viewport: 0,
            follow_selection: true,
        });
        self.page = Page::General;
        self.row = navigation.row;
        self.viewport = navigation.viewport;
        self.visible_rows = 0;
        self.follow_selection = navigation.follow_selection;
        self.activation_ready = false;
    }

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
            basis_values: &self.basis.values,
            #[cfg(test)]
            basis_source: &self.basis.source,
            #[cfg(test)]
            effective_values: &self.effective.values,
            #[cfg(test)]
            effective_source: &self.effective.source,
            selected: self.row,
            dirty: self.dirty(),
            active_task: self.has_active_task(),
            conflict: self.basis.source != self.effective.source
                || self.basis.values != self.effective.values,
            reconciliation: self.reconciliation,
            page: self.page,
            viewport: self.viewport,
            follow_selection: self.follow_selection,
            profile: self.profile,
            argument: self.argument,
            text: self.text.as_ref().map(|text| (text.text(), text.cursor())),
            invalid: self.invalid,
            invalid_field: self.invalid_field,
            confirmation: self.confirmation,
            query: self.filter.text(),
            search_cursor: self.filter.cursor(),
            searching: matches!(self.browse, BrowseMode::Search),
            picker: match self.browse {
                BrowseMode::Picker(picker) => Some(picker),
                _ => None,
            },
            reset_all: self.reset_all,
            reset_profiles: self.reset_profiles,
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
    presentation: PresentationPhase,
    surface_presented: bool,
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
        self.presentation = PresentationPhase::Pending;
        self.surface_presented = false;
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
            self.surface_presented = false;
            return false;
        }
        self.surface_presented = false;
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            return false;
        };
        self.state = State::Draft(Box::new(Draft {
            token,
            values: basis.values.clone(),
            effective: basis.clone(),
            basis,
            row: Row::Language,
            page: Page::General,
            viewport: 0,
            visible_rows: 0,
            follow_selection: true,
            activation_ready: false,
            parent: None,
            profile: 0,
            argument: 0,
            text: None,
            invalid: false,
            invalid_field: None,
            reset_all: false,
            reset_profiles: false,
            confirmation: None,
            reconciliation: None,
            drawn: false,
            filter: LineEditor::new(String::new(), LengthLimit::Bytes(256)),
            browse: BrowseMode::Browse,
        }));
        true
    }
    /// Clean views follow confirmed storage; active tasks retain their original CAS basis while
    /// projecting the newest confirmed values independently.
    pub(crate) fn observe(&mut self, effective: &Basis) -> Observation {
        let draft = match &mut self.state {
            State::Draft(draft) => draft,
            State::Saving { draft, .. } => {
                if draft.effective.source != effective.source {
                    draft.effective = effective.clone();
                }
                return Observation::Unchanged;
            }
            _ => return Observation::Unchanged,
        };
        if draft.effective.source == effective.source {
            return Observation::Unchanged;
        }
        draft.effective = effective.clone();
        if draft.has_active_task() {
            self.redraw();
            return Observation::Conflict;
        }
        let selected = &draft.values.launch_profiles.profiles[draft.profile].id;
        draft.profile = effective
            .values
            .launch_profiles
            .profiles
            .iter()
            .position(|profile| &profile.id == selected)
            .unwrap_or(0);
        draft.argument = 0;
        draft.values = effective.values.clone();
        draft.basis = effective.clone();
        draft.invalid = false;
        draft.invalid_field = None;
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
            self.surface_presented = false;
        }
        matches
    }
    /// Failed or unconfirmed saves retain the exact draft and original CAS basis. An explicit
    /// reconciliation reason blocks another Save until a matched explicit Open completes.
    pub(crate) fn failed(&mut self, token: Token, reconciliation: Option<Reconciliation>) -> bool {
        if matches!(&self.state, State::Saving { draft, .. } if draft.token == token)
            && let State::Saving { mut draft, visible } = std::mem::take(&mut self.state)
        {
            self.surface_presented = false;
            if visible {
                draft.drawn = false;
                draft.activation_ready = false;
                draft.visible_rows = 0;
                draft.reconciliation = reconciliation;
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
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.presentation
    }
    pub(crate) fn input_context(&self) -> Option<SettingsInputContext> {
        let token = self.view()?.token();
        let readiness = if self.presentation.interactive() && self.surface_presented {
            SettingsInputReadiness::Interactive
        } else {
            SettingsInputReadiness::Recovery
        };
        Some(SettingsInputContext {
            token,
            phase: self.presentation,
            readiness,
            remaining_text_bytes: if readiness == SettingsInputReadiness::Interactive {
                self.remaining_text_bytes()
            } else {
                0
            },
        })
    }
    pub(crate) fn saving(&self) -> bool {
        matches!(self.state, State::Saving { .. })
    }
    pub(crate) fn retire(&mut self) {
        self.state = State::Closed;
        self.surface_presented = false;
    }
    /// Borrows reversible presentation values without publishing runtime preferences.
    ///
    /// Locally edited leaves overlay the newest confirmed appearance without rebasing the
    /// frozen draft. Hidden/replaced episodes and an unknown Save have no preview authority.
    /// A visible accepted Save retains preview until its confirmed snapshot arrives.
    pub(crate) fn appearance_preview(&self) -> Option<AppearancePreview<'_>> {
        let draft = match &self.state {
            State::Draft(draft)
            | State::Saving {
                draft,
                visible: true,
            } => draft,
            _ => return None,
        };
        (draft.reconciliation != Some(Reconciliation::OutcomeUnknown)).then(|| {
            let mut preview = AppearancePreview::project(
                &draft.basis.values,
                &draft.values,
                &draft.effective.values,
                draft.reset_all,
            );
            if let BrowseMode::Picker(picker) = draft.browse {
                picker.candidate().preview(&mut preview);
            }
            preview
        })
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

    pub(crate) fn presented(&mut self, draw: SettingsDraw) {
        if self.view().is_none_or(|view| view.token() != draw.token()) {
            return;
        }
        self.presentation.presented();
        self.surface_presented = true;
        if let State::Draft(draft) = &mut self.state {
            draft.drawn = true;
            if let Some((first, visible)) = draw.rows {
                draft.visible_rows = visible;
                if let BrowseMode::Picker(picker) = &mut draft.browse {
                    picker.viewport = first;
                    draft.activation_ready =
                        (first..first.saturating_add(visible)).contains(&picker.cursor);
                } else {
                    draft.viewport = first;
                    draft.activation_ready = draw.painted & draft.row.mask() != 0;
                }
            } else {
                draft.visible_rows = 0;
                draft.activation_ready = false;
            }
        }
    }
    pub(crate) fn not_presented(&mut self) {
        self.surface_presented = false;
        if let State::Draft(draft) = &mut self.state {
            draft.drawn = false;
            draft.activation_ready = false;
            draft.visible_rows = 0;
        }
    }
    /// Settles an evaluated geometry block for this Settings incarnation.
    ///
    /// A first blocked frame refuses the surface without cancelling accepted
    /// read/save work. Once a frame has been admitted, the same block only
    /// suspends interaction and retains the complete local draft.
    pub(crate) fn geometry_blocked(&mut self) -> Option<GeometrySettlement> {
        self.view()?;
        self.not_presented();
        let settlement = self.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.state = match std::mem::take(&mut self.state) {
                State::Loading { token, .. } => State::Loading {
                    token,
                    visible: false,
                },
                State::Saving { draft, .. } => State::Saving {
                    draft,
                    visible: false,
                },
                State::Draft(_) => State::Closed,
                State::Closed => unreachable!("visible settings state"),
            };
        }
        Some(settlement)
    }
    pub(crate) fn paste(&mut self, text: &str) {
        if self.surface_presented
            && let State::Draft(draft) = &mut self.state
        {
            let editor = if matches!(draft.browse, BrowseMode::Search) {
                Some(&mut draft.filter)
            } else {
                draft.text.as_mut()
            };
            let Some(editor) = editor else { return };
            draft.invalid = text.contains('\0') || editor.insert(text).is_err();
            draft.invalid_field = None;
            if matches!(draft.browse, BrowseMode::Search) {
                draft.reconcile_search();
            }
            self.redraw();
        }
    }
    pub(crate) fn reject_paste(&mut self) {
        if self.surface_presented
            && let State::Draft(draft) = &mut self.state
            && (draft.text.is_some() || matches!(draft.browse, BrowseMode::Search))
        {
            draft.invalid = true;
            draft.invalid_field = None;
            self.redraw();
        }
    }
    pub(crate) fn remaining_text_bytes(&self) -> usize {
        match &self.state {
            State::Draft(draft) if matches!(draft.browse, BrowseMode::Search) => {
                256_usize.saturating_sub(draft.filter.text().len())
            }
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
            self.surface_presented = false;
            return;
        };
        if let State::Draft(draft) = &mut self.state {
            draft.token = token;
            draft.drawn = false;
            draft.activation_ready = draft.follow_selection && self.surface_presented;
        }
    }
    pub(crate) fn edit(&mut self, edit: Edit) -> Option<Submission> {
        if matches!(edit, Edit::Cancel) {
            let mut closes_surface = false;
            match &mut self.state {
                State::Draft(draft) if draft.text.is_some() => {
                    draft.text = None;
                    draft.invalid = false;
                    draft.invalid_field = None;
                    self.redraw();
                }
                State::Draft(draft) if draft.confirmation.is_some() => {
                    draft.confirmation = None;
                    self.redraw();
                }
                State::Draft(draft) if matches!(draft.browse, BrowseMode::Picker(_)) => {
                    draft.browse = BrowseMode::Browse;
                    self.redraw();
                }
                State::Draft(draft) if matches!(draft.browse, BrowseMode::Search) => {
                    draft.browse = BrowseMode::Browse;
                    draft.filter.clear();
                    draft.invalid = false;
                    draft.reconcile_search();
                    self.redraw();
                }
                State::Draft(draft) if draft.page == Page::Profiles => {
                    draft.return_from_profiles();
                    self.redraw();
                }
                State::Draft(draft) if !draft.filter.text().is_empty() => {
                    draft.filter.clear();
                    draft.reconcile_search();
                    self.redraw();
                }
                State::Loading { visible, .. } | State::Saving { visible, .. } => {
                    *visible = false;
                    closes_surface = true;
                }
                _ => {
                    self.state = State::Closed;
                    closes_surface = true;
                }
            }
            if closes_surface {
                self.surface_presented = false;
            }
            return None;
        }
        let State::Draft(draft) = &mut self.state else {
            return None;
        };
        #[cfg(test)]
        let fixture_selection = matches!(edit, Edit::Select(_));
        #[cfg(not(test))]
        let fixture_selection = false;
        if !self.surface_presented && !fixture_selection {
            return None;
        }
        #[cfg(test)]
        if fixture_selection {
            draft.browse = BrowseMode::Browse;
        }
        if draft.text.is_some() {
            values::edit_text(draft, edit);
            self.redraw();
            return None;
        }
        if draft.confirmation.is_none() {
            if let BrowseMode::Picker(mut picker) = draft.browse {
                match edit {
                    Edit::Next | Edit::Adjust(true) => picker.navigate(true),
                    Edit::Previous | Edit::Adjust(false) => picker.navigate(false),
                    Edit::Home => picker.cursor = 0,
                    Edit::End => picker.cursor = Choice::options(picker.row).len() - 1,
                    Edit::Activate if draft.drawn => {
                        picker.candidate().apply(&mut draft.values);
                        draft.browse = BrowseMode::Browse;
                        self.redraw();
                        return None;
                    }
                    _ => return None,
                }
                draft.browse = BrowseMode::Picker(picker);
                self.redraw();
                return None;
            }
            if matches!(draft.browse, BrowseMode::Search) {
                match edit {
                    Edit::Activate if draft.project().content_rows().is_empty() => return None,
                    Edit::Activate | Edit::Next | Edit::Previous => {
                        draft.browse = BrowseMode::Browse;
                        draft.reconcile_search();
                    }
                    Edit::Insert(character) => {
                        draft.invalid = draft.filter.insert(&character.to_string()).is_err()
                    }
                    Edit::Backspace => {
                        draft.filter.backspace();
                        draft.invalid = false;
                    }
                    Edit::WordBack => {
                        draft.filter.word_back();
                        draft.invalid = false;
                    }
                    Edit::Delete => {
                        draft.filter.delete();
                        draft.invalid = false;
                    }
                    Edit::Adjust(false) => {
                        draft.filter.left();
                    }
                    Edit::Adjust(true) => {
                        draft.filter.right();
                    }
                    Edit::Home => {
                        draft.filter.home();
                    }
                    Edit::End => {
                        draft.filter.end();
                    }
                    Edit::Clear => {
                        draft.filter.clear();
                        draft.invalid = false;
                    }
                    _ => return None,
                }
                draft.reconcile_search();
                self.redraw();
                return None;
            }
            if matches!(edit, Edit::Search) && draft.page == Page::General {
                draft.browse = BrowseMode::Search;
                draft.invalid = false;
                self.redraw();
                return None;
            }
            if matches!(edit, Edit::Save) && draft.page == Page::General && draft.drawn {
                if !draft.project().enabled(Row::Save) {
                    return None;
                }
                draft.row = Row::Save;
                draft.activation_ready = true;
                return self.edit(Edit::Activate);
            }
        }
        if let Some(confirmation) = draft.confirmation {
            if !matches!(edit, Edit::Activate) || !draft.drawn {
                return None;
            }
            draft.confirmation = None;
            draft.invalid = false;
            draft.invalid_field = None;
            match confirmation {
                Confirmation::Reload => {
                    let token = draft.token;
                    self.state = State::Loading {
                        token,
                        visible: true,
                    };
                    self.surface_presented = false;
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
        if matches!(
            edit,
            Edit::Activate | Edit::Adjust(_) | Edit::AdjustLarge(_)
        ) && !draft.activation_ready
        {
            return None;
        }
        if matches!(
            edit,
            Edit::Activate | Edit::Adjust(_) | Edit::AdjustLarge(_)
        ) && !draft.project().enabled(draft.row)
        {
            return None;
        }
        let direction = match edit {
            Edit::Adjust(forward) | Edit::AdjustLarge(forward) => Some(forward),
            _ => None,
        };
        if let Some(forward) = direction
            && let Some(adjustment) = draft.project().adjustment(draft.row)
            && !(if forward {
                adjustment.forward
            } else {
                adjustment.backward
            })
        {
            return None;
        }
        if matches!(edit, Edit::Activate) && draft.row == Row::Save {
            let changes = draft.patch();
            if !draft.drawn || changes.is_empty() {
                return None;
            }
            if draft.values.validate().is_err() {
                draft.invalid = true;
                draft.invalid_field = draft.values.launch_profiles.invalid_field();
                if let Some((index, field)) = draft.invalid_field {
                    let row = invalid_row(field);
                    if draft.page != Page::Profiles {
                        draft.enter_profiles();
                    }
                    draft.profile = index;
                    draft.argument = match field {
                        UserLaunchField::Argument(index) => index,
                        _ => 0,
                    };
                    draft.row = row;
                    draft.follow_selection = true;
                    self.redraw();
                    return None;
                }
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
            self.surface_presented = false;
            return Some(Submission {
                token,
                operation: NativeSettingsOperation::Save { source, changes },
            });
        }
        let rows = draft.project().navigation_rows();
        let index = rows.iter().position(|row| *row == draft.row).unwrap_or(0);
        draft.invalid = false;
        draft.invalid_field = None;
        match edit {
            Edit::Next => {
                draft.row = rows[(index + 1) % rows.len()];
                draft.follow_selection = true;
            }
            Edit::Previous => {
                draft.row = rows[(index + rows.len() - 1) % rows.len()];
                draft.follow_selection = true;
            }
            Edit::Home => {
                draft.row = rows[0];
                draft.follow_selection = true;
            }
            Edit::End => {
                draft.row = rows[rows.len() - 1];
                draft.follow_selection = true;
            }
            Edit::Page(forward) => {
                let amount = draft.visible_rows.saturating_sub(1).max(1);
                let next = if forward {
                    index.saturating_add(amount).min(rows.len() - 1)
                } else {
                    index.saturating_sub(amount)
                };
                draft.row = rows[next];
                draft.follow_selection = true;
            }
            #[cfg(test)]
            Edit::Select(row) if rows.contains(&row) => {
                draft.row = row;
                draft.follow_selection = true;
            }
            Edit::Adjust(forward) => values::adjust(draft, forward, AdjustmentStep::Small),
            Edit::AdjustLarge(forward) => values::adjust(draft, forward, AdjustmentStep::Large),
            Edit::Activate => {
                if let Some(picker) = ChoicePicker::open(draft.row, &draft.values) {
                    draft.browse = BrowseMode::Picker(picker);
                } else if let Some(limit) = draft.row.text_limit() {
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
        if draft.token == token
            && draft.drawn
            && let BrowseMode::Picker(picker) = &mut draft.browse
        {
            let visible = draft.visible_rows.max(1);
            let maximum = Choice::options(picker.row).len().saturating_sub(visible);
            let next = if up {
                picker.viewport.saturating_sub(lines)
            } else {
                picker.viewport.saturating_add(lines).min(maximum)
            };
            if next == picker.viewport {
                return Some(token);
            }
            picker.viewport = next;
            picker.cursor = picker.cursor.clamp(next, next.saturating_add(visible - 1));
            self.redraw();
            return self.view().map(|view| view.token());
        }
        if draft.token != token
            || !draft.drawn
            || draft.text.is_some()
            || draft.confirmation.is_some()
            || !matches!(draft.browse, BrowseMode::Browse)
        {
            return None;
        }
        let next = if up {
            draft.viewport.saturating_sub(lines)
        } else {
            draft.viewport.saturating_add(lines).min(
                draft
                    .project()
                    .entries()
                    .len()
                    .saturating_sub(draft.visible_rows.max(1)),
            )
        };
        if next == draft.viewport {
            return Some(token);
        }
        draft.viewport = next;
        draft.follow_selection = false;
        draft.activation_ready = false;
        self.redraw();
        self.view().map(|view| view.token())
    }

    pub(crate) fn click(&mut self, token: Token, row: Row) -> Option<Submission> {
        if !matches!(&self.state, State::Draft(draft) if draft.token == token
            && draft.drawn
            && draft.text.is_none()
            && draft.confirmation.is_none()
            && !matches!(draft.browse, BrowseMode::Picker(_))
            && draft.project().navigation_rows().contains(&row))
        {
            return None;
        }
        if let State::Draft(draft) = &mut self.state {
            draft.row = row;
            draft.follow_selection = false;
            draft.activation_ready = true;
            if matches!(draft.browse, BrowseMode::Search) {
                draft.browse = BrowseMode::Browse;
                self.redraw();
                return None;
            }
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
            self.surface_presented = false;
            return None;
        }
        let State::Draft(draft) = &mut self.state else {
            return None;
        };
        if !draft.drawn {
            return None;
        }
        match control {
            Control::Search
                if draft.page == Page::General
                    && draft.text.is_none()
                    && draft.confirmation.is_none()
                    && matches!(draft.browse, BrowseMode::Browse) =>
            {
                self.edit(Edit::Search)
            }
            Control::Choose(choice) if draft.text.is_none() && draft.confirmation.is_none() => {
                let BrowseMode::Picker(mut picker) = draft.browse else {
                    return None;
                };
                let cursor = Choice::options(picker.row)
                    .iter()
                    .position(|candidate| *candidate == choice)?;
                picker.cursor = cursor;
                draft.browse = BrowseMode::Picker(picker);
                self.edit(Edit::Activate)
            }
            Control::Adjust { row, forward }
                if draft.text.is_none()
                    && draft.confirmation.is_none()
                    && matches!(draft.browse, BrowseMode::Browse)
                    && draft.page.rows().contains(&row)
                    && draft.project().enabled(row)
                    && draft.project().adjustment(row).is_some_and(|adjustment| {
                        if forward {
                            adjustment.forward
                        } else {
                            adjustment.backward
                        }
                    }) =>
            {
                draft.row = row;
                draft.follow_selection = false;
                draft.activation_ready = true;
                self.edit(Edit::Adjust(forward))
            }
            Control::Accept
                if draft.text.is_some()
                    || draft.confirmation.is_some()
                    || !matches!(draft.browse, BrowseMode::Browse) =>
            {
                self.edit(Edit::Activate)
            }
            Control::Back => self.edit(Edit::Cancel),
            _ => None,
        }
    }
}
