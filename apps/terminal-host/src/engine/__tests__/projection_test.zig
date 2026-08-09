const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const subject = @import("terminal-host-engine").testing.projection;

const FrameKind = subject.FrameKind;
const max_frame_bytes = subject.testing.frame_byte_limit;
const Projection = subject.Projection;
const StringTable = subject.testing.StringTableType;
const linkTableBoundary = subject.testing.linkTableBoundaryFn;
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
    try std.testing.expectEqual(@as(usize, 1), try subject.testing.repeatedLinkCountFn(std.testing.allocator));
}

test "projection link cache enforces aggregate and distinct-entry budgets" {
    try linkTableBoundary(std.testing.allocator);
}
