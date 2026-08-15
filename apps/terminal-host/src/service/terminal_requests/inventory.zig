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

pub fn handlePing(
    self: anytype,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireEmptyControl(envelope, correlation_id, outgoing)) return;
    _ = try host_response.appendControlSuccess(
        outgoing,
        self.success,
        control_encode.empty_control,
        host_response.responseDelivery(correlation_id, self.context.hostEpoch()),
    );
}

pub fn handleList(
    self: anytype,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeListTerminals(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "ListTerminals payload is invalid",
        );
    defer decoded.deinit();
    const request = decoded.value;
    const offset = if (request.cursor) |cursor| cursor.offset else 0;
    if (request.cursor) |cursor| {
        if (cursor.generation != self.context.runtime_aggregate.inventoryGeneration()) {
            try self.context.addFailure(
                outgoing,
                correlation_id,
                "stale-inventory-cursor",
                "terminal inventory membership changed",
                true,
            );
            return;
        }
    }
    if (offset > self.context.runtime_aggregate.terminalCount()) {
        try self.context.addFailure(
            outgoing,
            correlation_id,
            "invalid-field",
            "inventory offset exceeds terminal count",
            false,
        );
        return;
    }
    const inventory = self.context.runtime_aggregate.terminalInventory(
        offset,
        request.maximum_terminals,
    ) catch unreachable;
    const payload = control_encode.terminalList(
        self.context.allocator,
        inventory.host_epoch,
        inventory.generation,
        inventory.terminals,
        offset,
        request.maximum_terminals,
    ) catch |err| switch (err) {
        error.InventoryTerminalTooLarge => {
            try self.context.addFailure(
                outgoing,
                correlation_id,
                "limit-exceeded",
                "one terminal inventory summary exceeds the control payload limit",
                false,
            );
            return;
        },
        else => return err,
    };
    defer self.context.allocator.free(payload);
    _ = try host_response.appendControlSuccess(
        outgoing,
        self.success,
        payload,
        host_response.responseDelivery(correlation_id, self.context.hostEpoch()),
    );
}
