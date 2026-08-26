const std = @import("std");

const builtin = @import("builtin");
const thread_sync = @import("thread_sync.zig");

/// Resolves the process temporary directory to an owned canonical path. Kitty
/// temporary-file validation borrows this slice for the Terminal lifetime, so
/// callers must retain it until after `Terminal.deinit`.
pub fn resolveOwned(allocator: std.mem.Allocator) ![:0]u8 {
    const candidate = candidate: {
        const names = if (builtin.os.tag == .windows)
            [_][*:0]const u8{ "TEMP", "TMP" }
        else
            [_][*:0]const u8{ "TMPDIR", "TMP" };
        for (names) |name| {
            const value = std.c.getenv(name) orelse continue;
            const path = std.mem.span(value);
            if (path.len > 0) break :candidate path;
        }
        break :candidate if (builtin.os.tag == .windows) "C:\\Windows\\Temp" else "/tmp";
    };
    return std.Io.Dir.cwd().realPathFileAlloc(thread_sync.io(), candidate, allocator);
}
