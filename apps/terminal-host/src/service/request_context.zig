const std = @import("std");

const control_decode = @import("terminal-host-control").decode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

pub const ControlDecodeFailure = struct {
    code: []const u8,
    kind: Kind,
    retryable: bool,

    pub const Kind = enum {
        inspection_resource,
        invalid_identifier,
        invalid_payload,
    };
};

pub fn classifyControlDecodeError(
    err: control_decode.DecodeError,
) error{OutOfMemory}!ControlDecodeFailure {
    return switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        error.CommandInspectionOutOfMemory => .{
            .code = "resource-exhausted",
            .kind = .inspection_resource,
            .retryable = true,
        },
        error.InvalidIdentifier => .{
            .code = "invalid-identifier",
            .kind = .invalid_identifier,
            .retryable = false,
        },
        error.InvalidUtf8 => .{
            .code = "invalid-utf8",
            .kind = .invalid_payload,
            .retryable = false,
        },
        error.InvalidJson => .{
            .code = "invalid-json",
            .kind = .invalid_payload,
            .retryable = false,
        },
        error.InvalidField => .{
            .code = "invalid-field",
            .kind = .invalid_payload,
            .retryable = false,
        },
    };
}

/// The complete authority available to one post-handshake request. Request
/// families share this context instead of importing HostService or reaching
/// into connection lifecycle state.
pub const Context = struct {
    allocator: std.mem.Allocator,
    runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,

    pub fn hostEpoch(self: *const Context) core_module.HostEpoch {
        return self.runtime_aggregate.hostEpoch();
    }

    pub fn requireReadableInteraction(
        self: *Context,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !?host_runtime_aggregate.ReadableAttachment {
        return self.runtime_aggregate.readableAttachment(connection_id, attachment_id) catch |err| {
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
        const failure = try classifyControlDecodeError(err);
        const message = switch (failure.kind) {
            .inspection_resource => "Host could not inspect the control command",
            .invalid_identifier => "control payload contains an invalid identifier",
            .invalid_payload => invalid_message,
        };
        return self.addFailure(
            outgoing,
            correlation_id,
            failure.code,
            message,
            failure.retryable,
        );
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
            host_response.responseDelivery(correlation_id, self.hostEpoch()),
        );
    }
};
