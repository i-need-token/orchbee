//! The words of a Workspace's move in the desktop window, as the TUI words them: the question it
//! asks before it stops a program, the notice it leaves, and its pending word. The process's mover
//! runs the move; `apps/orchbee/docs/machines.md#change-machine` owns the rules.
//
// module-layer: presentation

use crate::{
    localization::Locale,
    machine_change::{Name, Notice, Stops},
    terminal_text::sanitize_terminal_text,
    terminal_title::display_name,
};
use serde_json::{Value, json};

/// The question `question` asks before moving `workspace` to `to`, empty for this computer: what
/// the move would stop, an Agent by its name, else a title that names a program Orchbee knows.
#[must_use]
pub fn move_question(
    locale: Locale,
    question: u64,
    workspace: &str,
    to: &str,
    stops: &Stops,
) -> Value {
    let names: Vec<String> = match stops {
        Stops::Busy { names, .. } => names
            .iter()
            .filter_map(|name| match name {
                Name::Agent(agent) => Some(sanitize_terminal_text(agent).into_owned()),
                Name::Title(title) => display_name(&sanitize_terminal_text(title), locale),
            })
            .collect(),
        Stops::Unread => Vec::new(),
    };
    let busy = match stops {
        Stops::Busy { count, .. } => Some((*count, names.as_slice())),
        Stops::Unread => None,
    };
    json!({
        "question": question,
        "title": locale.machine_move_title(),
        "workspace": workspace,
        "sentence": locale.machine_move_sentence(&sanitize_terminal_text(to), busy),
        "files": locale.machine_move_files(),
        "move": locale.machine_move_confirm(),
        "cancel": locale.workspace_close_cancel_label(),
    })
}

/// What a move that did not just land where it was asked to says, in its complete form.
#[must_use]
pub fn move_notice(locale: Locale, notice: &Notice) -> String {
    locale
        .machine_move_notice(notice)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// The word that stands while a move runs.
#[must_use]
pub const fn move_pending(locale: Locale) -> &'static str {
    locale.machine_moving()
}
