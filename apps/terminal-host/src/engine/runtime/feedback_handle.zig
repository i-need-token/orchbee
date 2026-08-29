const std = @import("std");
const contracts = @import("contracts.zig");
const feedback_ops = @import("feedback.zig");
const state = @import("state.zig");

pub fn Type(comptime Runtime: type) type {
    return struct {
        pub fn prepareClipboardWriteRequest(
            self: *Runtime,
        ) !?contracts.PreparedClipboardWriteRequest {
            return feedback_ops.prepareClipboardWriteRequest(stateFrom(self));
        }

        pub fn rollbackClipboardWriteRequest(
            self: *Runtime,
            prepared: contracts.PreparedClipboardWriteRequest,
        ) void {
            return feedback_ops.rollbackClipboardWriteRequest(stateFrom(self), prepared);
        }

        pub fn commitClipboardWriteRequest(
            self: *Runtime,
            prepared: contracts.PreparedClipboardWriteRequest,
        ) void {
            return feedback_ops.commitClipboardWriteRequest(stateFrom(self), prepared);
        }

        pub fn prepareApplicationSignal(
            self: *Runtime,
            allocator: std.mem.Allocator,
        ) !?contracts.ApplicationSignalEvent {
            return feedback_ops.prepareApplicationSignal(stateFrom(self), allocator);
        }

        pub fn commitApplicationSignal(
            self: *Runtime,
            event: contracts.ApplicationSignalEvent,
        ) void {
            return feedback_ops.commitApplicationSignal(stateFrom(self), event);
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }
    };
}
