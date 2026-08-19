const std = @import("std");

const common = @import("encode_common.zig");

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
        .{common.identifierText(terminal_id, &terminal_buffer)},
    );
}

pub fn terminalStartAccepted(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    outcome: anytype,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalStartAccepted(&writer.writer, host_epoch, outcome);
    return writer.toOwnedSlice();
}

pub fn writeTerminalStartAccepted(
    writer: *std.Io.Writer,
    host_epoch: anytype,
    outcome: anytype,
) !void {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const transition = switch (outcome.transition) {
        .already_started => "already-started",
        .started => "started",
    };
    try writer.print(
        "{{\"geometry\":{{\"cellPixelHeight\":{d},\"cellPixelWidth\":{d},\"columns\":{d},\"rows\":{d}}},\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"transition\":\"{s}\"}}",
        .{
            outcome.geometry.cell_pixel_height,
            outcome.geometry.cell_pixel_width,
            outcome.geometry.columns,
            outcome.geometry.rows,
            common.identifierText(outcome.identity.creation_id, &creation_buffer),
            common.identifierText(host_epoch, &epoch_buffer),
            common.identifierText(outcome.identity.terminal_id, &terminal_buffer),
            transition,
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
            common.identifierText(attachment.attachment_id, &attachment_buffer),
            attachment.capabilities.input,
            attachment.capabilities.read,
            attachment.capabilities.resize,
            attachment.resize_lease,
            common.identifierText(attachment.terminal_id, &terminal_buffer),
        },
    );
}

pub fn resizeLeaseChanged(allocator: std.mem.Allocator, change: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeResizeLeaseChanged(&writer.writer, change);
    return writer.toOwnedSlice();
}

pub fn writeResizeLeaseChanged(writer: *std.Io.Writer, change: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"resizeLease\":{},\"terminalId\":\"{s}\"}}",
        .{
            common.identifierText(change.attachment_id, &attachment_buffer),
            change.resize_lease,
            common.identifierText(change.terminal_id, &terminal_buffer),
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
        .{common.identifierText(attachment_id, &attachment_buffer)},
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
        .{ accepted.byte_count, common.identifierText(accepted.attachment_id, &attachment_buffer) },
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
        .{ common.identifierText(attachment_id, &attachment_buffer), @tagName(demand) },
    );
}

pub fn writeFrameApplied(writer: *std.Io.Writer, applied: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{
            common.identifierText(applied.attachment_id, &attachment_buffer),
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
            common.identifierText(request.attachment_id, &attachment_buffer),
            reason,
            request.sequence,
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
            common.identifierText(attachment_id, &attachment_buffer),
            reason_text,
            revision,
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
            common.identifierText(resized.attachment_id, &attachment_buffer),
            resized.geometry.cell_pixel_height,
            resized.geometry.cell_pixel_width,
            resized.geometry.columns,
            resized.revision,
            resized.geometry.rows,
        },
    );
}

pub fn writeTerminalControlAccepted(writer: *std.Io.Writer, update: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"operation\":\"{s}\",\"revision\":\"{d}\",\"terminalId\":\"{s}\"}}",
        .{
            common.identifierText(update.attachment_id, &attachment_buffer),
            update.generation,
            @tagName(update.operation),
            update.revision,
            common.identifierText(update.terminal_id, &terminal_buffer),
        },
    );
}
