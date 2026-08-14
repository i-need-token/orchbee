const builtin = @import("builtin");

pub const decode = @import("decode.zig");
pub const encode = @import("encode.zig");

pub const testing = if (builtin.is_test) struct {
    pub const wire = @import("wire.zig");
} else struct {};
