//! Text editing owns printable keys; navigation and editing share canonical C0 bindings.
use super::Edit;
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};

pub(crate) fn key_edit(event: KeyEvent, text_entry: bool) -> Option<Edit> {
    if event.kind == KeyKind::Release {
        return None;
    }
    let bindings = [
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
        (Binding::new(KeyCode::Down, Modifiers::NONE), Edit::Next),
        (
            Binding::new(KeyCode::Left, Modifiers::NONE),
            Edit::Adjust(false),
        ),
        (
            Binding::new(KeyCode::Right, Modifiers::NONE),
            Edit::Adjust(true),
        ),
        (Binding::new(KeyCode::Home, Modifiers::NONE), Edit::Home),
        (Binding::new(KeyCode::End, Modifiers::NONE), Edit::End),
        (
            Binding::new(KeyCode::Char('a'), Modifiers::NONE.with_ctrl()),
            Edit::Home,
        ),
        (
            Binding::new(KeyCode::Char('e'), Modifiers::NONE.with_ctrl()),
            Edit::End,
        ),
        (
            Binding::new(KeyCode::Char('u'), Modifiers::NONE.with_ctrl()),
            Edit::Clear,
        ),
        (
            Binding::new(KeyCode::Backspace, Modifiers::NONE),
            Edit::Backspace,
        ),
        (Binding::new(KeyCode::Delete, Modifiers::NONE), Edit::Delete),
    ];
    if let Some(edit) = bindings
        .into_iter()
        .find_map(|(binding, edit)| binding.matches(event).then_some(edit))
    {
        return Some(edit);
    }
    if event.mods == Modifiers::NONE || event.mods == Modifiers::NONE.with_shift() {
        match event.code {
            KeyCode::Char(character) if text_entry => Some(Edit::Insert(character)),
            KeyCode::Space | KeyCode::Char(' ') => Some(if text_entry {
                Edit::Insert(' ')
            } else {
                Edit::Activate
            }),
            _ => None,
        }
    } else {
        None
    }
}
