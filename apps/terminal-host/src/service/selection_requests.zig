const std = @import("std");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_interaction = @import("interaction.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Context = host_request_context.Context;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const eventDelivery = host_response.eventDelivery;
const interactionFailure = host_failure.fromInteraction;
const prepareControlResponse = host_response.prepareControl;
const prepareResponse = host_response.prepare;
const requireAppended = host_response.requireAppended;
const responseDelivery = host_response.responseDelivery;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    success_response: host_response.SuccessResponse,
    connection_id: core_module.ConnectionId,
    trace_interactions: bool,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    const success = try host_response.requireControlJsonSuccess(success_response);
    var handler: Handler = .{ .context = context, .trace_interactions = trace_interactions };
    var selection_export_handoff: ?host_interaction.SelectionExportHandoff = null;
    defer if (selection_export_handoff) |*handoff| handoff.deinit();

    switch (operation) {
        .selection_gesture => try handler.handleSelectionGesture(
            success,
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        .selection_command => try handler.handleSelectionCommand(
            success,
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        .selection_export => try handler.handleSelectionExport(
            success,
            connection_id,
            envelope,
            correlation_id,
            outgoing,
            &selection_export_handoff,
        ),
        .selection_export_chunk_applied => try handler.handleSelectionExportChunkApplied(
            success,
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        .selection_export_cancel => try handler.handleSelectionExportCancel(
            success,
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        else => unreachable,
    }
    if (selection_export_handoff) |*handoff| handoff.commit();
}

const Handler = struct {
    context: *Context,
    trace_interactions: bool,

    fn handleSelectionGesture(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeSelectionGesture(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SelectionGesture payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireSelectionAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        const snapshot = interaction.applySelectionGesture(.{
            .event = request.event,
            .generation = request.generation,
        }) catch |err| {
            response.commitFailure(interactionFailure(err, "SelectionGesture failed"));
            return;
        };
        var output = response.writer();
        control_encode.writeSelectionChanged(
            &output,
            request.attachment_id,
            snapshot,
            snapshot.previewText(),
        ) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }

    fn handleSelectionCommand(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeSelectionCommand(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SelectionCommand payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireSelectionAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        const snapshot = interaction.applySelectionCommand(.{
            .command = request.command,
            .generation = request.generation,
            .revision = request.revision,
        }) catch |err| {
            response.commitFailure(interactionFailure(err, "SelectionCommand failed"));
            return;
        };
        var output = response.writer();
        control_encode.writeSelectionChanged(
            &output,
            request.attachment_id,
            snapshot,
            snapshot.previewText(),
        ) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }

    fn handleSelectionExport(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
        handoff: *?host_interaction.SelectionExportHandoff,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeSelectionExport(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SelectionExport payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireSelectionAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        handoff.* = interaction.startSelectionExport(
            request.export_sequence,
            request.format,
        ) catch |err| {
            try self.context.addInteractionFailure(outgoing, correlation_id, err, "SelectionExport failed");
            return;
        };
        // Keep the replacement export provisional until every record caused by
        // this request has been built. A later allocator failure must not leave
        // an unannounced export live.
        const result = handoff.*.?.result();
        for (result.cancellations[0..result.cancellation_count]) |cancellation| {
            _ = try requireAppended(try host_interaction.appendSelectionExportFinished(
                self.context.allocator,
                outgoing,
                .{
                    .attachment_id = cancellation.attachment_id,
                    .byte_length = cancellation.byte_length,
                    .export_sequence = cancellation.export_sequence,
                },
                .cancelled,
                eventDelivery(self.context.hostEpoch()),
            ));
        }
        const started = result.started;
        const payload = try control_encode.selectionExportStarted(
            self.context.allocator,
            request.attachment_id,
            started,
        );
        defer self.context.allocator.free(payload);
        _ = try host_response.appendControlSuccess(
            outgoing,
            success,
            payload,
            responseDelivery(correlation_id, self.context.hostEpoch()),
        );
        if (self.trace_interactions) {
            std.debug.print(
                "{{\"byteLength\":{d},\"lockHoldNanoseconds\":{d},\"type\":\"orchbee.terminal-host.selection-export\"}}\n",
                .{ started.byte_length, result.lock_hold_nanoseconds },
            );
        }
    }

    fn handleSelectionExportChunkApplied(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeSelectionExportChunkApplied(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SelectionExportChunkApplied payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireSelectionAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        var finished_event = try prepareResponse(
            outgoing,
            protocol.Limits.control_json_bytes,
            eventDelivery(self.context.hostEpoch()),
        );
        defer finished_event.deinit();
        const finished = interaction.selectionExportChunkApplied(
            request.export_sequence,
            request.chunk_sequence,
        ) catch |err| {
            response.commitFailure(interactionFailure(err, "SelectionExportChunkApplied failed"));
            return;
        };
        var response_output = response.writer();
        control_encode.writeSelectionExportChunkApplied(
            &response_output,
            request,
        ) catch unreachable;
        _ = response.commitControlSuccess(success, &response_output);
        if (finished) |value| {
            var output = finished_event.writer();
            control_encode.writeSelectionExportFinished(
                &output,
                value,
                host_interaction.SelectionExportStatus.completed,
            ) catch unreachable;
            _ = finished_event.commitControl(MessageKind.selection_export_finished, &output);
        }
    }

    fn handleSelectionExportCancel(
        self: *Handler,
        success: host_response.ControlJsonSuccess,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeSelectionExportCancel(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(outgoing, correlation_id, err, "SelectionExportCancel payload is invalid");
        defer decoded.deinit();
        const request = decoded.value;
        const interaction = try self.context.requireSelectionAccess(
            connection_id,
            request.attachment_id,
            correlation_id,
            outgoing,
        ) orelse return;
        var response = try prepareControlResponse(outgoing, correlation_id, self.context.hostEpoch());
        defer response.deinit();
        const finished = interaction.cancelSelectionExport(request.export_sequence) catch |err| {
            response.commitFailure(interactionFailure(err, "SelectionExportCancel failed"));
            return;
        };
        var output = response.writer();
        control_encode.writeSelectionExportFinished(
            &output,
            finished,
            host_interaction.SelectionExportStatus.cancelled,
        ) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }
};
