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

pub fn handleCreate(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeCreateTerminal(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "CreateTerminal payload is invalid",
        );
    defer decoded.deinit();

    var preparation = self.context.runtime_aggregate.prepareCreateTerminal(
        connection_id,
        decoded.value.creation_id,
        decoded.value.launch,
    ) catch |err| {
        switch (err) {
            error.LaunchPreparationOutOfMemory => try self.context.addFailure(
                outgoing,
                correlation_id,
                "resource-exhausted",
                "Host could not retain the terminal environment",
                true,
            ),
            else => try self.context.addCoreFailure(
                outgoing,
                correlation_id,
                @errorCast(err),
            ),
        }
        return;
    };
    defer preparation.deinit();
    const identity = preparation.identity();
    const payload = try control_encode.terminalIdentity(
        self.context.allocator,
        identity.creation_id,
        self.context.hostEpoch(),
        identity.terminal_id,
    );
    defer self.context.allocator.free(payload);
    const response_token = try host_response.appendControlSuccess(
        outgoing,
        self.success,
        payload,
        host_response.responseDelivery(correlation_id, self.context.hostEpoch()),
    );
    if (!preparation.requiresEffect()) return;
    const started_token = host_response.appendRequired(
        outgoing,
        MessageKind.terminal_started,
        .control_json,
        payload,
        host_response.eventDelivery(self.context.hostEpoch()),
    ) catch |err| {
        outgoing.rollback(response_token) catch unreachable;
        return err;
    };
    const creation = self.context.runtime_aggregate.commitPreparedTerminal(&preparation) catch |err| {
        outgoing.rollback(started_token) catch unreachable;
        outgoing.rollback(response_token) catch unreachable;
        switch (err) {
            error.RuntimeAllocationOutOfMemory => try self.context.addFailure(
                outgoing,
                correlation_id,
                "resource-exhausted",
                "Host could not allocate terminal runtime state",
                true,
            ),
            error.RuntimeSpawnFailed => try self.context.addFailure(
                outgoing,
                correlation_id,
                "spawn-failed",
                "PTY or child creation failed",
                true,
            ),
        }
        return;
    };
    std.debug.assert(creation.isCreated());
}
