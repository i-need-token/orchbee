const std = @import("std");

const subject = @import("terminal-host-core").state;

test "HostState prepares and commits terminal defaults without exposing mutable storage" {
    var core = subject.HostCore.initWithDefaults(
        std.testing.allocator,
        subject.HostEpoch.fromBytes([_]u8{0x41} ** 16),
        .{ .scrollback_bytes = 20_000_000 },
    );
    defer core.deinit();

    try std.testing.expectEqual(@as(u64, 1), core.terminalDefaultsRevision());
    try std.testing.expectEqual(@as(usize, 20_000_000), core.terminalDefaults().scrollback_bytes);

    const replay = try core.prepareTerminalDefaultsUpdate(.{ .scrollback_bytes = 20_000_000 });
    try std.testing.expect(!replay.changed);
    try std.testing.expectEqual(@as(u64, 1), replay.revision);
    core.commitTerminalDefaultsUpdate(replay);
    try std.testing.expectEqual(@as(u64, 1), core.terminalDefaultsRevision());

    const update = try core.prepareTerminalDefaultsUpdate(.{ .scrollback_bytes = 25_000_000 });
    try std.testing.expect(update.changed);
    try std.testing.expectEqual(@as(u64, 2), update.revision);
    try std.testing.expectEqual(@as(usize, 20_000_000), core.terminalDefaults().scrollback_bytes);
    core.commitTerminalDefaultsUpdate(update);
    try std.testing.expectEqual(@as(u64, 2), core.terminalDefaultsRevision());
    try std.testing.expectEqual(@as(usize, 25_000_000), core.terminalDefaults().scrollback_bytes);
}

test "HostState rejects changed defaults at revision exhaustion without mutation" {
    var core = subject.HostCore.init(
        std.testing.allocator,
        subject.HostEpoch.fromBytes([_]u8{0x42} ** 16),
    );
    defer core.deinit();

    subject.testing.setTerminalDefaultsRevision(&core, std.math.maxInt(u64));
    const before = core.terminalDefaults();
    const replay = try core.prepareTerminalDefaultsUpdate(before);
    try std.testing.expectEqual(std.math.maxInt(u64), replay.revision);
    try std.testing.expectError(
        error.RevisionOverflow,
        core.prepareTerminalDefaultsUpdate(.{ .scrollback_bytes = before.scrollback_bytes + 1 }),
    );
    try std.testing.expectEqualDeep(before, core.terminalDefaults());
    try std.testing.expectEqual(std.math.maxInt(u64), core.terminalDefaultsRevision());
}
