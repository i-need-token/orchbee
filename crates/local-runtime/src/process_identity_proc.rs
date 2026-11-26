//! The Linux reads of a process's start, program file and children through `/proc`, which a
//! registration names a process by, as `process_identity_native` reads them on macOS. Each is a file
//! read, so none needs unsafe code.
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// What a process id names now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Started {
    /// No live process has the id, or its process has ended and waits as a zombie to be reaped.
    Gone,
    /// A live process has the id and started at this point of this boot (see [`started`]).
    At(u64),
    /// The process's `/proc` entry cannot be read, as under `hidepid=1` for another account's.
    Denied,
}

/// When the process `pid` started: its start in clock ticks since boot, field 22 of
/// `/proc/<pid>/stat`, in the low 40 bits, under a tag of this boot's `boot_id` with the top bit
/// set. The tick count alone repeats across boots, and a process a login starts at a fixed point
/// of every boot can come back with the same ID and tick, so a record kept across a restart must
/// not name it. The value is only ever compared for equality.
pub(crate) fn started(pid: u32) -> io::Result<Started> {
    let stat = match fs::read(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) => {
            return match error.kind() {
                io::ErrorKind::NotFound => Ok(Started::Gone),
                io::ErrorKind::PermissionDenied => Ok(Started::Denied),
                // The process ended between opening its entry and reading it.
                _ if error.raw_os_error() == Some(nix::libc::ESRCH) => Ok(Started::Gone),
                _ => Err(error),
            };
        }
    };
    let fields = Fields::parse(&stat)?;
    if fields.ended {
        return Ok(Started::Gone);
    }
    if fields.start_ticks >= 1 << 40 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Started::At(
        (u64::from(boot_tag()?) << 40) | fields.start_ticks,
    ))
}

/// The file name of the program the process `pid` runs, as [`program_path`] reads it.
pub(crate) fn program_file_name(pid: u32) -> Option<OsString> {
    program_path(pid)?.file_name().map(ToOwned::to_owned)
}

/// The full path of the program the process `pid` runs, the target of `/proc/<pid>/exe`; `None`
/// when no process has the id or the link cannot be read, as for another account's process. A
/// program file removed since the process started reads with the ` (deleted)` mark the kernel
/// appends, so it names no existing file.
pub(crate) fn program_path(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

/// The IDs of the child processes of `parent`, ended ones still waiting to be reaped included: the
/// processes `/proc` lists whose parent field names it. A process that ends while the list is
/// read is left out.
pub(crate) fn children(parent: u32) -> io::Result<Vec<u32>> {
    let mut children = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read(entry.path().join("stat")) else {
            continue;
        };
        if Fields::parse(&stat)?.parent == parent {
            children.push(pid);
        }
    }
    Ok(children)
}

/// The fields of one `/proc/<pid>/stat` this module reads.
#[derive(Debug, Eq, PartialEq)]
struct Fields {
    /// The state is `Z`, a zombie, or `X`, dead.
    ended: bool,
    parent: u32,
    start_ticks: u64,
}

impl Fields {
    fn parse(stat: &[u8]) -> io::Result<Self> {
        let invalid = || io::Error::from(io::ErrorKind::InvalidData);
        // The command name in parentheses may hold any byte, `)` and spaces included, so the fields
        // after it start past the last `)`.
        let close = stat
            .iter()
            .rposition(|byte| *byte == b')')
            .ok_or_else(invalid)?;
        let rest = std::str::from_utf8(stat.get(close + 1..).ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let mut fields = rest.split_ascii_whitespace();
        // Field 3, then field 4.
        let state = fields.next().ok_or_else(invalid)?;
        let parent = fields
            .next()
            .and_then(|parent| parent.parse().ok())
            .ok_or_else(invalid)?;
        // Fields 5 to 21 are skipped; field 22 is the start.
        let start_ticks = fields
            .nth(17)
            .and_then(|start| start.parse().ok())
            .ok_or_else(invalid)?;
        Ok(Self {
            ended: matches!(state, "Z" | "X"),
            parent,
            start_ticks,
        })
    }
}

/// This boot's tag: 23 bits of an FNV-1a hash of `/proc/sys/kernel/random/boot_id`, with bit 23 set
/// so that no start reads as zero.
fn boot_tag() -> io::Result<u32> {
    static TAG: OnceLock<u32> = OnceLock::new();
    if let Some(tag) = TAG.get() {
        return Ok(*tag);
    }
    let tag = tag_of(&fs::read(Path::new("/proc/sys/kernel/random/boot_id"))?);
    Ok(*TAG.get_or_init(|| tag))
}

fn tag_of(boot_id: &[u8]) -> u32 {
    let hash = boot_id
        .trim_ascii()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    // The mask keeps 23 bits, which fit in a `u32`.
    u32::try_from(hash & 0x7f_ffff).unwrap_or(0) | 0x80_0000
}

#[cfg(test)]
mod tests;
