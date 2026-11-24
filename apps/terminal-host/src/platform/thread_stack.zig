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
