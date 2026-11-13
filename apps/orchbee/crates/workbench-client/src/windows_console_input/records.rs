//! Preserve VT characters before the semantic decoder. Crossterm's Windows
//! event parser discards control characters with virtual-key code zero, which
//! includes the ESC characters in ConPTY bracketed-paste delimiters.
//!
//! Typed characters may be a login answer, so what this reader holds of a key after passing it on
//! is wiped: the encoded bytes of a key, each batch of records read from the console, and the
//! collected output, which never grows into a second allocation.
use super::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind, encode};
use crossterm::event::{KeyEvent, MouseEvent};
use orchbee_machine_protocol::wipe;
use orchbee_windows_console_host::{
    ConsoleInput, ConsoleOutput, InputRecord, InputWait, InputWake, KeyRecord,
};
use std::{
    fmt, io,
    sync::Arc,
    time::{Duration, Instant},
};

const BURST: usize = 4096;
/// Records taken from the console in one read. Every console call is a round trip to the console
/// host, so a burst read in one call still arrives whole when that round trip is slow.
const READ_BATCH: usize = 256;
const MOUSE_MOVED: u32 = 0x0001;
const DOUBLE_CLICK: u32 = 0x0002;
const MOUSE_WHEELED: u32 = 0x0004;
const MOUSE_HWHEELED: u32 = 0x0008;
const UNWOKEN_WAIT_SLICE: Duration = Duration::from_millis(1);

pub(crate) struct Reader {
    source: NativeRecordSource,
    codec: Codec,
    pending: Option<Pending>,
}
impl fmt::Debug for Reader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsoleReader").finish_non_exhaustive()
    }
}
impl Reader {
    pub(crate) fn new(input: Arc<ConsoleInput>, output: Arc<ConsoleOutput>) -> Self {
        Self {
            source: NativeRecordSource {
                input,
                output,
                wake: None,
                wait_slice: None,
                signalled_without_record: false,
                ready: Vec::new(),
                taken: 0,
            },
            codec: Codec::default(),
            pending: None,
        }
    }
    pub(crate) fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        self.source.wake = None;
        self.source.wait_slice = None;
        collect_input(
            &mut self.source,
            &mut self.codec,
            &mut self.pending,
            timeout,
            || false,
        )
    }

    pub(crate) fn next_input_until(
        &mut self,
        timeout: Duration,
        wake: Option<&Arc<InputWake>>,
        interrupted: impl FnMut() -> bool,
    ) -> io::Result<Option<Vec<u8>>> {
        self.source.wake = wake.cloned();
        self.source.wait_slice = wake.is_none().then_some(UNWOKEN_WAIT_SLICE);
        collect_input(
            &mut self.source,
            &mut self.codec,
            &mut self.pending,
            timeout,
            interrupted,
        )
    }
}

struct NativeRecordSource {
    input: Arc<ConsoleInput>,
    output: Arc<ConsoleOutput>,
    wake: Option<Arc<InputWake>>,
    /// Bounds an interruptible wait whose notification has no event to end it.
    wait_slice: Option<Duration>,
    signalled_without_record: bool,
    /// The last batch read from the console, in order; the first `taken` were collected.
    ready: Vec<InputRecord>,
    taken: usize,
}

impl NativeRecordSource {
    /// Overwrites every record of the batch, so no key of it outlives its collection.
    fn wipe_ready(&mut self) {
        let blank = InputRecord::Key(KeyRecord {
            key_down: false,
            repeat_count: 0,
            virtual_key_code: 0,
            virtual_scan_code: 0,
            utf16_unit: 0,
            control_key_state: 0,
        });
        self.ready.fill(blank);
        std::hint::black_box(&self.ready);
        self.ready.clear();
        self.taken = 0;
    }
}

impl Drop for NativeRecordSource {
    fn drop(&mut self) {
        self.wipe_ready();
    }
}

// Isolate console readiness and waiting so the same bounded read loop can be
// exercised with a controlled clock, without opening a native console window.
trait RecordSource {
    fn read_ready(&mut self) -> io::Result<Option<InputRecord>>;
    fn window_origin(&self) -> io::Result<(i16, i16)>;
    fn now(&self) -> Instant {
        Instant::now()
    }
    /// Wait for input or a worker notification, at most `duration`.
    fn park(&mut self, duration: Duration);
}

impl RecordSource for NativeRecordSource {
    fn read_ready(&mut self) -> io::Result<Option<InputRecord>> {
        if self.taken == self.ready.len() {
            self.wipe_ready();
            // No other application-owned reader races this endpoint. External OS
            // aliases remain outside that single-reader guarantee.
            self.ready = self.input.read_ready_records(READ_BATCH)?;
        }
        let record = self.ready.get(self.taken).copied();
        self.taken += usize::from(record.is_some());
        self.signalled_without_record &= record.is_none();
        Ok(record)
    }

    fn window_origin(&self) -> io::Result<(i16, i16)> {
        let window = self.output.window_info()?;
        Ok((window.left, window.top))
    }

    fn park(&mut self, duration: Duration) {
        // A console that signals with no record to read must cost a cadence, never a spin.
        if std::mem::take(&mut self.signalled_without_record) {
            std::thread::sleep(duration.min(UNWOKEN_WAIT_SLICE));
            return;
        }
        let duration = self
            .wait_slice
            .map_or(duration, |slice| duration.min(slice));
        match self.input.wait_ready(self.wake.as_deref(), duration) {
            Ok(InputWait::Input) => self.signalled_without_record = true,
            Ok(InputWait::Woken | InputWait::TimedOut) => {}
            Err(_) => std::thread::sleep(duration.min(UNWOKEN_WAIT_SLICE)),
        }
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
            // The output holds at most one burst: reserved once, it never leaves a copy behind
            // in an outgrown allocation.
            if output.capacity() == 0 {
                output.reserve_exact(BURST);
            }
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
        let Some(record) = source.read_ready()? else {
            if !output.is_empty() || interrupted() || source.now() >= wait {
                break;
            }
            // A worker publication ends this wait. Its pending notification
            // is checked above, including the check/wait race.
            source.park(wait.saturating_duration_since(source.now()));
            continue;
        };
        // The burst bound starts once the first record is in hand: the console read that brought
        // it, slow on a busy machine, must not leave the rest of its batch to a later frame.
        work.get_or_insert_with(|| source.now() + Duration::from_millis(4));
        let window_origin = if matches!(&record, InputRecord::Mouse(_)) {
            Some(source.window_origin()?)
        } else {
            None
        };
        *pending = codec.record(record, window_origin)?;
    }
    Ok((!output.is_empty()).then_some(output))
}

struct Pending {
    bytes: Vec<u8>,
    remaining: u16,
    offset: usize,
}
impl Drop for Pending {
    fn drop(&mut self) {
        wipe(&mut self.bytes);
    }
}
impl Pending {
    fn new(mut bytes: Vec<u8>, remaining: u16) -> Option<Self> {
        if bytes.is_empty() || remaining == 0 {
            wipe(&mut bytes);
            return None;
        }
        Some(Self {
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
    high: Option<Surrogate>,
    prefix_down: bool,
    buttons: u8,
}

struct Surrogate {
    unit: u16,
    repeat: u16,
    modifiers: KeyModifiers,
}

impl Codec {
    fn record(
        &mut self,
        record: InputRecord,
        window_origin: Option<(i16, i16)>,
    ) -> io::Result<Option<Pending>> {
        let mut bytes = Vec::new();
        match record {
            InputRecord::Key(key) => {
                return self.key(
                    key.virtual_key_code,
                    key.utf16_unit,
                    key.key_down,
                    key.repeat_count,
                    modifiers(key.control_key_state),
                    key.control_key_state & 1 != 0 && key.control_key_state & 12 != 0,
                );
            }
            InputRecord::Focus(focused) => encode(
                if focused {
                    Event::FocusGained
                } else {
                    Event::FocusLost
                },
                &mut bytes,
            ),
            InputRecord::Mouse(mouse) => {
                let (left, top) = window_origin.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "console mouse record is missing its output-window origin",
                    )
                })?;
                let x = i32::from(mouse.x) - i32::from(left);
                let y = i32::from(mouse.y) - i32::from(top);
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
            InputRecord::Resize { .. } | InputRecord::Menu { .. } | InputRecord::Unknown { .. } => {
            }
        }
        Ok(Pending::new(bytes, 1))
    }
    fn mouse(
        &mut self,
        column: u16,
        row: u16,
        button_state: u32,
        event_flags: u32,
        mods: KeyModifiers,
    ) -> Option<Pending> {
        let mut bytes = Vec::new();
        let current = u8::from(button_state & 1 != 0)
            | (u8::from(button_state & 4 != 0) << 1)
            | (u8::from(button_state & (2 | 8 | 16) != 0) << 2);
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
            0 | DOUBLE_CLICK => {
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
            MOUSE_MOVED => emit(if current != 0 {
                MouseEventKind::Drag(if current & 1 != 0 {
                    MouseButton::Left
                } else if current & 2 != 0 {
                    MouseButton::Middle
                } else {
                    MouseButton::Right
                })
            } else {
                // Preserve button-free Win32 mouse records as SGR any-motion
                // input. The dispatcher owns whether admitted global
                // any-motion observation may consume it.
                MouseEventKind::Moved
            }),
            MOUSE_WHEELED => emit(if (button_state as i32) < 0 {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            }),
            MOUSE_HWHEELED => emit(if (button_state as i32) < 0 {
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
    ) -> io::Result<Option<Pending>> {
        if vk == 0x42 && !down {
            self.prefix_down = false;
        }
        let alt_code = vk == 0x12 && !down && unit != 0;
        if !down && !alt_code {
            return Ok(None);
        }
        if matches!(vk, 0x10..=0x12) && !alt_code {
            return Ok(None);
        }
        let is_prefix = vk == 0x42 && unit == 2;
        if is_prefix && std::mem::replace(&mut self.prefix_down, true) {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        // VK=0 denotes already translated VT characters, including literal C0.
        if vk == 0 || alt_graph || alt_code {
            mods = KeyModifiers::NONE;
        }
        let repeat = repeat.max(1);
        if (0xd800..=0xdbff).contains(&unit) {
            if self.high.take().is_some() {
                return Err(io::ErrorKind::InvalidData.into());
            }
            self.high = Some(Surrogate {
                unit,
                repeat,
                modifiers: mods,
            });
            return Ok(None);
        }
        if (0xdc00..=0xdfff).contains(&unit) {
            let high = self.high.take().ok_or(io::ErrorKind::InvalidData)?;
            if high.repeat != repeat || high.modifiers != mods {
                return Err(io::ErrorKind::InvalidData.into());
            }
            // Both ranges were checked, so decoding must yield exactly one scalar.
            let ch = char::decode_utf16([high.unit, unit])
                .next()
                .expect("checked surrogate pair")
                .expect("checked high and low surrogate ranges");
            encode(
                Event::Key(KeyEvent::new(KeyCode::Char(ch), mods)),
                &mut bytes,
            );
            return Ok(Pending::new(bytes, repeat));
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
        let code = if let Some(key) = special {
            key
        } else if unit != 0
            || vk == 0
            || (mods.contains(KeyModifiers::CONTROL) && matches!(vk, 32 | 50))
        {
            KeyCode::Char(char::from_u32(u32::from(unit)).expect("non-surrogate u16"))
        } else {
            // A key record without text or a supported key is metadata, not a
            // Unicode boundary. IME/process-key records may separate a pair.
            return Ok(None);
        };
        if self.high.take().is_some() {
            // Never manufacture replacement text that a pane or editor could
            // accept. The existing input-error path refuses this read instead.
            return Err(io::ErrorKind::InvalidData.into());
        }
        encode(Event::Key(KeyEvent::new(code, mods)), &mut bytes);
        Ok(Pending::new(bytes, if is_prefix { 1 } else { repeat }))
    }
}
fn modifiers(state: u32) -> KeyModifiers {
    let mut mods = KeyModifiers::NONE;
    if state & 0x10 != 0 {
        mods |= KeyModifiers::SHIFT;
    }
    if state & 3 != 0 {
        mods |= KeyModifiers::ALT;
    }
    if state & 12 != 0 {
        mods |= KeyModifiers::CONTROL;
    }
    mods
}

#[cfg(test)]
mod tests;
