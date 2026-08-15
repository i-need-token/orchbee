const std = @import("std");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("../failure.zig");
const host_outgoing = @import("../outgoing.zig");
const host_request_context = @import("../request_context.zig");
const host_response = @import("../response.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const terminal_configuration = model.configuration;

const Context = host_request_context.Context;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

pub fn handleTerminate(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeTerminalTarget(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "TerminateTerminal payload is invalid",
        );
    defer decoded.deinit();
    const terminal_id = decoded.value;
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    _ = self.context.runtime_aggregate.requestTermination(
        connection_id,
        terminal_id,
        .graceful,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.TerminalRuntimeMissing => unreachable,
            error.TerminationDispatchFailed => .{ .code = "termination-failed", .message = "platform termination request failed", .retryable = true },
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    var output = response.writer();
    control_encode.writeTerminalAcknowledged(&output, terminal_id) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}

pub fn handleForceTerminate(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeTerminalTarget(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "ForceTerminateTerminal payload is invalid",
        );
    defer decoded.deinit();
    const terminal_id = decoded.value;
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    _ = self.context.runtime_aggregate.requestTermination(
        connection_id,
        terminal_id,
        .force,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.TerminalRuntimeMissing => unreachable,
            error.TerminationDispatchFailed => .{ .code = "termination-failed", .message = "platform force-termination request failed", .retryable = true },
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    var output = response.writer();
    control_encode.writeTerminalAcknowledged(&output, terminal_id) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}

pub fn handleDispose(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeTerminalTarget(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "DisposeTerminal payload is invalid",
        );
    defer decoded.deinit();
    const terminal_id = decoded.value;
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    const disposal = self.context.runtime_aggregate.disposeTerminal(
        connection_id,
        terminal_id,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.TerminalRuntimeMissing => unreachable,
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    switch (disposal) {
        .transition => {},
        .cleanup_blocked => |failure| {
            response.commitFailure(host_failure.fromCleanup(failure));
            return;
        },
    }
    var output = response.writer();
    control_encode.writeTerminalAcknowledged(&output, terminal_id) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}
