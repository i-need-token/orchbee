//! What this computer has free, read when asked: the processors' cumulative tick counters, from
//! which a caller derives the free share between two readings; the physical memory programs can
//! still get without paging; and the space a folder's volume has left for this user. Each read is
//! at most a few system calls or one small file read and never waits on a peer. A value the system
//! refuses, or reports implausibly, reads as `None` and the other reads still report. How often to
//! read and over which window belongs to the caller.

use std::path::Path;

/// The cumulative idle and busy time of every processor since the system started, in the
/// platform's own unit. Only the difference between two readings means anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuTicks {
    idle: u64,
    busy: u64,
}

/// macOS counts processor ticks in 32-bit counters that wrap; the other platforms count in 64 bits.
const TICK_MASK: u64 = if cfg!(target_os = "macos") {
    u32::MAX as u64
} else {
    u64::MAX
};

impl CpuTicks {
    /// The share of processor time spent idle from `earlier` to `self`, in whole percent from 0 to
    /// 100; `None` when no processor time passed between the two readings.
    #[must_use]
    pub fn free_percent_since(self, earlier: Self) -> Option<u8> {
        let idle = u128::from(self.idle.wrapping_sub(earlier.idle) & TICK_MASK);
        let busy = u128::from(self.busy.wrapping_sub(earlier.busy) & TICK_MASK);
        let total = idle + busy;
        if total == 0 {
            return None;
        }
        u8::try_from((idle * 100 + total / 2) / total).ok()
    }
}

/// Physical memory: the whole, and what programs can still get without paging out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Memory {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// A volume's space: the whole, and what this user may still write to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Disk {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// The processors' tick counters now.
#[must_use]
pub fn cpu_ticks() -> Option<CpuTicks> {
    platform::cpu_ticks()
}

/// How many processors a program started now may run on.
#[must_use]
pub fn logical_processors() -> Option<u32> {
    std::thread::available_parallelism()
        .ok()
        .and_then(|count| u32::try_from(count.get()).ok())
}

/// The physical memory now. Windows reports its available bytes, Linux `MemAvailable`, and macOS
/// its free and inactive pages, the measure `vm_stat` prints as `Pages free`, `Pages speculative`
/// and `Pages inactive` together.
#[must_use]
pub fn memory() -> Option<Memory> {
    platform::memory().filter(|memory| plausible(memory.total_bytes, memory.free_bytes))
}

/// The space of the volume that holds `folder`, as this user may use it.
#[must_use]
pub fn disk(folder: &Path) -> Option<Disk> {
    platform::disk(folder).filter(|disk| plausible(disk.total_bytes, disk.free_bytes))
}

/// A whole of zero, or a free part above the whole, is a reading nobody can trust.
const fn plausible(total: u64, free: u64) -> bool {
    total > 0 && free <= total
}

#[cfg(windows)]
mod platform {
    use super::{CpuTicks, Disk, Memory};
    use std::path::Path;
    use winsafe::{GetDiskFreeSpaceEx, GetSystemTimes, GlobalMemoryStatusEx};

    pub(super) fn cpu_ticks() -> Option<CpuTicks> {
        let (idle, kernel, user) = GetSystemTimes().ok()?;
        let idle = u64::from(idle);
        // Kernel time includes idle time.
        let busy = u64::from(kernel)
            .checked_sub(idle)?
            .checked_add(u64::from(user))?;
        Some(CpuTicks { idle, busy })
    }

    pub(super) fn memory() -> Option<Memory> {
        let status = GlobalMemoryStatusEx().ok()?;
        Some(Memory {
            total_bytes: status.ullTotalPhys,
            free_bytes: status.ullAvailPhys,
        })
    }

    pub(super) fn disk(folder: &Path) -> Option<Disk> {
        let (mut free, mut total) = (0, 0);
        GetDiskFreeSpaceEx(
            Some(folder.to_str()?),
            Some(&mut free),
            Some(&mut total),
            None,
        )
        .ok()?;
        Some(Disk {
            total_bytes: total,
            free_bytes: free,
        })
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{CpuTicks, Memory};
    use crate::host_capacity_native as native;

    pub(super) use super::statvfs::disk;

    pub(super) fn cpu_ticks() -> Option<CpuTicks> {
        let load = native::cpu_load()?;
        Some(CpuTicks {
            idle: u64::from(load.idle),
            // One 32-bit counter of its own, so the wrapping difference stays exact.
            busy: u64::from(load.user.wrapping_add(load.system).wrapping_add(load.nice)),
        })
    }

    pub(super) fn memory() -> Option<Memory> {
        let total_bytes = native::memory_size()?;
        // The kernel's page size; a translated process sees its own smaller one in `hw.pagesize`.
        let page = native::page_size().filter(|page| page.is_power_of_two() && *page >= 4096)?;
        let free_bytes = native::reclaimable_pages()?.checked_mul(page)?;
        Some(Memory {
            total_bytes,
            free_bytes,
        })
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::{CpuTicks, Memory};

    pub(super) use super::statvfs::disk;

    /// The first line of `/proc/stat`: `cpu` and the user, nice, system, idle, iowait, irq,
    /// softirq and steal times; guest time is already inside user time.
    pub(super) fn cpu_ticks() -> Option<CpuTicks> {
        let text = std::fs::read_to_string("/proc/stat").ok()?;
        let mut fields = text
            .lines()
            .next()?
            .strip_prefix("cpu ")?
            .split_whitespace();
        let mut times = [0_u64; 8];
        for (index, time) in times.iter_mut().enumerate() {
            match fields.next() {
                Some(field) => *time = field.parse().ok()?,
                // Kernels before 2.6.11 list fewer fields; the first four are always there.
                None if index >= 4 => break,
                None => return None,
            }
        }
        let [user, nice, system, idle, iowait, irq, softirq, steal] = times;
        Some(CpuTicks {
            idle: idle.checked_add(iowait)?,
            busy: [nice, system, irq, softirq, steal]
                .into_iter()
                .try_fold(user, u64::checked_add)?,
        })
    }

    /// `MemTotal` and `MemAvailable` of `/proc/meminfo`, in kibibytes there.
    pub(super) fn memory() -> Option<Memory> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let field = |name: &str| -> Option<u64> {
            let line = text.lines().find_map(|line| line.strip_prefix(name))?;
            let kibibytes = line.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()?;
            kibibytes.checked_mul(1024)
        };
        Some(Memory {
            total_bytes: field("MemTotal:")?,
            free_bytes: field("MemAvailable:")?,
        })
    }
}

#[cfg(unix)]
mod statvfs {
    use super::Disk;
    use nix::sys::statvfs::statvfs;
    use std::path::Path;

    /// The volume's fragments and those an unprivileged user may still take.
    pub(super) fn disk(folder: &Path) -> Option<Disk> {
        let stat = statvfs(folder).ok()?;
        let fragment = widen(stat.fragment_size());
        Some(Disk {
            total_bytes: widen(stat.blocks()).checked_mul(fragment)?,
            free_bytes: widen(stat.blocks_available()).checked_mul(fragment)?,
        })
    }

    /// The field types differ in width between platforms; every one fits in 64 bits.
    fn widen(value: impl Into<u64>) -> u64 {
        value.into()
    }
}

#[cfg(test)]
mod tests;
