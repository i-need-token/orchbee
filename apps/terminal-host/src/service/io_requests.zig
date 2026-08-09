const std = @import("std");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Context = host_request_context.Context;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const coreError = host_failure.fromCore;
const inputFailure = host_failure.fromInput;
const prepareControlResponse = host_response.prepareControl;
const protocolErrorCode = host_failure.protocolCode;
const semanticInputFailureMessage = host_failure.semanticInputMessage;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    connection_id: core_module.ConnectionId,
    negotiated_input_bytes: usize,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    var handler: Handler = .{ .context = context };
    switch (operation) {
        .write_input => try handler.handleInput(
            connection_id,
            negotiated_input_bytes,
            envelope,
            correlation_id,
            outgoing,
        ),
        .semantic_input => try handler.handleSemanticInput(
            connection_id,
            negotiated_input_bytes,
            envelope,
            correlation_id,
            outgoing,
        ),
        .resize_terminal => try handler.handleResize(connection_id, envelope, correlation_id, outgoing),
        .frame_applied => try handler.handleFrameApplied(connection_id, envelope, correlation_id, outgoing),
        .need_snapshot => try handler.handleNeedSnapshot(connection_id, envelope, correlation_id, outgoing),
        .set_projection_demand => try handler.handleProjectionDemand(
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        else => unreachable,
    }
}

const Handler = struct {
    context: *Context,

    fn handleInput(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        negotiated_bytes: usize,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.raw_bytes)) {
            try self.context.addFailure(outgoing, correlation_id, "invalid-field", "WriteInput requires raw bytes", false);
            return;
        }
        const input = protocol.validateWriteInput(envelope.payload) catch |err| {
            try self.context.addFailure(outgoing, correlation_id, protocolErrorCode(err), "WriteInput payload is invalid", false);
            return;
        };
        if (input.bytes.len > negotiated_bytes) {
            try self.context.addFailure(outgoing, correlation_id, "limit-exceeded", "WriteInput exceeds negotiated inputBytes", false);
            return;
        }
        const attachment_id = core_module.AttachmentId.fromBytes(input.attachment_id);
        var preparation = self.context.runtime_aggregate.prepareRawInput(
            connection_id,
            attachment_id,
            input.bytes,
        );
        defer preparation.deinit();
        const accepted = try self.context.requirePreparedInput(
            &preparation,
            correlation_id,
            outgoing,
            "input could not enter the bounded write queue",
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.core().epoch);
        defer response.deinit();
        var output = response.writer();
        control_encode.writeInputAccepted(&output, accepted) catch unreachable;
        switch (preparation.commit()) {
            .accepted => |committed| {
                std.debug.assert(committed.attachment_id.eql(accepted.attachment_id));
                std.debug.assert(committed.byte_count == accepted.byte_count);
                _ = response.commitControl(MessageKind.input_accepted, &output);
            },
            .runtime_rejected => |err| {
                const failure = inputFailure(err);
                response.commitFailure(.{
                    .code = failure.code,
                    .message = "input could not enter the bounded write queue",
                    .retryable = failure.retryable,
                });
            },
            .core_rejected, .runtime_missing => unreachable,
        }
    }

    fn handleSemanticInput(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        negotiated_bytes: usize,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.raw_bytes)) {
            try self.context.addFailure(outgoing, correlation_id, "invalid-field", "semantic input requires packed raw bytes", false);
            return;
        }
        const decoded = protocol.validateSemanticInput(envelope.kind, envelope.payload) catch |err| {
            try self.context.addFailure(
                outgoing,
                correlation_id,
                protocolErrorCode(err),
                semanticInputFailureMessage(envelope.kind),
                false,
            );
            return;
        };
        const attachment_id = core_module.AttachmentId.fromBytes(decoded.attachment_id);
        var preparation = self.context.runtime_aggregate.prepareSemanticInput(
            connection_id,
            attachment_id,
            decoded.input,
            negotiated_bytes,
        );
        defer preparation.deinit();
        const accepted = try self.context.requirePreparedInput(
            &preparation,
            correlation_id,
            outgoing,
            "semantic input could not enter the bounded write queue",
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.core().epoch);
        defer response.deinit();
        var output = response.writer();
        control_encode.writeInputAccepted(&output, accepted) catch unreachable;
        switch (preparation.commit()) {
            .accepted => |committed| {
                std.debug.assert(committed.attachment_id.eql(accepted.attachment_id));
                std.debug.assert(committed.byte_count == accepted.byte_count);
                _ = response.commitControl(MessageKind.input_accepted, &output);
            },
            .runtime_rejected => |err| {
                const failure = inputFailure(err);
                response.commitFailure(.{
                    .code = failure.code,
                    .message = "semantic input could not enter the bounded write queue",
                    .retryable = failure.retryable,
                });
            },
            .core_rejected, .runtime_missing => unreachable,
        }
    }

    fn handleFrameApplied(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeFrameApplied(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "FrameApplied payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.core().epoch);
        defer response.deinit();
        _ = self.context.core().frameApplied(
            connection_id,
            request.attachment_id,
            request.sequence,
            request.revision,
            request.generation,
        ) catch |err| {
            response.commitFailure(coreError(err));
            return;
        };
        var output = response.writer();
        control_encode.writeFrameApplied(&output, request) catch unreachable;
        _ = response.commitControl(MessageKind.frame_applied, &output);
    }

    fn handleNeedSnapshot(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeNeedSnapshot(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "NeedSnapshot payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.core().epoch);
        defer response.deinit();
        _ = self.context.core().needSnapshot(
            connection_id,
            request.attachment_id,
            request.sequence,
        ) catch |err| {
            response.commitFailure(coreError(err));
            return;
        };
        var output = response.writer();
        control_encode.writeNeedSnapshot(&output, request) catch unreachable;
        _ = response.commitControl(MessageKind.need_snapshot, &output);
    }

    fn handleProjectionDemand(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeProjectionDemand(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SetProjectionDemand payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.core().epoch);
        defer response.deinit();
        _ = self.context.runtime_aggregate.setProjectionDemand(
            connection_id,
            request.attachment_id,
            request.demand,
        ) catch |err| {
            response.commitFailure(coreError(err));
            return;
        };
        var output = response.writer();
        control_encode.writeProjectionDemandAccepted(
            &output,
            request.attachment_id,
            request.demand,
        ) catch unreachable;
        _ = response.commitControl(MessageKind.projection_demand_accepted, &output);
    }

    fn handleResize(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeResizeTerminal(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "ResizeTerminal payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.core().epoch);
        defer response.deinit();
        const resized = self.context.runtime_aggregate.resizeTerminal(
            connection_id,
            request.attachment_id,
            request.geometry,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.InvalidGeometry => .{ .code = "invalid-field", .message = "terminal dimensions are invalid", .retryable = false },
                error.MissingConnection, error.MissingResource, error.ResizeLeaseDenied => .{ .code = "resize-lease-denied", .message = "attachment does not own the resize lease", .retryable = false },
                error.RuntimeResizeFailed => .{ .code = "resize-failed", .message = "PTY resize failed", .retryable = true },
                error.RuntimeStateDiverged => host_failure.runtime_state_diverged,
                error.TerminalRevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal revision space is exhausted", .retryable = false },
                error.TerminalRuntimeMissing => unreachable,
                error.TerminalRuntimeUnavailable => coreError(error.InvalidPhase),
                error.UnsafeRuntimeAfterResizeFailure => .{ .code = "terminal-resize-failed", .message = "terminal state became unsafe after resize failure", .retryable = false },
                else => coreError(@errorCast(err)),
            });
            return;
        };
        var output = response.writer();
        control_encode.writeResizeAccepted(&output, resized) catch unreachable;
        _ = response.commitControl(MessageKind.resize_accepted, &output);
    }
};
