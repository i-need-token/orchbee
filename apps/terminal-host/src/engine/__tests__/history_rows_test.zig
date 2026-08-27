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

test "idle manager owns no tracked pins or row capture" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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

test "current-view viewport rows survive revision churn while exact reads remain pinned" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha\r\nbeta");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x31);
    try manager.attach(attachment);

    const current = try manager.readRows(std.testing.allocator, &terminal, 2, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .tail = 0 },
        .purpose = .viewport,
        .request_sequence = 1,
        .revision = 1,
        .revision_policy = .current_view,
        .row_count = 2,
    });
    defer std.testing.allocator.free(current.bytes);
    try std.testing.expectEqual(@as(u64, 2), current.revision);
    try std.testing.expectEqual(@as(u64, 1), current.generation);
    try manager.rowsApplied(&terminal, attachment, 1, 2, 1);

    try std.testing.expectError(
        error.StaleRevision,
        manager.readRows(std.testing.allocator, &terminal, 2, 1, 8, 16, .{
            .attachment_id = attachment,
            .detail = .text,
            .generation = 1,
            .host_epoch = hostEpoch(1),
            .maximum_bytes = max_row_window_bytes,
            .origin = .{ .absolute = 0 },
            .purpose = .search,
            .request_sequence = 2,
            .revision = 1,
            .revision_policy = .exact,
            .row_count = 1,
        }),
    );
    try std.testing.expectError(
        error.StaleGeneration,
        manager.readRows(std.testing.allocator, &terminal, 2, 2, 8, 16, .{
            .attachment_id = attachment,
            .detail = .text,
            .generation = 1,
            .host_epoch = hostEpoch(1),
            .maximum_bytes = max_row_window_bytes,
            .origin = .{ .tail = 0 },
            .purpose = .viewport,
            .request_sequence = 2,
            .revision = 1,
            .revision_policy = .current_view,
            .row_count = 1,
        }),
    );
}

test "current-view row policy rejects non-viewport and absolute origins" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x32);
    try manager.attach(attachment);

    inline for (.{
        .{ .origin = subject.Origin{ .absolute = 0 }, .purpose = subject.Purpose.viewport },
        .{ .origin = subject.Origin{ .tail = 0 }, .purpose = subject.Purpose.search },
    }) |invalid| {
        try std.testing.expectError(
            error.InvalidRequest,
            manager.readRows(std.testing.allocator, &terminal, 1, 1, 8, 16, .{
                .attachment_id = attachment,
                .detail = .text,
                .generation = 1,
                .host_epoch = hostEpoch(1),
                .maximum_bytes = max_row_window_bytes,
                .origin = invalid.origin,
                .purpose = invalid.purpose,
                .request_sequence = 1,
                .revision = 1,
                .revision_policy = .current_view,
                .row_count = 1,
            }),
        );
    }
}
test "render row window writes directly into caller-owned storage" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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
test "retained history render rows include graphics while text rows exclude them" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .max_scrollback_bytes = 1_000_000,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    terminal.width_px = @as(u32, terminal.cols) * 8;
    terminal.height_px = @as(u32, terminal.rows) * 16;
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b_Ga=T,t=d,f=24,s=1,v=1,i=1,p=1,C=1,z=-1073741825;/wAA\x1b\\" ++
            "\r\none\r\ntwo",
    );
    try std.testing.expect(terminal.screens.active.pages.total_rows > terminal.rows);

    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    manager.animation_tick = 17;
    const attachment = attachmentId(0x0f);
    try manager.attach(attachment);
    const render_rows = try manager.readRows(std.testing.allocator, &terminal, 4, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = .render,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = 1,
        .revision = 4,
        .row_count = 1,
    });
    defer std.testing.allocator.free(render_rows.bytes);
    _ = try protocol.validateTerminalRowWindow(std.testing.allocator, render_rows.bytes);
    const projection_length: usize = @intCast(std.mem.readInt(
        u32,
        render_rows.bytes[101..105],
        .little,
    ));
    const graphics_length: usize = @intCast(std.mem.readInt(
        u32,
        render_rows.bytes[105..109],
        .little,
    ));
    const graphics_start = protocol.terminal_row_window_header_bytes + projection_length;
    const graphics = render_rows.bytes[graphics_start..][0..graphics_length];
    const graphics_summary = try protocol.validateTerminalGraphics(graphics);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.snapshot, graphics_summary.kind);
    try std.testing.expectEqual(@as(u16, 1), graphics_summary.image_count);
    try std.testing.expectEqual(@as(u16, 1), graphics_summary.placement_count);
    try std.testing.expectEqual(@as(u64, 17), graphics_summary.animation_tick);
    try std.testing.expectEqual(
        @as(i32, -1073741825),
        std.mem.readInt(i32, graphics[123..127], .little),
    );

    const text_rows = try manager.readRows(std.testing.allocator, &terminal, 4, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = 2,
        .revision = 4,
        .row_count = 1,
    });
    defer std.testing.allocator.free(text_rows.bytes);
    const text_summary = try protocol.validateTerminalRowWindow(std.testing.allocator, text_rows.bytes);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, text_summary.detail);
    try std.testing.expectEqual(@as(u32, 0), std.mem.readInt(
        u32,
        text_rows.bytes[101..105],
        .little,
    ));
    try std.testing.expectEqual(@as(u32, 0), std.mem.readInt(
        u32,
        text_rows.bytes[105..109],
        .little,
    ));
}
test "render row window reuses one long OSC 8 hyperlink byte range across 80 by 24 cells" {
    const columns = 80;
    const row_count = 24;
    const cell_count = columns * row_count;
    const hyperlink_prefix = "https://example.com/";
    var hyperlink = [_]u8{'a'} ** 1_061;
    @memcpy(hyperlink[0..hyperlink_prefix.len], hyperlink_prefix);
    var linked_text = [_]u8{'X'} ** cell_count;

    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = columns,
        .rows = row_count,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b]8;;");
    stream.nextSlice(&hyperlink);
    stream.nextSlice("\x1b\\");
    stream.nextSlice(&linked_text);
    stream.nextSlice("\x1b]8;;\x1b\\");

    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x0d);
    try manager.attach(attachment);
    const rows = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = .render,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = 1,
        .revision = 1,
        .row_count = row_count,
    });
    defer std.testing.allocator.free(rows.bytes);

    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
    try std.testing.expectEqual(protocol.RowWindowDetail.render, summary.detail);
    try std.testing.expectEqual(@as(u16, row_count), summary.row_count);
    try std.testing.expectEqual(@as(u32, cell_count), summary.entry_count);
    try std.testing.expect(rows.bytes.len <= max_row_window_bytes);

    const frame = rows.bytes[protocol.terminal_row_window_header_bytes..];
    const hyperlink_offsets_start: usize = @intCast(std.mem.readInt(u32, frame[200..204], .little));
    const hyperlink_lengths_start: usize = @intCast(std.mem.readInt(u32, frame[208..212], .little));
    const strings_start: usize = @intCast(std.mem.readInt(u32, frame[280..284], .little));
    const strings_length: usize = @intCast(std.mem.readInt(u32, frame[284..288], .little));
    try std.testing.expectEqual(@as(usize, cell_count + hyperlink.len), strings_length);
    for (0..cell_count) |cell_index| {
        try std.testing.expectEqual(
            @as(u32, 1),
            std.mem.readInt(
                u32,
                frame[hyperlink_offsets_start + cell_index * 4 ..][0..4],
                .little,
            ),
        );
        try std.testing.expectEqual(
            @as(u16, hyperlink.len),
            std.mem.readInt(
                u16,
                frame[hyperlink_lengths_start + cell_index * 2 ..][0..2],
                .little,
            ),
        );
    }
    const strings = frame[strings_start..][0..strings_length];
    try std.testing.expectEqual(@as(usize, 1), countByteSequence(strings, &hyperlink));
}
test "render row window preserves ProjectionTooLarge for genuinely distinct hyperlink bytes" {
    const columns = 64;
    const row_count = 16;
    const cell_count = columns * row_count;
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = columns,
        .rows = row_count,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    for (0..cell_count) |cell_index| {
        var hyperlink = [_]u8{'d'} ** 2_040;
        _ = try std.fmt.bufPrint(hyperlink[0..64], "https://example.com/{d}/", .{cell_index});
        stream.nextSlice("\x1b]8;;");
        stream.nextSlice(&hyperlink);
        stream.nextSlice("\x1b\\X\x1b]8;;\x1b\\");
    }

    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x0f);
    try manager.attach(attachment);
    try std.testing.expectError(
        error.ProjectionTooLarge,
        manager.readRows(std.testing.allocator, &terminal, 1, 1, 8, 16, .{
            .attachment_id = attachment,
            .detail = .render,
            .generation = 1,
            .host_epoch = hostEpoch(1),
            .maximum_bytes = 2 * 1024 * 1024,
            .origin = .{ .absolute = 0 },
            .purpose = .viewport,
            .request_sequence = 1,
            .revision = 1,
            .row_count = row_count,
        }),
    );
    const state = findState(&manager, attachment).?;
    try std.testing.expectEqual(@as(u64, 0), state.last_row_request_sequence);
    try std.testing.expect(state.pending == null);
}
test "render row hyperlink scratch allocations clean up without consuming request state" {
    try std.testing.checkAllAllocationFailures(
        std.testing.allocator,
        exerciseRenderRowHyperlinkAllocations,
        .{},
    );
}
test "one empty text row fits exact caller storage with one-kibibyte scratch" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 1,
        .max_scrollback_bytes = 10_000_000,
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .max_scrollback_bytes = 10_000_000,
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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

fn countByteSequence(bytes: []const u8, value: []const u8) usize {
    if (value.len == 0 or value.len > bytes.len) return 0;
    var count: usize = 0;
    var offset: usize = 0;
    while (offset <= bytes.len - value.len) : (offset += 1) {
        if (!std.mem.eql(u8, bytes[offset..][0..value.len], value)) continue;
        count += 1;
        offset += value.len - 1;
    }
    return count;
}

fn exerciseRenderRowHyperlinkAllocations(allocator: std.mem.Allocator) !void {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 2,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b]8;;https://allocation.example\x1b\\XX\x1b]8;;\x1b\\");

    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = attachmentId(0x0e);
    try manager.attach(attachment);
    var destination: [2_048]u8 = undefined;
    const rows = manager.readRowsInto(
        allocator,
        &destination,
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
            .maximum_bytes = destination.len,
            .origin = .{ .absolute = 0 },
            .purpose = .viewport,
            .request_sequence = 1,
            .revision = 1,
            .row_count = 1,
        },
    ) catch |err| {
        const state = findState(&manager, attachment).?;
        try std.testing.expectEqual(@as(u64, 0), state.last_row_request_sequence);
        try std.testing.expect(state.pending == null);
        return err;
    };
    _ = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
}
