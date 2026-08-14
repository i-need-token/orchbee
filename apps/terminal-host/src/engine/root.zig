const builtin = @import("builtin");

pub const history = @import("history.zig");
pub const launch = @import("launch.zig");
pub const profile = @import("profile.zig");
pub const runtime = @import("runtime.zig");

pub const testing = if (builtin.is_test) struct {
    pub const input_ownership = @import("input_ownership.zig");
    pub const input_queue = @import("input_queue.zig");
    pub const projection = @import("projection.zig");
    pub const runtime_completion = @import("runtime_completion.zig");
    pub const runtime_containment = @import("runtime_containment.zig");
    pub const runtime_launch = @import("runtime_launch.zig");
    pub const runtime_startup = @import("runtime_startup.zig");
    pub const selection = @import("selection.zig");
    pub const selection_export = @import("selection_export.zig");
    pub const terminal_stream = @import("terminal_stream.zig");
} else struct {};
