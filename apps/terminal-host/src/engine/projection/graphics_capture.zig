const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");

pub const ScreenIdentity = struct {
    generation: usize,
    key: ghostty_vt.ScreenSet.Key,
};

pub const ImageFormat = enum(u8) {
    gray = 1,
    gray_alpha = 2,
    rgb = 3,
    rgba = 4,
};

pub const Image = struct {
    data: []u8,
    format: ImageFormat,
    generation: u64,
    height: u32,
    id: u32,
    width: u32,

    pub fn deinit(self: *Image, allocator: std.mem.Allocator) void {
        allocator.free(self.data);
        self.* = undefined;
    }
};

pub const Placement = struct {
    column: i32,
    destination_height: u32,
    destination_width: u32,
    image_id: u32,
    internal_id: bool,
    offset_x: u32,
    offset_y: u32,
    placement_id: u32,
    row: i32,
    source_height: u32,
    source_width: u32,
    source_x: u32,
    source_y: u32,
    z: i32,
};

pub const ImageReference = struct {
    data: []const u8,
    format: ImageFormat,
    generation: u64,
    height: u32,
    id: u32,
    width: u32,
};

pub const PendingRelative = struct {
    horizontal_offset: i32,
    image_id: u32,
    key: ghostty_vt.kitty.graphics.ImageStorage.PlacementKey,
    placement: ghostty_vt.kitty.graphics.ImageStorage.Placement,
    root_key: ghostty_vt.kitty.graphics.ImageStorage.PlacementKey,
    vertical_offset: i32,
};

pub const VirtualOrigin = struct {
    column: u32,
    row: u32,
};

pub fn sameScreen(current: ?ScreenIdentity, captured: ScreenIdentity) bool {
    const value = current orelse return false;
    return value.key == captured.key and value.generation == captured.generation;
}

pub fn imageReference(image: *const ghostty_vt.kitty.graphics.Image) !ImageReference {
    if (image.id == 0 or image.generation == 0 or image.width == 0 or image.height == 0) {
        return error.ProjectionTooLarge;
    }
    const format: ImageFormat = switch (image.format) {
        .gray => .gray,
        .gray_alpha => .gray_alpha,
        .rgb => .rgb,
        .rgba => .rgba,
        .png => return error.ProjectionTooLarge,
    };
    if (image.compression != .none) return error.ProjectionTooLarge;
    const bytes_per_pixel: usize = switch (format) {
        .gray => 1,
        .gray_alpha => 2,
        .rgb => 3,
        .rgba => 4,
    };
    const pixels = std.math.mul(usize, image.width, image.height) catch
        return error.ProjectionTooLarge;
    const expected = std.math.mul(usize, pixels, bytes_per_pixel) catch
        return error.ProjectionTooLarge;
    const data = image.renderData().bytes() orelse return error.ProjectionTooLarge;
    if (expected != data.len or image.width > protocol.Limits.terminal_graphics_dimension or
        image.height > protocol.Limits.terminal_graphics_dimension)
    {
        return error.ProjectionTooLarge;
    }
    return .{
        .data = data,
        .format = format,
        .generation = image.generation,
        .height = image.height,
        .id = image.id,
        .width = image.width,
    };
}

pub fn containsImageReference(references: []const ImageReference, image_id: u32) bool {
    for (references) |reference| if (reference.id == image_id) return true;
    return false;
}

pub fn sameImages(images: []const Image, references: []const ImageReference) bool {
    if (images.len != references.len) return false;
    for (images, references) |image, reference| {
        if (image.id != reference.id or image.generation != reference.generation or
            image.width != reference.width or image.height != reference.height or
            image.format != reference.format)
        {
            return false;
        }
    }
    return true;
}

pub fn appendBoundedPlacement(
    allocator: std.mem.Allocator,
    placements: *std.ArrayList(Placement),
    placement: Placement,
) !void {
    if (placements.items.len >= protocol.Limits.terminal_graphics_placements) {
        return error.ProjectionTooLarge;
    }
    try placements.append(allocator, placement);
}

pub fn captureCanonicalPlacement(
    terminal: *const ghostty_vt.Terminal,
    image: *const ghostty_vt.kitty.graphics.Image,
    key: ghostty_vt.kitty.graphics.ImageStorage.PlacementKey,
    placement: *const ghostty_vt.kitty.graphics.ImageStorage.Placement,
    column_i64: i64,
    row_i64: i64,
    range_rows: u64,
) !?Placement {
    const grid = placement.gridSize(image.*, terminal);
    if (grid.cols == 0 or grid.rows == 0) return null;
    const bottom = row_i64 + grid.rows - 1;
    const right = column_i64 + grid.cols - 1;
    if (row_i64 >= range_rows or bottom < 0 or
        column_i64 >= terminal.cols or right < 0)
    {
        return null;
    }
    const row = std.math.cast(i32, row_i64) orelse return error.ProjectionTooLarge;
    const column = std.math.cast(i32, column_i64) orelse return error.ProjectionTooLarge;
    const source = placement.sourceRect(image.*);
    const destination = placement.pixelSize(image.*, terminal);
    const offset = placement.cellOffset(terminal);
    if (source.width == 0 or source.height == 0 or
        destination.width == 0 or destination.height == 0)
    {
        return null;
    }
    try validatePlacementGeometry(
        image,
        source.x,
        source.y,
        source.width,
        source.height,
        destination.width,
        destination.height,
    );
    return .{
        .column = column,
        .destination_height = destination.height,
        .destination_width = destination.width,
        .image_id = image.id,
        .internal_id = key.placement_id.tag == .internal,
        .offset_x = offset.x,
        .offset_y = offset.y,
        .placement_id = key.placement_id.id,
        .row = row,
        .source_height = source.height,
        .source_width = source.width,
        .source_x = source.x,
        .source_y = source.y,
        .z = placement.z,
    };
}

pub fn validatePlacementGeometry(
    image: *const ghostty_vt.kitty.graphics.Image,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
) !void {
    const source_right = std.math.add(u32, source_x, source_width) catch
        return error.ProjectionTooLarge;
    const source_bottom = std.math.add(u32, source_y, source_height) catch
        return error.ProjectionTooLarge;
    if (source_width > protocol.Limits.terminal_graphics_dimension or
        source_height > protocol.Limits.terminal_graphics_dimension or
        source_right > image.width or source_bottom > image.height or
        destination_width > protocol.Limits.terminal_graphics_destination_pixels or
        destination_height > protocol.Limits.terminal_graphics_destination_pixels)
    {
        return error.ProjectionTooLarge;
    }
}
