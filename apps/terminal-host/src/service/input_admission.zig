const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;

pub const AcceptedInput = struct {
    attachment_id: core_module.AttachmentId,
    byte_count: usize,
    capability_revision: u64,
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
        error.MissingAttachment, error.TerminalExited => .terminal_exited,
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
    attachment_admission_sequence: u64,
    connection_id: core_module.ConnectionId,
    core_state: *core_module.HostCore,
    runtime_input: terminal_runtime.PreparedInput,
    terminal_id: core_module.TerminalId,
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
        core_state: *core_module.HostCore,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        capability_revision: u64,
        attachment_admission_sequence: u64,
        terminal_id: core_module.TerminalId,
        runtime_input: terminal_runtime.PreparedInput,
    ) Preparation {
        return .{ .prepared = .{
            .accepted = .{
                .attachment_id = attachment_id,
                .byte_count = runtime_input.byteCount(),
                .capability_revision = capability_revision,
            },
            .attachment_admission_sequence = attachment_admission_sequence,
            .connection_id = connection_id,
            .core_state = core_state,
            .runtime_input = runtime_input,
            .terminal_id = terminal_id,
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
                const attachment = pending.core_state.authorizeInput(
                    pending.connection_id,
                    accepted_input.attachment_id,
                    accepted_input.capability_revision,
                ) catch |err| {
                    pending.runtime_input.deinit();
                    self.* = .consumed;
                    break :blk .{ .core_rejected = err };
                };
                if (attachment.admission_sequence != pending.attachment_admission_sequence or
                    !attachment.terminal_id.eql(pending.terminal_id))
                {
                    pending.runtime_input.deinit();
                    self.* = .consumed;
                    break :blk .{ .core_rejected = error.MissingResource };
                }
                const affects_close_activity =
                    pending.runtime_input.affectsCloseActivity();
                const byte_count = pending.runtime_input.commit() catch |err| {
                    self.* = .consumed;
                    break :blk .{ .runtime_rejected = fromRuntimeError(err) };
                };
                std.debug.assert(byte_count == accepted_input.byte_count);
                if (affects_close_activity) {
                    // Final Core reauthorization above proves this retained
                    // terminal and attachment incarnation. Host commands are
                    // single-owner, so the post-engine projection invalidation
                    // is allocation-free and infallible.
                    pending.core_state.invalidateTerminalPresentations(
                        pending.terminal_id,
                    ) catch unreachable;
                }
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
