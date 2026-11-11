//! Keys, clicks and wheel turns on the page. A change on a category's row becomes one edit the
//! Server applies at once; the launch-profile editor applies its catalog when it closes.
use super::{
    AdjustmentStep, Category, Choice, ChoicePicker, Control, Edit, Editor, IntegrationJob, Page,
    Pane, Row, State, Submission, Token, changes::Change, page::text_key, values,
};
use crate::terminal_text::line_editor::{LengthLimit, LineEditor};
use orchbee_workbench_protocol::{
    SettingChange, SettingPath, SettingValue, UserLaunchField, UserSettingsValues,
};

impl Editor {
    /// One key's edit. Keys act on the state the keys before them left, so every key of a packet
    /// applies in order once the page has painted; only pointer actions need the current frame.
    pub(crate) fn edit(&mut self, edit: Edit) -> Option<Submission> {
        if matches!(edit, Edit::Cancel) {
            return self.cancel();
        }
        #[cfg(any(test, feature = "test-support"))]
        let fixture_selection = matches!(edit, Edit::Select(_));
        #[cfg(not(any(test, feature = "test-support")))]
        let fixture_selection = false;
        let presented = self.surface_presented;
        let State::Open(page) = &mut self.state else {
            return None;
        };
        if !presented && !fixture_selection {
            return None;
        }
        #[cfg(any(test, feature = "test-support"))]
        if let Edit::Select(row) = edit {
            page.searching = false;
            page.picker = None;
            if !page.rows().contains(&row)
                && page.page == Page::General
                && let Some(category) = super::Category::ALL.into_iter().find(|category| {
                    category
                        .groups()
                        .iter()
                        .any(|group| group.rows().contains(&row))
                })
            {
                page.show_category(category);
            }
            if !page.rows().contains(&row) {
                return None;
            }
            page.pane = Pane::Rows;
            page.row = row;
            page.follow_selection = true;
            self.redraw();
            return None;
        }
        if page.text.is_some() {
            if matches!(edit, Edit::Activate) {
                return self.accept_text();
            }
            values::edit_text(page, edit);
            self.redraw();
            return None;
        }
        if page.confirm_delete {
            if matches!(edit, Edit::Activate) {
                page.confirm_delete = false;
                values::delete_profile(page);
                self.redraw();
            }
            return None;
        }
        if let Some(mut picker) = page.picker {
            match edit {
                Edit::Next | Edit::Adjust(true) => picker.navigate(true),
                Edit::Previous | Edit::Adjust(false) => picker.navigate(false),
                Edit::Home => picker.cursor = 0,
                Edit::End => picker.cursor = Choice::options(picker.row).len() - 1,
                Edit::Activate => {
                    page.picker = None;
                    let mut requested = page.values.clone();
                    picker.candidate().apply(&mut requested);
                    self.redraw();
                    return self.commit(picker.row, requested);
                }
                _ => return None,
            }
            page.picker = Some(picker);
            page.follow_selection = true;
            self.redraw();
            return None;
        }
        if page.searching {
            match edit {
                Edit::Activate | Edit::Next if page.rows().is_empty() => return None,
                Edit::Activate | Edit::Next => {
                    page.searching = false;
                    page.pane = Pane::Rows;
                    page.row = page.rows()[0];
                    page.follow_selection = true;
                }
                Edit::Insert(character) => {
                    page.invalid = page.filter.insert(&character.to_string()).is_err();
                }
                Edit::Backspace => {
                    page.filter.backspace();
                    page.invalid = false;
                }
                Edit::Delete => {
                    page.filter.delete();
                    page.invalid = false;
                }
                Edit::Adjust(false) => {
                    page.filter.left();
                }
                Edit::Adjust(true) => {
                    page.filter.right();
                }
                Edit::Home => {
                    page.filter.home();
                }
                Edit::End => {
                    page.filter.end();
                }
                Edit::Line(key) => {
                    page.invalid = page.filter.apply(key).is_err();
                }
                _ => return None,
            }
            page.viewport = 0;
            page.follow_selection = true;
            page.settle_focus();
            self.redraw();
            return None;
        }
        match edit {
            Edit::Search if page.page == Page::General => {
                page.searching = true;
                page.invalid = false;
                self.redraw();
                return None;
            }
            Edit::Explain => {
                let id = match (page.page, page.row) {
                    (Page::Integrations, Row::Agent(id)) if page.activation_ready => id,
                    (Page::Diagnosis(id), _) => id,
                    _ => return None,
                };
                if !self.integrations.begin(IntegrationJob::Explain(id)) {
                    return None;
                }
                if page.page == Page::Integrations {
                    page.enter_page(Page::Diagnosis(id), Row::Back);
                }
                self.redraw();
                return None;
            }
            _ => {}
        }
        if page.pane == Pane::Categories && page.page == Page::General {
            let index = Category::ALL
                .iter()
                .position(|category| *category == page.category)
                .unwrap_or(0);
            let category = match edit {
                Edit::Next => Category::ALL.get(index + 1).copied(),
                Edit::Previous => index.checked_sub(1).map(|index| Category::ALL[index]),
                Edit::Home => Some(Category::ALL[0]),
                Edit::End => Some(Category::ALL[Category::ALL.len() - 1]),
                Edit::Adjust(true) | Edit::Pane(true) | Edit::Activate => {
                    if page.rows().is_empty() {
                        return None;
                    }
                    page.pane = Pane::Rows;
                    page.follow_selection = true;
                    page.settle_focus();
                    None
                }
                _ => return None,
            };
            if let Some(category) = category {
                page.show_category(category);
            }
            self.redraw();
            return None;
        }
        let rows = page.rows();
        let index = rows.iter().position(|row| *row == page.row).unwrap_or(0);
        page.invalid = false;
        page.invalid_field = None;
        match edit {
            Edit::Next | Edit::Previous | Edit::Home | Edit::End | Edit::Page(_)
                if rows.is_empty() =>
            {
                return None;
            }
            Edit::Next => page.row = rows[(index + 1).min(rows.len() - 1)],
            Edit::Previous => page.row = rows[index.saturating_sub(1)],
            Edit::Home => page.row = rows[0],
            Edit::End => page.row = rows[rows.len() - 1],
            Edit::Page(forward) => {
                let amount = (page.visible_rows / 2).max(1);
                page.row = rows[if forward {
                    index.saturating_add(amount).min(rows.len() - 1)
                } else {
                    index.saturating_sub(amount)
                }];
            }
            Edit::Pane(false) | Edit::Adjust(false)
                if page.page == Page::General
                    || !matches!(page.row, Row::Profile | Row::Argument) =>
            {
                if page.page == Page::General {
                    page.pane = Pane::Categories;
                }
            }
            Edit::Adjust(forward) => {
                if !page.activation_ready
                    || !matches!(page.row, Row::Profile | Row::Argument)
                    || !page.enabled(&self.integrations, page.row)
                {
                    return None;
                }
                values::adjust(page, forward, AdjustmentStep::Small);
            }
            Edit::Delete => return self.restore(None),
            Edit::Activate => return self.activate(),
            _ => return None,
        }
        if let State::Open(page) = &mut self.state {
            page.follow_selection = true;
        }
        self.redraw();
        None
    }

    /// `Esc`: the one step back the page offers now.
    fn cancel(&mut self) -> Option<Submission> {
        let mut close = false;
        match &mut self.state {
            State::Open(page) if page.text.is_some() => {
                page.text = None;
                page.invalid = false;
                page.invalid_field = None;
            }
            State::Open(page) if page.confirm_delete => page.confirm_delete = false,
            State::Open(page) if page.picker.is_some() => page.picker = None,
            State::Open(page) if page.searching && !page.filter.text().is_empty() => {
                page.filter.clear();
                page.invalid = false;
                page.settle_focus();
            }
            State::Open(page) if page.searching => page.searching = false,
            State::Open(page) if page.page == Page::Profiles => return self.leave_profiles(),
            State::Open(page) if page.page.is_child() => page.return_to_parent(),
            State::Open(page) if !page.filter.text().is_empty() => {
                page.filter.clear();
                page.viewport = 0;
                page.settle_focus();
            }
            // A focused theme line not yet in use previews; leaving it returns to the one in use.
            State::Open(page)
                if page.pane == Pane::Rows
                    && matches!(page.row, Row::ThemeOption(theme) if !theme.current(&page.values)) =>
            {
                let current = page.rows().into_iter().find(|row| {
                    matches!((row, page.row), (Row::ThemeOption(candidate), Row::ThemeOption(focused))
                        if candidate.current(&page.values)
                            && candidate.theme().polarity() == focused.theme().polarity())
                });
                if let Some(current) = current {
                    page.row = current;
                    page.follow_selection = true;
                }
            }
            State::Open(page) if page.narrow && page.pane == Pane::Rows => {
                page.pane = Pane::Categories;
            }
            State::Loading { visible, .. } => {
                *visible = false;
                close = true;
            }
            _ => {
                self.state = State::Closed;
                close = true;
            }
        }
        if close {
            self.surface_presented = false;
        } else {
            self.redraw();
        }
        None
    }

    /// `Enter`, `Space` or a click on the focused row: its own primary action.
    fn activate(&mut self) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        let row = page.row;
        if !page.activation_ready || !page.enabled(&self.integrations, row) {
            return None;
        }
        let job = match row {
            Row::Agent(id) => self.integrations.change(id),
            Row::RefreshIntegrations => Some(IntegrationJob::Refresh),
            _ => None,
        };
        if let Some(job) = job {
            self.integrations.begin(job);
            self.redraw();
            return None;
        }
        match row {
            Row::LaunchProfiles => {
                page.enter_page(Page::Profiles, Row::Profile);
                page.profile = page
                    .values
                    .launch_profiles
                    .profiles
                    .iter()
                    .position(|profile| {
                        profile.id == page.values.launch_profiles.default_profile_id
                    })
                    .unwrap_or(0);
                page.argument = 0;
            }
            Row::AgentIntegrations => {
                page.enter_page(
                    Page::Integrations,
                    Row::Agent(orchbee_agent_integration::IntegrationId::ALL[0]),
                );
                // Opening the page is the explicit request to inspect providers.
                self.integrations.begin(IntegrationJob::Refresh);
            }
            Row::Back if page.page == Page::Profiles => return self.leave_profiles(),
            Row::Back => page.return_to_parent(),
            Row::ThemeOption(theme) => {
                let mut requested = page.values.clone();
                Choice::ColorTheme(theme).apply(&mut requested);
                return self.commit(row, requested);
            }
            _ if page.page == Page::Profiles => {
                if let Some(limit) = row.text_limit() {
                    if let Some(value) = values::text(page) {
                        page.text = Some(LineEditor::new(value, LengthLimit::Bytes(limit)));
                    }
                } else {
                    values::activate(page);
                }
            }
            _ => {
                if let Some(picker) = ChoicePicker::open(row, &page.values) {
                    page.picker = Some(picker);
                    page.follow_selection = true;
                } else if let Some(limit) = row.text_limit() {
                    if let Some(value) = values::text(page) {
                        page.text = Some(LineEditor::new(value, LengthLimit::Bytes(limit)));
                    }
                } else {
                    let before = page.values.clone();
                    values::adjust(page, true, AdjustmentStep::Small);
                    let requested = std::mem::replace(&mut page.values, before);
                    return self.commit(row, requested);
                }
            }
        }
        self.redraw();
        None
    }

    /// `Enter` in a row editor: a valid text applies; an invalid one stays open and says so.
    fn accept_text(&mut self) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        let row = page.row;
        let value = page.text.as_ref()?.text().to_owned();
        if text_key(row).is_some() {
            page.text = None;
            self.redraw();
            return self.commit_text(row, value);
        }
        if page.page == Page::Profiles {
            if values::accept_text(page, value) {
                page.text = None;
            } else {
                page.invalid = true;
            }
            self.redraw();
            return None;
        }
        let before = page.values.clone();
        if !values::accept_text(page, value) {
            page.invalid = true;
            self.redraw();
            return None;
        }
        page.text = None;
        let requested = std::mem::replace(&mut page.values, before);
        self.redraw();
        self.commit(row, requested)
    }

    /// Asks for the edit that turns the shown values into `requested`. It is kept in order behind
    /// the changes before it and sent once they are answered; the row shows the request meanwhile.
    fn commit(&mut self, row: Row, requested: UserSettingsValues) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        let changes = orchbee_local_runtime::value_changes(&page.values, &requested, &[]);
        if changes.is_empty() {
            return None;
        }
        let change = Change::new(row, changes, None, &page.values, &requested);
        self.queue(change)
    }

    /// Asks for a text key's new value; an empty text restores its default.
    fn commit_text(&mut self, row: Row, value: String) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        let key = SettingPath::parse(text_key(row)?).ok()?;
        if page.text_value(row).unwrap_or_default() == value {
            return None;
        }
        let (change, text) = if value.is_empty() {
            (SettingChange::Restore { key }, None)
        } else {
            (
                SettingChange::Set {
                    key,
                    value: SettingValue::Text(value.clone()),
                },
                Some(value),
            )
        };
        let change = Change::new(row, vec![change], Some(text), &page.values, &page.values);
        self.queue(change)
    }

    /// Asks to restore a row's keys to their defaults: `row`, or the focused row. A row the file
    /// sets, or one with a change in flight or waiting, can be restored.
    fn restore(&mut self, row: Option<Row>) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        let row = row.unwrap_or(page.row);
        if !(page.changed(row) || page.changes.waits(row)) {
            return None;
        }
        let changes = page.restore_changes(row);
        if changes.is_empty() {
            return None;
        }
        page.row = row;
        let text = text_key(row).map(|_| None);
        let restored = page.restored_values(row);
        let change = Change::new(row, changes, text, &page.values, &restored);
        self.queue(change)
    }

    /// Keeps a change in order and sends it when nothing is in flight. A request beyond the
    /// waiting limit is refused with the busy notice and changes nothing.
    fn queue(&mut self, change: Change) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        if !page.changes.push(change) {
            self.notice = Some(crate::localization::SettingsNotice::Busy);
            self.redraw();
            return None;
        }
        page.follow_confirmed();
        self.redraw();
        self.next_submission()
    }

    /// Leaves the launch-profile editor. An edited catalog applies as one edit; an invalid one
    /// first names its field, and leaving again discards it.
    fn leave_profiles(&mut self) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        let catalog = page.values.launch_profiles.clone();
        let mut requested = page.confirmed.values.clone();
        requested.launch_profiles = catalog;
        if requested.launch_profiles == page.confirmed.values.launch_profiles {
            page.return_to_parent();
            self.redraw();
            return None;
        }
        if !requested.is_valid() {
            if page.invalid {
                page.values.launch_profiles = page.confirmed.values.launch_profiles.clone();
                page.return_to_parent();
            } else {
                page.invalid = true;
                page.invalid_field = requested.launch_profiles.invalid_field();
                if let Some((index, field)) = page.invalid_field {
                    page.profile = index;
                    (page.row, page.argument) = match field {
                        UserLaunchField::Name => (Row::ProfileName, 0),
                        UserLaunchField::Executable => (Row::Executable, 0),
                        UserLaunchField::Argument(index) => (Row::ArgumentValue, index),
                    };
                    page.follow_selection = true;
                }
            }
            self.redraw();
            return None;
        }
        let changes = orchbee_local_runtime::value_changes(&page.confirmed.values, &requested, &[]);
        let change = Change::new(
            Row::LaunchProfiles,
            changes,
            None,
            &page.confirmed.values,
            &requested,
        );
        page.return_to_parent();
        self.queue(change)
    }

    pub(crate) fn scroll(&mut self, token: Token, up: bool, lines: usize) -> Option<Token> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        if page.token != token || !page.drawn || page.confirm_delete {
            return None;
        }
        let maximum = page.total_lines().saturating_sub(page.visible_rows.max(1));
        let next = if up {
            page.viewport.saturating_sub(lines)
        } else {
            page.viewport.saturating_add(lines).min(maximum)
        };
        if next == page.viewport {
            return Some(token);
        }
        page.viewport = next;
        page.follow_selection = false;
        page.activation_ready = false;
        self.redraw();
        self.view().map(|view| view.token())
    }

    pub(crate) fn click(&mut self, token: Token, row: Row) -> Option<Submission> {
        let State::Open(page) = &mut self.state else {
            return None;
        };
        if page.token != token
            || !page.drawn
            || page.text.is_some()
            || page.confirm_delete
            || page.picker.is_some()
            || !page.rows().contains(&row)
        {
            return None;
        }
        page.searching = false;
        page.pane = Pane::Rows;
        page.row = row;
        page.follow_selection = false;
        page.activation_ready = true;
        self.activate()
    }

    pub(crate) fn control(&mut self, token: Token, control: Control) -> Option<Submission> {
        if self.view().is_none_or(|view| view.token() != token) {
            return None;
        }
        if control == Control::Close {
            match &mut self.state {
                State::Loading { visible, .. } => *visible = false,
                _ => self.state = State::Closed,
            }
            self.surface_presented = false;
            return None;
        }
        let State::Open(page) = &mut self.state else {
            return None;
        };
        if !page.drawn {
            return None;
        }
        let idle = page.text.is_none() && !page.confirm_delete;
        match control {
            Control::Search if idle && page.page == Page::General => {
                page.picker = None;
                page.searching = true;
                page.invalid = false;
                self.redraw();
                None
            }
            Control::Category(category) if idle => {
                let submission = if page.page == Page::Profiles {
                    self.leave_profiles()
                } else {
                    None
                };
                let State::Open(page) = &mut self.state else {
                    return submission;
                };
                while page.page.is_child() && page.page != Page::Profiles {
                    page.return_to_parent();
                }
                if page.page == Page::General {
                    page.show_category(category);
                    page.pane = if page.narrow {
                        Pane::Rows
                    } else {
                        Pane::Categories
                    };
                    page.settle_focus();
                }
                self.redraw();
                submission
            }
            Control::Choose(choice) if idle => {
                let mut picker = page.picker?;
                picker.cursor = Choice::options(picker.row)
                    .iter()
                    .position(|candidate| *candidate == choice)?;
                page.picker = Some(picker);
                self.edit(Edit::Activate)
            }
            Control::Restore(row) if idle && page.picker.is_none() => self.restore(Some(row)),
            Control::Adjust { row, forward }
                if idle
                    && page.picker.is_none()
                    && page.rows().contains(&row)
                    && page.enabled(&self.integrations, row)
                    && page.adjustment(row).is_some_and(|adjustment| {
                        if forward {
                            adjustment.forward
                        } else {
                            adjustment.backward
                        }
                    }) =>
            {
                page.pane = Pane::Rows;
                page.row = row;
                page.follow_selection = false;
                page.activation_ready = true;
                if page.page == Page::Profiles {
                    values::adjust(page, forward, AdjustmentStep::Small);
                    self.redraw();
                    return None;
                }
                let before = page.values.clone();
                values::adjust(page, forward, AdjustmentStep::Small);
                let requested = std::mem::replace(&mut page.values, before);
                self.commit(row, requested)
            }
            Control::Explain(id) if idle && page.page == Page::Integrations => {
                page.row = Row::Agent(id);
                page.follow_selection = false;
                page.activation_ready = true;
                self.edit(Edit::Explain)
            }
            Control::Back => {
                if page.page.is_child() || page.text.is_some() || page.picker.is_some() {
                    return self.cancel();
                }
                page.pane = Pane::Categories;
                self.redraw();
                None
            }
            _ => None,
        }
    }
}
