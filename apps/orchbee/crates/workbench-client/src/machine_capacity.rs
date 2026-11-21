//! What a machine has free, in the one short form every list of machines shows in every language:
//! the free processor share, memory and disk, as in `88% · 24 GB · 310 GB`, with `—` for a fact
//! its source could not read. A gigabyte is 2³⁰ bytes, the unit Windows and macOS show memory in.

// module-layer: values

use orchbee_machine_protocol::capacity::Capacity;

/// The mark of a fact that could not be read.
pub const UNKNOWN: &str = "—";

/// The short form of `capacity`; `None`, nothing read at all, is one [`UNKNOWN`].
#[must_use]
pub fn summary(capacity: Option<&Capacity>) -> String {
    let Some(capacity) = capacity else {
        return UNKNOWN.to_owned();
    };
    let cpu = capacity.cpu.map_or_else(
        || UNKNOWN.to_owned(),
        |cpu| format!("{}%", cpu.free_percent()),
    );
    let memory = capacity
        .memory
        .map_or_else(|| UNKNOWN.to_owned(), |memory| gigabytes(memory.free()));
    let disk = capacity
        .disk
        .as_ref()
        .map_or_else(|| UNKNOWN.to_owned(), |disk| gigabytes(disk.bytes.free()));
    format!("{cpu} · {memory} · {disk}")
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
