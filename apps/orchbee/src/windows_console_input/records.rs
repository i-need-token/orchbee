//! Preserve VT characters before the semantic decoder. Crossterm's Windows
//! event parser discards control characters with virtual-key code zero, which
//! includes the ESC characters in ConPTY bracketed-paste delimiters.
use super::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind, encode};
use crossterm::event::{KeyEvent, MouseEvent};
use crossterm_winapi::{
    ButtonState, Console, ControlKeyState, EventFlags, Handle, HandleType, InputRecord,
    ScreenBuffer,
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
        self.next_input_until(timeout, || false)
    }

    pub(crate) fn next_input_until(
        &mut self,
        timeout: Duration,
        interrupted: impl FnMut() -> bool,
    ) -> io::Result<Option<Vec<u8>>> {
        collect_input(
            &mut self.console,
            &mut self.codec,
            &mut self.pending,
            timeout,
            interrupted,
        )
    }
}

// Isolate console readiness and waiting so the same bounded read loop can be
// exercised with a controlled clock, without opening a native console window.
trait RecordSource {
    fn read_ready(&mut self) -> io::Result<Option<InputRecord>>;
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn park(&mut self, duration: Duration) {
        std::thread::park_timeout(duration);
    }
}

impl RecordSource for Console {
    fn read_ready(&mut self) -> io::Result<Option<InputRecord>> {
        if self.number_of_console_input_events()? == 0 {
            return Ok(None);
        }
        // This inherited terminal's single reader owns every record. Readiness
        // cannot race another console input consumer.
        self.read_single_input_event().map(Some)
    }
}

fn collect_input(
    source: &mut impl RecordSource,
    codec: &mut Codec,
    pending: &mut Option<Pending>,
    timeout: Duration,
    mut interrupted: impl FnMut() -> bool,
) -> io::Result<Option<Vec<u8>>> {
    let wait = source.now() + timeout;
    let mut work = None;
    let mut output = Vec::new();
    for _ in 0..BURST {
        if let Some(value) = pending.as_mut() {
            value.drain(&mut output);
            if value.remaining == 0 {
                *pending = None;
            }
            if output.len() == BURST {
                break;
            }
        }
        if work.is_some_and(|deadline| source.now() >= deadline) {
            break;
        }
        let observed_at = source.now();
        let Some(record) = source.read_ready()? else {
            if !output.is_empty() || interrupted() || source.now() >= wait {
                break;
            }
            // A worker publication unparks this same thread. Its pending
            // notification is checked above, including the check/park race.
            source.park(Duration::from_millis(1).min(wait.saturating_duration_since(source.now())));
            continue;
        };
        work.get_or_insert(observed_at + Duration::from_millis(4));
        *pending = codec.record(record)?;
    }
    Ok((!output.is_empty()).then_some(output))
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
                return Ok(self.mouse(
                    column,
                    row,
                    mouse.button_state,
                    mouse.event_flags,
                    modifiers(mouse.control_key_state),
                ));
            }
            _ => {}
        }
        Ok(Pending::new(bytes, 1))
    }
    fn mouse(
        &mut self,
        column: u16,
        row: u16,
        button_state: ButtonState,
        event_flags: EventFlags,
        mods: KeyModifiers,
    ) -> Option<Pending> {
        let mut bytes = Vec::new();
        let current = u8::from(button_state.left_button())
            | (u8::from(button_state.middle_button()) << 1)
            | (u8::from(button_state.right_button()) << 2);
        let mut emit = |kind| {
            encode(
                Event::Mouse(MouseEvent {
                    kind,
                    column,
                    row,
                    modifiers: mods,
                }),
                &mut bytes,
            )
        };
        match event_flags {
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
            EventFlags::MouseMoved => emit(if current != 0 {
                MouseEventKind::Drag(if current & 1 != 0 {
                    MouseButton::Left
                } else if current & 2 != 0 {
                    MouseButton::Middle
                } else {
                    MouseButton::Right
                })
            } else {
                // Preserve button-free Win32 mouse records as SGR any-motion
                // input. The dispatcher owns whether a current menu lease
                // consumes the motion.
                MouseEventKind::Moved
            }),
            EventFlags::MouseWheeled => emit(if button_state.scroll_down() {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            }),
            EventFlags::MouseHwheeled => emit(if button_state.scroll_left() {
                MouseEventKind::ScrollLeft
            } else {
                MouseEventKind::ScrollRight
            }),
            _ => {}
        }
        self.buttons = current;
        Pending::new(bytes, 1)
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
mod tests;
