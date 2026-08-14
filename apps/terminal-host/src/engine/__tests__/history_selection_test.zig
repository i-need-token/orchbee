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

test "screen replacement publishes one selection eviction event" {
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

    const selected = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(selected));

    manager.screenChanged(&terminal);
    const event = (try manager.takeSelectionEvent(attachment)).?;
    try std.testing.expectEqual(SelectionSnapshot.evicted, std.meta.activeTag(event));
    try manager.rollbackSelectionEvent(attachment, event);
    try std.testing.expectEqual(
        SelectionSnapshot.evicted,
        std.meta.activeTag((try manager.takeSelectionEvent(attachment)).?),
    );
    try std.testing.expect((try manager.takeSelectionEvent(attachment)) == null);
}
test "invalid gesture start leaves tracked state unchanged" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    const initial_pins = terminal.screens.active.pages.countTrackedPins();

    try std.testing.expectError(
        error.InvalidRequest,
        manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 0, 0, 0, false)),
    );
    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.selection_state.gesture.left_click_pin == null);
    try std.testing.expect(!state.selection_state.gesture_active);
    try std.testing.expect(state.selection_state.selection == null);
    try std.testing.expectEqual(initial_pins, terminal.screens.active.pages.countTrackedPins());
}
test "selection validation precedes attachment lookup" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);

    try std.testing.expectError(
        error.StaleRevision,
        manager.applySelectionCommand(&terminal, 2, 2, .{
            .attachment_id = attachmentId(92),
            .command = .clear,
            .generation = 1,
            .revision = 1,
        }),
    );
}
test "single click release clears selection without creating a one-cell range" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(3);
    try manager.attach(attachment);

    const pressed = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureStart(attachment, 1, 2, 0, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.none, std.meta.activeTag(pressed));

    const released = try manager.applyGesture(&terminal, 1, 1, gestureEnd(attachment, 2, 0));
    try std.testing.expectEqual(SelectionSnapshot.none, std.meta.activeTag(released));
}
test "cell drag and repeated clicks retain terminal selection semantics" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 12,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha beta");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(4);
    try manager.attach(attachment);

    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 1, 0, 0, false));
    const dragged = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 2, 0, 29, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(dragged));
    try std.testing.expectEqual(@as(u16, 0), dragged.active.start.column);
    try std.testing.expectEqual(@as(u16, 2), dragged.active.end.column);
    _ = try manager.applyGesture(&terminal, 1, 1, gestureEnd(attachment, 2, 0));

    const word = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureStart(attachment, 2, 7, 0, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(word));
    try std.testing.expectEqual(@as(u16, 6), word.active.start.column);
    try std.testing.expectEqual(@as(u16, 9), word.active.end.column);

    const line = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureStart(attachment, 3, 1, 0, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(line));
}
test "gesture cancellation preserves selection and is idempotent" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 12,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha beta");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(4);
    try manager.attach(attachment);

    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 1, 0, 0, false));
    const selected = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 2, 0, 29, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(selected));

    const cancelled = try manager.applyGesture(&terminal, 1, 1, gestureCancel(attachment));
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(cancelled));
    try std.testing.expectEqual(selected.active.start.column, cancelled.active.start.column);
    try std.testing.expectEqual(selected.active.end.column, cancelled.active.end.column);
    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.selection_state.gesture.left_click_pin == null);
    try std.testing.expect(!state.selection_state.gesture_active);
    const tracked_pins = terminal.screens.active.pages.countTrackedPins();

    const repeated = try manager.applyGesture(&terminal, 1, 1, gestureCancel(attachment));
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(repeated));
    try std.testing.expectEqual(tracked_pins, terminal.screens.active.pages.countTrackedPins());
}
test "cell drag applies the within-cell endpoint threshold and rectangle mode" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("abcdef");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(4);
    try manager.attach(attachment);

    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 1, 2, 0, false));
    const below_threshold = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 2, 0, 25, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.none, std.meta.activeTag(below_threshold));

    const at_threshold = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 2, 0, 26, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(at_threshold));
    try std.testing.expectEqual(@as(u16, 2), at_threshold.active.start.column);
    try std.testing.expectEqual(@as(u16, 2), at_threshold.active.end.column);
    _ = try manager.applyGesture(&terminal, 1, 1, gestureEnd(attachment, 2, 0));

    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 1, 1, 0, false));
    const rectangle = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 3, 0, 39, true),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(rectangle));
    try std.testing.expect(rectangle.active.rectangle);
    try std.testing.expectEqual(@as(u16, 1), rectangle.active.start.column);
    try std.testing.expectEqual(@as(u16, 3), rectangle.active.end.column);
}
test "double click drag through empty cells uses the nearest word endpoints" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 20,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha beta");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(4);
    try manager.attach(attachment);

    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 2, 1, 0, false));
    const dragged = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 15, 0, 159, false),
    );

    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(dragged));
    try std.testing.expectEqual(@as(u16, 0), dragged.active.start.column);
    try std.testing.expectEqual(@as(u16, 9), dragged.active.end.column);
}
test "modifier triple click drag expands across semantic output blocks" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 10,
        .rows = 8,
    });
    defer terminal.deinit(std.testing.allocator);
    terminal.screens.active.cursorSetSemanticContent(.output);
    try terminal.screens.active.testWriteString("one\n");
    terminal.screens.active.cursorSetSemanticContent(.{ .prompt = .initial });
    try terminal.screens.active.testWriteString("$ ");
    terminal.screens.active.cursorSetSemanticContent(.{ .input = .clear_explicit });
    try terminal.screens.active.testWriteString("run\n");
    terminal.screens.active.cursorSetSemanticContent(.output);
    try terminal.screens.active.testWriteString("two\nthree");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(4);
    try manager.attach(attachment);

    const first = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureStart(attachment, 3, 1, 0, true),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(first));
    try std.testing.expectEqual(@as(u64, 0), first.active.start.row);
    try std.testing.expectEqual(@as(u64, 0), first.active.end.row);

    const expanded = try manager.applyGesture(
        &terminal,
        1,
        1,
        gestureUpdate(attachment, 1, 3, 19, false),
    );
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(expanded));
    try std.testing.expectEqual(@as(u64, 0), expanded.active.start.row);
    try std.testing.expectEqual(@as(u64, 3), expanded.active.end.row);
}
test "explicit range command preserves endpoints and rectangle semantics" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("abcdef");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(5);
    try manager.attach(attachment);

    const selected = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .{ .set_range = .{
            .end = .{ .column = 4, .row = 0 },
            .rectangle = true,
            .start = .{ .column = 1, .row = 0 },
        } },
        .generation = 1,
        .revision = 1,
    });

    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(selected));
    try std.testing.expectEqual(@as(u16, 1), selected.active.start.column);
    try std.testing.expectEqual(@as(u16, 4), selected.active.end.column);
    try std.testing.expect(selected.active.rectangle);
}
test "gesture update and end require an active press" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    for ([_]SelectionGesture{
        gestureUpdate(attachment, 1, 0, 19, false),
        gestureEnd(attachment, 1, 0),
    }) |request| {
        try std.testing.expectError(
            error.InvalidRequest,
            manager.applyGesture(&terminal, 1, 1, request),
        );
        const state = findState(&manager, attachment).?;
        try std.testing.expect(state.selection_state.gesture.left_click_pin == null);
        try std.testing.expect(!state.selection_state.gesture_active);
    }
}
test "row request sequences are strictly monotonic and allocation failure is retryable" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
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
    try std.testing.expectError(
        error.InvalidRequest,
        manager.readRows(std.testing.allocator, &terminal, 1, 1, 1, 1, .{
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
        }),
    );

    var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{ .fail_index = 0 });
    try std.testing.expectError(
        error.OutOfMemory,
        manager.readRows(failing.allocator(), &terminal, 1, 1, 1, 1, .{
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
        }),
    );
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
    // THCR-029: a replay of the already-applied window 1 is an idempotent
    // no-op even while window 2 is pending, and it must not clear the newer
    // flight.
    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);
    try std.testing.expect(findState(&manager, attachment).?.pending != null);
    try manager.rowsApplied(&terminal, attachment, 2, 1, 1);
}
test "history events wait for pending viewport rows" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(6);
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
    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.pending != null);

    try terminal.printString("x");
    manager.noteMutation(&terminal, 2, 1);

    try std.testing.expect((try manager.takeHistoryEvent(&terminal, attachment, 2, 1)) == null);
    try std.testing.expect(state.history_dirty);

    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);
    const event = (try manager.takeHistoryEvent(&terminal, attachment, 2, 1)).?;
    try std.testing.expect(attachment.eql(event.attachment_id));
    try std.testing.expectEqual(@as(u64, 2), event.revision);
    try std.testing.expectEqual(@as(u64, 1), event.generation);
    try std.testing.expectEqual(@as(u64, 2), event.total_rows);
    try manager.rollbackHistoryEvent(attachment, event);
    const retried = (try manager.takeHistoryEvent(&terminal, attachment, 2, 1)).?;
    // A rolled-back event is retried with the identical content but a fresh,
    // monotonic sequence identity.
    try std.testing.expectEqual(event.anchor_status, retried.anchor_status);
    try std.testing.expect(event.attachment_id.eql(retried.attachment_id));
    try std.testing.expectEqual(event.generation, retried.generation);
    try std.testing.expectEqual(event.revision, retried.revision);
    try std.testing.expectEqual(event.total_rows, retried.total_rows);
    try std.testing.expectEqual(event.sequence + 1, retried.sequence);
    try manager.historyObserved(attachment, retried.sequence, retried.revision, retried.generation);
    // A duplicate acknowledgement of the observed sequence is an idempotent no-op.
    try manager.historyObserved(attachment, retried.sequence, retried.revision, retried.generation);
}
test "invalid selection command preserves an active gesture" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 1, 0, 0, false));
    const gesture_anchor = findState(&manager, attachment).?.selection_state.gesture.left_click_pin.?;
    const tracked_pins = terminal.screens.active.pages.countTrackedPins();

    try std.testing.expectError(
        error.InvalidPoint,
        manager.applySelectionCommand(&terminal, 1, 1, .{
            .attachment_id = attachment,
            .command = .{ .select_output = .{ .column = 0, .row = 10_000 } },
            .generation = 1,
            .revision = 1,
        }),
    );
    const state = findState(&manager, attachment).?;
    try std.testing.expectEqual(gesture_anchor, state.selection_state.gesture.left_click_pin.?);
    try std.testing.expect(state.selection_state.gesture_active);
    try std.testing.expect(state.selection_state.selection == null);
    try std.testing.expectEqual(tracked_pins, terminal.screens.active.pages.countTrackedPins());
}
test "screen changes invalidate anchors gesture selection and export" {
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
    try manager.setDemand(&terminal, attachment, .viewing_history);
    const initial_pins = terminal.screens.active.pages.countTrackedPins();
    const initial_history_event = (try manager.takeHistoryEvent(
        &terminal,
        attachment,
        1,
        1,
    )).?;
    try std.testing.expectEqual(@as(u64, 1), initial_history_event.revision);

    const first_rows = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 1, 1, .{
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
    defer std.testing.allocator.free(first_rows.bytes);
    try manager.rowsApplied(&terminal, attachment, 1, 1, 1);
    const second_rows = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 1, 1, .{
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
    defer std.testing.allocator.free(second_rows.bytes);
    _ = try manager.applyGesture(&terminal, 1, 1, gestureStart(attachment, 2, 0, 0, false));
    const started = try manager.startExport(&terminal, attachment, 7, .plain);
    const active_state = findState(&manager, attachment).?;
    try std.testing.expect(active_state.committed != null);
    try std.testing.expect(active_state.pending != null);
    try std.testing.expect(active_state.selection_state.gesture.left_click_pin != null);
    try std.testing.expect(active_state.selection_state.gesture_active);
    try std.testing.expect(active_state.selection_state.selection != null);
    try std.testing.expect(active_state.export_ledger.active != null);

    manager.screenChanged(&terminal);

    const invalidated = findState(&manager, attachment).?;
    try std.testing.expect(invalidated.committed == null);
    try std.testing.expect(invalidated.pending == null);
    try std.testing.expect(invalidated.selection_state.gesture.left_click_pin == null);
    try std.testing.expect(!invalidated.selection_state.gesture_active);
    try std.testing.expect(invalidated.selection_state.selection == null);
    try std.testing.expect(invalidated.export_ledger.active == null);
    try std.testing.expect(invalidated.history_dirty);
    try std.testing.expect(invalidated.history_in_flight == null);
    try std.testing.expectEqual(initial_pins, terminal.screens.active.pages.countTrackedPins());
    try std.testing.expectEqual(
        SelectionSnapshot.evicted,
        std.meta.activeTag((try manager.takeSelectionEvent(attachment)).?),
    );
    const cancellation = (try manager.takeSelectionExportCancellation(attachment)).?;
    try std.testing.expect(attachment.eql(cancellation.attachment_id));
    try std.testing.expectEqual(started.byte_length, cancellation.byte_length);
    try std.testing.expectEqual(started.export_sequence, cancellation.export_sequence);
    try std.testing.expect((try manager.takeSelectionExportCancellation(attachment)) == null);
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.rowsApplied(&terminal, attachment, 2, 1, 1),
    );
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.cancelExport(attachment, 7),
    );
    // A late acknowledgement of the abandoned event is an idempotent no-op,
    // while a sequence that was never issued is still rejected.
    try manager.historyObserved(attachment, initial_history_event.sequence, 1, 1);
    try std.testing.expectError(
        error.InvalidAcknowledgement,
        manager.historyObserved(attachment, 99, 1, 1),
    );
    const current_history_event = (try manager.takeHistoryEvent(
        &terminal,
        attachment,
        2,
        2,
    )).?;
    try std.testing.expectEqual(@as(u64, 2), current_history_event.revision);
    try std.testing.expectEqual(@as(u64, 2), current_history_event.generation);
    try std.testing.expectEqual(initial_history_event.sequence + 1, current_history_event.sequence);
    try manager.historyObserved(
        attachment,
        current_history_event.sequence,
        current_history_event.revision,
        current_history_event.generation,
    );
}
