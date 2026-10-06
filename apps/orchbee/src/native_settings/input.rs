//! Canonical bindings preserve legacy C0 and enhanced-host key equivalence.
use super::{Edit, Row};
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};

pub(crate) fn key_edit(event: KeyEvent) -> Option<Edit> {
    if event.kind == KeyKind::Release {
        return None;
    }
    [
        (Binding::new(KeyCode::Esc, Modifiers::NONE), Edit::Cancel),
        (
            Binding::new(KeyCode::Enter, Modifiers::NONE),
            Edit::Activate,
        ),
        (
            Binding::new(KeyCode::Tab, Modifiers::NONE.with_shift()),
            Edit::Previous,
        ),
        (Binding::new(KeyCode::Tab, Modifiers::NONE), Edit::Next),
        (Binding::new(KeyCode::Up, Modifiers::NONE), Edit::Previous),
        (
            Binding::new(KeyCode::Char('k'), Modifiers::NONE),
            Edit::Previous,
        ),
        (Binding::new(KeyCode::Down, Modifiers::NONE), Edit::Next),
        (
            Binding::new(KeyCode::Char('j'), Modifiers::NONE),
            Edit::Next,
        ),
        (
            Binding::new(KeyCode::Left, Modifiers::NONE),
            Edit::Adjust(false),
        ),
        (
            Binding::new(KeyCode::Right, Modifiers::NONE),
            Edit::Adjust(true),
        ),
        (
            Binding::new(KeyCode::Space, Modifiers::NONE),
            Edit::Adjust(true),
        ),
        (
            Binding::new(KeyCode::Home, Modifiers::NONE),
            Edit::Select(Row::Theme),
        ),
        (
            Binding::new(KeyCode::End, Modifiers::NONE),
            Edit::Select(Row::Save),
        ),
    ]
    .into_iter()
    .find_map(|(binding, edit)| binding.matches(event).then_some(edit))
}
