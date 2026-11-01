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
    success: host_response.ControlJsonSuccess,

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
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        _ = self.context.runtime_aggregate.frameApplied(
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
        _ = response.commitControlSuccess(self.success, &output);
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
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        _ = self.context.runtime_aggregate.needSnapshot(
            connection_id,
            request.attachment_id,
            request.sequence,
            request.reason == .decoder_rejected,
        ) catch |err| {
            response.commitFailure(coreError(err));
            return;
        };
        var output = response.writer();
        control_encode.writeNeedSnapshot(&output, request) catch unreachable;
        _ = response.commitControlSuccess(self.success, &output);
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
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
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
        _ = response.commitControlSuccess(self.success, &output);
    }
};
