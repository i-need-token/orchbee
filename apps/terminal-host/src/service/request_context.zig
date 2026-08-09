const std = @import("std");

const control_decode = @import("terminal-host-control").decode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_interaction = @import("interaction.zig");
const host_outgoing = @import("outgoing.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

/// The complete authority available to one post-handshake request. Request
/// families share this context instead of importing HostService or reaching
/// into connection lifecycle state.
pub const Context = struct {
    allocator: std.mem.Allocator,
    runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,

    pub fn core(self: *Context) *core_module.HostCore {
        return self.runtime_aggregate.core();
    }

    pub fn requireReadableInteraction(
        self: *Context,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !?host_interaction.ReadableAttachment {
        const controller = host_interaction.Controller.init(self.runtime_aggregate);
        return controller.readableAttachment(connection_id, attachment_id) catch |err| {
            switch (err) {
                error.ReadableAttachmentUnavailable => try self.addFailure(
                    outgoing,
                    correlation_id,
                    "missing-resource",
                    "attachment has no readable history capability",
                    false,
                ),
                error.TerminalRuntimeMissing => unreachable,
            }
            return null;
        };
    }

    pub fn requirePreparedInput(
        self: *Context,
        preparation: *const host_runtime_aggregate.InputPreparation,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
        runtime_message: []const u8,
    ) !?host_runtime_aggregate.AcceptedInput {
        if (preparation.accepted()) |accepted| return accepted;
        switch (preparation.*) {
            .core_rejected => |err| try self.addCoreFailure(outgoing, correlation_id, err),
            .runtime_missing => unreachable,
            .runtime_rejected => |err| {
                const failure = host_failure.fromInput(err);
                try self.addFailure(
                    outgoing,
                    correlation_id,
                    failure.code,
                    runtime_message,
                    failure.retryable,
                );
            },
            .prepared, .consumed => unreachable,
        }
        return null;
    }

    pub fn requireControl(
        self: *Context,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !bool {
        if (envelope.payload_encoding == @intFromEnum(protocol.PayloadEncoding.control_json)) {
            return true;
        }
        try self.addFailure(
            outgoing,
            correlation_id,
            "invalid-field",
            "message requires control JSON",
            false,
        );
        return false;
    }

    pub fn requireEmptyControl(
        self: *Context,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !bool {
        if (!try self.requireControl(envelope, correlation_id, outgoing)) return false;
        var decoded = control_decode.decodeEmpty(self.allocator, envelope.payload) catch |err| {
            try self.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "message requires an empty control object",
            );
            return false;
        };
        decoded.deinit();
        return true;
    }

    pub fn rejectControlDecode(
        self: *Context,
        outgoing: *Outgoing,
        correlation_id: WireIdentifier,
        err: control_decode.DecodeError,
        invalid_message: []const u8,
    ) !void {
        return switch (err) {
            error.OutOfMemory => error.OutOfMemory,
            error.CommandInspectionOutOfMemory => self.addFailure(
                outgoing,
                correlation_id,
                "resource-exhausted",
                "Host could not inspect the control command",
                true,
            ),
            error.InvalidIdentifier => self.addFailure(
                outgoing,
                correlation_id,
                "invalid-identifier",
                "control payload contains an invalid identifier",
                false,
            ),
            error.InvalidControlJson, error.InvalidField => self.addFailure(
                outgoing,
                correlation_id,
                "invalid-field",
                invalid_message,
                false,
            ),
        };
    }

    pub fn addCoreFailure(
        self: *Context,
        outgoing: *Outgoing,
        correlation_id: WireIdentifier,
        err: host_failure.CoreFailure,
    ) !void {
        const mapped = host_failure.fromCore(err);
        try self.addFailure(
            outgoing,
            correlation_id,
            mapped.code,
            mapped.message,
            mapped.retryable,
        );
    }

    pub fn addInteractionFailure(
        self: *Context,
        outgoing: *Outgoing,
        correlation_id: WireIdentifier,
        err: host_failure.InteractionFailure,
        message: []const u8,
    ) !void {
        const mapped = host_failure.fromInteraction(err, message);
        try self.addFailure(
            outgoing,
            correlation_id,
            mapped.code,
            mapped.message,
            mapped.retryable,
        );
    }

    pub fn addFailure(
        self: *Context,
        outgoing: *Outgoing,
        correlation_id: WireIdentifier,
        code: []const u8,
        message: []const u8,
        retryable: bool,
    ) !void {
        try host_response.appendFailure(
            self.allocator,
            outgoing,
            .{ .code = code, .message = message, .retryable = retryable },
            host_response.responseDelivery(correlation_id, self.core().epoch),
        );
    }
};
