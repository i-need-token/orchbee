//! The reviewed macOS adapter that reads what this computer has free through three calls:
//! `host_statistics` with `HOST_CPU_LOAD_INFO` for the processors' tick counters,
//! `host_statistics64` with `HOST_VM_INFO64` for the virtual memory page counts, and
//! `sysctlbyname` for two numbers, the physical memory size `hw.memsize` and the kernel's page size
//! `vm.pagesize`. Both host calls need the host name port, which `mach_host_self` gives once per
//! process. [`crate::host_capacity`] is the only caller. Each function returns plain numbers, or
//! `None` when the call is refused or reports a count or length other than the one read; the crate
//! README's safety contract lists every invariant the unsafe blocks below rely on.
use nix::libc;
use std::{
    ffi::CStr,
    mem::{self, MaybeUninit},
    ptr,
    sync::OnceLock,
};

unsafe extern "C" {
    // As `<mach/mach_init.h>` declares it. The libc crate's declaration is deprecated in favour
    // of another crate, which this adapter does not take for one function.
    fn mach_host_self() -> libc::mach_port_t;
}

/// The processors' cumulative ticks in each state, as `HOST_CPU_LOAD_INFO` reports them. Each is
/// a 32-bit counter that wraps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CpuLoad {
    pub(crate) user: u32,
    pub(crate) system: u32,
    pub(crate) idle: u32,
    pub(crate) nice: u32,
}

/// The host name port, taken once and kept for the life of the process. Every `mach_host_self`
/// call adds one user reference to the same send right; taking it once holds exactly one, which
/// the kernel releases with the task, so there is nothing to deallocate and the count never grows.
fn host() -> Option<libc::mach_port_t> {
    static HOST: OnceLock<libc::mach_port_t> = OnceLock::new();
    // SAFETY: `mach_host_self` takes no argument, cannot fail in a way that breaks memory safety,
    // and returns a port name, `MACH_PORT_NULL` (0) at worst, which the check below refuses.
    let host = *HOST.get_or_init(|| unsafe { mach_host_self() });
    (host != 0).then_some(host)
}

/// The processors' tick counters now.
pub(crate) fn cpu_load() -> Option<CpuLoad> {
    let host = host()?;
    let mut load = MaybeUninit::<libc::host_cpu_load_info>::zeroed();
    // The in/out count is the buffer's size in `integer_t` units: `host_cpu_load_info` holds
    // `CPU_STATE_MAX` (4) `natural_t` counters, so the count is 4, `HOST_CPU_LOAD_INFO_COUNT`.
    let capacity = mem::size_of::<libc::host_cpu_load_info>() / mem::size_of::<libc::integer_t>();
    let mut count = libc::mach_msg_type_number_t::try_from(capacity).ok()?;
    // SAFETY: `load` is a zeroed, writable `host_cpu_load_info`, and `count` states its size in
    // `integer_t` units, so the kernel writes at most `count` integers into it and stores in
    // `count` how many it wrote. Both pointers live across the call. `host` is a send right this
    // process holds.
    let result = unsafe {
        libc::host_statistics(
            host,
            libc::HOST_CPU_LOAD_INFO,
            load.as_mut_ptr().cast::<libc::integer_t>(),
            &raw mut count,
        )
    };
    // A short count leaves counters unwritten and a larger one is impossible for this buffer;
    // either way the reading is not used.
    if result != libc::KERN_SUCCESS || usize::try_from(count).ok()? != capacity {
        return None;
    }
    // SAFETY: the buffer started zeroed and holds only integers, so every bit pattern is a valid
    // value, and the kernel wrote all `capacity` integers.
    let load = unsafe { load.assume_init() };
    let ticks = load.cpu_ticks;
    Some(CpuLoad {
        user: *ticks.get(usize::try_from(libc::CPU_STATE_USER).ok()?)?,
        system: *ticks.get(usize::try_from(libc::CPU_STATE_SYSTEM).ok()?)?,
        idle: *ticks.get(usize::try_from(libc::CPU_STATE_IDLE).ok()?)?,
        nice: *ticks.get(usize::try_from(libc::CPU_STATE_NICE).ok()?)?,
    })
}

/// The pages programs can still get without paging out: the free pages, which include the
/// speculative ones, and the inactive pages.
pub(crate) fn reclaimable_pages() -> Option<u64> {
    let host = host()?;
    let mut statistics = MaybeUninit::<libc::vm_statistics64>::zeroed();
    // The in/out count is the buffer's size in `integer_t` units, `HOST_VM_INFO64_COUNT` for the
    // libc declaration. A kernel older than that declaration writes fewer integers, its own
    // revision of the structure, and says so in `count`.
    let capacity = mem::size_of::<libc::vm_statistics64>() / mem::size_of::<libc::integer_t>();
    let mut count = libc::mach_msg_type_number_t::try_from(capacity).ok()?;
    // SAFETY: `statistics` is a zeroed, writable `vm_statistics64`, and `count` states its size
    // in `integer_t` units, so the kernel writes at most `count` integers into it and stores in
    // `count` how many it wrote. Both pointers live across the call. `host` is a send right this
    // process holds.
    let result = unsafe {
        libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            statistics.as_mut_ptr().cast::<libc::integer_t>(),
            &raw mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return None;
    }
    let written = usize::try_from(count).ok()?;
    // The two counters read below must lie inside what the kernel reported as written; a count
    // above the buffer's is refused as well.
    let needed = (mem::offset_of!(libc::vm_statistics64, inactive_count)
        + mem::size_of::<libc::natural_t>())
    .div_ceil(mem::size_of::<libc::integer_t>());
    if written < needed || written > capacity {
        return None;
    }
    // SAFETY: the buffer started zeroed and holds only integers, so every bit pattern is a valid
    // value; the fields read are copied out, inside the `written` integers checked above.
    let statistics = unsafe { statistics.assume_init() };
    let (free, inactive) = (statistics.free_count, statistics.inactive_count);
    Some(u64::from(free) + u64::from(inactive))
}

/// The physical memory in bytes.
pub(crate) fn memory_size() -> Option<u64> {
    number(c"hw.memsize")
}

/// The kernel's page size in bytes, the unit of the page counts.
pub(crate) fn page_size() -> Option<u64> {
    number(c"vm.pagesize")
}

/// The integer value of the read-only sysctl `name`, which the kernel stores in 4 or 8 bytes.
fn number(name: &CStr) -> Option<u64> {
    let mut value = [0_u8; 8];
    let mut length: libc::size_t = value.len();
    // SAFETY: `name` is NUL-terminated; `value` is a writable buffer of `length` (8) bytes, so the
    // kernel writes at most that many, refusing with `ENOMEM` a value that does not fit, and
    // stores in `length` how many it wrote. No new value is set: the new-value pointer is null and
    // its length 0. Every pointer lives across the call.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            value.as_mut_ptr().cast::<libc::c_void>(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return None;
    }
    // Only the bytes the kernel reported as written are read.
    match length {
        4 => value
            .first_chunk::<4>()
            .copied()
            .map(u32::from_ne_bytes)
            .map(u64::from),
        8 => Some(u64::from_ne_bytes(value)),
        _ => None,
    }
}

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
