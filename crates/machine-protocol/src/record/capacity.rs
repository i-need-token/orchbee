//! A machine's free capacity, the value the link's `capacity` reply and the hub's `capacity` reply
//! share: the free processor share, memory and disk, each present only when its source could read
//! it.

use serde::{Deserialize, Serialize};

use crate::{Count, InvalidValue, PathText, protocol::Refused};

/// What a machine has free. A fact its source could not read is absent.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Capacity {
    /// The processors.
    pub cpu: Option<Cpu>,
    /// The physical memory.
    pub memory: Option<Memory>,
    /// The volume that holds the home folder.
    pub disk: Option<Disk>,
}

/// The processors: how many a program may run on, and the share of their time that went unused
/// over the last few seconds.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Cpu {
    logical: Count,
    free_percent: u8,
}

impl Cpu {
    /// At least one processor and a share of at most 100 percent.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] for no processor, a count above the count range, or a share above
    /// 100.
    pub fn new(logical: u64, free_percent: u8) -> Result<Self, InvalidValue> {
        if logical == 0 || free_percent > 100 {
            return Err(InvalidValue);
        }
        Ok(Self {
            logical: Count::new(logical)?,
            free_percent,
        })
    }

    /// The processors a program may run on.
    #[must_use]
    pub const fn logical(self) -> u64 {
        self.logical.get()
    }

    /// The unused share of processor time, from 0 to 100.
    #[must_use]
    pub const fn free_percent(self) -> u8 {
        self.free_percent
    }
}

/// A whole and the part of it that is free, in bytes; the free part never exceeds a whole above 0.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Bytes {
    total: Count,
    free: Count,
}

impl Bytes {
    /// # Errors
    ///
    /// Returns [`InvalidValue`] for a whole of 0, a free part above the whole, or a value above
    /// the count range.
    pub fn new(total: u64, free: u64) -> Result<Self, InvalidValue> {
        if total == 0 || free > total {
            return Err(InvalidValue);
        }
        Ok(Self {
            total: Count::new(total)?,
            free: Count::new(free)?,
        })
    }

    /// The whole.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.total.get()
    }

    /// The free part.
    #[must_use]
    pub const fn free(self) -> u64 {
        self.free.get()
    }
}

/// The physical memory and what programs can still get without paging out.
pub type Memory = Bytes;

/// A volume's space as the reporting user may use it, and the folder it was read for.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Disk {
    /// The folder whose volume was read: the home folder on the machine that reported it.
    pub path: PathText,
    /// The volume's whole and free bytes.
    pub bytes: Bytes,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireCapacity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cpu: Option<WireCpu>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memory: Option<WireBytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    disk: Option<WireDisk>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireCpu {
    logical: Count,
    free_percent: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireBytes {
    total_bytes: Count,
    free_bytes: Count,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireDisk {
    path: PathText,
    total_bytes: Count,
    free_bytes: Count,
}

impl Capacity {
    pub(crate) fn decode(wire: WireCapacity) -> Result<Self, Refused> {
        let bytes = |total: Count, free: Count| Bytes::new(total.get(), free.get());
        Ok(Self {
            cpu: wire
                .cpu
                .map(|cpu| Cpu::new(cpu.logical.get(), cpu.free_percent))
                .transpose()
                .map_err(|_| Refused)?,
            memory: wire
                .memory
                .map(|memory| bytes(memory.total_bytes, memory.free_bytes))
                .transpose()
                .map_err(|_| Refused)?,
            disk: wire
                .disk
                .map(|disk| {
                    Ok::<_, InvalidValue>(Disk {
                        bytes: bytes(disk.total_bytes, disk.free_bytes)?,
                        path: disk.path,
                    })
                })
                .transpose()
                .map_err(|_| Refused)?,
        })
    }

    pub(crate) fn encode(&self) -> WireCapacity {
        WireCapacity {
            cpu: self.cpu.map(|cpu| WireCpu {
                logical: cpu.logical,
                free_percent: cpu.free_percent,
            }),
            memory: self.memory.map(|memory| WireBytes {
                total_bytes: memory.total,
                free_bytes: memory.free,
            }),
            disk: self.disk.as_ref().map(|disk| WireDisk {
                path: disk.path.clone(),
                total_bytes: disk.bytes.total,
                free_bytes: disk.bytes.free,
            }),
        }
    }
}
