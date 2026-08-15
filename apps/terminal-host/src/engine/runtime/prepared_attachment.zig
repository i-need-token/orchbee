const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const PreparedAttachmentState = enum {
    pending,
    consumed,
};

/// Holds the terminal-state lock and exactly the input/history capacity needed
/// for a new attachment until Core completes all fallible admission work.
/// Committing this value is allocation-free, so Core and runtime-derived
/// attachment state become observable as one Host transition.
pub fn Type(comptime Runtime: type) type {
    return struct {
        attachment_id: contracts.AttachmentId,
        bell_sequence: u64,
        history_reserved: bool,
        input_state_reserved: bool,
        runtime: *Runtime,
        state: PreparedAttachmentState = .pending,

        const Self = @This();

        fn runtimeState(self: *const Self) *state.State {
            return @ptrCast(@alignCast(self.runtime));
        }

        pub fn bellSequence(self: *const Self) u64 {
            std.debug.assert(self.state == .pending);
            return self.bell_sequence;
        }

        pub fn commit(self: *Self) void {
            std.debug.assert(self.state == .pending);
            const runtime = self.runtimeState();
            if (self.history_reserved) {
                runtime.history.commitReservedAttachment(self.attachment_id);
            }
            if (self.input_state_reserved) {
                runtime.input_mutex.lock();
                runtime.input_ownership.commitAttachment(self.attachment_id);
                runtime.input_mutex.unlock();
            }
            runtime.terminal_mutex.unlock();
            self.state = .consumed;
        }

        pub fn deinit(self: *Self) void {
            if (self.state == .pending) self.runtimeState().terminal_mutex.unlock();
            self.* = undefined;
        }
    };
}

pub const Internal = Type(state.State);
