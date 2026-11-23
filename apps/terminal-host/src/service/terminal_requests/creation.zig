const std = @import("std");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const creation_pipeline = @import("../creation_pipeline.zig");
const host_failure = @import("../failure.zig");
const host_outgoing = @import("../outgoing.zig");
const host_request_context = @import("../request_context.zig");
const host_response = @import("../response.zig");
const host_runtime_aggregate = @import("../runtime_aggregate/root.zig");
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
    // A drained Host still answers a replay of a retained creation, which adds no terminal.
    if (preparation.requiresEffect() and self.context.terminal_admission == .closed) {
        try self.context.addFailure(
            outgoing,
            correlation_id,
            "host-draining",
            "Terminal Host is draining and accepts no new terminals",
            false,
        );
        return;
    }
    const identity = preparation.identity();
    const payload = try control_encode.terminalIdentity(
        self.context.allocator,
        identity.creation_id,
        self.context.hostEpoch(),
        identity.terminal_id,
    );
    defer self.context.allocator.free(payload);
    const delivery = host_response.responseDelivery(correlation_id, self.context.hostEpoch());
    if (preparation.requiresEffect() and preparation.startsImmediately()) {
        return beginBuild(self, connection_id, correlation_id, payload, payload, .{ .create = &preparation }, outgoing);
    }
    const response_token = try host_response.appendControlSuccess(outgoing, self.success, payload, delivery);
    if (!preparation.requiresEffect()) return;
    _ = self.context.runtime_aggregate.commitAwaitingStartTerminal(&preparation) catch {
        outgoing.rollback(response_token) catch unreachable;
        try self.context.addFailure(
            outgoing,
            correlation_id,
            "resource-exhausted",
            "Host could not allocate terminal runtime state",
            true,
        );
    };
}

pub fn handleStart(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeStartTerminal(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "StartTerminal payload is invalid",
        );
    defer decoded.deinit();

    var preparation = self.context.runtime_aggregate.prepareStartTerminal(
        connection_id,
        decoded.value.terminal_id,
        decoded.value.geometry,
    ) catch |err| {
        try self.context.addCoreFailure(outgoing, correlation_id, @errorCast(err));
        return;
    };
    const outcome = preparation.outcome();
    const payload = try control_encode.terminalStartAccepted(
        self.context.allocator,
        self.context.hostEpoch(),
        outcome,
    );
    defer self.context.allocator.free(payload);
    if (!preparation.requiresEffect()) {
        _ = try host_response.appendControlSuccess(
            outgoing,
            self.success,
            payload,
            host_response.responseDelivery(correlation_id, self.context.hostEpoch()),
        );
        return;
    }
    const started_payload = try control_encode.terminalIdentity(
        self.context.allocator,
        outcome.identity.creation_id,
        self.context.hostEpoch(),
        outcome.identity.terminal_id,
    );
    defer self.context.allocator.free(started_payload);
    return beginBuild(self, connection_id, correlation_id, payload, started_payload, .{ .start = &preparation }, outgoing);
}

const BuildPreparation = union(enum) {
    create: *host_runtime_aggregate.CreatePreparation,
    start: *host_runtime_aggregate.StartPreparation,
};

/// Hands a creation to the background build. Its response and `TerminalStarted` are encoded now
/// and sent when the creation settles; the request's own batch stays empty unless the build
/// cannot begin, which answers it here.
fn beginBuild(
    self: anytype,
    connection_id: core_module.ConnectionId,
    correlation_id: WireIdentifier,
    response_payload: []const u8,
    started_payload: []const u8,
    preparation: BuildPreparation,
    outgoing: *Outgoing,
) !void {
    var reply = Outgoing.init(self.context.allocator);
    var reply_owned = true;
    defer if (reply_owned) reply.deinit();
    _ = try host_response.appendControlSuccess(
        &reply,
        self.success,
        response_payload,
        host_response.responseDelivery(correlation_id, self.context.hostEpoch()),
    );
    _ = try host_response.appendRequired(
        &reply,
        MessageKind.terminal_started,
        .control_json,
        started_payload,
        host_response.eventDelivery(self.context.hostEpoch()),
    );
    const request: creation_pipeline.Request = .{
        .connection_id = connection_id,
        .correlation_id = correlation_id,
        .host_epoch = self.context.hostEpoch(),
        .reply = reply,
        .keepers = if (self.context.keepers) |keepers| &keepers.attachments else null,
    };
    const begun = switch (preparation) {
        .create => |create| self.context.creations.beginCreate(self.context.runtime_aggregate, request, create),
        .start => |start| self.context.creations.beginStart(self.context.runtime_aggregate, request, start),
    };
    begun catch |err| return switch (err) {
        error.TerminalRuntimeMissing => self.context.addFailure(
            outgoing,
            correlation_id,
            "internal-error",
            "deferred terminal launch state is missing",
            false,
        ),
        error.CreationInProgress,
        error.CreatorUnavailable,
        error.OutOfMemory,
        error.RuntimeAllocationOutOfMemory,
        => self.context.addFailure(
            outgoing,
            correlation_id,
            "resource-exhausted",
            "Host could not begin building the terminal",
            true,
        ),
    };
    reply_owned = false;
}
