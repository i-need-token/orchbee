const builtin = @import("builtin");

const posix = @import("local_endpoint_posix.zig");
const windows = @import("local_endpoint_windows.zig");

const implementation = switch (builtin.os.tag) {
    .freebsd, .linux, .macos => posix,
    .windows => windows,
    else => @compileError("terminal-host has no local endpoint helper for this target"),
};

pub const prepareRuntime = implementation.prepareRuntime;
pub const connect = implementation.connect;
