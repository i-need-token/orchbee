const builtin = @import("builtin");

pub const process = @import("process.zig");

pub const testing = if (builtin.is_test) struct {
    pub const pty = @import("pty.zig");
    pub const windows_api = @import("windows_api.zig");
} else struct {};
