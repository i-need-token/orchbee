const builtin = @import("builtin");

pub const decode = @import("decode.zig");
pub const encode = @import("encode.zig");

pub const testing = if (builtin.is_test) struct {
    pub const decode_terminal = @import("decode_terminal.zig").testing;
    pub const wire = @import("wire.zig");
} else struct {};
