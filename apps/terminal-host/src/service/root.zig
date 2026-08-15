const builtin = @import("builtin");

pub const service = @import("service.zig");

pub const testing = if (builtin.is_test) struct {
    pub const input_admission = @import("input_admission.zig");
    pub const interaction = @import("interaction.zig");
    pub const outgoing = @import("outgoing.zig");
    pub const projection = @import("projection.zig");
    pub const response = @import("response.zig");
    pub const runtime_aggregate = @import("runtime_aggregate/root.zig");
} else struct {};
