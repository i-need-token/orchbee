const contracts = @import("contracts.zig");
const projection_ops = @import("projection.zig");
const state = @import("state.zig");

/// What the projection scheduler reads and writes between captures: its deadlines, whether a
/// capture is due, and when an activity change must publish.
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

        pub fn closeActivitySampleDue(self: *Runtime, now: i128) bool {
            return projection_ops.closeActivitySampleDue(stateFrom(self), now);
        }

        pub fn closeActivityPublication(
            self: *Runtime,
            now: i128,
            settle_ns: i128,
        ) contracts.CloseActivityPublication {
            return projection_ops.closeActivityPublication(stateFrom(self), now, settle_ns);
        }

        pub fn nextCloseActivitySampleAt(self: *const Runtime) i128 {
            return projection_ops.nextCloseActivitySampleAt(stateFromConst(self));
        }

        pub fn setNextCloseActivitySampleAt(self: *Runtime, value: i128) void {
            return projection_ops.setNextCloseActivitySampleAt(stateFrom(self), value);
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }

        fn stateFromConst(runtime: *const Runtime) *const state.State {
            return @ptrCast(@alignCast(runtime));
        }
    };
}
