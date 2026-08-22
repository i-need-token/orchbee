const state = @import("state.zig");
/// Serializes the allocation-free frame handoff against activity invalidation
/// and failure publication. A successful guard retains projection_mutex then
/// terminal_mutex until the caller has committed or rolled back both the
/// outgoing record and the matching Core flight.
pub fn Type(comptime Runtime: type) type {
    return struct {
        runtime: *Runtime,

        const Self = @This();

        pub fn deinit(self: *Self) void {
            const runtime: *state.State = @ptrCast(@alignCast(self.runtime));
            runtime.terminal_mutex.unlock();
            runtime.projection_mutex.unlock();
            self.* = undefined;
        }
    };
}

pub const Internal = Type(state.State);
