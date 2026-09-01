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
const interactionFailure = host_failure.fromInteraction;
const prepareControlResponse = host_response.prepareControl;
const prepareOwnedResponse = host_response.prepareOwned;
const responseDelivery = host_response.responseDelivery;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    success_response: host_response.SuccessResponse,
    connection_id: core_module.ConnectionId,
    negotiated_row_window_bytes: usize,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    var handler: Handler = .{ .context = context };
    switch (operation) {
        .read_rows => {
            const success = try host_response.requireRowWindowSuccess(success_response);
            try handler.handleReadRows(
                success,
                connection_id,
                negotiated_row_window_bytes,
                envelope,
                correlation_id,
                outgoing,
            );
        },
        else => {
            const success = try host_response.requireControlJsonSuccess(success_response);
            switch (operation) {
                .rows_applied => try handler.handleRowsApplied(success, connection_id, envelope, correlation_id, outgoing),
                .set_history_demand => try handler.handleHistoryDemand(
                    success,
                    connection_id,
                    envelope,
                    correlation_id,
                    outgoing,
                ),
                .history_observed => try handler.handleHistoryObserved(
                    success,
                    connection_id,
                    envelope,
                    correlation_id,
                    outgoing,
                ),
                else => unreachable,
            }
        },
    }
}

const Handler = struct {
    context: *Context,

    fn handleReadRows(
        self: *Handler,
        success: host_response.RowWindowSuccess,
        connection_id: core_module.ConnectionId,
        negotiated_bytes: usize,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeReadRows(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "ReadRows payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireHistoryAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        if (request.maximum_bytes > negotiated_bytes) {
            try self.context.addFailure(outgoing, correlation_id, "limit-exceeded", "ReadRows exceeds negotiated rowWindowBytes", false);
            return;
        }
        var response = try prepareOwnedResponse(
            outgoing,
            request.maximum_bytes,
            responseDelivery(correlation_id, self.context.hostEpoch()),
        );
        defer response.deinit();
        var rows = interaction.readRowsOwned(
            outgoing.ownedPayloadAllocator(),
            host_outgoing.owned_payload_prefix_bytes,
            .{
                .detail = request.detail,
                .generation = request.generation,
                .host_epoch = self.context.hostEpoch(),
                .maximum_bytes = request.maximum_bytes,
                .origin = request.origin,
                .purpose = request.purpose,
                .request_sequence = request.request_sequence,
                .revision = request.revision,
                .revision_policy = request.revision_policy,
                .row_count = request.row_count,
                .theme_revision = interaction.theme_revision,
            },
        ) catch |err| {
            response.commitFailure(interactionFailure(err, "ReadRows failed"));
            return;
        };
        defer if (rows.allocation.len != 0) {
            outgoing.ownedPayloadAllocator().free(rows.allocation);
        };
        _ = response.commitRowWindowSuccess(
            success,
            rows.allocation,
            rows.encoded.bytes,
        );
        rows.allocation = &.{};
        rows.encoded.bytes = &.{};
    }

    fn handleRowsApplied(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeRowsApplied(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "RowsApplied payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireHistoryAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        interaction.rowsApplied(
            request.request_sequence,
            request.revision,
            request.generation,
        ) catch |err| {
            response.commitFailure(interactionFailure(err, "RowsApplied failed"));
            return;
        };
        var output = response.writer();
        control_encode.writeRowsApplied(&output, request) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }

    fn handleHistoryDemand(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeHistoryDemand(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SetHistoryDemand payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireHistoryAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        interaction.setHistoryDemand(request.demand) catch |err| {
            response.commitFailure(interactionFailure(err, "SetHistoryDemand failed"));
            return;
        };
        var output = response.writer();
        control_encode.writeHistoryDemandAccepted(&output, request) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }

    fn handleHistoryObserved(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeHistoryObserved(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "HistoryObserved payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireHistoryAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        interaction.historyObserved(request.sequence, request.revision, request.generation) catch |err| {
            response.commitFailure(interactionFailure(err, "HistoryObserved failed"));
            return;
        };
        var output = response.writer();
        control_encode.writeHistoryObserved(&output, request) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }
};
