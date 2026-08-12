const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;

pub const AcceptedInput = struct {
    attachment_id: core_module.AttachmentId,
    byte_count: usize,
};

/// Closed semantic reasons why the engine can reject an input reservation or
/// its final publication. Raw dependency errors are classified immediately at
/// the runtime boundary and never escape through Host request handling.
pub const Rejection = enum {
    backpressure,
    encoding_failed,
    input_limit_exceeded,
    invalid_input,
    paste_mode_changed,
    resource_exhausted,
    sequence_exhausted,
    terminal_exited,
    terminal_runtime_unavailable,
};

pub fn fromRuntimeError(err: anyerror) Rejection {
    return switch (err) {
        error.InputBackpressure => .backpressure,
        error.OutOfMemory => .resource_exhausted,
        error.InputSequenceExhausted => .sequence_exhausted,
        error.TerminalExited => .terminal_exited,
        error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => .terminal_runtime_unavailable,
        error.InputLimitExceeded => .input_limit_exceeded,
        error.PasteModeChanged => .paste_mode_changed,
        error.InvalidKey, error.EmptyInput => .invalid_input,
        else => .encoding_failed,
    };
}

pub const Admission = union(enum) {
    accepted: AcceptedInput,
    core_rejected: core_module.CoreError,
    runtime_missing,
    runtime_rejected: Rejection,
};

const Pending = struct {
    accepted: AcceptedInput,
    runtime_input: terminal_runtime.PreparedInput,
};

/// Owns one provisional engine queue reservation until its exact success reply
/// has been prepared. Dropping a pending value releases the reservation;
/// committing consumes it exactly once.
pub const Preparation = union(enum) {
    prepared: Pending,
    core_rejected: core_module.CoreError,
    runtime_missing,
    runtime_rejected: Rejection,
    consumed,

    pub fn initPrepared(
        attachment_id: core_module.AttachmentId,
        runtime_input: terminal_runtime.PreparedInput,
    ) Preparation {
        return .{ .prepared = .{
            .accepted = .{
                .attachment_id = attachment_id,
                .byte_count = runtime_input.byteCount(),
            },
            .runtime_input = runtime_input,
        } };
    }

    pub fn initCoreRejected(err: core_module.CoreError) Preparation {
        return .{ .core_rejected = err };
    }

    pub fn initRuntimeMissing() Preparation {
        return .runtime_missing;
    }

    pub fn initRuntimeRejected(err: anyerror) Preparation {
        return .{ .runtime_rejected = fromRuntimeError(err) };
    }

    pub fn accepted(self: *const Preparation) ?AcceptedInput {
        return switch (self.*) {
            .prepared => |pending| pending.accepted,
            .core_rejected, .runtime_missing, .runtime_rejected, .consumed => null,
        };
    }

    pub fn commit(self: *Preparation) Admission {
        return switch (self.*) {
            .prepared => |*pending| blk: {
                const accepted_input = pending.accepted;
                const byte_count = pending.runtime_input.commit() catch |err| {
                    self.* = .consumed;
                    break :blk .{ .runtime_rejected = fromRuntimeError(err) };
                };
                std.debug.assert(byte_count == accepted_input.byte_count);
                self.* = .consumed;
                break :blk .{ .accepted = accepted_input };
            },
            .core_rejected => |err| blk: {
                self.* = .consumed;
                break :blk .{ .core_rejected = err };
            },
            .runtime_missing => blk: {
                self.* = .consumed;
                break :blk .runtime_missing;
            },
            .runtime_rejected => |rejection| blk: {
                self.* = .consumed;
                break :blk .{ .runtime_rejected = rejection };
            },
            .consumed => unreachable,
        };
    }

    pub fn deinit(self: *Preparation) void {
        switch (self.*) {
            .prepared => |*pending| pending.runtime_input.deinit(),
            .core_rejected, .runtime_missing, .runtime_rejected, .consumed => {},
        }
        self.* = .consumed;
    }
};
