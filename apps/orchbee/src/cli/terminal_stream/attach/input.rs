//! Attach-local prefix ownership over the existing bounded terminal decoder.
use super::super::input::{Direction, Scroll, ScrollSource};
use crate::{
    client_view::{KeyCode, KeyKind, Modifiers},
    terminal_input::{DecodedItem, DecodedKind, LegacyDecoder, MouseButton, MouseEvent},
};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub(super) enum Action {
    Input(Vec<u8>),
    Paste(String),
    Mouse(MouseEvent),
    Scroll(Scroll),
    Focus(bool),
    Notice,
    Detach,
}
pub(super) struct Input {
    decoder: LegacyDecoder,
    prefix: Option<Vec<u8>>,
    mouse: bool,
    paste_rejected: bool,
}
impl Default for Input {
    fn default() -> Self {
        Self::new(true)
    }
}
impl Input {
    pub(super) fn new(mouse: bool) -> Self {
        Self {
            decoder: LegacyDecoder::new(Duration::from_millis(50)),
            prefix: None,
            mouse,
            paste_rejected: false,
        }
    }
    pub(super) fn read(
        &mut self,
        bytes: &[u8],
        now: Instant,
        rows: u16,
        application_cursor: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        for &byte in bytes {
            let paste = self.decoder.paste_pending();
            for item in self.decoder.push(byte, now, false) {
                self.decoded(item, paste, rows, application_cursor, &mut actions);
                if matches!(actions.last(), Some(Action::Detach)) {
                    return actions;
                }
            }
            if !self.decoder.paste_pending() {
                self.paste_rejected = false;
            }
        }
        actions
    }
    pub(super) fn tick(
        &mut self,
        now: Instant,
        rows: u16,
        application_cursor: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let paste = self.decoder.paste_pending();
        for item in self.decoder.tick(now) {
            self.decoded(item, paste, rows, application_cursor, &mut actions);
        }
        actions
    }
    fn decoded(
        &mut self,
        item: DecodedItem,
        paste: bool,
        rows: u16,
        application_cursor: bool,
        actions: &mut Vec<Action>,
    ) {
        if paste && matches!(item.kind, DecodedKind::Nondecision) {
            if !self.paste_rejected {
                if let Some(held) = self.prefix.take() {
                    push_input(actions, held);
                }
                actions.push(Action::Notice);
                self.paste_rejected = true;
            }
            return;
        }
        self.item(item, rows, application_cursor, actions);
    }
    fn item(
        &mut self,
        item: DecodedItem,
        rows: u16,
        application_cursor: bool,
        actions: &mut Vec<Action>,
    ) {
        if !self.mouse && matches!(item.kind, DecodedKind::Mouse(_)) {
            return;
        }
        if matches!(
            item.kind,
            DecodedKind::Ignored | DecodedKind::Focus(_) | DecodedKind::HostReply(_)
        ) {
            if let DecodedKind::Focus(focused) = item.kind {
                actions.push(Action::Focus(focused));
            }
            return;
        }
        if let DecodedKind::Key(key) = item.kind {
            let prefix = key.normalized() == (KeyCode::Char('b'), Modifiers::CTRL);
            if key.kind == KeyKind::Release || (prefix && key.kind == KeyKind::Repeat) {
                return;
            }
            if let Some(held) = self.prefix.take() {
                if key.normalized() == (KeyCode::Char('q'), Modifiers::NONE)
                    && key.kind == KeyKind::Press
                {
                    actions.push(Action::Detach);
                    return;
                }
                push_input(actions, held);
                if prefix {
                    return;
                }
            } else if prefix {
                self.prefix = Some(item.raw.into_vec());
                return;
            }
            if key.mods == Modifiers::NONE {
                if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
                    actions.push(Action::Scroll(Scroll {
                        direction: if key.code == KeyCode::PageUp {
                            Direction::Up
                        } else {
                            Direction::Down
                        },
                        lines: rows.saturating_sub(1).max(1),
                        source: ScrollSource::PageKey,
                        column: None,
                        row: None,
                        modifiers: 0,
                    }));
                    return;
                }
                let final_byte = match key.code {
                    KeyCode::Up => Some(b'A'),
                    KeyCode::Down => Some(b'B'),
                    KeyCode::Right => Some(b'C'),
                    KeyCode::Left => Some(b'D'),
                    KeyCode::Home => Some(b'H'),
                    KeyCode::End => Some(b'F'),
                    _ => None,
                };
                if application_cursor && let Some(final_byte) = final_byte {
                    push_input(actions, vec![27, b'O', final_byte]);
                    return;
                }
                if let KeyCode::F(index @ 1..=4) = key.code {
                    push_input(actions, vec![27, b'O', b'P' + index - 1]);
                    return;
                }
            }
        } else if let Some(held) = self.prefix.take() {
            push_input(actions, held);
        }
        match item.kind {
            DecodedKind::Paste { text } => actions.push(text.map_or(Action::Notice, Action::Paste)),
            DecodedKind::Mouse(mouse)
                if matches!(mouse.button, MouseButton::WheelUp | MouseButton::WheelDown) =>
            {
                let mods = mouse.mods;
                actions.push(Action::Scroll(Scroll {
                    direction: if mouse.button == MouseButton::WheelUp {
                        Direction::Up
                    } else {
                        Direction::Down
                    },
                    lines: 3,
                    source: ScrollSource::Wheel,
                    column: Some(mouse.column),
                    row: Some(mouse.row),
                    modifiers: u8::from(mods.shift)
                        + 2 * u8::from(mods.ctrl)
                        + 4 * u8::from(mods.alt)
                        + 8 * u8::from(mods.super_),
                }));
            }
            DecodedKind::Mouse(mouse) => actions.push(Action::Mouse(mouse)),
            DecodedKind::Key(_) | DecodedKind::Nondecision => {
                push_input(actions, item.raw.into_vec())
            }
            DecodedKind::Focus(_) | DecodedKind::Ignored | DecodedKind::HostReply(_) => {}
        }
    }
}
fn push_input(actions: &mut Vec<Action>, bytes: Vec<u8>) {
    if let Some(Action::Input(previous)) = actions.last_mut() {
        previous.extend(bytes);
    } else {
        actions.push(Action::Input(bytes));
    }
}

#[cfg(test)]
mod tests;
