//! The `images` section: each screen's Kitty images as decoded pixels, and their placements on that
//! screen's pages. docs/handover-record.md#images is its reference. The section knows no Ghostty
//! type; the runtime turns a screen's image storage into these values and back.
const std = @import("std");
const wire = @import("wire.zig");

/// The decoded pixels one screen holds at most: the Host's graphics storage limit.
pub const max_screen_pixel_bytes: usize = 32 * 1024 * 1024;
/// Ghostty's own bound on an image's width and height.
pub const max_dimension: u32 = 10000;
pub const max_images: usize = 4096;
pub const max_placements: usize = 4096;

pub const ScreenKey = enum { primary, alternate };
pub const Format = enum { rgb, rgba, gray, gray_alpha };

pub const Image = struct {
    id: u32,
    number: u32,
    width: u32,
    height: u32,
    format: Format,
    transient: bool,
    implicit_id: bool,
    /// Width times height pixels in `format`; borrows the record when decoded.
    pixels: []const u8,
};

pub const PlacementId = union(enum) {
    internal: u32,
    external: u32,
};

pub const Placement = struct {
    image_id: u32,
    placement_id: PlacementId,
    /// Counted from the top of the screen's history.
    row: u32,
    column: u16,
    x_offset: u32,
    y_offset: u32,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
    columns: u32,
    rows: u32,
    z: i32,
};

pub const Screen = struct {
    key: ScreenKey,
    /// The next id the screen gives an image transmitted without one.
    next_image_id: u32,
    /// The next id the screen gives a placement without an external id.
    next_internal_placement_id: u32,
    images: []const Image,
    placements: []const Placement,
};

const ScreenCode = wire.Code(ScreenKey, &.{ .primary, .alternate });
const FormatCode = wire.Code(Format, &.{ .rgb, .rgba, .gray, .gray_alpha });

/// The bytes each pixel of `format` takes.
pub fn channels(format: Format) u32 {
    return switch (format) {
        .gray => 1,
        .gray_alpha => 2,
        .rgb => 3,
        .rgba => 4,
    };
}

const image_fixed_bytes = 4 * 4 + 1 + 2 + 4;
const placement_fixed_bytes = 4 + 1 + 4 + 4 + 2 + 4 * 8 + 4;
const screen_fixed_bytes = 1 + 4 + 4 + 2 + 2;

/// Writes `screens`, which the encoder checks as the decoder does, so an export never writes what
/// an import refuses.
pub fn encode(writer: wire.Writer, screens: []const Screen) (wire.Error || std.mem.Allocator.Error)!void {
    try check(screens);
    try writer.int(u8, @intCast(screens.len));
    for (screens) |screen| {
        try writer.int(u8, ScreenCode.encode(screen.key));
        try writer.int(u32, screen.next_image_id);
        try writer.int(u32, screen.next_internal_placement_id);
        try writer.int(u16, @intCast(screen.images.len));
        for (screen.images) |image| {
            try writer.int(u32, image.id);
            try writer.int(u32, image.number);
            try writer.int(u32, image.width);
            try writer.int(u32, image.height);
            try writer.int(u8, FormatCode.encode(image.format));
            try writer.flag(image.transient);
            try writer.flag(image.implicit_id);
            try writer.counted(u32, image.pixels);
        }
        try writer.int(u16, @intCast(screen.placements.len));
        for (screen.placements) |placement| {
            try writer.int(u32, placement.image_id);
            switch (placement.placement_id) {
                .internal => |id| {
                    try writer.int(u8, 1);
                    try writer.int(u32, id);
                },
                .external => |id| {
                    try writer.int(u8, 2);
                    try writer.int(u32, id);
                },
            }
            try writer.int(u32, placement.row);
            try writer.int(u16, placement.column);
            for ([_]u32{
                placement.x_offset,     placement.y_offset,      placement.source_x, placement.source_y,
                placement.source_width, placement.source_height, placement.columns,  placement.rows,
            }) |value| try writer.int(u32, value);
            try writer.int(i32, placement.z);
        }
    }
}

/// Reads untrusted bytes into `arena`; the pixels borrow `bytes`.
pub fn decode(arena: std.mem.Allocator, bytes: []const u8) (wire.Error || std.mem.Allocator.Error)![]const Screen {
    var reader: wire.Reader = .init(bytes);
    const screen_count = try reader.count(u8, 2, screen_fixed_bytes);
    const screens = try arena.alloc(Screen, screen_count);
    for (screens) |*screen| {
        const key = try ScreenCode.decode(try reader.int(u8));
        const next_image_id = try reader.int(u32);
        const next_internal_placement_id = try reader.int(u32);
        const images = try arena.alloc(Image, try reader.count(u16, max_images, image_fixed_bytes));
        for (images) |*image| {
            image.* = .{
                .id = try reader.int(u32),
                .number = try reader.int(u32),
                .width = try reader.int(u32),
                .height = try reader.int(u32),
                .format = try FormatCode.decode(try reader.int(u8)),
                .transient = try reader.flag(),
                .implicit_id = try reader.flag(),
                .pixels = try reader.counted(u32, max_screen_pixel_bytes),
            };
        }
        const placements = try arena.alloc(Placement, try reader.count(u16, max_placements, placement_fixed_bytes));
        for (placements) |*placement| {
            const image_id = try reader.int(u32);
            const placement_id: PlacementId = switch (try reader.int(u8)) {
                1 => .{ .internal = try reader.int(u32) },
                2 => .{ .external = try reader.int(u32) },
                else => return error.InvalidRecord,
            };
            placement.* = .{
                .image_id = image_id,
                .placement_id = placement_id,
                .row = try reader.int(u32),
                .column = try reader.int(u16),
                .x_offset = try reader.int(u32),
                .y_offset = try reader.int(u32),
                .source_x = try reader.int(u32),
                .source_y = try reader.int(u32),
                .source_width = try reader.int(u32),
                .source_height = try reader.int(u32),
                .columns = try reader.int(u32),
                .rows = try reader.int(u32),
                .z = try reader.int(i32),
            };
        }
        screen.* = .{
            .key = key,
            .next_image_id = next_image_id,
            .next_internal_placement_id = next_internal_placement_id,
            .images = images,
            .placements = placements,
        };
    }
    try reader.finish();
    try check(screens);
    return screens;
}

/// The rules both directions keep: one or two screens in order, each with an image; images in ascending id order, each with exactly its pixels and within Ghostty's
/// bounds, together within the screen's limit; each placement naming one of its screen's images,
/// external ids nonzero and each key once.
fn check(screens: []const Screen) wire.Error!void {
    if (screens.len == 0 or screens.len > 2) return error.InvalidRecord;
    for (screens, 0..) |screen, index| {
        if (index > 0 and @intFromEnum(screen.key) <= @intFromEnum(screens[index - 1].key)) return error.InvalidRecord;
        if (screen.images.len == 0) return error.InvalidRecord;
        if (screen.images.len > max_images or screen.placements.len > max_placements) return error.InvalidRecord;
        var total: usize = 0;
        for (screen.images, 0..) |image, image_index| {
            if (image.id == 0) return error.InvalidRecord;
            if (image_index > 0 and image.id <= screen.images[image_index - 1].id) return error.InvalidRecord;
            if (image.width == 0 or image.height == 0) return error.InvalidRecord;
            if (image.width > max_dimension or image.height > max_dimension) return error.InvalidRecord;
            const expected = @as(usize, image.width) * image.height * channels(image.format);
            if (image.pixels.len != expected) return error.InvalidRecord;
            total += expected;
            if (total > max_screen_pixel_bytes) return error.InvalidRecord;
        }
        for (screen.placements, 0..) |placement, placement_index| {
            if (!hasImage(screen.images, placement.image_id)) return error.InvalidRecord;
            switch (placement.placement_id) {
                .internal => {},
                .external => |id| if (id == 0) return error.InvalidRecord,
            }
            for (screen.placements[0..placement_index]) |earlier| {
                if (earlier.image_id == placement.image_id and
                    std.meta.eql(earlier.placement_id, placement.placement_id)) return error.InvalidRecord;
            }
        }
    }
}

fn hasImage(images: []const Image, id: u32) bool {
    var low: usize = 0;
    var high = images.len;
    while (low < high) {
        const middle = low + (high - low) / 2;
        if (images[middle].id == id) return true;
        if (images[middle].id < id) low = middle + 1 else high = middle;
    }
    return false;
}
