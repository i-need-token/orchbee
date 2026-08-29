const std = @import("std");

const common = @import("encode_common.zig");

pub fn writeRowsApplied(writer: *std.Io.Writer, applied: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"requestSequence\":\"{d}\",\"revision\":\"{d}\"}}",
        .{
            common.identifierText(applied.attachment_id, &attachment_buffer),
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
        .{ common.identifierText(request.attachment_id, &attachment_buffer), demand },
    );
}

pub fn writeHistoryObserved(writer: *std.Io.Writer, observed: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{
            common.identifierText(observed.attachment_id, &attachment_buffer),
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
            common.identifierText(applied.attachment_id, &attachment_buffer),
            applied.chunk_sequence,
            applied.export_sequence,
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
            common.identifierText(event.attachment_id, &attachment_buffer),
            event.generation,
            event.revision,
            event.sequence,
            event.total_rows,
        },
    );
}

pub fn terminalBell(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    sequence: u64,
) ![]u8 {
    std.debug.assert(sequence > 0);
    var attachment_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"attachmentId\":\"{s}\",\"sequence\":\"{d}\"}}",
        .{ common.identifierText(attachment_id, &attachment_buffer), sequence },
    );
}

pub fn terminalClipboardWriteRequest(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    request: anytype,
) ![]u8 {
    std.debug.assert(request.sequence > 0);
    var attachment_buffer: [32]u8 = undefined;
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writer.writer.print(
        "{{\"attachmentId\":\"{s}\",\"sequence\":\"{d}\",\"text\":",
        .{ common.identifierText(attachment_id, &attachment_buffer), request.sequence },
    );
    try std.json.Stringify.value(request.text, .{}, &writer.writer);
    try writer.writer.writeAll("}");
    return writer.toOwnedSlice();
}

pub fn selectionChanged(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    snapshot: anytype,
    preview: ?[]const u8,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeSelectionChanged(&writer.writer, attachment_id, snapshot, preview);
    return writer.toOwnedSlice();
}

pub fn writeSelectionChanged(
    writer: *std.Io.Writer,
    attachment_id: anytype,
    snapshot: anytype,
    preview: ?[]const u8,
) !void {
    var attachment_buffer: [32]u8 = undefined;
    const attachment_text = common.identifierText(attachment_id, &attachment_buffer);
    switch (snapshot) {
        .none => try writer.print(
            "{{\"attachmentId\":\"{s}\",\"state\":\"none\"}}",
            .{attachment_text},
        ),
        .evicted => try writer.print(
            "{{\"attachmentId\":\"{s}\",\"state\":\"evicted\"}}",
            .{attachment_text},
        ),
        .active => |active| {
            try writer.print(
                "{{\"attachmentId\":\"{s}\",\"end\":{{\"column\":{d},\"row\":\"{d}\"}},\"preview\":",
                .{
                    attachment_text,
                    active.end.column,
                    active.end.row,
                },
            );
            if (preview) |text| {
                try std.json.Stringify.value(text, .{}, writer);
            } else {
                try writer.writeAll("null");
            }
            try writer.print(
                ",\"rectangle\":{},\"start\":{{\"column\":{d},\"row\":\"{d}\"}},\"state\":\"active\"}}",
                .{
                    active.rectangle,
                    active.start.column,
                    active.start.row,
                },
            );
        },
    }
}

pub fn selectionExportStarted(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    started: anytype,
) ![]u8 {
    if (started.format != .plain) return error.UnsupportedSelectionExportFormat;
    var attachment_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"format\":\"plain\"}}",
        .{
            common.identifierText(attachment_id, &attachment_buffer),
            started.byte_length,
            started.export_sequence,
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
            common.identifierText(finished.attachment_id, &attachment_buffer),
            finished.byte_length,
            finished.export_sequence,
            @tagName(status),
        },
    );
}
