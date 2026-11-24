//! Settings › Machines on the page: the facts the TUI last reported, the request the person made,
//! and the entries both give. The page never reaches a machine itself: the TUI takes the request
//! and reports what came of it in the next facts.
use super::{PageState, PageView, Pane};
use crate::machine_color::MachineColor;
use crate::machine_settings::{
    FILTER_THRESHOLD, Facts, Machine, Outcome, Request, State as MachineState,
};
use crate::native_settings::{
    Category, Choice, ChoicePicker, Editor, Entry, Group, Page, Row, State,
};
use crate::terminal_text::line_editor::{LengthLimit, LineEditor};

/// What the TUI reported and what the person asked; kept while Settings is closed, as About's
/// update state is.
#[derive(Debug, Default)]
pub(crate) struct Machines {
    facts: Facts,
    request: Option<Request>,
    /// How the last request ended, until the next input clears it.
    outcome: Option<Outcome>,
}

/// What the open page lists of Settings › Machines; it follows the facts.
#[derive(Clone, Debug, Default)]
pub(in crate::native_settings) struct Listing {
    /// The machine whose page is open.
    machine: Option<String>,
    /// The first Enter on Stop its terminals asked; the second stops them.
    pub(in crate::native_settings) confirm_stop: bool,
    /// The list's filter, which applies past the threshold.
    query: String,
    /// Every saved machine's ID, in the facts' order.
    ids: Vec<String>,
    /// The places in that order of the machines the list shows.
    rows: Vec<u16>,
    /// The open page's machine is online, so its page shows Free now.
    online: bool,
    /// The system password store can be used, so a machine's page shows Saved password.
    passwords: bool,
}

impl Listing {
    /// The list row of the machine `id`, while the list shows it.
    fn row_of(&self, id: &str) -> Option<Row> {
        let index = self.ids.iter().position(|candidate| candidate == id)?;
        let index = u16::try_from(index).ok()?;
        self.rows.contains(&index).then_some(Row::Machine(index))
    }

    /// Leaving a machine's page: the list focuses that machine's row wherever it now is.
    pub(in crate::native_settings) fn leave_page(&mut self, row: &mut Row) {
        self.confirm_stop = false;
        if let Some(id) = self.machine.take()
            && let Some(listed) = self.row_of(&id)
        {
            *row = listed;
        }
    }
}

/// The entries of the Machines list or of a machine's page; `None` for every other page.
pub(in crate::native_settings) fn entries(page: &PageState) -> Option<Vec<Entry>> {
    let listing = &page.machines;
    match page.page {
        Page::Machine => {
            let mut entries = vec![
                Entry::Heading(Group::Machine),
                Entry::Setting(Row::MachineName),
                Entry::Setting(Row::MachineColor),
            ];
            // The open color list sits under its row.
            if page
                .picker
                .is_some_and(|picker| picker.row() == Row::MachineColor)
            {
                entries.extend(
                    Choice::options(Row::MachineColor)
                        .iter()
                        .copied()
                        .map(Entry::Option),
                );
            }
            entries.extend([
                Entry::Setting(Row::MachineAddress),
                Entry::Setting(Row::MachineStatus),
            ]);
            if listing.online {
                entries.push(Entry::Setting(Row::MachineFree));
            }
            if listing.passwords {
                entries.push(Entry::Setting(Row::MachinePassword));
            }
            entries.extend([
                Entry::End,
                Entry::Heading(Group::MachineEnd),
                Entry::Setting(Row::MachineStop),
                Entry::Setting(Row::MachineRemove),
                Entry::End,
            ]);
            Some(entries)
        }
        Page::General if page.category == Category::Machines && page.filter.text().is_empty() => {
            let mut entries = vec![Entry::Setting(Row::AddMachine)];
            if listing.ids.len() > FILTER_THRESHOLD {
                entries.push(Entry::Setting(Row::FindMachine));
            }
            entries.push(Entry::Heading(Group::MachineList));
            entries.push(Entry::Setting(Row::ThisComputer));
            entries.extend(
                listing
                    .rows
                    .iter()
                    .map(|index| Entry::Setting(Row::Machine(*index))),
            );
            if listing.rows.is_empty() && !listing.ids.is_empty() {
                entries.push(Entry::Note(Group::MachineList));
            }
            entries.push(Entry::End);
            Some(entries)
        }
        _ => None,
    }
}

/// Whether a row of Settings › Machines acts now; `None` for every other row. Status acts while
/// it offers an action, Saved password while the store can be used, Stop while the machine is
/// online, and nothing acts on a machine whose terminals are being stopped.
pub(in crate::native_settings) fn enabled(
    page: &PageState,
    machines: &Machines,
    row: Row,
) -> Option<bool> {
    let shown = machines.machine(page.machines.machine.as_deref());
    let idle = shown.is_some_and(|machine| !machine.stopping);
    Some(match row {
        Row::AddMachine
        | Row::FindMachine
        | Row::ThisComputer
        | Row::Machine(_)
        | Row::MachineAddress
        | Row::MachineFree => true,
        Row::MachineName | Row::MachineColor | Row::MachineRemove => idle,
        Row::MachineStatus => shown.is_some_and(|machine| machine.action().is_some()),
        Row::MachinePassword => idle && machines.facts.passwords,
        Row::MachineStop => {
            idle && shown.is_some_and(|machine| machine.state == MachineState::Online)
        }
        _ => return None,
    })
}

impl Machines {
    pub(in crate::native_settings) const fn facts(&self) -> &Facts {
        &self.facts
    }

    /// The machine `id` names, while it is saved.
    fn machine(&self, id: Option<&str>) -> Option<&Machine> {
        let id = id?;
        self.facts.machines.iter().find(|machine| machine.id == id)
    }
}

impl PageState {
    /// Whether `Esc` clears the Machines list's filter: the list shows and the filter holds a
    /// query.
    pub(in crate::native_settings) fn machine_query_clears(&self) -> bool {
        self.page == Page::General
            && self.category == Category::Machines
            && self.pane == Pane::Rows
            && !self.machines.query.is_empty()
    }

    /// Clears the Machines list's filter.
    pub(in crate::native_settings) fn clear_machine_query(&mut self, facts: &Facts) {
        self.machines.query.clear();
        self.follow_machines(facts);
    }

    /// Follows `facts`: the list keeps the machines its filter admits and the focused machine
    /// wherever it now is, and a page whose machine went returns to the list.
    pub(in crate::native_settings) fn follow_machines(&mut self, facts: &Facts) {
        let focused = match self.row {
            Row::Machine(index) => self.machines.ids.get(usize::from(index)).cloned(),
            _ => None,
        };
        let query = match (&self.text, self.row) {
            (Some(text), Row::FindMachine) => text.text().to_owned(),
            _ => self.machines.query.clone(),
        };
        let listing = &mut self.machines;
        listing.ids = facts
            .machines
            .iter()
            .map(|machine| machine.id.clone())
            .collect();
        let filtered = facts.machines.len() > FILTER_THRESHOLD;
        listing.rows = facts
            .machines
            .iter()
            .enumerate()
            .filter(|(_, machine)| !filtered || machine.matches(&query))
            .filter_map(|(index, _)| u16::try_from(index).ok())
            .collect();
        listing.passwords = facts.passwords;
        let shown = listing
            .machine
            .as_ref()
            .and_then(|id| facts.machines.iter().find(|machine| &machine.id == id));
        listing.online = shown.is_some_and(|machine| machine.state == MachineState::Online);
        // A stop that can no longer happen asks no more.
        if !shown.is_some_and(|machine| machine.state == MachineState::Online && !machine.stopping)
        {
            listing.confirm_stop = false;
        }
        let gone = shown.is_none();
        if let Some(id) = focused
            && let Some(row) = listing.row_of(&id)
        {
            self.row = row;
        }
        if self.page == Page::Machine && gone {
            self.text = None;
            self.return_to_parent();
        }
        if self.text.is_some() && !self.rows().contains(&self.row) {
            self.text = None;
        }
        self.settle_focus();
    }

    /// Focuses the machine `id` on the Machines list, while the list shows it.
    fn focus_machine(&mut self, id: &str) {
        if self.page == Page::General
            && self.category == Category::Machines
            && let Some(row) = self.machines.row_of(id)
        {
            self.pane = Pane::Rows;
            self.row = row;
            self.follow_selection = true;
        }
    }
}

impl Editor {
    /// Follows the facts the TUI reports; answers whether a visible page changed.
    pub(crate) fn observe_machines(&mut self, mut facts: Facts) -> bool {
        let outcome = facts.outcome.take();
        let added = facts.added.take();
        if self.machines.facts == facts && outcome.is_none() && added.is_none() {
            return false;
        }
        if outcome.is_some() {
            self.machines.outcome = outcome;
        }
        self.machines.facts = facts;
        if let State::Open(page) = &mut self.state {
            page.follow_machines(&self.machines.facts);
            if let Some(added) = added {
                page.focus_machine(&added);
            }
        }
        self.redraw();
        self.visible()
    }

    /// The person's last request on Settings › Machines, once.
    pub(crate) fn take_machine_request(&mut self) -> Option<Request> {
        self.machines.request.take()
    }

    /// The next input clears how the last request ended, as it clears the Settings notice.
    pub(crate) fn clear_machine_outcome(&mut self) {
        self.machines.outcome = None;
    }

    /// Whether Settings › Machines shows: its list without a search, or a machine's page.
    pub(crate) fn machines_visible(&self) -> bool {
        matches!(
            &self.state,
            State::Open(page) if page.page == Page::Machine
                || (page.page == Page::General
                    && page.category == Category::Machines
                    && page.filter.text().is_empty())
        )
    }

    /// `Enter` on a row of Settings › Machines; `true` when the row is one of them.
    pub(in crate::native_settings) fn activate_machine_row(&mut self, row: Row) -> bool {
        let State::Open(page) = &mut self.state else {
            return false;
        };
        let machines = &mut self.machines;
        let shown = machines.machine(page.machines.machine.as_deref()).cloned();
        match row {
            Row::AddMachine => machines.request = Some(Request::Add),
            Row::FindMachine => {
                if let Some(limit) = row.text_limit() {
                    page.text = Some(LineEditor::new(
                        page.machines.query.clone(),
                        LengthLimit::Bytes(limit),
                    ));
                }
            }
            Row::ThisComputer | Row::MachineAddress | Row::MachineFree => {}
            Row::Machine(index) => {
                let Some(machine) = machines.facts.machines.get(usize::from(index)) else {
                    return true;
                };
                page.machines.machine = Some(machine.id.clone());
                page.machines.confirm_stop = false;
                page.enter_page(Page::Machine, Row::MachineName);
                page.follow_machines(&machines.facts);
            }
            Row::MachineName => {
                if let Some(machine) = shown
                    && let Some(limit) = row.text_limit()
                {
                    page.text = Some(
                        LineEditor::new(machine.label, LengthLimit::Bytes(limit)).select_all(),
                    );
                }
            }
            // The list opens on the machine's color; choosing another sets it at once.
            Row::MachineColor => {
                if let Some(machine) = shown {
                    page.picker = ChoicePicker::open(row, Choice::MachineColor(machine.color));
                    page.follow_selection = true;
                }
            }
            Row::MachineStatus => {
                if let Some(machine) = shown
                    && let Some(action) = machine.action()
                {
                    machines.request = Some(Request::Act {
                        machine: machine.id,
                        action,
                    });
                }
            }
            Row::MachinePassword => {
                if let Some(machine) = shown {
                    machines.request = Some(Request::ForgetPassword(machine.id));
                }
            }
            // The first Enter states what stops; the second stops it.
            Row::MachineStop => {
                if let Some(machine) = shown {
                    if page.machines.confirm_stop {
                        page.machines.confirm_stop = false;
                        machines.request = Some(Request::Stop(machine.id));
                    } else {
                        page.machines.confirm_stop = true;
                    }
                }
            }
            Row::MachineRemove => {
                if let Some(machine) = shown {
                    machines.request = Some(Request::Remove(machine.id));
                }
            }
            _ => return false,
        }
        self.redraw();
        true
    }

    /// Choosing `color` in the open machine's color list asks for it when it differs.
    pub(in crate::native_settings) fn choose_machine_color(&mut self, color: MachineColor) {
        let State::Open(page) = &self.state else {
            return;
        };
        if let Some(machine) = self.machines.machine(page.machines.machine.as_deref())
            && machine.color != color
        {
            self.machines.request = Some(Request::SetColor {
                machine: machine.id.clone(),
                color,
            });
        }
        self.redraw();
    }

    /// `Enter` in the editor of a Machines row; `true` when it was one.
    pub(in crate::native_settings) fn accept_machine_text(&mut self) -> bool {
        let State::Open(page) = &mut self.state else {
            return false;
        };
        match page.row {
            Row::FindMachine => {
                page.machines.query = page
                    .text
                    .take()
                    .map(|text| text.text().to_owned())
                    .unwrap_or_default();
                page.follow_machines(&self.machines.facts);
            }
            Row::MachineName => {
                let label = page
                    .text
                    .take()
                    .map(|text| text.text().trim().to_owned())
                    .unwrap_or_default();
                if let Some(machine) = self.machines.machine(page.machines.machine.as_deref())
                    && machine.label != label
                {
                    self.machines.request = Some(Request::Rename {
                        machine: machine.id.clone(),
                        label,
                    });
                }
            }
            _ => return false,
        }
        self.redraw();
        true
    }

    /// The list follows the filter's draft as it is typed, and its committed filter again when
    /// the draft is dropped.
    pub(in crate::native_settings) fn follow_machine_query(&mut self) {
        if let State::Open(page) = &mut self.state
            && page.row == Row::FindMachine
        {
            page.follow_machines(&self.machines.facts);
        }
    }
}

impl<'a> PageView<'a> {
    pub(crate) fn machine_facts(self) -> &'a Facts {
        &self.machines.facts
    }

    /// The machine the open machine page shows.
    pub(crate) fn shown_machine(self) -> Option<&'a Machine> {
        self.machines
            .machine(self.state.machines.machine.as_deref())
    }

    /// The saved machine at `index` in the facts' order.
    pub(crate) fn listed_machine(self, index: u16) -> Option<&'a Machine> {
        self.machines.facts.machines.get(usize::from(index))
    }

    pub(crate) const fn confirm_stop(self) -> bool {
        self.state.machines.confirm_stop
    }

    /// The list's committed filter.
    pub(crate) fn machine_query(self) -> &'a str {
        &self.state.machines.query
    }

    /// How the person's last request on Settings › Machines ended, until the next input.
    pub(crate) const fn machine_outcome(self) -> Option<&'a Outcome> {
        self.machines.outcome.as_ref()
    }
}
