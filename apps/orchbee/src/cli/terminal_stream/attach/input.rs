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
        if matches!(item.kind, DecodedKind::Ignored | DecodedKind::Focus(_)) {
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
            DecodedKind::Focus(_) | DecodedKind::Ignored => {}
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
mod tests {
    use super::*;
    #[test]
    fn timed_out_and_oversized_pastes_never_submit_fragments_or_detach() {
        let now = Instant::now();
        let mut input = Input::default();
        assert!(
            input
                .read(b"\x1b[200~partial\x02q", now, 24, false)
                .is_empty()
        );
        assert_eq!(
            input.tick(now + Duration::from_millis(60), 24, false),
            [Action::Notice]
        );
        assert!(input.read(&[b'x'; 256], now, 24, false).is_empty());
        assert_eq!(
            input.read(b"\x02q\x1b[201~ok", now, 24, false),
            [Action::Input(b"ok".to_vec())]
        );
        for size in [256 * 1024 + 1, 4 * 1024 * 1024 + 100] {
            let mut paste = b"\x1b[200~".to_vec();
            paste.extend(std::iter::repeat_n(b'x', size));
            paste.extend_from_slice(b"\x02q\x1b[201~after");
            assert_eq!(
                input.read(&paste, now, 24, false),
                [Action::Notice, Action::Input(b"after".to_vec())]
            );
        }
    }

    #[test]
    fn disabled_mouse_and_focus_do_not_consume_detach_prefix() {
        let now = Instant::now();
        let mut input = Input::new(false);
        assert_eq!(
            input.read(b"\x02\x1b[<0;1;1M\x1b[<64;1;1M\x1b[Iq", now, 24, false),
            [Action::Focus(true), Action::Detach]
        );
    }

    #[test]
    fn fragmented_unicode_and_invalid_paste_keep_input_boundaries() {
        let now = Instant::now();
        let mut input = Input::default();
        assert!(input.read(&[0xe4, 0xb8], now, 24, false).is_empty());
        assert_eq!(
            input.read(&[0xad], now, 24, false),
            [Action::Input("中".as_bytes().to_vec())]
        );
        assert_eq!(
            input.read(b"\x1b[200~\xff\x02q\x1b[201~x", now, 24, false),
            [Action::Notice, Action::Input(vec![b'x'])]
        );
    }

    #[test]
    fn prefix_is_lossless_across_fragments_and_paste_never_detaches() {
        let now = Instant::now();
        let mut input = Input::default();
        assert_eq!(
            input.read(b"before\x02", now, 24, false),
            [Action::Input(b"before".to_vec())]
        );
        assert_eq!(input.read(b"qdiscard", now, 24, false), [Action::Detach]);
        let mut input = Input::default();
        assert_eq!(
            input.read(b"\x02\x02x\x02y", now, 24, false),
            [Action::Input(b"\x02x\x02y".to_vec())]
        );
        assert!(input.read(b"\x02\x1b[200~\x02q", now, 24, false).is_empty());
        assert_eq!(
            input.read(b"\x1b[201~", now, 24, false),
            [Action::Input(vec![2]), Action::Paste("\x02q".to_owned())]
        );
    }
    #[test]
    fn shell_pages_mouse_and_application_keys_have_distinct_ownership() {
        let now = Instant::now();
        let mut input = Input::default();
        assert_eq!(
            input.read(b"\x1b[A\x1b[11~", now, 24, true),
            [Action::Input(b"\x1bOA\x1bOP".to_vec())]
        );
        assert!(matches!(
            &input.read(b"\x1b[5~", now, 24, false)[..],
            [Action::Scroll(Scroll {
                lines: 23,
                source: ScrollSource::PageKey,
                ..
            })]
        ));
        assert!(matches!(
            &input.read(b"\x1b[<68;2;3M", now, 24, false)[..],
            [Action::Scroll(Scroll {
                modifiers: 1,
                column: Some(1),
                row: Some(2),
                lines: 3,
                ..
            })]
        ));
        assert!(input.read(b"\x1b", now, 24, false).is_empty());
        assert_eq!(
            input.tick(now + Duration::from_millis(60), 24, false),
            [Action::Input(vec![27])]
        );
    }
}
