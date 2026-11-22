//! A Change machine list's own presentation: each row's free capacity or state word, the column
//! note above them, and what its query finds.

use crate::action_menu::{MenuAction, MenuView};
use crate::localization::{Locale, MachineText};
use crate::machine_capacity::{UNKNOWN, parts};
use crate::machine_change::Row;
use crate::machine_settings::State;
use crate::tui_theme::Role;
use std::borrow::Cow;

/// What a row shows at its end: what the machine has free, `88% · 24 GB · 310 GB` with a low
/// number in amber, while it can take a Workspace; otherwise the word for its state.
pub(super) fn detail(row: &Row, locale: Locale) -> Vec<(Cow<'static, str>, Role)> {
    if let Some(machine) = row
        .machine
        .as_ref()
        .filter(|machine| machine.state != State::Online || machine.stopping)
    {
        return vec![(
            locale.machine_state_word(machine).into(),
            crate::tui_render::settings::machines::state_role(machine),
        )];
    }
    let Some(capacity) = row.capacity() else {
        return vec![(UNKNOWN.into(), Role::TextSecondary)];
    };
    let mut spans = Vec::new();
    for (index, part) in parts(capacity).into_iter().enumerate() {
        if index > 0 {
            spans.push((" · ".into(), Role::TextSecondary));
        }
        let role = if part.low {
            Role::StatusWarning
        } else {
            Role::TextSecondary
        };
        spans.push((part.text.into(), role));
    }
    spans
}

/// The words above the numbers, at the end of the list's heading.
pub(super) fn column_note(locale: Locale) -> &'static str {
    locale.machine_text(MachineText::ColumnNote)
}

/// The rows a query finds: a machine by any part of its name or address, without regard to case,
/// and Manage machines… by its title.
pub(super) fn results(
    menu: &MenuView<'_>,
    rows: &[Row],
    query: &str,
    locale: Locale,
) -> Vec<usize> {
    let here = locale.machine_change_here();
    let wanted = query.trim().to_lowercase();
    menu.actions
        .iter()
        .enumerate()
        .filter(|(_, action)| match action {
            MenuAction::MoveToMachine(index) => rows
                .get(usize::from(*index))
                .is_some_and(|row| row.matches(query, here)),
            action => action
                .object_title(locale, None)
                .to_lowercase()
                .contains(&wanted),
        })
        .map(|(index, _)| index)
        .collect()
}
