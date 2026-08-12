const builtin = @import("builtin");

pub const history = @import("history.zig");
pub const launch = @import("launch.zig");
pub const profile = @import("profile.zig");
pub const runtime = @import("runtime.zig");

pub const testing = if (builtin.is_test) struct {
    pub const projection = @import("projection.zig");
    pub const terminal_stream = @import("terminal_stream.zig");
} else struct {};
