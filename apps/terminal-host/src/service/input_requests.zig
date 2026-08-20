const std = @import("std");

const control_encode = @import("terminal-host-control").encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Context = host_request_context.Context;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const inputFailure = host_failure.fromInput;
const prepareControlResponse = host_response.prepareControl;
const protocolErrorCode = host_failure.protocolCode;
const semanticInputFailureMessage = host_failure.semanticInputMessage;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    success_response: host_response.SuccessResponse,
    connection_id: core_module.ConnectionId,
    negotiated_input_bytes: usize,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    const success = try host_response.requireControlJsonSuccess(success_response);
    var handler: Handler = .{
        .context = context,
        .success = success,
    };
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
        else => unreachable,
    }
}

const Handler = struct {
    context: *Context,
    success: host_response.ControlJsonSuccess,

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
            input.capability_revision,
            input.bytes,
        );
        defer preparation.deinit();
        const accepted = try self.context.requirePreparedInput(
            &preparation,
            correlation_id,
            outgoing,
            "input could not enter the bounded write queue",
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        var output = response.writer();
        control_encode.writeInputAccepted(&output, accepted) catch unreachable;
        switch (preparation.commit()) {
            .accepted => |committed| {
                std.debug.assert(committed.attachment_id.eql(accepted.attachment_id));
                std.debug.assert(committed.byte_count == accepted.byte_count);
                std.debug.assert(committed.capability_revision == accepted.capability_revision);
                _ = response.commitControlSuccess(self.success, &output);
            },
            .runtime_rejected => |err| {
                const failure = inputFailure(err);
                response.commitFailure(.{
                    .code = failure.code,
                    .message = "input could not enter the bounded write queue",
                    .retryable = failure.retryable,
                });
            },
            .core_rejected => |err| response.commitFailure(host_failure.fromCore(err)),
            .runtime_missing => unreachable,
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
            decoded.capability_revision,
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
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        var output = response.writer();
        control_encode.writeInputAccepted(&output, accepted) catch unreachable;
        switch (preparation.commit()) {
            .accepted => |committed| {
                std.debug.assert(committed.attachment_id.eql(accepted.attachment_id));
                std.debug.assert(committed.byte_count == accepted.byte_count);
                std.debug.assert(committed.capability_revision == accepted.capability_revision);
                _ = response.commitControlSuccess(self.success, &output);
            },
            .runtime_rejected => |err| {
                const failure = inputFailure(err);
                response.commitFailure(.{
                    .code = failure.code,
                    .message = "semantic input could not enter the bounded write queue",
                    .retryable = failure.retryable,
                });
            },
            .core_rejected => |err| response.commitFailure(host_failure.fromCore(err)),
            .runtime_missing => unreachable,
        }
    }
};
