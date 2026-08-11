const std = @import("std");

const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

pub const empty_control = "{}";
// Cross-language contract: search for `typed-failure-wire-limit` before
// changing either bound, then update every occurrence together.
const maximum_failure_code_bytes = 128;
const maximum_failure_message_bytes = 1024;

pub fn protocolRejected(
    allocator: std.mem.Allocator,
    client_major: u16,
    client_minor: u16,
    client_patch: u16,
    host_major: u16,
    host_minor: u16,
    host_patch: u16,
) ![]u8 {
    return std.fmt.allocPrint(
        allocator,
        "{{\"client\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"host\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"reason\":\"version-mismatch\"}}",
        .{ client_major, client_minor, client_patch, host_major, host_minor, host_patch },
    );
}

pub fn helloAccepted(
    allocator: std.mem.Allocator,
    limits: anytype,
    host_epoch: anytype,
    host_version: []const u8,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeHelloAccepted(&writer.writer, limits, host_epoch, host_version);
    return writer.toOwnedSlice();
}

pub fn writeHelloAccepted(
    writer: *std.Io.Writer,
    limits: anytype,
    host_epoch: anytype,
    host_version: []const u8,
) !void {
    var epoch_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"frameBytes\":{d},\"hostEpoch\":\"{s}\",\"hostVersion\":",
        .{
            limits.frame_bytes,
            identifierText(host_epoch, &epoch_buffer),
        },
    );
    try std.json.Stringify.value(host_version, .{}, writer);
    try writer.print(
        ",\"inputBytes\":{d},\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":{d},\"supportedOptionalFeatures\":[]}}",
        .{
            limits.input_bytes,
            protocol.current_protocol_major,
            protocol.current_protocol_minor,
            protocol.current_protocol_patch,
            limits.row_window_bytes,
        },
    );
}

pub fn typedFailure(
    allocator: std.mem.Allocator,
    code: []const u8,
    message: []const u8,
    retryable: bool,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTypedFailure(&writer.writer, code, message, retryable);
    return writer.toOwnedSlice();
}

pub fn writeTypedFailure(
    writer: *std.Io.Writer,
    code: []const u8,
    message: []const u8,
    retryable: bool,
) !void {
    if (code.len == 0 or code.len > maximum_failure_code_bytes or
        message.len == 0 or message.len > maximum_failure_message_bytes or
        !std.unicode.utf8ValidateSlice(code) or
        !std.unicode.utf8ValidateSlice(message))
    {
        return error.InvalidControlValue;
    }
    try writer.writeAll("{\"code\":");
    try std.json.Stringify.value(code, .{}, writer);
    try writer.writeAll(",\"message\":");
    try std.json.Stringify.value(message, .{}, writer);
    try writer.print(",\"retryable\":{}}}", .{retryable});
}

pub fn terminalIdentity(
    allocator: std.mem.Allocator,
    creation_id: anytype,
    host_epoch: anytype,
    terminal_id: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}}",
        .{
            identifierText(creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
}

pub fn terminalAcknowledged(allocator: std.mem.Allocator, terminal_id: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalAcknowledged(&writer.writer, terminal_id);
    return writer.toOwnedSlice();
}

pub fn writeTerminalAcknowledged(writer: *std.Io.Writer, terminal_id: anytype) !void {
    var terminal_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
}

pub fn terminalDefaultsAccepted(
    allocator: std.mem.Allocator,
    scrollback_bytes: usize,
    revision: u64,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalDefaultsAccepted(&writer.writer, scrollback_bytes, revision);
    return writer.toOwnedSlice();
}

pub fn writeTerminalDefaultsAccepted(
    writer: *std.Io.Writer,
    scrollback_bytes: usize,
    revision: u64,
) !void {
    try writer.print(
        "{{\"configuration\":{{\"scrollbackBytes\":{d}}},\"revision\":\"{d}\"}}",
        .{ scrollback_bytes, revision },
    );
}

pub fn terminalConfigurationAccepted(allocator: std.mem.Allocator, update: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalConfigurationAccepted(&writer.writer, update);
    return writer.toOwnedSlice();
}

pub fn writeTerminalConfigurationAccepted(writer: *std.Io.Writer, update: anytype) !void {
    var terminal_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"configuration\":{{\"scrollbackBytes\":{d}}},\"configurationRevision\":\"{d}\",\"generation\":\"{d}\",\"historyRowsEvicted\":\"{d}\",\"revision\":\"{d}\",\"terminalId\":\"{s}\"}}",
        .{
            update.configuration.scrollback_bytes,
            update.configuration_revision,
            update.generation,
            update.history_rows_evicted,
            update.revision,
            identifierText(update.terminal_id, &terminal_buffer),
        },
    );
}

pub fn attachmentCapabilities(allocator: std.mem.Allocator, attachment: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeAttachmentCapabilities(&writer.writer, attachment);
    return writer.toOwnedSlice();
}

pub fn writeAttachmentCapabilities(writer: *std.Io.Writer, attachment: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":{},\"read\":{},\"resize\":{}}},\"resizeLease\":{},\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment.attachment_id, &attachment_buffer),
            attachment.capabilities.input,
            attachment.capabilities.read,
            attachment.capabilities.resize,
            attachment.resize_lease,
            identifierText(attachment.terminal_id, &terminal_buffer),
        },
    );
}

pub fn attachmentAcknowledged(allocator: std.mem.Allocator, attachment_id: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeAttachmentAcknowledged(&writer.writer, attachment_id);
    return writer.toOwnedSlice();
}

pub fn writeAttachmentAcknowledged(writer: *std.Io.Writer, attachment_id: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\"}}",
        .{identifierText(attachment_id, &attachment_buffer)},
    );
}

pub fn inputAccepted(allocator: std.mem.Allocator, accepted: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeInputAccepted(&writer.writer, accepted);
    return writer.toOwnedSlice();
}

pub fn writeInputAccepted(writer: *std.Io.Writer, accepted: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"acceptedBytes\":{d},\"attachmentId\":\"{s}\"}}",
        .{ accepted.byte_count, identifierText(accepted.attachment_id, &attachment_buffer) },
    );
}

pub fn projectionDemandAccepted(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    demand: anytype,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeProjectionDemandAccepted(&writer.writer, attachment_id, demand);
    return writer.toOwnedSlice();
}

pub fn writeProjectionDemandAccepted(
    writer: *std.Io.Writer,
    attachment_id: anytype,
    demand: anytype,
) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"demand\":\"{s}\"}}",
        .{ identifierText(attachment_id, &attachment_buffer), @tagName(demand) },
    );
}

pub fn writeFrameApplied(writer: *std.Io.Writer, applied: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{
            identifierText(applied.attachment_id, &attachment_buffer),
            applied.generation,
            applied.revision,
            applied.sequence,
        },
    );
}

pub fn writeNeedSnapshot(writer: *std.Io.Writer, request: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    const reason = switch (request.reason) {
        .base_mismatch => "base-mismatch",
        .decoder_rejected => "decoder-rejected",
        .dimensions_mismatch => "dimensions-mismatch",
        .generation_mismatch => "generation-mismatch",
        .identity_mismatch => "identity-mismatch",
        .sequence_gap => "sequence-gap",
        .store_reset => "store-reset",
    };
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"reason\":\"{s}\",\"sequence\":\"{d}\"}}",
        .{
            identifierText(request.attachment_id, &attachment_buffer),
            reason,
            request.sequence,
        },
    );
}

pub fn writeRowsApplied(writer: *std.Io.Writer, applied: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"requestSequence\":\"{d}\",\"revision\":\"{d}\"}}",
        .{
            identifierText(applied.attachment_id, &attachment_buffer),
            applied.generation,
            applied.request_sequence,
            applied.revision,
        },
    );
}

pub fn writeHistoryDemandAccepted(writer: *std.Io.Writer, request: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    const demand = switch (request.demand) {
        .following_bottom => "following-bottom",
        .viewing_history => "viewing-history",
    };
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"demand\":\"{s}\"}}",
        .{ identifierText(request.attachment_id, &attachment_buffer), demand },
    );
}

pub fn writeHistoryObserved(writer: *std.Io.Writer, observed: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{
            identifierText(observed.attachment_id, &attachment_buffer),
            observed.generation,
            observed.revision,
            observed.sequence,
        },
    );
}

pub fn writeSelectionExportChunkApplied(writer: *std.Io.Writer, applied: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"chunkSequence\":{d},\"exportSequence\":\"{d}\"}}",
        .{
            identifierText(applied.attachment_id, &attachment_buffer),
            applied.chunk_sequence,
            applied.export_sequence,
        },
    );
}

pub fn projectionBlocked(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    reason: anytype,
    revision: u64,
) ![]u8 {
    var attachment_buffer: [32]u8 = undefined;
    const reason_text = switch (reason) {
        .capture_allocation_failed => "capture-allocation-failed",
        .projection_too_large => "projection-too-large",
    };
    return std.fmt.allocPrint(
        allocator,
        "{{\"attachmentId\":\"{s}\",\"reason\":\"{s}\",\"revision\":\"{d}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            reason_text,
            revision,
        },
    );
}

pub fn historyChanged(allocator: std.mem.Allocator, event: anytype) ![]u8 {
    var attachment_buffer: [32]u8 = undefined;
    const anchor_status = switch (event.anchor_status) {
        .evicted => "evicted",
        .none => "none",
        .valid => "valid",
    };
    return std.fmt.allocPrint(
        allocator,
        "{{\"anchorStatus\":\"{s}\",\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\",\"totalRows\":\"{d}\"}}",
        .{
            anchor_status,
            identifierText(event.attachment_id, &attachment_buffer),
            event.generation,
            event.revision,
            event.sequence,
            event.total_rows,
        },
    );
}

pub fn selectionChanged(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    snapshot: anytype,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeSelectionChanged(&writer.writer, attachment_id, snapshot);
    return writer.toOwnedSlice();
}

pub fn writeSelectionChanged(
    writer: *std.Io.Writer,
    attachment_id: anytype,
    snapshot: anytype,
) !void {
    var attachment_buffer: [32]u8 = undefined;
    const attachment_text = identifierText(attachment_id, &attachment_buffer);
    switch (snapshot) {
        .none => try writer.print(
            "{{\"attachmentId\":\"{s}\",\"state\":\"none\"}}",
            .{attachment_text},
        ),
        .evicted => try writer.print(
            "{{\"attachmentId\":\"{s}\",\"state\":\"evicted\"}}",
            .{attachment_text},
        ),
        .active => |active| try writer.print(
            "{{\"attachmentId\":\"{s}\",\"end\":{{\"column\":{d},\"row\":\"{d}\"}},\"preview\":null,\"rectangle\":{},\"start\":{{\"column\":{d},\"row\":\"{d}\"}},\"state\":\"active\"}}",
            .{
                attachment_text,
                active.end.column,
                active.end.row,
                active.rectangle,
                active.start.column,
                active.start.row,
            },
        ),
    }
}

pub fn selectionExportStarted(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    started: anytype,
) ![]u8 {
    var attachment_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"format\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            started.byte_length,
            started.export_sequence,
            @tagName(started.format),
        },
    );
}

pub fn selectionExportFinished(
    allocator: std.mem.Allocator,
    finished: anytype,
    status: anytype,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeSelectionExportFinished(&writer.writer, finished, status);
    return writer.toOwnedSlice();
}

pub fn writeSelectionExportFinished(
    writer: *std.Io.Writer,
    finished: anytype,
    status: anytype,
) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"status\":\"{s}\"}}",
        .{
            identifierText(finished.attachment_id, &attachment_buffer),
            finished.byte_length,
            finished.export_sequence,
            @tagName(status),
        },
    );
}

pub fn resizeAccepted(allocator: std.mem.Allocator, resized: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeResizeAccepted(&writer.writer, resized);
    return writer.toOwnedSlice();
}

pub fn writeResizeAccepted(writer: *std.Io.Writer, resized: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"cellPixelHeight\":{d},\"cellPixelWidth\":{d},\"columns\":{d},\"revision\":\"{d}\",\"rows\":{d}}}",
        .{
            identifierText(resized.attachment_id, &attachment_buffer),
            resized.geometry.cell_pixel_height,
            resized.geometry.cell_pixel_width,
            resized.geometry.columns,
            resized.revision,
            resized.geometry.rows,
        },
    );
}

pub fn terminalList(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    inventory_generation: u64,
    terminals: anytype,
    offset: usize,
    maximum_terminals: usize,
) ![]u8 {
    std.debug.assert(offset <= terminals.len);
    std.debug.assert(maximum_terminals > 0);
    const requested_length = @min(maximum_terminals, terminals.len - offset);
    var summaries: std.ArrayList([]u8) = .empty;
    defer {
        for (summaries.items) |summary| allocator.free(summary);
        summaries.deinit(allocator);
    }
    var summaries_bytes: usize = 0;
    for (terminals[offset .. offset + requested_length]) |terminal| {
        const summary = try terminalSummary(allocator, host_epoch, terminal);
        errdefer allocator.free(summary);
        const candidate_end = offset + summaries.items.len + 1;
        const framing_bytes = terminalListFramingLength(
            inventory_generation,
            candidate_end,
            terminals.len,
        );
        const separator_bytes = summaries.items.len;
        const available_summary_bytes = protocol.Limits.control_json_bytes -| framing_bytes -| separator_bytes;
        if (summaries_bytes > available_summary_bytes or
            summary.len > available_summary_bytes - summaries_bytes)
        {
            if (summaries.items.len == 0) return error.InventoryTerminalTooLarge;
            allocator.free(summary);
            break;
        }
        try summaries.append(allocator, summary);
        summaries_bytes += summary.len;
    }
    const end = offset + summaries.items.len;
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalListPrefix(
        &writer.writer,
        inventory_generation,
        end,
        terminals.len,
    );
    for (summaries.items, 0..) |summary, index| {
        if (index != 0) try writer.writer.writeByte(',');
        try writer.writer.writeAll(summary);
    }
    try writer.writer.writeAll("]}");
    const payload = try writer.toOwnedSlice();
    errdefer allocator.free(payload);
    std.debug.assert(payload.len <= protocol.Limits.control_json_bytes);
    return payload;
}

fn terminalListFramingLength(
    inventory_generation: u64,
    end: usize,
    terminal_count: usize,
) usize {
    var storage: [256]u8 = undefined;
    var writer: std.Io.Writer = .fixed(&storage);
    writeTerminalListPrefix(
        &writer,
        inventory_generation,
        end,
        terminal_count,
    ) catch unreachable;
    return writer.buffered().len + "]}".len;
}

fn writeTerminalListPrefix(
    writer: *std.Io.Writer,
    inventory_generation: u64,
    end: usize,
    terminal_count: usize,
) !void {
    try writer.print(
        "{{\"inventoryGeneration\":\"{d}\",\"nextCursor\":",
        .{inventory_generation},
    );
    if (end < terminal_count) {
        try writer.print(
            "{{\"inventoryGeneration\":\"{d}\",\"offset\":\"{d}\"}}",
            .{ inventory_generation, end },
        );
    } else {
        try writer.writeAll("null");
    }
    try writer.writeAll(",\"terminals\":[");
}

pub fn terminalSummary(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    terminal: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const exit_json = try optionalExitResultJson(allocator, terminal.exitResult());
    defer allocator.free(exit_json);
    const failure_json = if (terminal.runtimeFailure()) |failure|
        try runtimeFailureJson(allocator, failure)
    else
        try allocator.dupe(u8, "null");
    defer allocator.free(failure_json);
    const containment_json = if (terminal.failureContainment()) |containment|
        try failureContainmentJson(allocator, containment)
    else
        try allocator.dupe(u8, "null");
    defer allocator.free(containment_json);
    const process_json = if (terminal.failedProcess()) |process|
        try processObservationJson(allocator, process)
    else
        try allocator.dupe(u8, "null");
    defer allocator.free(process_json);
    const configuration = terminal.effectiveConfiguration();
    const geometry = terminal.observedGeometry();
    return std.fmt.allocPrint(
        allocator,
        "{{\"columns\":{d},\"configuration\":{{\"scrollbackBytes\":{d}}},\"configurationRevision\":\"{d}\",\"exit\":{s},\"failure\":{s},\"failureContainment\":{s},\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"phase\":\"{s}\",\"process\":{s},\"revision\":\"{d}\",\"rows\":{d},\"terminateRequested\":{}}}",
        .{
            geometry.columns,
            configuration.scrollback_bytes,
            terminal.configurationRevision(),
            exit_json,
            failure_json,
            containment_json,
            identifierText(terminal.creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(terminal.terminal_id, &terminal_buffer),
            terminalPhaseText(terminal.phase()),
            process_json,
            terminal.observedRevision(),
            geometry.rows,
            terminal.terminateRequested(),
        },
    );
}

pub fn terminalExited(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    notice: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const exit_json = try exitResultJson(allocator, notice.result);
    defer allocator.free(exit_json);
    return std.fmt.allocPrint(
        allocator,
        "{{\"exit\":{s},\"finalRevision\":\"{d}\",\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}}}}",
        .{
            exit_json,
            notice.final_revision,
            identifierText(notice.creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(notice.terminal_id, &terminal_buffer),
        },
    );
}

pub fn terminalFailed(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    notice: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const failure_json = try runtimeFailureJson(allocator, notice.failure);
    defer allocator.free(failure_json);
    const process_json = try processObservationJson(allocator, notice.process);
    defer allocator.free(process_json);
    return std.fmt.allocPrint(
        allocator,
        "{{\"failure\":{s},\"finalRevision\":\"{d}\",\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"process\":{s}}}",
        .{
            failure_json,
            notice.final_revision,
            identifierText(notice.creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(notice.terminal_id, &terminal_buffer),
            process_json,
        },
    );
}

pub fn identifierText(identifier: anytype, buffer: *[32]u8) []const u8 {
    return identifiers.formatLowerHex(identifier, buffer);
}

fn optionalExitResultJson(allocator: std.mem.Allocator, result: anytype) ![]u8 {
    const value = result orelse return allocator.dupe(u8, "null");
    return exitResultJson(allocator, value);
}

fn exitResultJson(allocator: std.mem.Allocator, result: anytype) ![]u8 {
    return switch (result) {
        .exit_code => |code| std.fmt.allocPrint(allocator, "{{\"code\":{d},\"kind\":\"exit-code\"}}", .{code}),
        .signal => |signal| {
            std.debug.assert(signal != 0);
            return std.fmt.allocPrint(allocator, "{{\"kind\":\"signal\",\"signal\":{d}}}", .{signal});
        },
        .unknown => |unknown| std.fmt.allocPrint(allocator, "{{\"kind\":\"unknown\",\"value\":{d}}}", .{unknown}),
    };
}

fn terminalPhaseText(phase: anytype) []const u8 {
    return switch (phase) {
        .running => "running",
        .finishing => "finishing",
        .containing_failure => "containing-failure",
        .exited => "exited",
        .failed => "failed",
    };
}

fn runtimeFailureKindText(kind: anytype) []const u8 {
    return switch (kind) {
        .process_control => "process-control",
        .process_observation => "process-observation",
        .pty_read => "pty-read",
        .pty_write => "pty-write",
        .resize => "resize",
        .terminal_reply => "terminal-reply",
        .worker_integrity => "worker-integrity",
    };
}

fn runtimeFailureStageText(stage: anytype) []const u8 {
    return switch (stage) {
        .admit_terminal_reply => "admit-terminal-reply",
        .apply_resize => "apply-resize",
        .contain_process => "contain-process",
        .dispatch_termination => "dispatch-termination",
        .observe_process => "observe-process",
        .read_output => "read-output",
        .release_output => "release-output",
        .release_process_identity => "release-process-identity",
        .synchronize_worker => "synchronize-worker",
        .write_input => "write-input",
    };
}

fn runtimeFailureJson(allocator: std.mem.Allocator, failure: anytype) ![]u8 {
    const Failure = @TypeOf(failure);
    const kind = if (@hasField(Failure, "kind")) failure.kind else failure.kind();
    const stage = if (@hasField(Failure, "stage")) failure.stage else failure.stage();
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writer.writer.print(
        "{{\"kind\":\"{s}\",\"stage\":\"{s}\"}}",
        .{ runtimeFailureKindText(kind), runtimeFailureStageText(stage) },
    );
    return writer.toOwnedSlice();
}

fn processObservationJson(allocator: std.mem.Allocator, observation: anytype) ![]u8 {
    return switch (observation) {
        .observed => |result| {
            const result_json = try exitResultJson(allocator, result);
            defer allocator.free(result_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"kind\":\"observed\",\"result\":{s}}}",
                .{result_json},
            );
        },
        .unavailable => |failure| {
            const failure_json = try runtimeFailureJson(allocator, failure);
            defer allocator.free(failure_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"failure\":{s},\"kind\":\"unavailable\"}}",
                .{failure_json},
            );
        },
    };
}

fn failureContainmentJson(allocator: std.mem.Allocator, containment: anytype) ![]u8 {
    return switch (containment) {
        .stopping => |state| {
            const process_json = if (state.process) |process|
                try processObservationJson(allocator, process)
            else
                try allocator.dupe(u8, "null");
            defer allocator.free(process_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"kind\":\"stopping\",\"process\":{s}}}",
                .{process_json},
            );
        },
        .blocked => |state| {
            const cleanup_json = try runtimeFailureJson(allocator, state.cleanup_failure);
            defer allocator.free(cleanup_json);
            const process_json = if (state.process) |process|
                try processObservationJson(allocator, process)
            else
                try allocator.dupe(u8, "null");
            defer allocator.free(process_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"cleanupFailure\":{s},\"kind\":\"blocked\",\"process\":{s}}}",
                .{ cleanup_json, process_json },
            );
        },
    };
}
