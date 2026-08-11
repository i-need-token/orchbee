const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const subject = @import("terminal-host-engine").history;

const findState = subject.testing.findStateFn;
const Manager = subject.Manager;
const max_row_window_bytes = subject.testing.row_window_byte_limit;
const max_pending_export_cancellations = subject.max_pending_export_cancellations;
const Detail = subject.Detail;
const ExportCancellation = subject.ExportCancellation;
const SelectionGestureAction = subject.SelectionGestureAction;
const SelectionSnapshot = subject.SelectionSnapshot;
const AttachmentId = subject.AttachmentId;
const HostEpoch = subject.HostEpoch;

const RecordingAllocator = struct {
    backing: std.mem.Allocator,
    alloc_request_count: usize = 0,
    free_count: usize = 0,
    max_requested: usize = 0,
    row_window_request_count: usize = 0,

    fn init(backing: std.mem.Allocator) RecordingAllocator {
        return .{ .backing = backing };
    }

    fn allocator(self: *RecordingAllocator) std.mem.Allocator {
        return .{ .ptr = self, .vtable = &vtable };
    }

    fn reset(self: *RecordingAllocator) void {
        self.alloc_request_count = 0;
        self.free_count = 0;
        self.max_requested = 0;
        self.row_window_request_count = 0;
    }

    fn recordRequest(self: *RecordingAllocator, len: usize) void {
        self.max_requested = @max(self.max_requested, len);
        if (len == protocol.Limits.row_window_bytes) self.row_window_request_count += 1;
    }

    fn alloc(
        context: *anyopaque,
        len: usize,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) ?[*]u8 {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.alloc_request_count += 1;
        self.recordRequest(len);
        return self.backing.rawAlloc(len, alignment, return_address);
    }

    fn resize(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) bool {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.recordRequest(new_len);
        return self.backing.rawResize(memory, alignment, new_len, return_address);
    }

    fn remap(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) ?[*]u8 {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.recordRequest(new_len);
        return self.backing.rawRemap(memory, alignment, new_len, return_address);
    }

    fn free(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) void {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.free_count += 1;
        self.backing.rawFree(memory, alignment, return_address);
    }

    const vtable: std.mem.Allocator.VTable = .{
        .alloc = alloc,
        .resize = resize,
        .remap = remap,
        .free = free,
    };
};

fn attachmentId(byte: u8) AttachmentId {
    return AttachmentId.fromBytes([_]u8{byte} ** 16);
}

fn hostEpoch(byte: u8) HostEpoch {
    return HostEpoch.fromBytes([_]u8{byte} ** 16);
}

fn readSingleRow(
    terminal: *ghostty_vt.Terminal,
    manager: *Manager,
    attachment: AttachmentId,
    detail: Detail,
    request_sequence: u64,
) ![]u8 {
    try manager.attach(attachment);
    const rows = try manager.readRows(std.testing.allocator, terminal, 1, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = detail,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = request_sequence,
        .revision = 1,
        .row_count = 1,
    });
    return rows.bytes;
}

test "idle manager owns no tracked pins or row capture" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const initial_pins = terminal.screens.active.pages.countTrackedPins();
    try manager.attach(attachmentId(2));
    try std.testing.expectEqual(@as(usize, 1), manager.states.items.len);
    try std.testing.expectEqual(initial_pins, terminal.screens.active.pages.countTrackedPins());
}

test "text row window is bounded and cross-language valid" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha\r\nbeta");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    const rows = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = 1,
        .revision = 1,
        .row_count = 2,
    });
    defer std.testing.allocator.free(rows.bytes);
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u16, 2), summary.row_count);
    try std.testing.expectEqual(@as(u8, 0), rows.bytes[5]);
    try std.testing.expect(rows.bytes[112] & 0x04 != 0);
    try std.testing.expectEqual(
        @as(u32, 0),
        std.mem.readInt(u32, rows.bytes[120..124], .little),
    );

    rows.bytes[5] = 1;
    try std.testing.expectError(
        error.UnsupportedVersion,
        protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes),
    );
}

test "render row window writes directly into caller-owned storage" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("direct");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(9);
    try manager.attach(attachment);
    var storage: [4096]u8 = undefined;
    const rows = try manager.readRowsInto(
        std.testing.allocator,
        &storage,
        &terminal,
        1,
        1,
        8,
        16,
        .{
            .attachment_id = attachment,
            .detail = .render,
            .generation = 1,
            .host_epoch = hostEpoch(1),
            .maximum_bytes = storage.len,
            .origin = .{ .absolute = 0 },
            .purpose = .search,
            .request_sequence = 1,
            .revision = 1,
            .row_count = 1,
        },
    );
    try std.testing.expect(rows.bytes.ptr == &storage);
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
    try std.testing.expectEqual(protocol.RowWindowDetail.render, summary.detail);
    try std.testing.expectEqual(@as(u16, 1), summary.row_count);
}

test "one empty text row fits exact caller storage with one-kibibyte scratch" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 1,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x0a);
    try manager.attach(attachment);

    var destination: [124]u8 = undefined;
    var scratch_storage: [1024]u8 align(@alignOf(ghostty_vt.Pin)) = undefined;
    var scratch = std.heap.FixedBufferAllocator.init(scratch_storage[0..]);
    const rows = try manager.readRowsInto(
        scratch.allocator(),
        destination[0..],
        &terminal,
        1,
        1,
        8,
        16,
        .{
            .attachment_id = attachment,
            .detail = .text,
            .generation = 1,
            .host_epoch = hostEpoch(1),
            .maximum_bytes = destination.len,
            .origin = .{ .absolute = 0 },
            .purpose = .search,
            .request_sequence = 1,
            .revision = 1,
            .row_count = 1,
        },
    );

    try std.testing.expect(rows.bytes.ptr == destination[0..].ptr);
    try std.testing.expectEqual(@as(usize, 124), rows.bytes.len);
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u16, 1), summary.columns);
    try std.testing.expectEqual(@as(u16, 1), summary.row_count);
    try std.testing.expectEqual(@as(u32, 0), summary.entry_count);
}

test "empty text row scratch is exactly bounded by requested pin storage" {
    const maximum_rows: u16 = protocol.Limits.row_window_rows;
    const maximum_serialized_bytes = protocol.terminal_row_window_header_bytes +
        @as(usize, maximum_rows) * 12;
    const maximum_pin_storage_bytes = @sizeOf(ghostty_vt.Pin) * @as(usize, maximum_rows);
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 1,
        .max_scrollback = 10_000_000,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    while (terminal.screens.active.pages.total_rows < maximum_rows) {
        try terminal.printString("\n");
    }
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x0b);
    try manager.attach(attachment);
    var destination: [maximum_serialized_bytes]u8 = undefined;
    var scratch_storage: [maximum_pin_storage_bytes]u8 align(@alignOf(ghostty_vt.Pin)) = undefined;

    for ([_]u16{ 1, maximum_rows }, 1..) |row_count, request_sequence| {
        const pin_storage_bytes = @sizeOf(ghostty_vt.Pin) * @as(usize, row_count);
        var fixed = std.heap.FixedBufferAllocator.init(scratch_storage[0..pin_storage_bytes]);
        var recording = RecordingAllocator.init(fixed.allocator());
        recording.reset();
        const rows = try manager.readRowsInto(
            recording.allocator(),
            destination[0..],
            &terminal,
            1,
            1,
            8,
            16,
            .{
                .attachment_id = attachment,
                .detail = .text,
                .generation = 1,
                .host_epoch = hostEpoch(1),
                .maximum_bytes = destination.len,
                .origin = .{ .absolute = 0 },
                .purpose = .search,
                .request_sequence = @intCast(request_sequence),
                .revision = 1,
                .row_count = row_count,
            },
        );

        const expected_length = protocol.terminal_row_window_header_bytes +
            @as(usize, row_count) * 12;
        try std.testing.expectEqual(expected_length, rows.bytes.len);
        try std.testing.expect(rows.bytes.len <= protocol.Limits.row_window_bytes);
        const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
        try std.testing.expectEqual(row_count, summary.row_count);
        try std.testing.expectEqual(pin_storage_bytes, recording.max_requested);
        try std.testing.expectEqual(@as(usize, 1), recording.alloc_request_count);
        try std.testing.expectEqual(@as(usize, 1), recording.free_count);
        try std.testing.expectEqual(@as(usize, 0), recording.row_window_request_count);
        try std.testing.expect(recording.max_requested < protocol.Limits.row_window_bytes / 8);
    }
}

test "failed row encoding releases pin storage before request state can commit" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 1,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x0c);
    try manager.attach(attachment);
    var destination: [protocol.terminal_row_window_header_bytes]u8 = undefined;
    var recording = RecordingAllocator.init(std.testing.allocator);

    try std.testing.expectError(
        error.ProjectionTooLarge,
        manager.readRowsInto(
            recording.allocator(),
            destination[0..],
            &terminal,
            1,
            1,
            8,
            16,
            .{
                .attachment_id = attachment,
                .detail = .text,
                .generation = 1,
                .host_epoch = hostEpoch(1),
                .maximum_bytes = destination.len,
                .origin = .{ .absolute = 0 },
                .purpose = .viewport,
                .request_sequence = 1,
                .revision = 1,
                .row_count = 1,
            },
        ),
    );
    try std.testing.expectEqual(@sizeOf(ghostty_vt.Pin), recording.max_requested);
    try std.testing.expectEqual(@as(usize, 1), recording.alloc_request_count);
    try std.testing.expectEqual(@as(usize, 1), recording.free_count);
    try std.testing.expectEqual(@as(usize, 0), recording.row_window_request_count);
    const state = findState(&manager, attachment).?;
    try std.testing.expectEqual(@as(u64, 0), state.last_row_request_sequence);
    try std.testing.expect(state.pending == null);
}

test "render row pixel height rejects before consuming viewport request state" {
    const valid_row_count: u16 = 3_640;
    const invalid_row_count: u16 = valid_row_count + 1;
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .max_scrollback = 10_000_000,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    while (terminal.screens.active.pages.total_rows < invalid_row_count) {
        try terminal.printString("\n");
    }
    try std.testing.expectEqual(
        @as(usize, invalid_row_count),
        terminal.screens.active.pages.total_rows,
    );

    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(10);
    try manager.attach(attachment);
    const valid = try manager.readRows(
        std.testing.allocator,
        &terminal,
        1,
        1,
        8,
        18,
        .{
            .attachment_id = attachment,
            .detail = .render,
            .generation = 1,
            .host_epoch = hostEpoch(1),
            .maximum_bytes = max_row_window_bytes,
            .origin = .{ .absolute = 0 },
            .purpose = .search,
            .request_sequence = 1,
            .revision = 1,
            .row_count = valid_row_count,
        },
    );
    defer std.testing.allocator.free(valid.bytes);
    const valid_summary = try protocol.validateTerminalRowWindow(
        std.testing.allocator,
        valid.bytes,
    );
    try std.testing.expectEqual(valid_row_count, valid_summary.row_count);

    const destination = try std.testing.allocator.alloc(u8, max_row_window_bytes);
    defer std.testing.allocator.free(destination);
    var viewport_request: subject.ReadRequest = .{
        .attachment_id = attachment,
        .detail = .render,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .viewport,
        .request_sequence = 2,
        .revision = 1,
        .row_count = invalid_row_count,
    };
    try std.testing.expectError(
        error.ProjectionTooLarge,
        manager.readRowsInto(
            std.testing.allocator,
            destination,
            &terminal,
            1,
            1,
            8,
            18,
            viewport_request,
        ),
    );
    const state_after_rejection = findState(&manager, attachment).?;
    try std.testing.expectEqual(@as(u64, 1), state_after_rejection.last_row_request_sequence);
    try std.testing.expect(state_after_rejection.pending == null);

    viewport_request.row_count = valid_row_count;
    const retried = try manager.readRowsInto(
        std.testing.allocator,
        destination,
        &terminal,
        1,
        1,
        8,
        18,
        viewport_request,
    );
    const retried_summary = try protocol.validateTerminalRowWindow(
        std.testing.allocator,
        retried.bytes,
    );
    try std.testing.expectEqual(valid_row_count, retried_summary.row_count);
    try std.testing.expectEqual(@as(u64, 2), retried_summary.sequence);
    try manager.rowsApplied(&terminal, attachment, 2, 1, 1);
}

test "text row encoding preserves ordinary ASCII identity" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("abc");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(3);
    const rows = try readSingleRow(&terminal, &manager, attachment, .text, 1);
    defer std.testing.allocator.free(rows);

    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u32, 3), summary.entry_count);
    try std.testing.expectEqual(@as(usize, 127), rows.len);
    try std.testing.expectEqual(@as(u8, 0x04), rows[112] & 0x04);
    try std.testing.expectEqual(@as(u32, 3), std.mem.readInt(u32, rows[116..120], .little));
    try std.testing.expectEqual(@as(u32, 0), std.mem.readInt(u32, rows[120..124], .little));
    try std.testing.expectEqualSlices(u8, "abc", rows[124..127]);
}

test "text row encoding preserves combining grapheme bytes and mappings" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("e\u{0301}");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(4);
    const rows = try readSingleRow(&terminal, &manager, attachment, .text, 1);
    defer std.testing.allocator.free(rows);

    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows);
    const expected_text = [_]u8{ 'e', 0xcc, 0x81 };
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u32, 1), summary.entry_count);
    try std.testing.expectEqual(@as(u8, 0), rows[112] & 0x04);
    try std.testing.expectEqual(@as(u32, 3), std.mem.readInt(u32, rows[116..120], .little));
    try std.testing.expectEqual(@as(u32, 1), std.mem.readInt(u32, rows[120..124], .little));
    try std.testing.expectEqualSlices(u8, &expected_text, rows[124..127]);
    try std.testing.expectEqual(@as(u32, 0), std.mem.readInt(u32, rows[127..131], .little));
    try std.testing.expectEqual(@as(u32, 0), std.mem.readInt(u32, rows[131..135], .little));
    try std.testing.expectEqual(@as(u16, 0), std.mem.readInt(u16, rows[135..137], .little));
    try std.testing.expectEqual(@as(u8, 1), rows[137]);
    try std.testing.expectEqual(@as(u8, 0), rows[138]);
}

test "render row encoding preserves combining grapheme string mapping" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("e\u{0301}");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(5);
    const rows = try readSingleRow(&terminal, &manager, attachment, .render, 1);
    defer std.testing.allocator.free(rows);

    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows);
    const frame = rows[112..];
    const expected_text = [_]u8{ 'e', 0xcc, 0x81 };
    try std.testing.expectEqual(protocol.RowWindowDetail.render, summary.detail);
    try std.testing.expectEqual(@as(u32, 8), summary.entry_count);
    try std.testing.expectEqualSlices(u8, "OBPB", frame[0..4]);
    try std.testing.expectEqual(@as(u16, 1), std.mem.readInt(u16, frame[18..20], .little));
    try std.testing.expectEqual(@as(u32, 8), std.mem.readInt(u32, frame[20..24], .little));
    try std.testing.expectEqual(@as(u32, 3), std.mem.readInt(u32, frame[140..144], .little));
    const grapheme_offsets_start = std.mem.readInt(u32, frame[184..188], .little);
    const grapheme_lengths_start = std.mem.readInt(u32, frame[192..196], .little);
    const widths_start = std.mem.readInt(u32, frame[256..260], .little);
    const strings_start = std.mem.readInt(u32, frame[280..284], .little);
    try std.testing.expectEqualSlices(
        u8,
        &expected_text,
        frame[strings_start..][0..expected_text.len],
    );
    inline for (0..8) |column| {
        try std.testing.expectEqual(
            if (column == 0) @as(u32, 0) else @as(u32, 3),
            std.mem.readInt(
                u32,
                frame[grapheme_offsets_start + column * 4 ..][0..4],
                .little,
            ),
        );
        try std.testing.expectEqual(
            if (column == 0) @as(u16, 3) else @as(u16, 0),
            std.mem.readInt(
                u16,
                frame[grapheme_lengths_start + column * 2 ..][0..2],
                .little,
            ),
        );
        try std.testing.expectEqual(@as(u8, 0), frame[widths_start + column]);
    }
}

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
        manager.applyGesture(&terminal, 1, 1, .{
            .action = .start,
            .attachment_id = attachment,
            .click_count = 0,
            .generation = 1,
            .point = .{ .column = 0, .row = 0 },
            .rectangle = false,
            .revision = 1,
        }),
    );
    const state = findState(&manager, attachment).?;
    try std.testing.expect(state.gesture_anchor == null);
    try std.testing.expect(state.selection == null);
    try std.testing.expectEqual(initial_pins, terminal.screens.active.pages.countTrackedPins());
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

    const pressed = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 2, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    try std.testing.expectEqual(SelectionSnapshot.none, std.meta.activeTag(pressed));

    const released = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .end,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 2, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
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

    _ = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 0, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    const dragged = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .update,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 2, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(dragged));
    try std.testing.expectEqual(@as(u16, 0), dragged.active.start.column);
    try std.testing.expectEqual(@as(u16, 2), dragged.active.end.column);
    _ = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .end,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 2, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });

    const word = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 2,
        .generation = 1,
        .point = .{ .column = 7, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(word));
    try std.testing.expectEqual(@as(u16, 6), word.active.start.column);
    try std.testing.expectEqual(@as(u16, 9), word.active.end.column);

    const line = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 3,
        .generation = 1,
        .point = .{ .column = 1, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(line));
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

test "every gesture phase rejects an invalid click count" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(2);
    try manager.attach(attachment);
    _ = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 0, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });

    inline for (.{ SelectionGestureAction.update, SelectionGestureAction.end }) |action| {
        try std.testing.expectError(
            error.InvalidRequest,
            manager.applyGesture(&terminal, 1, 1, .{
                .action = action,
                .attachment_id = attachment,
                .click_count = 0,
                .generation = 1,
                .point = .{ .column = 1, .row = 0 },
                .rectangle = false,
                .revision = 1,
            }),
        );
        try std.testing.expect(findState(&manager, attachment).?.gesture_anchor != null);
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
    _ = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 1,
        .generation = 1,
        .point = .{ .column = 0, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    const gesture_anchor = findState(&manager, attachment).?.gesture_anchor.?.pin;
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
    try std.testing.expectEqual(gesture_anchor, state.gesture_anchor.?.pin);
    try std.testing.expect(state.selection == null);
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
    _ = try manager.applyGesture(&terminal, 1, 1, .{
        .action = .start,
        .attachment_id = attachment,
        .click_count = 2,
        .generation = 1,
        .point = .{ .column = 0, .row = 0 },
        .rectangle = false,
        .revision = 1,
    });
    const started = try manager.startExport(&terminal, attachment, 7, .plain);
    const active_state = findState(&manager, attachment).?;
    try std.testing.expect(active_state.committed != null);
    try std.testing.expect(active_state.pending != null);
    try std.testing.expect(active_state.gesture_anchor != null);
    try std.testing.expect(active_state.selection != null);
    try std.testing.expect(active_state.export_state != null);

    manager.screenChanged(&terminal);

    const invalidated = findState(&manager, attachment).?;
    try std.testing.expect(invalidated.committed == null);
    try std.testing.expect(invalidated.pending == null);
    try std.testing.expect(invalidated.gesture_anchor == null);
    try std.testing.expect(invalidated.selection == null);
    try std.testing.expect(invalidated.export_state == null);
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
    try std.testing.expect(state.selection != null);
    try std.testing.expect(state.export_state == null);
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
    if (state.selection) |*selection| selection.startPtr().garbage = true;

    manager.noteMutation(&terminal, 2, 1);

    try std.testing.expect(state.selection == null);
    try std.testing.expect(state.export_state == null);
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
    if (state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 2, 1);

    _ = try manager.applySelectionCommand(&terminal, 2, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 2,
    });
    const second_started = try manager.startExport(&terminal, attachment, 10, .plain);
    if (state.selection) |*selection| selection.startPtr().garbage = true;
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
    state.export_state.?.chunk_sequence = std.math.maxInt(u32);
    const chunk = (try manager.encodeNextExportChunk(std.testing.allocator, attachment)).?;
    defer std.testing.allocator.free(chunk.bytes);
    try std.testing.expectEqual(std.math.maxInt(u32), chunk.token.chunk_sequence);
    const finished = (try manager.exportChunkApplied(
        attachment,
        1,
        std.math.maxInt(u32),
    )).?;
    try std.testing.expectEqual(@as(u64, 1), finished.export_sequence);
    try std.testing.expect(state.export_state == null);
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
    if (state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 2, 1);

    _ = try manager.applySelectionCommand(&terminal, 2, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 2,
    });
    const second_started = try manager.startExport(&terminal, attachment, 2, .plain);
    if (state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 3, 1);

    _ = try manager.applySelectionCommand(&terminal, 3, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 3,
    });

    try std.testing.expectEqual(@as(usize, max_pending_export_cancellations), state.invalidated_export_count);
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
    if (state.selection) |*selection| selection.startPtr().garbage = true;
    manager.noteMutation(&terminal, 4, 1);
    try std.testing.expectEqual(@as(u64, 3), state.invalidated_exports[1].export_sequence);
    try std.testing.expectEqual(@as(usize, 1), state.pending_export_cancellation_count);
    try std.testing.expectEqual(@as(u64, 3), state.pending_export_cancellations[0].export_sequence);

    manager.rollbackExportStart(
        attachment,
        replacement.export_sequence,
        replacement_transaction.previous_completed_export_sequence,
        replacement_transaction.previous_last_export_sequence,
        cancellations[0..cancellation_count],
        invalidation_history,
    );

    try std.testing.expect(state.export_state == null);
    try std.testing.expectEqual(@as(usize, max_pending_export_cancellations), state.invalidated_export_count);
    try std.testing.expectEqual(@as(u64, 1), state.invalidated_exports[0].export_sequence);
    try std.testing.expectEqual(@as(u64, 2), state.invalidated_exports[1].export_sequence);
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

// THCR-029 regression: an exact duplicate of the already-applied viewport row
// window acknowledgement must be an idempotent no-op even while a newer row
// window is pending, and it must never clear or advance the newer flight.
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

// THCR-029 regression: a normal mutation refreshes the committed anchor's
// revision and generation, but the immutable acknowledgement tuple of the
// applied rows must remain replayable as a no-op without touching the
// mutation-refreshed anchor.
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

// THCR-029 regression: replaying the acknowledgement of the last completed
// export must be an idempotent no-op even while a newer export is in flight,
// and it must not advance the newer export's offset, chunk, or in-flight state.
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
    const export_before = state.export_state.?;

    // Replaying export 7's final chunk acknowledgement is an idempotent no-op
    // while export 8 is in flight.
    try std.testing.expect((try manager.exportChunkApplied(attachment, 7, 0)) == null);
    try std.testing.expectEqual(
        export_before.export_sequence,
        state.export_state.?.export_sequence,
    );
    try std.testing.expectEqual(export_before.next_offset, state.export_state.?.next_offset);
    try std.testing.expectEqual(export_before.chunk_sequence, state.export_state.?.chunk_sequence);
    try std.testing.expectEqual(export_before.chunk_in_flight, state.export_state.?.chunk_in_flight);

    _ = (try manager.exportChunkApplied(attachment, 8, 0)).?;
}
