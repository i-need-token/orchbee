const std = @import("std");

const subject = @import("terminal-host-engine").testing.selection_export;

test "selection export chunks retain identity across rollback and completion replay" {
    const attachment = attachmentId(1);
    var state: subject.State = .{
        .active = try active(attachment, 7, "selected"),
        .last_sequence = 7,
    };
    defer state.deinit(std.testing.allocator);

    const first = (try state.encodeNextChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(first.bytes);
    try state.rollbackChunk(first.token);

    const retry = (try state.encodeNextChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(retry.bytes);
    try std.testing.expectEqualDeep(first.token, retry.token);
    try std.testing.expectEqualSlices(u8, first.bytes, retry.bytes);

    const finished = (try state.chunkApplied(
        std.testing.allocator,
        attachment,
        retry.token.export_sequence,
        retry.token.chunk_sequence,
    )).?;
    try std.testing.expectEqual(@as(u64, 7), finished.export_sequence);
    try std.testing.expect(state.active == null);
    try std.testing.expect((try state.chunkApplied(
        std.testing.allocator,
        attachment,
        retry.token.export_sequence,
        retry.token.chunk_sequence,
    )) == null);
}

test "selection export invalidation owns its bounded cancellation FIFO" {
    const attachment = attachmentId(2);
    var state: subject.State = .{};
    defer state.deinit(std.testing.allocator);

    state.active = try active(attachment, 11, "first");
    state.last_sequence = 11;
    state.invalidate(std.testing.allocator);
    state.active = try active(attachment, 12, "second");
    state.last_sequence = 12;
    state.invalidate(std.testing.allocator);

    try std.testing.expectError(error.ExportConflict, state.validateStart(13));
    const prepared = state.prepareCancellation().?;
    try std.testing.expectEqual(@as(u64, 11), prepared.cancellation.export_sequence);
    state.commitPreparedCancellation(prepared);
    const second = state.takeCancellation().?;
    try std.testing.expectEqual(@as(u64, 12), second.export_sequence);
    try std.testing.expect(state.takeCancellation() == null);
}

fn active(
    attachment_id: subject.AttachmentId,
    export_sequence: u64,
    bytes: []const u8,
) !subject.Active {
    const storage = try std.testing.allocator.dupe(u8, bytes);
    return .{
        .attachment_id = attachment_id,
        .bytes_length = storage.len,
        .bytes_offset = 0,
        .export_sequence = export_sequence,
        .format = .plain,
        .storage = storage,
    };
}

fn attachmentId(byte: u8) subject.AttachmentId {
    return .fromBytes([_]u8{byte} ** 16);
}
