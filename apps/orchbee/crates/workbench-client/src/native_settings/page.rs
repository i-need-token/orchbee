//! What the page shows and what is in flight: the confirmed values, at most one edit the Server
//! has not answered, and where focus is. Drawing and key decoding stay outside.
use super::{
    Adjustment, Category, Choice, ChoicePicker, Confirmed, Entry, FileState, Group, Integrations,
    Page, Row, SettingAvailability, SettingControl, SettingsDraw, Token,
    catalog::{self, Filter},
    setting, setting_for_row,
};
use crate::terminal_text::line_editor::LineEditor;
use orchbee_local_runtime::UserSettingsField;
use orchbee_workbench_protocol::{
    ApplicationAppearance, SettingChange, UserLaunchCommand, UserLaunchField, UserLaunchProfile,
    UserSettingsValues,
};

/// The column that holds keyboard focus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pane {
    Categories,
    Rows,
}

/// The one edit the Server has not answered: the row it changes and, for a text key, the text it
/// asked for (`None` restores the default).
#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub(super) token: Token,
    pub(super) row: Row,
    pub(super) text: Option<Option<String>>,
}

/// Where a child page returns.
#[derive(Clone, Copy)]
pub(super) struct Navigation {
    pub(super) page: Page,
    pub(super) row: Row,
    pub(super) viewport: usize,
}

pub(super) struct PageState {
    pub(super) login_shell_supported: bool,
    pub(super) reduced_color: bool,
    pub(super) integrations_available: bool,
    pub(super) token: Token,
    pub(super) confirmed: Confirmed,
    pub(super) file: FileState,
    /// What the rows show: the confirmed values, a pending edit's requested value, and the
    /// launch-profile editor's catalog while it is open.
    pub(super) values: UserSettingsValues,
    pub(super) pending: Option<Pending>,
    pub(super) category: Category,
    pub(super) pane: Pane,
    pub(super) row: Row,
    pub(super) page: Page,
    pub(super) parents: Vec<Navigation>,
    /// First line of the right column the last frame drew, or the one a scroll asked for.
    pub(super) viewport: usize,
    pub(super) visible_rows: usize,
    /// Keyboard movement reveals the focused row; a wheel scroll keeps the viewport it chose.
    pub(super) follow_selection: bool,
    pub(super) activation_ready: bool,
    pub(super) drawn: bool,
    /// The last frame showed one column at a time.
    pub(super) narrow: bool,
    pub(super) profile: usize,
    pub(super) argument: usize,
    pub(super) text: Option<LineEditor>,
    pub(super) invalid: bool,
    pub(super) invalid_field: Option<(usize, UserLaunchField)>,
    pub(super) confirm_delete: bool,
    pub(super) filter: LineEditor,
    pub(super) searching: bool,
    pub(super) picker: Option<ChoicePicker>,
}

impl PageState {
    pub(super) fn entries(&self) -> Vec<Entry> {
        catalog::entries(
            self.page,
            self.category,
            Filter::of(
                self.filter.text(),
                &self.values,
                self.integrations_available,
            ),
            self.picker
                .map(|picker| (picker.row(), Choice::options(picker.row()))),
        )
    }

    /// The rows that take focus, in reading order.
    pub(super) fn rows(&self) -> Vec<Row> {
        self.entries()
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Setting(row) => Some(row),
                _ => None,
            })
            .collect()
    }

    /// Lines an entry takes in the right column. A described heading adds its description; a
    /// heading after another group, and the settings-file line, start with a blank line.
    pub(super) fn height(&self, entry: Entry, first: bool) -> usize {
        let gap = usize::from(!first);
        if self.page.is_child() {
            return match entry {
                Entry::Heading(_) => gap + 1,
                _ => 1,
            };
        }
        match entry {
            Entry::Heading(group) => gap + 1 + usize::from(self.described(group)),
            Entry::Setting(Row::ThemeOption(_)) => 1,
            Entry::Setting(_) => 2,
            Entry::Option(_) | Entry::End | Entry::NoResults => 1,
            Entry::ConfigFile => 2,
        }
    }

    /// A group whose heading carries a description line.
    pub(super) fn described(&self, group: Group) -> bool {
        match group {
            Group::LaunchProfiles => true,
            Group::LightThemes => self.values.application_theme == ApplicationAppearance::Dark,
            Group::DarkThemes => self.values.application_theme == ApplicationAppearance::Light,
            _ => false,
        }
    }

    /// Each entry's first line and height.
    fn spans(&self, entries: &[Entry]) -> Vec<(usize, usize)> {
        let mut line = 0;
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let height = self.height(*entry, index == 0);
                let span = (line, height);
                line += height;
                span
            })
            .collect()
    }

    /// The lines the focused row, or the open list's candidate, takes.
    fn focus_span(&self, entries: &[Entry], spans: &[(usize, usize)]) -> Option<(usize, usize)> {
        let index = match self.picker {
            Some(picker) => entries
                .iter()
                .position(|entry| *entry == Entry::Option(picker.candidate()))?,
            None => entries
                .iter()
                .position(|entry| *entry == Entry::Setting(self.row))?,
        };
        let (start, height) = spans[index];
        // Keep a heading beside its group's first row whenever both fit.
        let start = match index.checked_sub(1).map(|previous| entries[previous]) {
            Some(Entry::Heading(_)) => spans[index - 1].0,
            _ => start,
        };
        Some((start, spans[index].0 + height - start))
    }

    pub(super) fn total_lines(&self) -> usize {
        let entries = self.entries();
        self.spans(&entries)
            .last()
            .map_or(0, |(start, height)| start + height)
    }

    pub(super) fn first_visible(&self, visible: usize) -> usize {
        let visible = visible.max(1);
        let entries = self.entries();
        let spans = self.spans(&entries);
        let total = spans.last().map_or(0, |(start, height)| start + height);
        let requested = self.viewport.min(total.saturating_sub(visible));
        if !self.follow_selection {
            return requested;
        }
        let Some((start, height)) = self.focus_span(&entries, &spans) else {
            return requested;
        };
        if start < requested {
            start
        } else if start + height > requested + visible {
            (start + height).saturating_sub(visible).min(start)
        } else {
            requested
        }
    }

    /// Whether the focused row is wholly inside the drawn lines.
    pub(super) fn focus_drawn(&self, first: usize, visible: usize) -> bool {
        let entries = self.entries();
        let spans = self.spans(&entries);
        let index = match self.picker {
            Some(picker) => entries
                .iter()
                .position(|entry| *entry == Entry::Option(picker.candidate())),
            None => entries
                .iter()
                .position(|entry| *entry == Entry::Setting(self.row)),
        };
        index.is_some_and(|index| {
            let (start, height) = spans[index];
            start >= first && start + height <= first + visible
        })
    }

    pub(super) fn presentation(&self, visible: usize, narrow: bool) -> SettingsDraw {
        SettingsDraw {
            token: self.token,
            lines: Some((self.first_visible(visible), visible)),
            narrow,
        }
    }

    /// Focus stays on a row the page still shows; otherwise it moves to the first one.
    pub(super) fn settle_focus(&mut self) {
        let rows = self.rows();
        if !rows.contains(&self.row) {
            match rows.first() {
                Some(row) => self.row = *row,
                None if !self.page.is_child() => self.pane = Pane::Categories,
                None => {}
            }
        }
        if let Some(picker) = self.picker
            && !rows.contains(&picker.row())
        {
            self.picker = None;
        }
    }

    pub(super) fn show_category(&mut self, category: Category) {
        self.category = category;
        self.filter.clear();
        self.searching = false;
        self.picker = None;
        self.viewport = 0;
        self.follow_selection = true;
        self.row = self.rows().first().copied().unwrap_or(Row::Back);
    }

    pub(super) fn enter_page(&mut self, page: Page, row: Row) {
        self.parents.push(Navigation {
            page: self.page,
            row: self.row,
            viewport: self.viewport,
        });
        self.page = page;
        self.row = row;
        self.pane = Pane::Rows;
        self.picker = None;
        self.viewport = 0;
        self.visible_rows = 0;
        self.follow_selection = true;
        self.activation_ready = false;
    }

    pub(super) fn return_to_parent(&mut self) {
        let navigation = self.parents.pop().unwrap_or(Navigation {
            page: Page::General,
            row: self.page.opener(),
            viewport: 0,
        });
        self.page = navigation.page;
        self.row = navigation.row;
        self.viewport = navigation.viewport;
        self.visible_rows = 0;
        self.follow_selection = true;
        self.activation_ready = false;
        self.invalid = false;
        self.invalid_field = None;
        self.confirm_delete = false;
        self.settle_focus();
    }

    /// Shown values follow the newest confirmed ones unless an edit is in flight; the open
    /// launch-profile editor keeps its catalog.
    pub(super) fn follow_confirmed(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let catalog = (self.page == Page::Profiles).then(|| self.values.launch_profiles.clone());
        self.values = self.confirmed.values.clone();
        if let Some(catalog) = catalog {
            self.values.launch_profiles = catalog;
        }
        let profiles = self.values.launch_profiles.profiles.len();
        self.profile = self.profile.min(profiles.saturating_sub(1));
        self.settle_focus();
    }

    pub(super) fn current_profile(&self) -> &UserLaunchProfile {
        let profiles = &self.values.launch_profiles.profiles;
        &profiles[self.profile.min(profiles.len() - 1)]
    }

    /// The edit that puts a row's keys back to their defaults.
    pub(super) fn restore_changes(&self, row: Row) -> Vec<SettingChange> {
        if let Some(key) = text_key(row) {
            return orchbee_workbench_protocol::SettingPath::parse(key)
                .map(|key| vec![SettingChange::Restore { key }])
                .unwrap_or_default();
        }
        let Some(field) = setting_for_row(row).map(|metadata| metadata.key) else {
            return Vec::new();
        };
        orchbee_local_runtime::value_changes(&self.values, &self.values, &[field])
    }

    /// The file sets one of this row's keys, so the row shows as changed and can be restored.
    pub(super) fn changed(&self, row: Row) -> bool {
        !self.page.is_child()
            && self.restore_changes(row).iter().any(|change| match change {
                SettingChange::Restore { key } => self.confirmed.sets(key.as_str()),
                SettingChange::Set { .. } => false,
            })
    }

    /// The text a text-key row shows: a pending edit's request, else the file's value.
    pub(super) fn text_value(&self, row: Row) -> Option<&str> {
        if let Some(Pending {
            row: pending,
            text: Some(text),
            ..
        }) = &self.pending
            && *pending == row
        {
            return text.as_deref();
        }
        self.confirmed.text(text_key(row)?)
    }

    pub(super) fn enabled(&self, integrations: &Integrations, row: Row) -> bool {
        // Integration rows act through the Worker, independently of the settings file.
        match row {
            Row::Agent(id) => return integrations.block(id).is_none(),
            Row::RefreshIntegrations => return !integrations.pending(),
            Row::RemoteHosts => return false,
            Row::AgentIntegrations | Row::Back | Row::LaunchProfiles => return true,
            Row::ThemeOption(_) => return !self.reduced_color,
            _ => {}
        }
        let catalog = &self.values.launch_profiles;
        let profile = self.current_profile();
        let arguments = match &profile.command {
            UserLaunchCommand::CustomExecutable { arguments, .. } => Some(arguments),
            UserLaunchCommand::SystemShell { .. } => None,
        };
        match row {
            row if setting_for_row(row).is_some_and(|metadata| {
                metadata.availability == SettingAvailability::TerminalDisplayThemeCustom
            }) =>
            {
                self.values.terminal_display_theme
                    == orchbee_workbench_protocol::TerminalDisplayAppearance::Custom
            }
            row if setting_for_row(row).is_some_and(|metadata| {
                metadata.availability == SettingAvailability::NamedThemes
            }) =>
            {
                !self.reduced_color
            }
            Row::Profile => catalog.profiles.len() > 1,
            Row::Login => {
                self.login_shell_supported
                    && matches!(profile.command, UserLaunchCommand::SystemShell { .. })
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

    pub(super) fn checked(&self, integrations: &Integrations, row: Row) -> Option<bool> {
        let values = &self.values;
        match row {
            Row::Agent(id) => {
                Some(integrations.row(id).state == orchbee_agent_integration::State::Enabled)
            }
            Row::CopyOnSelect => Some(values.copy_on_select),
            Row::TrimAgents => Some(values.trim_trailing_blank_rows_for_agents),
            Row::ResumeAgents => Some(values.resume_agents_on_restore),
            Row::RestoreOutput => Some(values.restore_terminal_output),
            Row::Sound => Some(values.sound_enabled),
            Row::Login => match &self.current_profile().command {
                UserLaunchCommand::SystemShell { login } => Some(*login),
                UserLaunchCommand::CustomExecutable { .. } => None,
            },
            _ => None,
        }
    }

    pub(super) fn adjustment(&self, row: Row) -> Option<Adjustment> {
        let bounded = |field: UserSettingsField, current: u64| {
            let SettingControl::BoundedInteger {
                minimum, maximum, ..
            } = setting(field).control
            else {
                unreachable!("integer preference metadata")
            };
            Adjustment {
                backward: current > minimum,
                forward: current < maximum,
            }
        };
        let cycle = |index: usize, len: usize| Adjustment {
            backward: len > 1 && index < len,
            forward: len > 1 && index < len,
        };
        match row {
            Row::CompletionThreshold => Some(bounded(
                UserSettingsField::CompletionReminderMinimumSeconds,
                u64::from(self.values.completion_reminder_minimum_seconds),
            )),
            Row::Scrollback => Some(bounded(
                UserSettingsField::ScrollbackBytes,
                u64::from(self.values.scrollback_bytes),
            )),
            Row::Profile => Some(cycle(
                self.profile,
                self.values.launch_profiles.profiles.len(),
            )),
            Row::Argument => Some(match &self.current_profile().command {
                UserLaunchCommand::CustomExecutable { arguments, .. } => {
                    cycle(self.argument, arguments.len())
                }
                UserLaunchCommand::SystemShell { .. } => Adjustment {
                    backward: false,
                    forward: false,
                },
            }),
            _ => None,
        }
    }
}

/// The key of a row that edits one text key the common values do not carry.
pub(super) const fn text_key(row: Row) -> Option<&'static str> {
    match row {
        Row::NewTerminalFolder => Some("terminal.new_cwd"),
        Row::PrefixKey => Some("keys.prefix"),
        _ => None,
    }
}

/// The page as the renderer, the key map and the text projections read it.
#[derive(Clone, Copy)]
pub(crate) struct PageView<'a> {
    state: &'a PageState,
    integrations: &'a Integrations,
}

impl<'a> PageView<'a> {
    pub(super) const fn new(state: &'a PageState, integrations: &'a Integrations) -> Self {
        Self {
            state,
            integrations,
        }
    }
    pub(crate) const fn token(self) -> Token {
        self.state.token
    }
    pub(crate) const fn page(self) -> Page {
        self.state.page
    }
    pub(crate) const fn category(self) -> Category {
        self.state.category
    }
    pub(crate) const fn pane(self) -> Pane {
        self.state.pane
    }
    pub(crate) const fn selected(self) -> Row {
        self.state.row
    }
    pub(crate) const fn narrow(self) -> bool {
        self.state.narrow
    }
    pub(crate) fn query(self) -> &'a str {
        self.state.filter.text()
    }
    pub(crate) const fn searching(self) -> bool {
        self.state.searching
    }
    pub(crate) fn search_cursor(self) -> usize {
        self.state.filter.cursor()
    }
    pub(crate) const fn picker(self) -> Option<ChoicePicker> {
        self.state.picker
    }
    /// The open row editor's text and cursor.
    pub(crate) fn text(self) -> Option<(&'a str, usize)> {
        self.state
            .text
            .as_ref()
            .map(|text| (text.text(), text.cursor()))
    }
    pub(crate) const fn invalid(self) -> bool {
        self.state.invalid
    }
    pub(crate) const fn invalid_field(self) -> Option<(usize, UserLaunchField)> {
        self.state.invalid_field
    }
    pub(crate) const fn confirm_delete(self) -> bool {
        self.state.confirm_delete
    }
    pub(crate) const fn file(self) -> &'a FileState {
        &self.state.file
    }
    pub(crate) fn file_path(self) -> Option<&'a str> {
        self.state.confirmed.file_path.as_deref()
    }
    pub(crate) const fn values(self) -> &'a UserSettingsValues {
        &self.state.values
    }
    pub(crate) fn pending_row(self) -> Option<Row> {
        self.state.pending.as_ref().map(|pending| pending.row)
    }
    pub(crate) const fn profile(self) -> usize {
        self.state.profile
    }
    pub(crate) const fn argument(self) -> usize {
        self.state.argument
    }
    pub(crate) const fn integrations(self) -> &'a Integrations {
        self.integrations
    }
    pub(crate) const fn login_shell_supported(self) -> bool {
        self.state.login_shell_supported
    }
    pub(crate) const fn reduced_color(self) -> bool {
        self.state.reduced_color
    }
    pub(crate) fn entries(self) -> Vec<Entry> {
        self.state.entries()
    }
    pub(crate) fn height(self, entry: Entry, first: bool) -> usize {
        self.state.height(entry, first)
    }
    pub(crate) fn described(self, group: Group) -> bool {
        self.state.described(group)
    }
    pub(crate) fn enabled(self, row: Row) -> bool {
        self.state.enabled(self.integrations, row)
    }
    pub(crate) fn checked(self, row: Row) -> Option<bool> {
        self.state.checked(self.integrations, row)
    }
    pub(crate) fn adjustment(self, row: Row) -> Option<Adjustment> {
        self.state.adjustment(row)
    }
    /// The file sets this row, so it shows as changed and `Delete` restores its default.
    pub(crate) fn changed(self, row: Row) -> bool {
        self.state.changed(row)
    }
    pub(crate) fn text_value(self, row: Row) -> Option<&'a str> {
        self.state.text_value(row)
    }
    /// The theme list whose slot the app shows now: the slot Appearance fixes, or under System
    /// the one of the host's polarity.
    pub(crate) fn in_use(self, group: Group, system_dark: bool) -> bool {
        let dark = match self.state.values.application_theme {
            ApplicationAppearance::Light => false,
            ApplicationAppearance::Dark => true,
            ApplicationAppearance::System => system_dark,
        };
        match group {
            Group::LightThemes => !dark,
            Group::DarkThemes => dark,
            _ => false,
        }
    }
    pub(crate) fn first_visible(self, visible: usize) -> usize {
        self.state.first_visible(visible)
    }
    pub(crate) fn presentation(self, visible: usize, narrow: bool) -> SettingsDraw {
        self.state.presentation(visible, narrow)
    }
}
