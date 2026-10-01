const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Context = host_request_context.Context;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const coreError = host_failure.fromCore;
const prepareControlResponse = host_response.prepareControl;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    success_response: host_response.SuccessResponse,
    connection_id: core_module.ConnectionId,
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
        .resize_terminal => try handler.handleResize(connection_id, envelope, correlation_id, outgoing),
        .clear_terminal => try handler.handleTerminalControl(
            connection_id,
            .clear,
            envelope,
            correlation_id,
            outgoing,
        ),
        .reset_terminal => try handler.handleTerminalControl(
            connection_id,
            .reset,
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
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
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
        _ = response.commitControlSuccess(self.success, &output);
    }

    fn handleTerminalControl(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        operation: terminal_runtime.TerminalControlOperation,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var attachment_id: core_module.AttachmentId = undefined;
        var selected_operation = operation;
        if (operation == .clear) {
            var decoded = control_decode.decodeClearTerminal(self.context.allocator, envelope.payload) catch |err|
                return self.context.rejectControlDecode(outgoing, correlation_id, err, "ClearTerminal payload is invalid");
            defer decoded.deinit();
            attachment_id = decoded.value.attachment_id;
            if (decoded.value.preserve_prompt) selected_operation = .clear_prompt;
        } else {
            var decoded = control_decode.decodeTerminalControl(self.context.allocator, envelope.payload) catch |err|
                return self.context.rejectControlDecode(outgoing, correlation_id, err, "ResetTerminal payload is invalid");
            defer decoded.deinit();
            attachment_id = decoded.value;
        }
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        const update = self.context.runtime_aggregate.applyTerminalControl(
            connection_id,
            attachment_id,
            selected_operation,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.RuntimeStateDiverged => host_failure.runtime_state_diverged,
                error.TerminalRuntimeMissing => unreachable,
                error.TerminalClearAlternateScreen => .{ .code = "terminal-clear-alternate-screen", .message = "prompt-preserving clear requires the primary screen", .retryable = false },
                else => coreError(@errorCast(err)),
            });
            return;
        };
        var output = response.writer();
        control_encode.writeTerminalControlAccepted(&output, update) catch unreachable;
        _ = response.commitControlSuccess(self.success, &output);
    }
};
