//! Soft-wrap topology across incremental projection captures and row windows. Ghostty rewrites a
//! clean neighbouring row's wrap continuation or trailing spacer head without marking that row
//! dirty, and some edits leave the page's own adjacent rows disagreeing. Every frame, every client
//! grid merged from a snapshot and its delta, and every row window must still satisfy the
//! protocol's adjacent-row wrap and spacer-head invariants, or each viewer of the terminal rejects
//! the projection and asks again for the same rows.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const history = @import("history_harness.zig");
const subject = @import("terminal-host-engine").testing.projection;

const Projection = subject.Projection;
const max_frame_bytes = subject.testing.frame_byte_limit;

const wide_character = "\u{4e2d}";

/// An edit applied after a clean capture, and the rows it leaves behind.
const Edit = struct {
    before: []const u8,
    edit: []const u8,
};

/// Ghostty rewrites another row's metadata or final cell without marking that row dirty.
const clean_row_rewrites = [_]Edit{
    // Erasing the soft-wrapped row clears the next row's continuation.
    .{ .before = "0123456789ABCDE\r\n\r\n", .edit = "\x1b[1;1H\x1b[K" },
    // Printing over a wrapped wide character clears the spacer head above it.
    .{ .before = "123456789" ++ wide_character ++ "\r\n\r\n", .edit = "\x1b[2;1Hx" },
    // Erasing a wrapped wide character clears the spacer head above it.
    .{ .before = "123456789" ++ wide_character ++ "\r\n\r\n", .edit = "\x1b[2;1H\x1b[X" },
};

/// Ghostty leaves the page's own adjacent rows disagreeing.
const disagreeing_pages = [_]Edit{
    // A line inserted inside a soft-wrapped line is not a continuation; the row above still wraps.
    .{ .before = "0123456789ABCDEFGHIJabc", .edit = "\x1b[2;1H\x1b[L" },
    // A line deleted inside a soft-wrapped line leaves the row above wrapped.
    .{ .before = "0123456789ABCDEFGHIJabc", .edit = "\x1b[2;1H\x1b[M" },
    // Erasing the display above the cursor unwraps the rows above a continuation.
    .{ .before = "0123456789ABCDEFGHIJabc", .edit = "\x1b[1J" },
    // Erasing a row that starts with a wrapped wide character orphans the spacer head above it.
    .{ .before = "123456789" ++ wide_character ++ "\r\n\r\n", .edit = "\x1b[2;1H\x1b[2K" },
};

test "incremental projection follows wrap metadata that Ghostty rewrites on a clean row" {
    for (clean_row_rewrites) |edit| try expectConsistentProjection(edit);
}

test "projection joins rows by the row above when the page disagrees" {
    for (disagreeing_pages) |edit| try expectConsistentProjection(edit);
}

test "row windows join rows by the same topology as frames" {
    for (disagreeing_pages) |edit| {
        var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
            .cols = 10,
            .rows = 5,
        });
        defer terminal.deinit(std.testing.allocator);
        feed(&terminal, edit.before);
        feed(&terminal, edit.edit);
        var manager = history.Manager.init(std.testing.allocator);
        defer manager.deinit(&terminal);
        const attachment = history.attachmentId(3);
        try manager.attach(attachment);
        for ([_]history.Detail{ .render, .text }, 1..) |detail, sequence| {
            const rows = try manager.readRows(std.testing.allocator, &terminal, 1, 1, 8, 16, .{
                .attachment_id = attachment,
                .detail = detail,
                .generation = 1,
                .host_epoch = history.hostEpoch(1),
                .maximum_bytes = history.max_row_window_bytes,
                .origin = .{ .absolute = 0 },
                .purpose = .search,
                .request_sequence = sequence,
                .revision = 1,
                .row_count = 5,
            });
            defer std.testing.allocator.free(rows.bytes);
            _ = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
        }
    }
}

/// Captures the rows before and after `edit`. A retained client grid merges the snapshot with the
/// delta; it must stay valid and match a fresh snapshot of the same revision.
fn expectConsistentProjection(edit: Edit) !void {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 10,
        .rows = 5,
    });
    defer terminal.deinit(std.testing.allocator);
    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();

    feed(&terminal, edit.before);
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    var retained: ClientGrid = .{};
    try retained.apply(&projection, null, 1);

    feed(&terminal, edit.edit);
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try retained.apply(&projection, 1, 2);
    var fresh: ClientGrid = .{};
    try fresh.apply(&projection, null, 3);
    try std.testing.expectEqualDeep(fresh.rows, retained.rows);
}

fn feed(terminal: *ghostty_vt.Terminal, bytes: []const u8) void {
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(bytes);
}

/// One retained row as a client checks it against its neighbours.
const RetainedRow = struct {
    flags: u8,
    first_width: u8,
    last_width: u8,
};

/// The rows a client retains after applying each frame it accepted.
const ClientGrid = struct {
    rows: [5]?RetainedRow = @splat(null),

    /// Encodes a frame against `base`, or a snapshot without one, decodes it strictly, and merges
    /// its rows. The merged grid is checked as a client checks a delta's candidate.
    fn apply(self: *ClientGrid, projection: *Projection, base: ?u64, sequence: u64) !void {
        const frame = try projection.encode(.{
            .attachment_id = subject.AttachmentId.fromBytes([_]u8{2} ** 16),
            .base_generation = if (base == null) null else 1,
            .base_revision = base,
            .force_snapshot = base == null,
            .host_epoch = subject.HostEpoch.fromBytes([_]u8{1} ** 16),
            .maximum_bytes = max_frame_bytes,
            .sequence = sequence,
        });
        defer std.testing.allocator.free(frame.allocation);
        const bytes = frame.bytes;
        _ = try protocol.validateTerminalFrame(std.testing.allocator, bytes);

        const string_count = std.mem.readInt(u32, bytes[124..128], .little);
        const style_count = std.mem.readInt(u16, bytes[128..130], .little);
        const row_count = std.mem.readInt(u16, bytes[130..132], .little);
        var offset: usize = protocol.terminal_frame_header_bytes;
        for (0..string_count) |_| offset += 4 + std.mem.readInt(u32, bytes[offset..][0..4], .little);
        offset += @as(usize, style_count) * 20;
        for (0..row_count) |_| {
            const index = std.mem.readInt(u16, bytes[offset..][0..2], .little);
            const flags = bytes[offset + 2];
            const columns: usize = std.mem.readInt(u16, bytes[offset + 4 ..][0..2], .little);
            offset += 8;
            self.rows[index] = .{
                .flags = flags,
                .first_width = bytes[offset + 6] & 3,
                .last_width = bytes[offset + (columns - 1) * 8 + 6] & 3,
            };
            offset += columns * 8;
        }

        for (self.rows[0 .. self.rows.len - 1], self.rows[1..]) |upper_row, lower_row| {
            const upper = upper_row orelse continue;
            const lower = lower_row orelse continue;
            try std.testing.expectEqual(upper.flags & 1 != 0, lower.flags & 2 != 0);
            if (upper.last_width == 3) try std.testing.expectEqual(@as(u8, 1), lower.first_width);
        }
    }
};
