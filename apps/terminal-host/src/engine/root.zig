const builtin = @import("builtin");

pub const decodeGraphicsPng = @import("png_decoder.zig").decodeGraphicsPng;
pub const history = @import("history/root.zig");
pub const launch = @import("launch.zig");
pub const profile = @import("profile.zig");
pub const runtime = @import("runtime/root.zig");

pub const testing = if (builtin.is_test) struct {
    pub const input_ownership = @import("input_ownership.zig");
    pub const input_queue = @import("input_queue.zig");
    pub const png_decoder = @import("png_decoder.zig");
    pub const program_status = @import("program_status.zig");
    pub const projection = @import("projection/root.zig");
    pub const runtime_completion = @import("runtime_completion.zig");
    pub const runtime_containment = @import("runtime_containment.zig");
    pub const runtime_launch = @import("runtime_launch.zig");
    pub const runtime_startup = @import("runtime_startup.zig");
    pub const selection = @import("selection.zig");
    pub const selection_export = @import("selection_export.zig");
    pub const terminal_stream = @import("terminal_stream.zig");
} else struct {};
