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

pub fn handleSetTerminalDefaults(
    self: anytype,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeSetTerminalDefaults(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "terminal configuration is invalid",
        );
    defer decoded.deinit();
    const configuration = decoded.value;
    const update = self.context.runtime_aggregate.prepareTerminalDefaultsUpdate(configuration) catch |err| {
        switch (err) {
            error.RevisionOverflow => try self.context.addFailure(
                outgoing,
                correlation_id,
                "resource-exhausted",
                "terminal default revision space is exhausted",
                false,
            ),
            error.InvalidConfiguration => try self.context.addFailure(
                outgoing,
                correlation_id,
                "invalid-field",
                "terminal configuration is invalid",
                false,
            ),
            else => unreachable,
        }
        return;
    };
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    var output = response.writer();
    control_encode.writeTerminalDefaultsAccepted(
        &output,
        update.configuration.scrollback_bytes,
        update.revision,
    ) catch unreachable;
    self.context.runtime_aggregate.commitTerminalDefaultsUpdate(update);
    _ = response.commitControlSuccess(self.success, &output);
}

pub fn handleUpdateTerminalConfiguration(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeUpdateTerminalConfiguration(
        self.context.allocator,
        envelope.payload,
    ) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "UpdateTerminalConfiguration payload is invalid",
        );
    defer decoded.deinit();
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    const update = self.context.runtime_aggregate.updateTerminalConfiguration(
        connection_id,
        decoded.value.terminal_id,
        decoded.value.configuration,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.ConfigurationRevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal configuration revision space is exhausted", .retryable = false },
            error.InvalidConfiguration => .{ .code = "invalid-field", .message = "terminal configuration is invalid", .retryable = false },
            error.RuntimeStateDiverged => host_failure.runtime_state_diverged,
            error.TerminalRevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal revision space is exhausted", .retryable = false },
            error.TerminalRuntimeMissing => unreachable,
            error.TerminalRuntimeUnavailable => host_failure.fromCore(error.InvalidPhase),
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    var output = response.writer();
    control_encode.writeTerminalConfigurationAccepted(&output, update) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}
