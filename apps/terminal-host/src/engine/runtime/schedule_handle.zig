const projection_ops = @import("projection.zig");
const state = @import("state.zig");

/// What the projection scheduler reads and writes between captures: its deadline and whether a
/// capture is due.
pub fn Type(comptime Runtime: type) type {
    return struct {
        pub fn projectionPaintRemaining(self: *Runtime) ?u64 {
            return projection_ops.projectionPaintRemaining(stateFrom(self));
        }

        pub fn projectionNeedsCapture(self: *Runtime) bool {
            return projection_ops.projectionNeedsCapture(stateFrom(self));
        }

        pub fn nextProjectionAt(self: *const Runtime) i128 {
            return projection_ops.nextProjectionAt(stateFromConst(self));
        }

        pub fn setNextProjectionAt(self: *Runtime, value: i128) void {
            return projection_ops.setNextProjectionAt(stateFrom(self), value);
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }

        fn stateFromConst(runtime: *const Runtime) *const state.State {
            return @ptrCast(@alignCast(runtime));
        }
    };
}
