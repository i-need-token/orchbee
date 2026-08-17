const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");

const graphics_header_bytes = 32;
const image_record_bytes = 32;
const placement_record_bytes = 56;
const placement_flag_internal_id: u32 = 1;

const ScreenIdentity = struct {
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

    fn deinit(self: *Image, allocator: std.mem.Allocator) void {
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

const ImageReference = struct {
    data: []const u8,
    format: ImageFormat,
    generation: u64,
    height: u32,
    id: u32,
    width: u32,
};

pub const State = struct {
    allocator: std.mem.Allocator,
    images: std.ArrayList(Image) = .empty,
    placements: std.ArrayList(Placement) = .empty,
    resource_revision: u64 = 0,
    screen_identity: ?ScreenIdentity = null,

    pub fn init(allocator: std.mem.Allocator) State {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *State) void {
        self.reset();
        self.* = undefined;
    }

    pub fn reset(self: *State) void {
        for (self.images.items) |*image| image.deinit(self.allocator);
        self.images.deinit(self.allocator);
        self.placements.deinit(self.allocator);
        self.images = .empty;
        self.placements = .empty;
        self.resource_revision = 0;
        self.screen_identity = null;
    }

    pub fn capture(
        self: *State,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !void {
        const storage = &terminal.screens.active.kitty_images;
        const screen_identity: ScreenIdentity = .{
            .generation = terminal.screens.generation(terminal.screens.active_key),
            .key = terminal.screens.active_key,
        };
        var placements: std.ArrayList(Placement) = .empty;
        errdefer placements.deinit(self.allocator);
        var references: std.ArrayList(ImageReference) = .empty;
        defer references.deinit(self.allocator);

        var iterator = storage.placements.iterator();
        while (iterator.next()) |entry| {
            if (placements.items.len >= protocol.Limits.terminal_graphics_placements) {
                return error.ProjectionTooLarge;
            }
            const image = storage.images.getPtr(entry.key_ptr.image_id) orelse continue;
            const placement = capturePlacement(
                terminal,
                image,
                entry.key_ptr,
                entry.value_ptr,
                cell_pixel_width,
                cell_pixel_height,
            ) orelse continue;
            try placements.append(self.allocator, placement);
            if (!containsImageReference(references.items, image.id)) {
                if (references.items.len >= protocol.Limits.terminal_graphics_images) {
                    return error.ProjectionTooLarge;
                }
                try references.append(self.allocator, try imageReference(image));
            }
        }

        std.mem.sort(Placement, placements.items, {}, struct {
            fn lessThan(_: void, left: Placement, right: Placement) bool {
                if (left.z != right.z) return left.z < right.z;
                if (left.row != right.row) return left.row < right.row;
                if (left.column != right.column) return left.column < right.column;
                if (left.image_id != right.image_id) return left.image_id < right.image_id;
                if (left.internal_id != right.internal_id) return !left.internal_id;
                return left.placement_id < right.placement_id;
            }
        }.lessThan);
        std.mem.sort(ImageReference, references.items, {}, struct {
            fn lessThan(_: void, left: ImageReference, right: ImageReference) bool {
                return left.id < right.id;
            }
        }.lessThan);

        if (!sameScreen(self.screen_identity, screen_identity) or
            !sameImages(self.images.items, references.items))
        {
            var images: std.ArrayList(Image) = .empty;
            errdefer {
                for (images.items) |*image| image.deinit(self.allocator);
                images.deinit(self.allocator);
            }
            var total_bytes: usize = 0;
            for (references.items) |reference| {
                total_bytes = std.math.add(usize, total_bytes, reference.data.len) catch
                    return error.ProjectionTooLarge;
                if (total_bytes > protocol.Limits.terminal_graphics_resource_bytes) {
                    return error.ProjectionTooLarge;
                }
                try images.append(self.allocator, .{
                    .data = try self.allocator.dupe(u8, reference.data),
                    .format = reference.format,
                    .generation = reference.generation,
                    .height = reference.height,
                    .id = reference.id,
                    .width = reference.width,
                });
            }
            for (self.images.items) |*image| image.deinit(self.allocator);
            self.images.deinit(self.allocator);
            self.images = images;
            images = .empty;
            self.resource_revision = revision;
            self.screen_identity = screen_identity;
        }

        self.placements.deinit(self.allocator);
        self.placements = placements;
        placements = .empty;
    }

    pub fn requiresSnapshot(self: *const State, base_revision: ?u64) bool {
        return self.resource_revision > (base_revision orelse return true);
    }

    pub fn encodedLength(self: *const State, snapshot: bool) error{ProjectionTooLarge}!usize {
        var length: usize = graphics_header_bytes;
        if (snapshot) {
            for (self.images.items) |image| {
                length = std.math.add(usize, length, image_record_bytes) catch
                    return error.ProjectionTooLarge;
                length = std.math.add(usize, length, image.data.len) catch
                    return error.ProjectionTooLarge;
            }
        }
        const placement_bytes = std.math.mul(
            usize,
            self.placements.items.len,
            placement_record_bytes,
        ) catch return error.ProjectionTooLarge;
        length = std.math.add(usize, length, placement_bytes) catch
            return error.ProjectionTooLarge;
        if (length > protocol.Limits.terminal_graphics_extension_bytes or length > std.math.maxInt(u32)) {
            return error.ProjectionTooLarge;
        }
        return length;
    }

    pub fn encode(self: *const State, writer: anytype, snapshot: bool) !void {
        const length = try self.encodedLength(snapshot);
        var image_bytes: usize = 0;
        if (snapshot) {
            for (self.images.items) |image| {
                image_bytes = std.math.add(usize, image_bytes, image.data.len) catch
                    return error.ProjectionTooLarge;
            }
        }

        try writer.appendSlice(protocol.terminal_graphics_magic);
        try writer.appendInt(u8, protocol.terminal_graphics_format_major);
        try writer.appendInt(u8, protocol.terminal_graphics_format_minor);
        try writer.appendInt(u8, protocol.terminal_graphics_format_patch);
        try writer.appendInt(u8, if (snapshot) 1 else 2);
        try writer.appendInt(u32, @intCast(length));
        try writer.appendInt(u64, self.resource_revision);
        try writer.appendInt(u16, if (snapshot) @intCast(self.images.items.len) else 0);
        try writer.appendInt(u16, @intCast(self.placements.items.len));
        try writer.appendInt(u32, @intCast(image_bytes));
        try writer.appendInt(u32, 0);
        if (snapshot) {
            for (self.images.items) |image| {
                try writer.appendInt(u32, image.id);
                try writer.appendInt(u8, @intFromEnum(image.format));
                try writer.appendZeros(3);
                try writer.appendInt(u64, image.generation);
                try writer.appendInt(u32, image.width);
                try writer.appendInt(u32, image.height);
                try writer.appendInt(u32, @intCast(image.data.len));
                try writer.appendInt(u32, 0);
                try writer.appendSlice(image.data);
            }
        }
        for (self.placements.items) |placement| {
            try writer.appendInt(u32, placement.image_id);
            try writer.appendInt(u32, placement.placement_id);
            try writer.appendInt(u32, @bitCast(placement.column));
            try writer.appendInt(u32, @bitCast(placement.row));
            try writer.appendInt(u32, placement.offset_x);
            try writer.appendInt(u32, placement.offset_y);
            try writer.appendInt(u32, placement.source_x);
            try writer.appendInt(u32, placement.source_y);
            try writer.appendInt(u32, placement.source_width);
            try writer.appendInt(u32, placement.source_height);
            try writer.appendInt(u32, placement.destination_width);
            try writer.appendInt(u32, placement.destination_height);
            try writer.appendInt(u32, @bitCast(placement.z));
            try writer.appendInt(u32, if (placement.internal_id) placement_flag_internal_id else 0);
        }
    }
};

fn sameScreen(current: ?ScreenIdentity, captured: ScreenIdentity) bool {
    const value = current orelse return false;
    return value.key == captured.key and value.generation == captured.generation;
}

fn imageReference(image: *const ghostty_vt.kitty.graphics.Image) !ImageReference {
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
    if (expected != image.data.len or image.width > protocol.Limits.terminal_graphics_dimension or
        image.height > protocol.Limits.terminal_graphics_dimension)
    {
        return error.ProjectionTooLarge;
    }
    return .{
        .data = image.data,
        .format = format,
        .generation = image.generation,
        .height = image.height,
        .id = image.id,
        .width = image.width,
    };
}

fn containsImageReference(references: []const ImageReference, image_id: u32) bool {
    for (references) |reference| if (reference.id == image_id) return true;
    return false;
}

fn sameImages(images: []const Image, references: []const ImageReference) bool {
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

fn capturePlacement(
    terminal: *ghostty_vt.Terminal,
    image: *const ghostty_vt.kitty.graphics.Image,
    key: *const ghostty_vt.kitty.graphics.ImageStorage.PlacementKey,
    placement: *const ghostty_vt.kitty.graphics.ImageStorage.Placement,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) ?Placement {
    if (placement.z < 0 or cell_pixel_width == 0 or cell_pixel_height == 0) return null;
    const pin = switch (placement.location) {
        .pin => |value| value,
        .virtual => return null,
    };
    const pages = &terminal.screens.active.pages;
    const pin_screen = pages.pointFromPin(.screen, pin.*) orelse return null;
    const viewport_top = pages.pointFromPin(.screen, pages.getTopLeft(.viewport)) orelse return null;
    const row_i64 = @as(i64, @intCast(pin_screen.screen.y)) -
        @as(i64, @intCast(viewport_top.screen.y));
    const column_i64: i64 = @intCast(pin_screen.screen.x);
    const row = std.math.cast(i32, row_i64) orelse return null;
    const column = std.math.cast(i32, column_i64) orelse return null;

    const source_x = @min(placement.source_x, image.width);
    const source_y = @min(placement.source_y, image.height);
    const available_width = image.width - source_x;
    const available_height = image.height - source_y;
    const source_width = @min(
        if (placement.source_width > 0) placement.source_width else image.width,
        available_width,
    );
    const source_height = @min(
        if (placement.source_height > 0) placement.source_height else image.height,
        available_height,
    );
    if (source_width == 0 or source_height == 0) return null;

    const destination = destinationSize(
        placement.columns,
        placement.rows,
        source_width,
        source_height,
        cell_pixel_width,
        cell_pixel_height,
    ) orelse return null;
    const grid_width = divideCeil(
        @as(u64, destination.width) + placement.x_offset,
        cell_pixel_width,
    ) orelse return null;
    const grid_height = divideCeil(
        @as(u64, destination.height) + placement.y_offset,
        cell_pixel_height,
    ) orelse return null;
    const bottom = @as(i64, row) + (std.math.cast(i64, grid_height) orelse return null);
    const right = @as(i64, column) + (std.math.cast(i64, grid_width) orelse return null);
    if (bottom <= 0 or row >= terminal.rows or right <= 0 or column >= terminal.cols) return null;

    return .{
        .column = column,
        .destination_height = destination.height,
        .destination_width = destination.width,
        .image_id = image.id,
        .internal_id = key.placement_id.tag == .internal,
        .offset_x = placement.x_offset,
        .offset_y = placement.y_offset,
        .placement_id = key.placement_id.id,
        .row = row,
        .source_height = source_height,
        .source_width = source_width,
        .source_x = source_x,
        .source_y = source_y,
        .z = placement.z,
    };
}

fn destinationSize(
    columns: u32,
    rows: u32,
    source_width: u32,
    source_height: u32,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) ?struct { width: u32, height: u32 } {
    var width: u64 = source_width;
    var height: u64 = source_height;
    if (columns > 0 and rows > 0) {
        width = std.math.mul(u64, columns, cell_pixel_width) catch return null;
        height = std.math.mul(u64, rows, cell_pixel_height) catch return null;
    } else if (columns > 0) {
        width = std.math.mul(u64, columns, cell_pixel_width) catch return null;
        const scaled = std.math.mul(u64, width, source_height) catch return null;
        height = (scaled + source_width / 2) / source_width;
    } else if (rows > 0) {
        height = std.math.mul(u64, rows, cell_pixel_height) catch return null;
        const scaled = std.math.mul(u64, height, source_width) catch return null;
        width = (scaled + source_height / 2) / source_height;
    }
    if (width == 0 or height == 0 or
        width > protocol.Limits.terminal_graphics_destination_pixels or
        height > protocol.Limits.terminal_graphics_destination_pixels)
    {
        return null;
    }
    return .{ .width = @intCast(width), .height = @intCast(height) };
}

fn divideCeil(numerator: u64, denominator: u16) ?u64 {
    if (denominator == 0) return null;
    return std.math.divCeil(u64, numerator, denominator) catch null;
}
