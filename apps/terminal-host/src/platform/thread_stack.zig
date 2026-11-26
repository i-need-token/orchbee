const builtin = @import("builtin");
const std = @import("std");

/// Spawn configuration for a Host worker thread that keeps the toolchain's default stack budget.
///
/// On Windows `std.Thread.spawn` passes `stack_size` to `NtCreateThreadEx` as the committed size,
/// so Zig's 16 MiB default commits each worker's whole stack against the system commit limit; the
/// four runtime threads alone held 64 MiB behind every terminal. Requesting the 64 KiB minimum
/// commits only that much up front and takes the reservation from the executable image, whose
/// 16 MiB default keeps the same budget; the stack guard page commits further pages on demand, the
/// way POSIX thread stacks are already backed.
pub const default_budget: std.Thread.SpawnConfig = if (builtin.os.tag == .windows)
    .{ .stack_size = 64 * 1024 }
else
    .{};

/// Spawn configuration for a thread whose own frames need at most `bytes` of stack.
///
/// glibc carves a thread's static thread-local storage out of the stack it is asked for, and that
/// storage holds the alternative signal stack the standard library attaches to every thread under
/// runtime safety (`std.options.signal_stack_size`), so a request no larger than it fails
/// `pthread_create`. On glibc the request therefore adds that storage and a margin for the rest of
/// it and the guard page; macOS and musl place thread-local storage beside the stack.
pub fn sized(bytes: usize) std.Thread.SpawnConfig {
    return .{ .stack_size = bytes + thread_storage_bytes };
}

const thread_storage_bytes: usize = if (builtin.os.tag == .linux and builtin.abi.isGnu())
    (std.options.signal_stack_size orelse 0) + 64 * 1024
else
    0;
