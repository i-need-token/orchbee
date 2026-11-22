//! What a machine has free, in the one short form every list of machines shows in every language:
//! the free processor share, memory and disk, as in `88% · 24 GB · 310 GB`, with `—` for a fact
//! its source could not read. A gigabyte is 2³⁰ bytes, the unit Windows and macOS show memory in.

// module-layer: values

use orchbee_machine_protocol::capacity::Capacity;

/// The mark of a fact that could not be read.
pub const UNKNOWN: &str = "—";

/// Below these a fact is low, and a list that warns shows its number in amber: a fifth of the
/// processor, 4 GB of memory, 25 GB of disk.
const LOW_CPU_PERCENT: u8 = 20;
const LOW_MEMORY: u64 = 4 << 30;
const LOW_DISK: u64 = 25 << 30;

/// One fact of the short form: its text, and whether little of it is left.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Part {
    pub text: String,
    pub low: bool,
}

/// The three facts of the short form, free processor, memory and disk, in that order. A fact that
/// could not be read is [`UNKNOWN`] and never low.
#[must_use]
pub fn parts(capacity: &Capacity) -> [Part; 3] {
    let unknown = || Part {
        text: UNKNOWN.to_owned(),
        low: false,
    };
    [
        capacity.cpu.map_or_else(unknown, |cpu| Part {
            text: format!("{}%", cpu.free_percent()),
            low: cpu.free_percent() < LOW_CPU_PERCENT,
        }),
        capacity.memory.map_or_else(unknown, |memory| Part {
            text: gigabytes(memory.free()),
            low: memory.free() < LOW_MEMORY,
        }),
        capacity.disk.as_ref().map_or_else(unknown, |disk| Part {
            text: gigabytes(disk.bytes.free()),
            low: disk.bytes.free() < LOW_DISK,
        }),
    ]
}

/// The short form of `capacity`; `None`, nothing read at all, is one [`UNKNOWN`].
#[must_use]
pub fn summary(capacity: Option<&Capacity>) -> String {
    let Some(capacity) = capacity else {
        return UNKNOWN.to_owned();
    };
    let [cpu, memory, disk] = parts(capacity);
    format!("{} · {} · {}", cpu.text, memory.text, disk.text)
}

/// `bytes` in gigabytes with the unit, as [`gigabyte_number`] rounds them.
#[must_use]
pub fn gigabytes(bytes: u64) -> String {
    format!("{} GB", gigabyte_number(bytes))
}

/// `bytes` in gigabytes, rounded to the nearest: whole from 10 up, with one decimal below.
#[must_use]
pub fn gigabyte_number(bytes: u64) -> String {
    const GIGABYTE: u128 = 1 << 30;
    let bytes = u128::from(bytes);
    let whole = (bytes + GIGABYTE / 2) / GIGABYTE;
    if whole >= 10 {
        return whole.to_string();
    }
    let tenths = (bytes * 10 + GIGABYTE / 2) / GIGABYTE;
    format!("{}.{}", tenths / 10, tenths % 10)
}
