//! Preserve VT characters before the semantic decoder. Crossterm's Windows
//! event parser discards control characters with virtual-key code zero, which
//! includes the ESC characters in ConPTY bracketed-paste delimiters.
use super::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind, encode};
use crossterm::event::{KeyEvent, MouseEvent};
use crossterm_winapi::{
    Console, ControlKeyState, EventFlags, Handle, HandleType, InputRecord, ScreenBuffer,
};
use std::{
    fmt, io,
    time::{Duration, Instant},
};

const BURST: usize = 4096;

pub(crate) struct Reader {
    console: Console,
    codec: Codec,
    pending: Option<Pending>,
}
impl fmt::Debug for Reader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsoleReader").finish_non_exhaustive()
    }
}
impl Reader {
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            console: Console::from(Handle::new(HandleType::InputHandle)?),
            codec: Codec::default(),
            pending: None,
        })
    }
    pub(crate) fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let wait = Instant::now() + timeout;
        let mut work = None;
        let mut output = Vec::new();
        for _ in 0..BURST {
            if let Some(pending) = self.pending.as_mut() {
                pending.drain(&mut output);
                if pending.remaining == 0 {
                    self.pending = None;
                }
                if output.len() == BURST {
                    break;
                }
            }
            if work.is_some_and(|deadline| Instant::now() >= deadline) {
                break;
            }
            if self.console.number_of_console_input_events()? == 0 {
                if !output.is_empty() || Instant::now() >= wait {
                    break;
                }
                std::thread::sleep(
                    Duration::from_millis(1).min(wait.saturating_duration_since(Instant::now())),
                );
                continue;
            }
            work.get_or_insert_with(|| Instant::now() + Duration::from_millis(4));
            // This inherited terminal's single reader owns every record. The
            // readiness check cannot race another console input consumer.
            let record = self.console.read_single_input_event()?;
            self.pending = self.codec.record(record)?;
        }
        Ok((!output.is_empty()).then_some(output))
    }
}

struct Pending {
    bytes: Vec<u8>,
    remaining: u16,
    offset: usize,
}
impl Pending {
    fn new(bytes: Vec<u8>, remaining: u16) -> Option<Self> {
        (!bytes.is_empty() && remaining != 0).then_some(Self {
            bytes,
            remaining,
            offset: 0,
        })
    }
    fn drain(&mut self, output: &mut Vec<u8>) {
        while self.remaining != 0 && output.len() < BURST {
            let count = (BURST - output.len()).min(self.bytes.len() - self.offset);
            output.extend_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            if self.offset == self.bytes.len() {
                self.offset = 0;
                self.remaining -= 1;
            }
        }
    }
}

#[derive(Default)]
struct Codec {
    high: Option<u16>,
    prefix_down: bool,
    buttons: u8,
}
impl Codec {
    fn record(&mut self, record: InputRecord) -> io::Result<Option<Pending>> {
        let mut bytes = Vec::new();
        match record {
            InputRecord::KeyEvent(key) => {
                return Ok(self.key(
                    key.virtual_key_code,
                    key.u_char,
                    key.key_down,
                    key.repeat_count,
                    modifiers(key.control_key_state),
                    key.control_key_state.has_state(1) && key.control_key_state.has_state(12),
                ));
            }
            InputRecord::FocusEvent(focus) => encode(
                if focus.set_focus {
                    Event::FocusGained
                } else {
                    Event::FocusLost
                },
                &mut bytes,
            ),
            InputRecord::MouseEvent(mouse) => {
                let window = ScreenBuffer::current()?.info()?.terminal_window();
                let x = i32::from(mouse.mouse_position.x) - i32::from(window.left);
                let y = i32::from(mouse.mouse_position.y) - i32::from(window.top);
                let (Ok(column), Ok(row)) = (u16::try_from(x), u16::try_from(y)) else {
                    return Ok(None);
                };
                let current = u8::from(mouse.button_state.left_button())
                    | (u8::from(mouse.button_state.middle_button()) << 1)
                    | (u8::from(mouse.button_state.right_button()) << 2);
                let mut emit = |kind| {
                    encode(
                        Event::Mouse(MouseEvent {
                            kind,
                            column,
                            row,
                            modifiers: modifiers(mouse.control_key_state),
                        }),
                        &mut bytes,
                    )
                };
                match mouse.event_flags {
                    EventFlags::PressOrRelease | EventFlags::DoubleClick => {
                        for (bit, button) in [
                            (1, MouseButton::Left),
                            (2, MouseButton::Middle),
                            (4, MouseButton::Right),
                        ] {
                            if (self.buttons ^ current) & bit != 0 {
                                emit(if current & bit != 0 {
                                    MouseEventKind::Down(button)
                                } else {
                                    MouseEventKind::Up(button)
                                });
                            }
                        }
                    }
                    EventFlags::MouseMoved if current != 0 => {
                        emit(MouseEventKind::Drag(if current & 1 != 0 {
                            MouseButton::Left
                        } else if current & 2 != 0 {
                            MouseButton::Middle
                        } else {
                            MouseButton::Right
                        }))
                    }
                    EventFlags::MouseWheeled => emit(if mouse.button_state.scroll_down() {
                        MouseEventKind::ScrollDown
                    } else {
                        MouseEventKind::ScrollUp
                    }),
                    EventFlags::MouseHwheeled => emit(if mouse.button_state.scroll_left() {
                        MouseEventKind::ScrollLeft
                    } else {
                        MouseEventKind::ScrollRight
                    }),
                    _ => {}
                }
                self.buttons = current;
            }
            _ => {}
        }
        Ok(Pending::new(bytes, 1))
    }
    fn key(
        &mut self,
        vk: u16,
        unit: u16,
        down: bool,
        repeat: u16,
        mut mods: KeyModifiers,
        alt_graph: bool,
    ) -> Option<Pending> {
        if vk == 0x42 && !down {
            self.prefix_down = false;
        }
        let alt_code = vk == 0x12 && !down && unit != 0;
        if !down && !alt_code {
            return None;
        }
        if matches!(vk, 0x10..=0x12) && !alt_code {
            return None;
        }
        let is_prefix = vk == 0x42 && unit == 2;
        if is_prefix && std::mem::replace(&mut self.prefix_down, true) {
            return None;
        }
        let mut bytes = Vec::new();
        // VK=0 denotes already translated VT characters, including literal C0.
        if vk == 0 || alt_graph || alt_code {
            mods = KeyModifiers::NONE;
        }
        if (0xd800..=0xdbff).contains(&unit) {
            if self.high.replace(unit).is_some() {
                bytes.extend_from_slice("�".as_bytes());
            }
            return Pending::new(bytes, 1);
        }
        if (0xdc00..=0xdfff).contains(&unit) {
            let ch = self
                .high
                .take()
                .and_then(|high| char::decode_utf16([high, unit]).next()?.ok())
                .unwrap_or('�');
            encode(
                Event::Key(KeyEvent::new(KeyCode::Char(ch), mods)),
                &mut bytes,
            );
            return Pending::new(bytes, repeat.max(1));
        }
        if self.high.take().is_some() {
            bytes.extend_from_slice("�".as_bytes());
        }
        let special = match vk {
            8 => Some(KeyCode::Backspace),
            9 if mods.contains(KeyModifiers::SHIFT) => Some(KeyCode::BackTab),
            9 => Some(KeyCode::Tab),
            13 => Some(KeyCode::Enter),
            27 => Some(KeyCode::Esc),
            33 => Some(KeyCode::PageUp),
            34 => Some(KeyCode::PageDown),
            35 => Some(KeyCode::End),
            36 => Some(KeyCode::Home),
            37 => Some(KeyCode::Left),
            38 => Some(KeyCode::Up),
            39 => Some(KeyCode::Right),
            40 => Some(KeyCode::Down),
            45 => Some(KeyCode::Insert),
            46 => Some(KeyCode::Delete),
            112..=123 => Some(KeyCode::F((vk - 111) as u8)),
            _ => None,
        };
        if let Some(key) = special {
            encode(Event::Key(KeyEvent::new(key, mods)), &mut bytes);
        } else if unit != 0
            || vk == 0
            || (mods.contains(KeyModifiers::CONTROL) && matches!(vk, 32 | 50))
        {
            encode(
                Event::Key(KeyEvent::new(
                    KeyCode::Char(char::from_u32(u32::from(unit)).expect("non-surrogate u16")),
                    mods,
                )),
                &mut bytes,
            );
        }
        Pending::new(bytes, if is_prefix { 1 } else { repeat.max(1) })
    }
}
fn modifiers(state: ControlKeyState) -> KeyModifiers {
    let mut mods = KeyModifiers::NONE;
    if state.has_state(0x10) {
        mods |= KeyModifiers::SHIFT;
    }
    if state.has_state(3) {
        mods |= KeyModifiers::ALT;
    }
    if state.has_state(12) {
        mods |= KeyModifiers::CONTROL;
    }
    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(pending: Option<Pending>) -> Vec<u8> {
        let mut output = Vec::new();
        if let Some(mut pending) = pending {
            pending.drain(&mut output);
            assert_eq!(pending.remaining, 0);
        }
        output
    }
    #[test]
    fn zero_virtual_key_keeps_paste_delimiters_and_literal_control_characters() {
        let mut codec = Codec::default();
        let paste = "\x1b[200~中\x02q\x1b[201~";
        let mut actual = Vec::new();
        for unit in paste.encode_utf16() {
            actual.extend(bytes(codec.key(
                0,
                unit,
                true,
                1,
                KeyModifiers::NONE,
                false,
            )));
        }
        assert_eq!(actual, paste.as_bytes());
    }
    #[test]
    fn surrogate_state_and_large_repeats_survive_poll_boundaries_without_reordering() {
        let mut codec = Codec::default();
        assert!(
            codec
                .key(0, 0xd83d, true, 1, KeyModifiers::NONE, false)
                .is_none()
        );
        assert_eq!(
            bytes(codec.key(0, 0xde00, true, 1, KeyModifiers::NONE, false)),
            "😀".as_bytes()
        );
        let mut pending = codec
            .key(0, u16::from(b'x'), true, 5000, KeyModifiers::NONE, false)
            .unwrap();
        let mut first = Vec::new();
        pending.drain(&mut first);
        assert_eq!(first, vec![b'x'; BURST]);
        let mut second = Vec::new();
        pending.drain(&mut second);
        assert_eq!(second, vec![b'x'; 5000 - BURST]);
        assert_eq!(pending.remaining, 0);
    }
    #[test]
    fn physical_prefix_repeat_does_not_become_a_second_press_and_alt_graph_is_text() {
        let mut codec = Codec::default();
        assert_eq!(
            bytes(codec.key(0x42, 2, true, 5, KeyModifiers::CONTROL, false)),
            [2]
        );
        assert!(
            codec
                .key(0x42, 2, true, 1, KeyModifiers::CONTROL, false)
                .is_none()
        );
        assert!(
            codec
                .key(0x42, 2, false, 1, KeyModifiers::CONTROL, false)
                .is_none()
        );
        assert_eq!(
            bytes(codec.key(0x42, 2, true, 1, KeyModifiers::CONTROL, false)),
            [2]
        );
        assert_eq!(
            bytes(codec.key(
                0x51,
                u16::from(b'@'),
                true,
                1,
                KeyModifiers::CONTROL | KeyModifiers::ALT,
                true
            )),
            b"@"
        );
        assert_eq!(
            bytes(codec.key(0x26, 0, true, 1, KeyModifiers::CONTROL, false)),
            b"\x1b[1;5A"
        );
    }
}
