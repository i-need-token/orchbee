//! The `keeper` capability's answers and events on `worker-host`. Members appear in bytewise name
//! order, as every control encoder writes them.

const std = @import("std");
const common = @import("encode_common.zig");

/// A process as `{pid, creationTime}`.
pub const KeeperProcess = struct {
    pid: u32,
    created: u64,
};

/// `AttachKeeper`'s answer, and the payload of `KeeperLost` and `KeeperUnresponsive`.
pub fn keeperNamed(allocator: std.mem.Allocator, keeper: KeeperProcess) ![]u8 {
    return std.fmt.allocPrint(
        allocator,
        "{{\"keeper\":{{\"creationTime\":\"{d}\",\"pid\":{d}}}}}",
        .{ keeper.created, keeper.pid },
    );
}

/// `TerminalUnheld`: the terminal that runs unheld in `keeper`, and why. `reason` is a wire name.
pub fn terminalUnheld(
    allocator: std.mem.Allocator,
    keeper: KeeperProcess,
    terminal_id: anytype,
    reason: []const u8,
) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"keeper\":{{\"creationTime\":\"{d}\",\"pid\":{d}}},\"reason\":\"{s}\",\"terminalId\":\"{s}\"}}",
        .{ keeper.created, keeper.pid, reason, common.identifierText(terminal_id, &terminal_buffer) },
    );
}
