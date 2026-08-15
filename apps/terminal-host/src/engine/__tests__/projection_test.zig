const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const subject = @import("terminal-host-engine").testing.projection;

const FrameKind = subject.FrameKind;
const max_frame_bytes = subject.testing.frame_byte_limit;
const max_link_cache_bytes = subject.testing.link_cache_byte_limit;
const max_link_entries = subject.testing.link_entry_limit;
const max_link_value_bytes = subject.testing.link_value_byte_limit;
const capturedStyle = subject.testing.capturedStyleFn;
const LinkTable = subject.testing.LinkTableType;
const linkTableDeinit = subject.testing.linkTableDeinitFn;
const linkTableInit = subject.testing.linkTableInitFn;
const linkTableIntern = subject.testing.linkTableInternFn;
const Projection = subject.Projection;
const StringTable = subject.testing.StringTableType;
const AttachmentId = subject.AttachmentId;
const HostEpoch = subject.HostEpoch;

test "no-demand projection owns no render state or frame allocation" {
    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try std.testing.expect(projection.render_state == null);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.render_state_creations);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.captures);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.encodes);
}

test "projection captures Ghostty state into a valid OBTF 0.0.1 snapshot" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("A\x1b[31mB");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    const frame = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(@as(u64, 1), summary.revision);
    try std.testing.expectEqual(@as(u16, 4), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.rows);
}

test "projection ignores absent style data on background-only cells" {
    var cells: std.MultiArrayList(ghostty_vt.RenderState.Cell) = .{};
    defer cells.deinit(std.testing.allocator);
    try cells.append(std.testing.allocator, .{
        .raw = .{
            .content_tag = .bg_color_rgb,
            .content = .{ .color_rgb = .{ .r = 1, .g = 2, .b = 3 } },
        },
        .grapheme = undefined,
        .style = .{ .fg_color = .{ .palette = 7 } },
    });

    const captured = capturedStyle(cells.slice(), 0);
    try std.testing.expect(captured.default());
}

test "projection completes Ghostty render updates before exposing captured styles" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 2,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b[1mA");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();

    stream.nextSlice("\x1b[H\x1b[0;3mB");
    try projection.beginCapture(&terminal, 2, 1, 8, 16);

    const state = &projection.render_state.?;
    try std.testing.expectEqual(@as(usize, 0), state.pending_styles.items.len);
    const cell = state.row_data.items(.cells)[0].get(0);
    try std.testing.expectEqual(@as(u21, 'B'), cell.raw.codepoint());
    try std.testing.expect(!cell.style.flags.bold);
    try std.testing.expect(cell.style.flags.italic);

    projection.endCapture();
}

test "projection capture deduplicates repeated hyperlink URIs" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("ABCD");
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 1), projection.link_table.values.items.len);
    try std.testing.expectEqual(@as(usize, "https://example.com".len), projection.link_table.byte_count);
    try std.testing.expect(projection.link_table.ref_counts.items[0] >= 4);
}

test "projection capture compacts a dirty linked row" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("ABCD");
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 1), projection.link_table.values.items.len);

    try terminal.printString("\x1b[H    ");
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.items.len);
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.byte_count);
}

test "projection resets all hyperlink state after an aggregate frame limit failure" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("ABCD");
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectError(error.ProjectionTooLarge, projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = 160,
        .sequence = 1,
    }));
    projection.resetAfterCaptureFailure();
    try std.testing.expect(!projection.isResident());
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.items.len);
    try std.testing.expectEqual(@as(usize, 0), projection.links.capacity);
}

test "projection release frees resident capture resources and recreates lazily" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("resident");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expect(projection.isResident());
    try std.testing.expect(projection.row_revisions.capacity > 0);

    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("link");
    terminal.screens.active.endHyperlink();
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try std.testing.expect(projection.link_table.values.items.len > 0);

    projection.releaseResources();
    try std.testing.expect(!projection.isResident());
    try std.testing.expectEqual(@as(usize, 0), projection.row_revisions.capacity);
    try std.testing.expectEqual(@as(usize, 0), projection.links.capacity);
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.capacity);
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.items.len);
    try std.testing.expect(projection.captured_title == null);
    try std.testing.expect(projection.captured_current_directory == null);

    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try std.testing.expect(projection.isResident());
    try std.testing.expectEqual(@as(u64, 2), projection.counters.render_state_creations);
}

test "projection omits invalid UTF-8 optional strings" {
    var table = StringTable.init(std.testing.allocator);
    defer table.deinit();
    const invalid = [_]u8{0xff};
    try std.testing.expectEqual(std.math.maxInt(u32), try table.addOptional(&invalid));
    try std.testing.expectEqual(std.math.maxInt(u32), table.indexOfOptional(&invalid));
}

test "projection interns repeated hyperlink URIs" {
    try std.testing.expectEqual(@as(usize, 1), try repeatedLinkCount(std.testing.allocator));
}

test "projection link cache enforces aggregate and distinct-entry budgets" {
    try linkTableBoundary(std.testing.allocator);
}

test "production projection source excludes executable test scenarios" {
    const source = subject.testing.projection_source;
    inline for (.{ "std.testing", "linkTableBoundary", "repeatedLinkCount" }) |forbidden| {
        try std.testing.expect(std.mem.indexOf(u8, source, forbidden) == null);
    }
}

fn linkTableBoundary(allocator: std.mem.Allocator) !void {
    var aggregate: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&aggregate);
    var value: [max_link_value_bytes]u8 = undefined;
    var aggregate_entry_count: usize = 0;
    while (true) : (aggregate_entry_count += 1) {
        @memset(&value, 'x');
        const prefix = try std.fmt.bufPrint(value[0..8], "{d:0>8}", .{aggregate_entry_count});
        std.debug.assert(prefix.len == 8);
        _ = linkTableIntern(&aggregate, allocator, &value) catch |err| switch (err) {
            error.ProjectionTooLarge => break,
            else => return err,
        };
    }
    try std.testing.expectEqual(aggregate_entry_count, aggregate.values.items.len);
    try std.testing.expect(aggregate.byte_count <= max_link_cache_bytes);
    try std.testing.expectEqual(max_link_cache_bytes, aggregate.byte_count);

    var distinct: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&distinct);
    var small_value: [16]u8 = undefined;
    for (0..max_link_entries) |index| {
        @memset(&small_value, ' ');
        _ = try std.fmt.bufPrint(small_value[0..8], "{d:0>8}", .{index});
        _ = try linkTableIntern(&distinct, allocator, &small_value);
    }
    @memset(&small_value, ' ');
    _ = try std.fmt.bufPrint(small_value[0..8], "{d:0>8}", .{max_link_entries});
    try std.testing.expectError(
        error.ProjectionTooLarge,
        linkTableIntern(&distinct, allocator, &small_value),
    );
    try std.testing.expectEqual(max_link_entries, distinct.values.items.len);
}

fn repeatedLinkCount(allocator: std.mem.Allocator) !usize {
    var table: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&table);
    _ = try linkTableIntern(&table, allocator, "https://example.com");
    _ = try linkTableIntern(&table, allocator, "https://example.com");
    return table.values.items.len;
}
