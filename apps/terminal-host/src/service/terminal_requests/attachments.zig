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

pub fn handleAttach(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeAttachTerminal(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "AttachTerminal payload is invalid",
        );
    defer decoded.deinit();
    const request = decoded.value;
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    const attached = self.context.runtime_aggregate.attach(
        connection_id,
        request.attachment_id,
        request.terminal_id,
        request.capabilities,
        request.request_resize_lease,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.TerminalRuntimeMissing => unreachable,
            error.TerminalRuntimeUnavailable => host_failure.fromCore(error.InvalidPhase),
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    const attachment = attached.binding;
    var output = response.writer();
    control_encode.writeAttachmentCapabilities(&output, attachment) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}

pub fn handleDetach(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeDetachTerminal(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "DetachTerminal payload is invalid",
        );
    defer decoded.deinit();
    const attachment_id = decoded.value;
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    var lease_event = try host_response.prepare(
        outgoing,
        protocol.Limits.control_json_bytes,
        host_response.eventDelivery(self.context.hostEpoch()),
    );
    defer lease_event.deinit();
    const detached = self.context.runtime_aggregate.detach(connection_id, attachment_id) catch |err| {
        response.commitFailure(host_failure.fromCore(err));
        return;
    };
    if (detached.resize_lease_change) |change| {
        var event_output = lease_event.writer();
        control_encode.writeResizeLeaseChanged(&event_output, change) catch unreachable;
        _ = lease_event.commitControl(MessageKind.resize_lease_changed, &event_output);
    }
    var output = response.writer();
    control_encode.writeAttachmentAcknowledged(&output, attachment_id) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}

pub fn handleUpdateAttachmentInputCapability(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeUpdateAttachmentInputCapability(
        self.context.allocator,
        envelope.payload,
    ) catch |err| return self.context.rejectControlDecode(
        outgoing,
        correlation_id,
        err,
        "UpdateAttachmentInputCapability payload is invalid",
    );
    defer decoded.deinit();
    const request = decoded.value;
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    const attachment = self.context.runtime_aggregate.updateAttachmentInputCapability(
        connection_id,
        request.attachment_id,
        request.capability_revision,
        request.input,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.TerminalRuntimeMissing => unreachable,
            error.TerminalRuntimeUnavailable => host_failure.fromCore(error.InvalidPhase),
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    var output = response.writer();
    control_encode.writeAttachmentCapabilities(&output, attachment) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}

pub fn handleClaimResizeLease(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeClaimResizeLease(
        self.context.allocator,
        envelope.payload,
    ) catch |err| return self.context.rejectControlDecode(
        outgoing,
        correlation_id,
        err,
        "ClaimResizeLease payload is invalid",
    );
    defer decoded.deinit();
    var response = try host_response.prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    var revocation_event = host_response.prepare(
        outgoing,
        protocol.Limits.control_json_bytes,
        host_response.eventDelivery(self.context.hostEpoch()),
    ) catch |err| switch (err) {
        error.OutOfMemory => {
            response.commitFailure(host_failure.fromCore(error.OutOfMemory));
            return;
        },
        else => return err,
    };
    defer revocation_event.deinit();
    var grant_event = host_response.prepare(
        outgoing,
        protocol.Limits.control_json_bytes,
        host_response.eventDelivery(self.context.hostEpoch()),
    ) catch |err| switch (err) {
        error.OutOfMemory => {
            response.commitFailure(host_failure.fromCore(error.OutOfMemory));
            return;
        },
        else => return err,
    };
    defer grant_event.deinit();

    const claimed = self.context.runtime_aggregate.claimResizeLease(
        connection_id,
        decoded.value.attachment_id,
    ) catch |err| {
        response.commitFailure(host_failure.fromCore(err));
        return;
    };
    if (claimed.transition == .applied) {
        if (claimed.previous_owner) |previous_owner| {
            var event_output = revocation_event.writer();
            control_encode.writeResizeLeaseChanged(&event_output, .{
                .attachment_id = previous_owner,
                .resize_lease = false,
                .terminal_id = claimed.binding.terminal_id,
            }) catch unreachable;
            _ = revocation_event.commitControl(MessageKind.resize_lease_changed, &event_output);
        }
        var event_output = grant_event.writer();
        control_encode.writeResizeLeaseChanged(&event_output, .{
            .attachment_id = claimed.binding.attachment_id,
            .resize_lease = true,
            .terminal_id = claimed.binding.terminal_id,
        }) catch unreachable;
        _ = grant_event.commitControl(MessageKind.resize_lease_changed, &event_output);
    }
    var output = response.writer();
    control_encode.writeAttachmentCapabilities(&output, claimed.binding) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}
