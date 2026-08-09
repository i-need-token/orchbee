const std = @import("std");
const subject = @import("terminal-host-model").configuration;

const max_scrollback_bytes = subject.testing.scrollback_limit;
const valid = subject.valid;

test "scrollback configuration preserves zero and bounds the memory budget" {
    try std.testing.expect(valid(.{ .scrollback_bytes = 0 }));
    try std.testing.expect(valid(.{ .scrollback_bytes = max_scrollback_bytes }));
    try std.testing.expect(!valid(.{ .scrollback_bytes = max_scrollback_bytes + 1 }));
}
