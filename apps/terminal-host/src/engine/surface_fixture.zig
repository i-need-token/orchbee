const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const projection = @import("projection/root.zig");
const row_encoding = @import("row_encoding.zig");

pub const columns: u16 = 8;
pub const rows: u16 = 6;

// One stream covers PR-01 grapheme preservation, PR-02 width topology, PR-04
// line boundaries, PR-05 styles, PR-06 colors, PR-07 semantic metadata, and
// the primary half of the PR-08 metadata transition. The linked final wide
// grapheme wraps through spacer-head; the final plain cell follows an explicit hard line.
// Two further hard lines exercise every projected style family without a
// client-side VT or hand-built style record. The final row crosses OSC 133
// prompt, input, and output boundaries around the palette-colored cell.
const corpus = "\x1b[?2027h\x1b[?25l\x1b]10;rgb:d8/de/e9\x1b\\" ++
    "\x1b]11;rgb:10/14/18\x1b\\\x1b]12;rgb:f5/a9/7f\x1b\\" ++
    "\x1b]8;;https://example.com\x1b\\A\u{4e2d}e\u{0301}" ++
    "\u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}x\u{754c}\x1b]8;;\x1b\\\r\nH\r\n" ++
    "\x1b[38;2;1;2;3mF\x1b[0m\x1b[48;2;0;0;0mB\x1b[0m" ++
    "\x1b[7mI\x1b[0m\x1b[2mT\x1b[0m\x1b[8;48;2;10;11;12mV\x1b[0m" ++
    "\x1b[5mK\x1b[0m" ++
    "\x1b[53mO\x1b[0m\x1b[9mS\x1b[0m\r\n" ++
    "\x1b[4:1m1\x1b[0m\x1b[4:2m2\x1b[0m\x1b[4:3m3\x1b[0m" ++
    "\x1b[4:4m4\x1b[0m\x1b[4:5m5\x1b[0m\x1b[2;4;58;2;7;8;9mU\x1b[0m" ++
    "\x1b[1mD\x1b[0m\x1b[3mE\x1b[0m\r\n" ++
    "\x1b]133;A\x1b\\^\x1b]133;B\x1b\\" ++
    "\x1b]4;1;rgb:aa/bb/cc\x1b\\\x1b[31mP\x1b[0m" ++
    "\x1b]133;C\x1b\\." ++
    "\x1b]2;Orchbee primary\x1b\\" ++
    "\x1b]7;file:///workspace/primary\x1b\\" ++
    "\x1b[4 q\x1b[?1h\x1b=" ++
    "\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h" ++
    "\x1b[>4;2m\x1b[=31;1u";

// This is applied to the same Ghostty terminal and Projection after the primary
// snapshot. Every asserted metadata field changes in the same terminal mutation;
// the paired runtime regression proves that 1049 advances the Host generation.
const alternate_corpus = "\x1b[?1049h\x1b[?1007l" ++
    "\x1b]2;Orchbee alternate\x1b\\" ++
    "\x1b]7;file:///workspace/alternate\x1b\\" ++
    "\x1b[5 q\x1b[?25h\x1b[?1l\x1b>" ++
    "\x1b[?2004l\x1b[?1004l\x1b[?1002l\x1b[?1006l" ++
    "\x1b[>4;0m\x1b[?1003h\x1b[?1016h\x1b[=3;1u\x1b[HALT";

// The row-window fixture uses a separate terminal so the paired OBTF corpus
// remains byte-stable. DECSCA marks Z as protected, then restores the
// unprotected pen before u; both cells still pass through Ghostty VT state.
const render_protection_corpus = "\x1b[1\"qZ\x1b[0\"qu";

// The Kitty graphics frames use a third terminal at 8x16 pixel cells. A program
// transmits and places a direct 2x2 RGB image, red over blue, across two cells
// of the second row; a line feed on the last row then scrolls it to the first
// row; finally the program deletes every image. Each step is one Host frame.
const graphics_columns: u16 = 8;
const graphics_rows: u16 = 4;
const graphics_place_corpus = "\r\n\x1b_Ga=T,t=d,f=24,i=7,p=1,s=2,v=2,c=2,r=1;/wAA/wAAAAD/AAD/\x1b\\";
const graphics_scroll_corpus = "\x1b[4;1H\n";
const graphics_delete_corpus = "\x1b_Ga=d,d=A;\x1b\\";

pub const FixtureSet = struct {
    alternate: []u8,
    primary: []u8,
    render_row_window: []u8,
    graphics: GraphicsFrames,

    pub fn deinit(self: FixtureSet, allocator: std.mem.Allocator) void {
        allocator.free(self.primary);
        allocator.free(self.alternate);
        allocator.free(self.render_row_window);
        self.graphics.deinit(allocator);
    }
};

pub const GraphicsFrames = struct {
    placed: []u8,
    scrolled: []u8,
    deleted: []u8,

    fn deinit(self: GraphicsFrames, allocator: std.mem.Allocator) void {
        allocator.free(self.placed);
        allocator.free(self.scrolled);
        allocator.free(self.deleted);
    }
};

pub fn create(io: std.Io, allocator: std.mem.Allocator) !FixtureSet {
    var terminal: ghostty_vt.Terminal = try .init(io, allocator, .{
        .cols = columns,
        .rows = rows,
    });
    defer terminal.deinit(allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(corpus);

    var capture = projection.Projection.init(allocator);
    defer capture.deinit();
    try capture.beginCapture(&terminal, 1, 1, 8, 16);
    capture.endCapture();
    const primary_frame = try capture.encode(.{
        .attachment_id = projection.AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = projection.HostEpoch.fromBytes([_]u8{0x11} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 1,
    });
    errdefer allocator.free(primary_frame.allocation);
    std.debug.assert(primary_frame.bytes.ptr == primary_frame.allocation.ptr);
    std.debug.assert(primary_frame.bytes.len == primary_frame.allocation.len);

    stream.nextSlice(alternate_corpus);
    try capture.beginCapture(&terminal, 2, 2, 8, 16);
    capture.endCapture();
    const alternate_frame = try capture.encode(.{
        .attachment_id = projection.AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = projection.HostEpoch.fromBytes([_]u8{0x11} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 2,
    });
    errdefer allocator.free(alternate_frame.allocation);
    std.debug.assert(alternate_frame.bytes.ptr == alternate_frame.allocation.ptr);
    std.debug.assert(alternate_frame.bytes.len == alternate_frame.allocation.len);
    const render_row_window = try createRenderRowWindow(io, allocator);
    errdefer allocator.free(render_row_window);
    const graphics = try createGraphicsFrames(io, allocator);
    return .{
        .alternate = alternate_frame.allocation,
        .primary = primary_frame.allocation,
        .render_row_window = render_row_window,
        .graphics = graphics,
    };
}

fn createGraphicsFrames(io: std.Io, allocator: std.mem.Allocator) !GraphicsFrames {
    var terminal: ghostty_vt.Terminal = try .init(io, allocator, .{
        .cols = graphics_columns,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = graphics_rows,
    });
    defer terminal.deinit(allocator);
    terminal.width_px = @as(u32, graphics_columns) * 8;
    terminal.height_px = @as(u32, graphics_rows) * 16;
    var stream = terminal.vtStream();
    defer stream.deinit();
    var capture = projection.Projection.init(allocator);
    defer capture.deinit();

    stream.nextSlice(graphics_place_corpus);
    const placed = try encodeGraphicsFrame(&capture, &terminal, 1);
    errdefer allocator.free(placed);
    stream.nextSlice(graphics_scroll_corpus);
    const scrolled = try encodeGraphicsFrame(&capture, &terminal, 2);
    errdefer allocator.free(scrolled);
    stream.nextSlice(graphics_delete_corpus);
    const deleted = try encodeGraphicsFrame(&capture, &terminal, 3);
    return .{ .placed = placed, .scrolled = scrolled, .deleted = deleted };
}

/// The Host promotes a frame to a snapshot when its graphics resources change.
fn encodeGraphicsFrame(
    capture: *projection.Projection,
    terminal: *ghostty_vt.Terminal,
    revision: u64,
) ![]u8 {
    try capture.beginCapture(terminal, revision, 1, 8, 16);
    capture.endCapture();
    const frame = try capture.encode(.{
        .attachment_id = projection.AttachmentId.fromBytes([_]u8{0x66} ** 16),
        .base_generation = if (revision == 1) null else 1,
        .base_revision = if (revision == 1) null else revision - 1,
        .force_snapshot = revision == 1,
        .host_epoch = projection.HostEpoch.fromBytes([_]u8{0x55} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = revision,
    });
    std.debug.assert(frame.bytes.ptr == frame.allocation.ptr);
    std.debug.assert(frame.bytes.len == frame.allocation.len);
    return frame.allocation;
}

fn createRenderRowWindow(io: std.Io, allocator: std.mem.Allocator) ![]u8 {
    var terminal: ghostty_vt.Terminal = try .init(io, allocator, .{
        .cols = columns,
        .rows = rows,
    });
    defer terminal.deinit(allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(corpus);
    stream.nextSlice(render_protection_corpus);

    const screen = terminal.screens.active;
    const start_pin = screen.pages.pin(.{ .screen = .{ .x = 0, .y = 0 } }) orelse
        return error.InvalidFixture;
    return row_encoding.encode(allocator, &terminal, .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .hard_limit = protocol.Limits.row_window_bytes,
        .request = .{
            .animation_tick = 0,
            .attachment_id = projection.AttachmentId.fromBytes([_]u8{0x44} ** 16),
            .detail = .render,
            .generation = 13,
            .host_epoch = projection.HostEpoch.fromBytes([_]u8{0x33} ** 16),
            .request_sequence = 7,
            .revision = 11,
            .theme_revision = 17,
        },
        .row_count = rows,
        .start_pin = start_pin,
        .start_row = 0,
        .total_rows = @intCast(screen.pages.total_rows),
    });
}
