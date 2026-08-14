const std = @import("std");

const subject = @import("terminal-host-engine").testing.runtime_completion;

test "runtime completion classification preserves its first terminal outcome" {
    var normal: subject.Classification = .{};
    try expectState(.pending, normal.load());
    try std.testing.expect(normal.markNormal());
    try expectState(.normal, normal.load());

    normal.markFailure(.pty_read);
    try expectState(.normal, normal.load());
    try std.testing.expect(!normal.markNormal());

    var failed: subject.Classification = .{};
    failed.markFailure(.pty_read);
    failed.markFailure(.pty_write);
    switch (failed.load()) {
        .failed => |failure| try std.testing.expectEqual(subject.FailureKind.pty_read, failure),
        .pending, .normal => return error.ExpectedFirstFailure,
    }
}

fn expectState(expected: std.meta.Tag(subject.State), actual: subject.State) !void {
    try std.testing.expectEqual(expected, std.meta.activeTag(actual));
}
