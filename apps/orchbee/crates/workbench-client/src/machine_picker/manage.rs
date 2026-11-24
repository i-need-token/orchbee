//! Managing saved Machines from the picker: the fixed "Add machine…" row and its form, the
//! selected row's detail and action, the row action menu, the inline rename and the
//! remove-and-stop confirmation. This module holds the page and the person's input; `exchange`
//! carries what they asked for to discovery and the machines store.
//! `apps/orchbee/docs/machines.md` owns the rules.

use super::{Choice, Mode, Picker};
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use crate::machine_add::Stage;
use crate::machine_color::MachineColor;
use crate::machine_discovery::Observation;
use crate::machine_status::{Action, Explanation, Facts};
use crate::machines::{
    Profile, ProfileId, default_label, names_this_computer, valid_label, valid_target,
};
use crate::terminal_events::keys::Binding;
use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};
use orchbee_machine_protocol::hub::MachineStateName;
use std::sync::Arc;

mod exchange;

/// The key that opens the selected Machine's action menu, as a sidebar row's menu opens.
pub(crate) const MENU_KEY: Binding = Binding::new(KeyCode::F(2), Modifiers::NONE);
const ENTER: Binding = Binding::new(KeyCode::Enter, Modifiers::NONE);
const ESC: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);
const TAB: Binding = Binding::new(KeyCode::Tab, Modifiers::NONE);
const BACK_TAB: Binding = Binding::new(KeyCode::Tab, Modifiers::NONE.with_shift());
const UP: Binding = Binding::new(KeyCode::Up, Modifiers::NONE);
const DOWN: Binding = Binding::new(KeyCode::Down, Modifiers::NONE);
const UP_LETTER: Binding = Binding::new(KeyCode::Char('k'), Modifiers::NONE);
const DOWN_LETTER: Binding = Binding::new(KeyCode::Char('j'), Modifiers::NONE);
const HOME: Binding = Binding::new(KeyCode::Home, Modifiers::NONE);
const END: Binding = Binding::new(KeyCode::End, Modifiers::NONE);
const BACKSPACE: Binding = Binding::new(KeyCode::Backspace, Modifiers::NONE);
const DELETE: Binding = Binding::new(KeyCode::Delete, Modifiers::NONE);
const LEFT: Binding = Binding::new(KeyCode::Left, Modifiers::NONE);
const RIGHT: Binding = Binding::new(KeyCode::Right, Modifiers::NONE);
const LINE_START: Binding = Binding::new(KeyCode::Char('a'), Modifiers::CTRL);
const LINE_END: Binding = Binding::new(KeyCode::Char('e'), Modifiers::CTRL);

/// The session name a new Machine's record carries, as `machine add` saves it.
const SESSION: &str = "default";
/// The longest address and name a field takes, in Unicode scalars.
const FIELD_LIMIT: usize = 1024;
const NAME_LIMIT: usize = 128;

/// One entry of a Machine's action menu, in the design's order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuItem {
    Rename,
    Disable,
    Enable,
    Disconnect,
    ForgetPassword,
    Remove,
    RemoveAndStop,
}

/// A field of the add form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Address,
    Name,
    Color,
}

/// A painted control of the add form, the rename or the confirmation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormControl {
    Field(Field),
    /// The SSH config host at this position fills the address.
    Suggestion(usize),
    /// One choice of the color.
    Color(MachineColor),
    Cancel,
    Submit,
}

/// What the picker shows inside its frame.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum Page {
    #[default]
    List,
    Menu {
        machine: ProfileId,
        selected: usize,
    },
    Rename {
        machine: ProfileId,
        name: LineEditor,
    },
    Confirm {
        machine: ProfileId,
    },
    /// Boxed: the form outweighs every other page.
    Add(Box<AddForm>),
}

/// The page kinds a painted target is closed over.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PageKind {
    #[default]
    List,
    Menu,
    Rename,
    Confirm,
    Add,
}

/// The add form: what the person typed, and how the checks of the add stand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AddForm {
    pub(crate) address: LineEditor,
    pub(crate) name: LineEditor,
    /// Opens on the color the fewest saved machines use.
    pub(crate) color: MachineColor,
    pub(crate) field: Field,
    /// The highlighted SSH config host while the address field has focus.
    pub(crate) suggestion: Option<usize>,
    pub(crate) suggestions: Arc<[String]>,
    /// The suggestions were read, once, for this form, or their read failed or ran out.
    pub(crate) suggestions_read: bool,
    /// The read of the suggestions in progress, off the input thread.
    pub(crate) reading: Option<crate::machine_ssh_config::HostRead>,
    pub(crate) check: Option<Check>,
    pub(crate) problem: Option<Problem>,
    /// The id the add in progress is checked under and the name it will be saved with, so its
    /// login names the machine as the person typed it, never by that id.
    pub(crate) adding: Option<(ProfileId, String)>,
}

/// How the add in progress stands, as the form shows it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Check {
    /// Asked for; discovery has not reported a check yet.
    Starting,
    Checking(Stage),
    Failed {
        stage: Option<Stage>,
        explanation: Explanation,
        /// Enter adds the same address again; the design offers no way past a changed host key
        /// or an unsupported system.
        retry: bool,
    },
    /// Every check passed and the record is being saved.
    Saving,
}

/// Why the form or an action did not go ahead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Problem {
    Address,
    /// The name is not a label, or it means this computer.
    Name,
    Save,
}

/// The outcome the picker's status row names until the next key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    /// The record went; its remote Session keeps running.
    Removed(String),
    /// The remote Session stopped and the record went.
    RemovedStopped(String),
    /// The stop failed: the record stays.
    StopFailed(String),
    /// The Machine was not online, so nothing was stopped: the record stays.
    NotOnline(String),
    Failed(Problem),
}

/// A saved Machine as the store holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) id: ProfileId,
    pub(crate) label: String,
    pub(crate) target: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) supported: bool,
}

/// Something the person asked for that `exchange` carries out.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Work {
    Add {
        label: String,
        target: String,
        color: MachineColor,
    },
    Discard,
    Rename {
        machine: ProfileId,
        label: String,
    },
    Enable {
        machine: ProfileId,
        enabled: bool,
    },
    Disconnect(ProfileId),
    Remove(ProfileId),
    Stop(ProfileId),
}

/// The values a painted management target is closed over.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Basis {
    pub(crate) epoch: u64,
    pub(crate) page: PageKind,
    pub(crate) menu: Box<[(MenuItem, bool)]>,
    pub(crate) suggestions: usize,
    pub(crate) submit: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Manage {
    page: Page,
    /// Counts page changes, so a click painted for another page changes nothing.
    epoch: u64,
    /// The fixed "Add machine…" row holds the selection instead of a Machine row.
    add_focused: bool,
    records: Vec<Record>,
    explanations: Vec<(ProfileId, Explanation)>,
    notice: Option<Notice>,
    work: Vec<Work>,
    /// The Machine whose remote Session is being stopped before its record goes.
    stopping: Option<ProfileId>,
    /// The Machine the next catalog refresh selects: the one just added.
    select: Option<ProfileId>,
    /// Add Machine opened from Settings › Machines, with the machine it saved once it did: the
    /// picker closes as the add ends, and Settings focuses that machine.
    settings_add: Option<Option<ProfileId>>,
}

impl Manage {
    pub(crate) const fn page(&self) -> &Page {
        &self.page
    }

    pub(crate) const fn add_focused(&self) -> bool {
        self.add_focused
    }

    pub(crate) fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub(crate) fn stopping(&self, machine: &ProfileId) -> bool {
        self.stopping.as_ref() == Some(machine)
    }

    pub(crate) fn record(&self, machine: &ProfileId) -> Option<&Record> {
        self.records.iter().find(|record| &record.id == machine)
    }

    fn label(&self, machine: &ProfileId) -> String {
        self.record(machine).map_or_else(
            || machine.as_str().to_owned(),
            |record| record.label.clone(),
        )
    }

    fn set_page(&mut self, page: Page) {
        self.page = page;
        self.epoch = self.epoch.wrapping_add(1);
    }

    /// Leaves any page for the list; an add whose checks run is given up.
    fn back(&mut self) {
        if let Page::Add(form) = &self.page
            && matches!(form.check, Some(Check::Starting | Check::Checking(_)))
        {
            self.work.push(Work::Discard);
        }
        if !matches!(self.page, Page::List) {
            self.set_page(Page::List);
        }
    }
}

impl Picker {
    pub(crate) const fn manage(&self) -> &Manage {
        &self.manage
    }

    /// The remote Machine of the row at `index` in the Machines list.
    pub(crate) fn row_machine(&self, index: usize) -> Option<&ProfileId> {
        if self.mode != Mode::Machines {
            return None;
        }
        match &self.rows.get(index)?.choice {
            Choice::Remote(id) => Some(id),
            Choice::Local => None,
        }
    }

    /// Whether the picker reads `machine` online, so its remote Session can be stopped.
    fn machine_online(&self, machine: &ProfileId) -> bool {
        self.rows.iter().any(|row| {
            row.choice == Choice::Remote(machine.clone())
                && matches!(
                    row.observation,
                    Some(Observation::Online(_) | Observation::State(MachineStateName::Online))
                )
        })
    }

    /// The terminals `machine`'s Session holds, when its overview is observed.
    pub(crate) fn machine_terminals(&self, machine: &ProfileId) -> Option<usize> {
        self.rows.iter().find_map(|row| match &row.observation {
            Some(Observation::Online(overview))
                if row.choice == Choice::Remote(machine.clone()) =>
            {
                Some(overview.terminals)
            }
            _ => None,
        })
    }

    /// Why the Machine at `index` reads its word, and the one action that helps.
    pub(crate) fn row_explanation(&self, index: usize) -> Option<Explanation> {
        let machine = self.row_machine(index)?;
        let record = self.manage.record(machine)?;
        if !record.supported {
            return Some(Explanation::default());
        }
        if !record.enabled {
            return Some(Explanation::disabled());
        }
        self.manage
            .explanations
            .iter()
            .find(|(id, _)| id == machine)
            .map(|(_, explanation)| explanation.clone())
            .or_else(|| {
                // Before discovery explained the Machine, its observation explains it without the
                // facts only the hub reports.
                let observation = self.rows.get(index)?.observation.as_ref()?;
                Some(Explanation::of(
                    observation,
                    &Facts::default(),
                    record.target.as_deref().unwrap_or_default(),
                ))
            })
    }

    /// The menu of `machine`: each item it offers and whether it is available now.
    pub(crate) fn menu_items(&self, machine: &ProfileId) -> Vec<(MenuItem, bool)> {
        let Some(record) = self.manage.record(machine) else {
            return Vec::new();
        };
        let mut items = vec![(MenuItem::Rename, true)];
        if record.supported {
            items.push(if record.enabled {
                (MenuItem::Disable, true)
            } else {
                (MenuItem::Enable, true)
            });
        }
        if record.enabled && record.supported {
            items.push((MenuItem::Disconnect, true));
        }
        if self.forgettable {
            items.push((MenuItem::ForgetPassword, true));
        }
        items.push((MenuItem::Remove, true));
        items.push((MenuItem::RemoveAndStop, self.machine_online(machine)));
        items
    }

    pub(super) fn manage_basis(&self) -> Basis {
        let (page, menu, suggestions, submit) = match &self.manage.page {
            Page::List => (PageKind::List, Box::default(), 0, false),
            Page::Menu { machine, .. } => (
                PageKind::Menu,
                self.menu_items(machine).into_boxed_slice(),
                0,
                false,
            ),
            Page::Rename { .. } => (PageKind::Rename, Box::default(), 0, true),
            Page::Confirm { machine } => (
                PageKind::Confirm,
                Box::default(),
                0,
                !self.manage.stopping(machine),
            ),
            Page::Add(form) => (
                PageKind::Add,
                Box::default(),
                form.suggestions.len(),
                form.can_submit(),
            ),
        };
        Basis {
            epoch: self.manage.epoch,
            page,
            menu,
            suggestions,
            submit,
        }
    }

    /// Keeps each saved Machine's address and state as the store holds it; called with the rows.
    pub(super) fn manage_records(&mut self, profiles: &[Profile]) {
        self.manage.records = profiles
            .iter()
            .map(|profile| Record {
                id: profile.id.clone(),
                label: profile.draft.label.clone(),
                target: profile.draft.ssh_target().map(str::to_owned),
                enabled: profile.enabled,
                supported: profile.draft.connection.supported(),
            })
            .collect();
        // A page about a Machine that went has nothing left to act on.
        let gone = match &self.manage.page {
            Page::Menu { machine, .. }
            | Page::Rename { machine, .. }
            | Page::Confirm { machine } => self.manage.record(machine).is_none(),
            Page::List | Page::Add(_) => false,
        };
        if gone {
            self.manage.set_page(Page::List);
        }
        if let Some(machine) = self.manage.select.take()
            && let Some(index) = self
                .rows
                .iter()
                .position(|row| row.choice == Choice::Remote(machine.clone()))
        {
            self.mode = Mode::Machines;
            self.selected = index;
            self.manage.add_focused = false;
        }
    }

    /// Leaves every page and the Add row as the picker opens, closes or changes mode.
    pub(super) fn reset_manage(&mut self) {
        self.manage.back();
        self.manage.add_focused = false;
    }

    /// The Add row's selection ends as a Machine row is clicked.
    pub(super) fn leave_add_row(&mut self) {
        self.manage.add_focused = false;
    }

    /// Opens Add Machine for Settings › Machines: the picker shows its add form and closes once
    /// the add was saved or given up.
    pub fn open_add_from_settings(&mut self) {
        self.show();
        if self.open {
            self.open_add();
            self.manage.settings_add = Some(None);
        }
    }

    /// Once the add opened from Settings ended: closes the picker and returns the machine it
    /// saved, `Some(None)` when it saved none.
    pub fn take_settings_add(&mut self) -> Option<Option<ProfileId>> {
        if self.open && matches!(self.manage.page, Page::Add(_)) {
            return None;
        }
        let added = self.manage.settings_add.take()?;
        if self.open {
            self.close_picker();
        }
        Some(added)
    }

    pub(super) fn open_add(&mut self) {
        self.manage.add_focused = true;
        self.manage.set_page(Page::Add(Box::new(AddForm {
            address: LineEditor::new(String::new(), LengthLimit::Scalars(FIELD_LIMIT)),
            name: LineEditor::new(String::new(), LengthLimit::Scalars(NAME_LIMIT)),
            color: MachineColor::least_used(self.rows.iter().filter_map(|row| row.color)),
            field: Field::Address,
            suggestion: None,
            suggestions: Arc::from([]),
            suggestions_read: false,
            reading: None,
            check: None,
            problem: None,
            adding: None,
        })));
    }

    pub(super) fn open_menu(&mut self, index: usize) -> bool {
        let Some(machine) = self.row_machine(index).cloned() else {
            return false;
        };
        self.selected = index;
        self.manage.add_focused = false;
        self.manage.set_page(Page::Menu {
            machine,
            selected: 0,
        });
        true
    }

    /// Performs the detail action of the row at `index`: a disabled Machine is enabled; any other
    /// action starts a selection of the Machine, which reconnects, logs in or trusts as needed.
    pub(super) fn row_action(&mut self, index: usize) -> bool {
        self.manage.add_focused = false;
        if let Some(machine) = self.row_machine(index).cloned()
            && self.row_explanation(index).and_then(|e| e.action) == Some(Action::Enable)
        {
            self.manage.work.push(Work::Enable {
                machine,
                enabled: true,
            });
            return true;
        }
        self.choose(index);
        self.request.is_some()
    }

    /// Takes the menu item: an unavailable one changes nothing.
    pub(super) fn choose_menu_item(&mut self, item: MenuItem) -> bool {
        let Page::Menu { machine, .. } = &self.manage.page else {
            return false;
        };
        let machine = machine.clone();
        if !self
            .menu_items(&machine)
            .iter()
            .any(|(offered, available)| *offered == item && *available)
        {
            return false;
        }
        let next = match item {
            MenuItem::Rename => {
                let label = self.manage.label(&machine);
                Page::Rename {
                    machine,
                    name: LineEditor::new(label, LengthLimit::Scalars(NAME_LIMIT)).select_all(),
                }
            }
            MenuItem::RemoveAndStop => Page::Confirm { machine },
            MenuItem::Disable | MenuItem::Enable => {
                self.manage.work.push(Work::Enable {
                    machine,
                    enabled: item == MenuItem::Enable,
                });
                Page::List
            }
            MenuItem::Disconnect => {
                self.manage.work.push(Work::Disconnect(machine));
                Page::List
            }
            MenuItem::ForgetPassword => {
                self.forget_request = Some(machine);
                Page::List
            }
            MenuItem::Remove => {
                self.manage.work.push(Work::Remove(machine));
                Page::List
            }
        };
        self.manage.set_page(next);
        true
    }

    /// A painted control of the current page.
    pub(super) fn form_control(&mut self, control: FormControl) -> bool {
        match control {
            FormControl::Cancel => {
                self.manage.back();
                true
            }
            FormControl::Submit => self.submit_page(),
            FormControl::Field(field) => match &mut self.manage.page {
                Page::Add(form) if form.editable() => {
                    form.field = field;
                    form.suggestion = None;
                    true
                }
                _ => false,
            },
            FormControl::Suggestion(index) => match &mut self.manage.page {
                Page::Add(form) if form.editable() => form.take_suggestion(index),
                _ => false,
            },
            FormControl::Color(color) => match &mut self.manage.page {
                Page::Add(form) if form.editable() => {
                    form.color = color;
                    form.field = Field::Color;
                    form.suggestion = None;
                    true
                }
                _ => false,
            },
        }
    }

    fn submit_page(&mut self) -> bool {
        match &mut self.manage.page {
            Page::Add(form) => {
                let Some(work) = form.submit() else {
                    return true;
                };
                self.manage.work.push(work);
                true
            }
            Page::Rename { machine, name } => {
                let label = name.text().trim().to_owned();
                if !valid_label(&label) || names_this_computer(&label) {
                    self.manage.notice = Some(Notice::Failed(Problem::Name));
                    return true;
                }
                let machine = machine.clone();
                if self.manage.label(&machine) != label {
                    self.manage.work.push(Work::Rename { machine, label });
                }
                self.manage.set_page(Page::List);
                true
            }
            Page::Confirm { machine } => {
                let machine = machine.clone();
                if self.manage.stopping.is_some() {
                    return false;
                }
                if !self.machine_online(&machine) {
                    self.manage.notice = Some(Notice::NotOnline(self.manage.label(&machine)));
                    self.manage.set_page(Page::List);
                    return true;
                }
                // The page stays and shows the stop running until it ends.
                self.manage.stopping = Some(machine.clone());
                self.manage.work.push(Work::Stop(machine));
                true
            }
            Page::List | Page::Menu { .. } => false,
        }
    }

    /// A paste goes into the focused field of the form or the rename; otherwise it is returned.
    pub(crate) fn manage_paste(&mut self, text: String) -> Option<String> {
        if !self.open {
            return Some(text);
        }
        let single = text.replace(['\r', '\n'], "");
        match &mut self.manage.page {
            Page::List => Some(text),
            Page::Add(form) => {
                if form.editable()
                    && let Some(editor) = form.focused()
                {
                    let _ = editor.insert(&single);
                    form.edited();
                }
                None
            }
            Page::Rename { name, .. } => {
                let _ = name.insert(&single);
                None
            }
            Page::Menu { .. } | Page::Confirm { .. } => None,
        }
    }

    /// Routes a key to the open page, the Add row or the list's management keys; true when this
    /// module took it. `hidden` says the selection lies outside the painted viewport.
    pub(super) fn manage_key(&mut self, key: KeyEvent, hidden: bool) -> bool {
        self.manage.notice = None;
        match &self.manage.page {
            Page::List => self.list_key(key, hidden),
            Page::Menu { .. } => {
                self.menu_key(key);
                true
            }
            Page::Rename { .. } => {
                self.rename_key(key);
                true
            }
            Page::Confirm { .. } => {
                if ESC.matches(key) {
                    self.manage.back();
                } else if ENTER.matches(key) {
                    self.submit_page();
                }
                true
            }
            Page::Add(_) => {
                self.add_key(key);
                true
            }
        }
    }

    fn list_key(&mut self, key: KeyEvent, hidden: bool) -> bool {
        if self.mode != Mode::Machines {
            return false;
        }
        if self.manage.add_focused {
            if ENTER.matches(key) {
                self.open_add();
            } else if UP.matches(key) || UP_LETTER.matches(key) {
                self.manage.add_focused = false;
                self.selected = self.rows.len().saturating_sub(1);
            } else if HOME.matches(key) {
                self.manage.add_focused = false;
                self.selected = 0;
            } else if !(DOWN.matches(key) || DOWN_LETTER.matches(key) || END.matches(key)) {
                // The Add row has no Machine: the mode keys and Esc still reach the picker; row
                // keys have nothing to act on.
                if TAB.matches(key) || ESC.matches(key) {
                    self.manage.add_focused = false;
                    return false;
                }
            }
            return true;
        }
        if (DOWN.matches(key) || DOWN_LETTER.matches(key)) && self.selected + 1 >= self.rows.len() {
            self.manage.add_focused = true;
            return true;
        }
        if MENU_KEY.matches(key) {
            // On the Local row the key opens nothing, and nothing else takes it.
            self.open_menu(self.selected);
            return true;
        }
        if ENTER.matches(key)
            && !hidden
            && self.row_explanation(self.selected).and_then(|e| e.action) == Some(Action::Enable)
        {
            return self.row_action(self.selected);
        }
        false
    }

    fn menu_key(&mut self, key: KeyEvent) {
        let Page::Menu { machine, selected } = &self.manage.page else {
            return;
        };
        let items = self.menu_items(machine);
        let last = items.len().saturating_sub(1);
        let mut next = *selected;
        if ESC.matches(key) {
            self.manage.back();
            return;
        }
        if ENTER.matches(key) {
            if let Some((item, _)) = items.get(*selected) {
                self.choose_menu_item(*item);
            }
            return;
        }
        if UP.matches(key) || UP_LETTER.matches(key) {
            next = next.saturating_sub(1);
        } else if DOWN.matches(key) || DOWN_LETTER.matches(key) {
            next = (next + 1).min(last);
        } else if HOME.matches(key) {
            next = 0;
        } else if END.matches(key) {
            next = last;
        }
        if let Page::Menu { selected, .. } = &mut self.manage.page {
            *selected = next;
        }
    }

    fn rename_key(&mut self, key: KeyEvent) {
        if ESC.matches(key) {
            self.manage.back();
            return;
        }
        if ENTER.matches(key) {
            self.submit_page();
            return;
        }
        if let Page::Rename { name, .. } = &mut self.manage.page {
            edit(name, key);
        }
    }

    fn add_key(&mut self, key: KeyEvent) {
        if ESC.matches(key) {
            self.manage.back();
            return;
        }
        let Page::Add(form) = &mut self.manage.page else {
            return;
        };
        if !form.editable() {
            // While the checks run only Esc acts; a login question takes its own keys first.
            return;
        }
        if ENTER.matches(key) {
            if let Some(index) = form.suggestion {
                form.take_suggestion(index);
            } else {
                self.submit_page();
            }
            return;
        }
        if TAB.matches(key) || BACK_TAB.matches(key) {
            let forward = TAB.matches(key);
            form.field = match (form.field, forward) {
                (Field::Address, true) | (Field::Color, false) => Field::Name,
                (Field::Name, true) | (Field::Address, false) => Field::Color,
                (Field::Color, true) | (Field::Name, false) => Field::Address,
            };
            form.suggestion = None;
            return;
        }
        // The color is a single choice: the arrows move it, and it applies at once.
        if form.field == Field::Color && (LEFT.matches(key) || RIGHT.matches(key)) {
            let palette = MachineColor::PALETTE;
            let count = palette.len();
            let at = form.color as usize;
            form.color = palette[if RIGHT.matches(key) {
                (at + 1) % count
            } else {
                (at + count - 1) % count
            }];
            return;
        }
        if UP.matches(key) {
            match (form.field, form.suggestion) {
                (Field::Address, Some(0)) => form.suggestion = None,
                (Field::Address, Some(index)) => form.suggestion = Some(index - 1),
                (Field::Name, _) => {
                    form.field = Field::Address;
                    form.suggestion = None;
                }
                (Field::Color, _) => form.field = Field::Name,
                (Field::Address, None) => {}
            }
            return;
        }
        if DOWN.matches(key) {
            match (form.field, form.suggestion) {
                (Field::Address, None) if !form.suggestions.is_empty() => {
                    form.suggestion = Some(0);
                }
                (Field::Address, Some(index)) if index + 1 < form.suggestions.len() => {
                    form.suggestion = Some(index + 1);
                }
                (Field::Address, _) => {
                    form.field = Field::Name;
                    form.suggestion = None;
                }
                (Field::Name, _) => form.field = Field::Color,
                (Field::Color, _) => {}
            }
            return;
        }
        form.suggestion = None;
        if let Some(editor) = form.focused()
            && edit(editor, key)
        {
            form.edited();
        }
    }
}

impl AddForm {
    /// The fields take input unless the checks run or the record is being saved.
    pub(crate) const fn editable(&self) -> bool {
        matches!(self.check, None | Some(Check::Failed { .. }))
    }

    /// Enter would add: nothing runs, and a failure that offers no retry was answered by an edit.
    pub(crate) const fn can_submit(&self) -> bool {
        match &self.check {
            None => true,
            Some(Check::Failed { retry, .. }) => *retry,
            Some(Check::Starting | Check::Checking(_) | Check::Saving) => false,
        }
    }

    /// The text field with focus; none on the color.
    pub(crate) fn focused(&mut self) -> Option<&mut LineEditor> {
        match self.field {
            Field::Address => Some(&mut self.address),
            Field::Name => Some(&mut self.name),
            Field::Color => None,
        }
    }

    /// The person changed what to add: an old failure or problem no longer describes it.
    fn edited(&mut self) {
        self.check = None;
        self.problem = None;
    }

    fn take_suggestion(&mut self, index: usize) -> bool {
        let Some(host) = self.suggestions.get(index).cloned() else {
            return false;
        };
        self.address = LineEditor::new(host, LengthLimit::Scalars(FIELD_LIMIT));
        self.field = Field::Address;
        self.suggestion = None;
        self.edited();
        true
    }

    /// The add Enter asks for, once the address and name read as a Machine.
    fn submit(&mut self) -> Option<Work> {
        if !self.can_submit() {
            return None;
        }
        let target = self.address.text().trim().to_owned();
        if !valid_target(&target) {
            self.problem = Some(Problem::Address);
            self.field = Field::Address;
            return None;
        }
        let name = self.name.text().trim();
        let label = if name.is_empty() {
            default_label(&target)
        } else {
            name.to_owned()
        };
        if !valid_label(&label) || names_this_computer(&label) {
            self.problem = Some(Problem::Name);
            self.field = Field::Name;
            return None;
        }
        self.problem = None;
        self.suggestion = None;
        self.check = Some(Check::Starting);
        Some(Work::Add {
            label,
            target,
            color: self.color,
        })
    }
}

/// One caret motion or deletion of a single-line field.
type Motion = fn(&mut LineEditor) -> bool;

/// Applies one editing key to `editor`, as every single-line field takes it; false when the key
/// is not an editing key.
fn edit(editor: &mut LineEditor, key: KeyEvent) -> bool {
    if let Some(line_key) = LineKey::for_key(key) {
        let _ = editor.apply(line_key);
        return true;
    }
    let motions: [(Binding, Motion); 8] = [
        (BACKSPACE, LineEditor::backspace),
        (DELETE, LineEditor::delete),
        (LEFT, LineEditor::left),
        (RIGHT, LineEditor::right),
        (HOME, LineEditor::home),
        (LINE_START, LineEditor::home),
        (END, LineEditor::end),
        (LINE_END, LineEditor::end),
    ];
    if let Some((_, motion)) = motions.iter().find(|(binding, _)| binding.matches(key)) {
        motion(editor);
        return true;
    }
    let (code, mods) = key.normalized();
    match code {
        KeyCode::Char(character)
            if !character.is_control()
                && (mods == Modifiers::NONE || mods == Modifiers::NONE.with_shift()) =>
        {
            let character = if mods.shift {
                character.to_ascii_uppercase()
            } else {
                character
            };
            let _ = editor.insert(character.encode_utf8(&mut [0; 4]));
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
