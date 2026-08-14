const harness = @import("history_harness.zig");
const std = harness.std;
const ghostty_vt = harness.ghostty_vt;
const protocol = harness.protocol;
const subject = harness.subject;
const findState = harness.findState;
const Manager = harness.Manager;
const max_row_window_bytes = harness.max_row_window_bytes;
const max_pending_export_cancellations = harness.max_pending_export_cancellations;
const Detail = harness.Detail;
const ExportCancellation = harness.ExportCancellation;
const SelectionGesture = harness.SelectionGesture;
const SelectionSnapshot = harness.SelectionSnapshot;
const AttachmentId = harness.AttachmentId;
const HostEpoch = harness.HostEpoch;
const RecordingAllocator = harness.RecordingAllocator;
const attachmentId = harness.attachmentId;
const hostEpoch = harness.hostEpoch;
const gestureStart = harness.gestureStart;
const gestureUpdate = harness.gestureUpdate;
const gestureEnd = harness.gestureEnd;
const gestureCancel = harness.gestureCancel;
const readSingleRow = harness.readSingleRow;

test "reflow generation preserves a valid tracked selection and cancels stale export bytes" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("abcdef");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(8);
    try manager.attach(attachment);
    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .{ .set_range = .{
            .end = .{ .column = 4, .row = 0 },
            .rectangle = false,
            .start = .{ .column = 1, .row = 0 },
        } },
        .generation = 1,
        .revision = 1,
    });
    _ = try manager.startExport(&terminal, attachment, 9, .plain);

    try terminal.resize(std.testing.allocator, 4, 2);
    manager.generationChanged(&terminal, 2, 2);

    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.selection_state.selection != null);
    try std.testing.expect(state.export_ledger.active == null);
    try std.testing.expectEqual(
        SelectionSnapshot.active,
        std.meta.activeTag((try manager.takeSelectionEvent(attachment)).?),
    );
    try std.testing.expect((try manager.takeSelectionExportCancellation(attachment)) != null);
}
test "selection eviction cancels an in-flight export" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });
    const started = try manager.startExport(&terminal, attachment, 9, .plain);
    const state = findState(&manager, attachment).?;
    if (state.selection_state.selection) |*selection| selection.startPtr().garbage = true;

    manager.noteMutation(&terminal, 2, 1);

    try std.testing.expect(state.selection_state.selection == null);
    try std.testing.expect(state.export_ledger.active == null);
    try std.testing.expectEqual(
        SelectionSnapshot.evicted,
        std.meta.activeTag((try manager.takeSelectionEvent(attachment)).?),
    );
    const cancellation = (try manager.takeSelectionExportCancellation(attachment)).?;
    try std.testing.expect(attachment.eql(cancellation.attachment_id));
    try std.testing.expectEqual(started.byte_length, cancellation.byte_length);
    try std.testing.expectEqual(started.export_sequence, cancellation.export_sequence);
    try std.testing.expect((try manager.takeSelectionExportCancellation(attachment)) == null);
    try std.testing.expect((try manager.encodeNextExportChunk(std.testing.allocator, attachment)) == null);
    try std.testing.expect((try manager.exportChunkApplied(attachment, 9, 0)) == null);
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.exportChunkApplied(attachment, 8, 0),
    );
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.exportChunkApplied(attachment, 0, 0),
    );
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.cancelExport(attachment, 9),
    );

    _ = try manager.applySelectionCommand(&terminal, 2, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 2,
    });
    const newer_started = try manager.startExport(&terminal, attachment, 10, .plain);
    try std.testing.expect((try manager.exportChunkApplied(attachment, 9, 0)) == null);
    const newer_chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(newer_chunk.bytes);
    const newer_finished = (try manager.exportChunkApplied(attachment, 10, 0)).?;
    try std.testing.expect(attachment.eql(newer_finished.attachment_id));
    try std.testing.expectEqual(newer_started.byte_length, newer_finished.byte_length);
    try std.testing.expectEqual(@as(u64, 10), newer_finished.export_sequence);
    try std.testing.expect((try manager.exportChunkApplied(attachment, 10, 0)) == null);
}
test "repeated selection eviction preserves every queued export cancellation" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(7);
    try manager.attach(attachment);

    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });
    const first_started = try manager.startExport(&terminal, attachment, 9, .plain);
    const state = findState(&manager, attachment).?;
    if (state.selection_state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 2, 1);

    _ = try manager.applySelectionCommand(&terminal, 2, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 2,
    });
    const second_started = try manager.startExport(&terminal, attachment, 10, .plain);
    if (state.selection_state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 3, 1);

    const first_cancellation = (try manager.takeSelectionExportCancellation(attachment)).?;
    const second_cancellation = (try manager.takeSelectionExportCancellation(attachment)).?;
    try std.testing.expectEqual(@as(u64, 9), first_cancellation.export_sequence);
    try std.testing.expectEqual(first_started.byte_length, first_cancellation.byte_length);
    try std.testing.expectEqual(@as(u64, 10), second_cancellation.export_sequence);
    try std.testing.expectEqual(second_started.byte_length, second_cancellation.byte_length);
    try std.testing.expect((try manager.takeSelectionExportCancellation(attachment)) == null);
    try std.testing.expect((try manager.exportChunkApplied(attachment, 9, 0)) == null);
    try std.testing.expect((try manager.exportChunkApplied(attachment, 10, 0)) == null);
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.exportChunkApplied(attachment, 8, 0),
    );
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.exportChunkApplied(attachment, 0, 0),
    );
}
test "export sequences reject ABA acknowledgements and failed starts do not consume identity" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });

    _ = try manager.startExport(&terminal, attachment, 7, .plain);
    const first_chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(first_chunk.bytes);
    try manager.rollbackExportChunk(first_chunk.token);
    const retried_chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(retried_chunk.bytes);
    try std.testing.expectEqual(first_chunk.token.export_sequence, retried_chunk.token.export_sequence);
    try std.testing.expectEqual(first_chunk.token.chunk_sequence, retried_chunk.token.chunk_sequence);
    try std.testing.expectEqualSlices(u8, first_chunk.bytes, retried_chunk.bytes);
    _ = (try manager.exportChunkApplied(attachment, 7, 0)).?;
    try std.testing.expect((try manager.exportChunkApplied(attachment, 7, 0)) == null);
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.exportChunkApplied(attachment, 0, 0),
    );
    try std.testing.expectError(
        error.InvalidRequest,
        manager.startExport(&terminal, attachment, 7, .plain),
    );

    var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{ .fail_index = 0 });
    manager.allocator = failing.allocator();
    try std.testing.expectError(
        error.OutOfMemory,
        manager.startExport(&terminal, attachment, 8, .plain),
    );
    manager.allocator = std.testing.allocator;
    _ = try manager.startExport(&terminal, attachment, 8, .plain);
    const second_chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(second_chunk.bytes);
    // THCR-029: a replay of the completed export 7 acknowledgement is an
    // idempotent no-op even while export 8 is in flight.
    try std.testing.expect((try manager.exportChunkApplied(attachment, 7, 0)) == null);
    _ = (try manager.exportChunkApplied(attachment, 8, 0)).?;
}
test "a final export chunk does not require another chunk sequence" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(9);
    try manager.attach(attachment);
    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });
    _ = try manager.startExport(&terminal, attachment, 1, .plain);
    const state = findState(&manager, attachment).?;
    state.export_ledger.active.?.chunk_sequence = std.math.maxInt(u32);
    const chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(chunk.bytes);
    try std.testing.expectEqual(std.math.maxInt(u32), chunk.token.chunk_sequence);
    const finished = (try manager.exportChunkApplied(
        attachment,
        1,
        std.math.maxInt(u32),
    )).?;
    try std.testing.expectEqual(@as(u64, 1), finished.export_sequence);
    try std.testing.expect(state.export_ledger.active == null);
}
test "failed replacement rollback restores full invalidation history and cancellation FIFO" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(8);
    try manager.attach(attachment);

    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });
    const first_started = try manager.startExport(&terminal, attachment, 1, .plain);
    const state = findState(&manager, attachment).?;
    if (state.selection_state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 2, 1);

    _ = try manager.applySelectionCommand(&terminal, 2, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 2,
    });
    const second_started = try manager.startExport(&terminal, attachment, 2, .plain);
    if (state.selection_state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 3, 1);

    _ = try manager.applySelectionCommand(&terminal, 3, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 3,
    });

    try std.testing.expectEqual(
        @as(usize, max_pending_export_cancellations),
        state.export_ledger.invalidated_count,
    );
    const invalidation_history = try manager.snapshotInvalidatedExportHistory(attachment);
    try std.testing.expectEqual(@as(usize, max_pending_export_cancellations), invalidation_history.count);
    try std.testing.expectEqual(@as(u64, 1), invalidation_history.entries[0].export_sequence);
    try std.testing.expectEqual(@as(u64, 2), invalidation_history.entries[1].export_sequence);

    var cancellations = std.mem.zeroes([max_pending_export_cancellations]ExportCancellation);
    var cancellation_count: usize = 0;
    while (try manager.takeSelectionExportCancellation(attachment)) |cancellation| {
        cancellations[cancellation_count] = cancellation;
        cancellation_count += 1;
    }
    try std.testing.expectEqual(@as(usize, max_pending_export_cancellations), cancellation_count);
    try std.testing.expectEqual(first_started.export_sequence, cancellations[0].export_sequence);
    try std.testing.expectEqual(second_started.export_sequence, cancellations[1].export_sequence);

    const replacement_transaction = try manager.startExportTransaction(
        &terminal,
        attachment,
        3,
        .plain,
    );
    const replacement = replacement_transaction.started;
    if (state.selection_state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 4, 1);
    try std.testing.expectEqual(@as(u64, 3), state.export_ledger.invalidated[1].export_sequence);
    try std.testing.expectEqual(@as(usize, 1), state.export_ledger.pending_cancellation_count);
    try std.testing.expectEqual(
        @as(u64, 3),
        state.export_ledger.pending_cancellations[0].export_sequence,
    );

    manager.rollbackExportStart(
        attachment,
        replacement.export_sequence,
        replacement_transaction.previous_completed_export_sequence,
        replacement_transaction.previous_last_export_sequence,
        cancellations[0..cancellation_count],
        invalidation_history,
    );

    try std.testing.expect(state.export_ledger.active == null);
    try std.testing.expectEqual(
        @as(usize, max_pending_export_cancellations),
        state.export_ledger.invalidated_count,
    );
    try std.testing.expectEqual(@as(u64, 1), state.export_ledger.invalidated[0].export_sequence);
    try std.testing.expectEqual(@as(u64, 2), state.export_ledger.invalidated[1].export_sequence);
    const restored_first = (try manager.takeSelectionExportCancellation(attachment)).?;
    try std.testing.expectEqual(first_started.export_sequence, restored_first.export_sequence);
    try std.testing.expectEqual(first_started.byte_length, restored_first.byte_length);
    const restored_second = (try manager.takeSelectionExportCancellation(attachment)).?;
    try std.testing.expectEqual(second_started.export_sequence, restored_second.export_sequence);
    try std.testing.expectEqual(second_started.byte_length, restored_second.byte_length);
    try std.testing.expect((try manager.takeSelectionExportCancellation(attachment)) == null);
    _ = try manager.applySelectionCommand(&terminal, 4, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 4,
    });
    const retried_replacement = try manager.startExport(
        &terminal,
        attachment,
        replacement.export_sequence,
        .plain,
    );
    try std.testing.expectEqual(replacement.export_sequence, retried_replacement.export_sequence);
    _ = try manager.cancelExport(attachment, replacement.export_sequence);
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.exportChunkApplied(attachment, replacement.export_sequence, 0),
    );
    try std.testing.expect((try manager.exportChunkApplied(attachment, first_started.export_sequence, 0)) == null);
    try std.testing.expect((try manager.exportChunkApplied(attachment, second_started.export_sequence, 0)) == null);
}
test "THCR-013 re-entering viewing history after an acknowledgement reissues a distinct sequence" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x13);
    try manager.attach(attachment);
    try manager.setDemand(&terminal, attachment, .viewing_history);

    const first = (try manager.takeHistoryEvent(&terminal, attachment, 1, 1)).?;
    try std.testing.expectEqual(@as(u64, 1), first.sequence);
    try manager.historyObserved(attachment, first.sequence, first.revision, first.generation);

    // Leave viewing history without a terminal mutation and re-enter, so the
    // revision/generation identity of the next event is unchanged.
    try manager.setDemand(&terminal, attachment, .following_bottom);
    try manager.setDemand(&terminal, attachment, .viewing_history);

    const second = (try manager.takeHistoryEvent(&terminal, attachment, 1, 1)).?;
    try std.testing.expectEqual(first.sequence + 1, second.sequence);
    try std.testing.expectEqual(@as(u64, 1), second.revision);
    try manager.historyObserved(attachment, second.sequence, second.revision, second.generation);

    // A later ordinary mutation must publish a fresh event.
    try terminal.printString("x");
    manager.noteMutation(&terminal, 2, 1);
    const third = (try manager.takeHistoryEvent(&terminal, attachment, 2, 1)).?;
    try std.testing.expectEqual(second.sequence + 1, third.sequence);
    try std.testing.expectEqual(@as(u64, 2), third.revision);
    try manager.historyObserved(attachment, third.sequence, third.revision, third.generation);
}
test "THCR-013 replaying an earlier acknowledgement never clears a newer same-revision flight" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x14);
    try manager.attach(attachment);
    try manager.setDemand(&terminal, attachment, .viewing_history);

    const first = (try manager.takeHistoryEvent(&terminal, attachment, 1, 1)).?;
    try manager.historyObserved(attachment, first.sequence, first.revision, first.generation);

    try manager.setDemand(&terminal, attachment, .following_bottom);
    try manager.setDemand(&terminal, attachment, .viewing_history);
    const second = (try manager.takeHistoryEvent(&terminal, attachment, 1, 1)).?;
    try std.testing.expectEqual(first.sequence + 1, second.sequence);

    // Replaying the first event's acknowledgement must not clear the newer
    // same-revision flight, so no further event is available while it remains.
    try manager.historyObserved(attachment, first.sequence, first.revision, first.generation);
    try std.testing.expect((try manager.takeHistoryEvent(&terminal, attachment, 1, 1)) == null);

    // Acknowledging the second sequence clears exactly that flight.
    try manager.historyObserved(attachment, second.sequence, second.revision, second.generation);
    try terminal.printString("x");
    manager.noteMutation(&terminal, 2, 1);
    const third = (try manager.takeHistoryEvent(&terminal, attachment, 2, 1)).?;
    try std.testing.expectEqual(second.sequence + 1, third.sequence);
    try manager.historyObserved(attachment, third.sequence, third.revision, third.generation);
}
test "THCR-029 replaying an earlier row acknowledgement is idempotent while a newer window is pending" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x1a);
    try manager.attach(attachment);

    const first = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 1, 1, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = 4096,
        .origin = .{ .tail = 0 },
        .purpose = .viewport,
        .request_sequence = 1,
        .revision = 1,
        .row_count = 1,
    });
    defer std.testing.allocator.free(first.bytes);
    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);

    const second = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 1, 1, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = 4096,
        .origin = .{ .tail = 0 },
        .purpose = .viewport,
        .request_sequence = 2,
        .revision = 1,
        .row_count = 1,
    });
    defer std.testing.allocator.free(second.bytes);
    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.pending != null);
    try std.testing.expectEqual(@as(u64, 2), state.pending.?.request_sequence);

    // Replaying the applied window-1 acknowledgement must not clear the
    // pending window-2 flight.
    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);
    try std.testing.expect(state.pending != null);
    try std.testing.expectEqual(@as(u64, 2), state.pending.?.request_sequence);
    try std.testing.expectEqual(@as(u64, 1), state.pending.?.revision);
    try std.testing.expectEqual(@as(u64, 1), state.pending.?.generation);

    try manager.rowsApplied(&terminal, attachment, 2, 1, 1);
    try std.testing.expect(state.pending == null);
    try std.testing.expectEqual(@as(u64, 2), state.committed.?.request_sequence);
}
test "THCR-029 row replay identity survives mutation-refreshed anchors" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x1b);
    try manager.attach(attachment);
    try manager.setDemand(&terminal, attachment, .viewing_history);

    const rows = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 1, 1, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = 4096,
        .origin = .{ .tail = 0 },
        .purpose = .viewport,
        .request_sequence = 1,
        .revision = 1,
        .row_count = 1,
    });
    defer std.testing.allocator.free(rows.bytes);
    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);
    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.committed != null);
    const anchor_before = state.committed.?;

    try terminal.printString("x");
    manager.noteMutation(&terminal, 2, 1);
    try std.testing.expectEqual(@as(u64, 2), state.committed.?.revision);
    try std.testing.expectEqual(@as(u64, 1), state.committed.?.generation);

    // Replaying the original tuple must succeed and leave the refreshed anchor
    // byte-for-byte unchanged.
    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);
    try std.testing.expectEqual(@as(u64, 2), state.committed.?.revision);
    try std.testing.expectEqual(@as(u64, 1), state.committed.?.generation);
    try std.testing.expectEqual(anchor_before.pin, state.committed.?.pin);
}
test "THCR-029 replaying a completed export acknowledgement leaves a newer export untouched" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x1c);
    try manager.attach(attachment);
    _ = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });

    _ = try manager.startExport(&terminal, attachment, 7, .plain);
    const first_chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(first_chunk.bytes);
    _ = (try manager.exportChunkApplied(attachment, 7, 0)).?;

    _ = try manager.startExport(&terminal, attachment, 8, .plain);
    const second_chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(second_chunk.bytes);
    const state = findState(&manager, attachment).?;
    const export_before = state.export_ledger.active.?;

    // Replaying export 7's final chunk acknowledgement is an idempotent no-op
    // while export 8 is in flight.
    try std.testing.expect((try manager.exportChunkApplied(attachment, 7, 0)) == null);
    try std.testing.expectEqual(
        export_before.export_sequence,
        state.export_ledger.active.?.export_sequence,
    );
    try std.testing.expectEqual(export_before.next_offset, state.export_ledger.active.?.next_offset);
    try std.testing.expectEqual(
        export_before.chunk_sequence,
        state.export_ledger.active.?.chunk_sequence,
    );
    try std.testing.expectEqual(
        export_before.chunk_in_flight,
        state.export_ledger.active.?.chunk_in_flight,
    );

    _ = (try manager.exportChunkApplied(attachment, 8, 0)).?;
}
