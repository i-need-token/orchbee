const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const capture = @import("graphics_capture.zig");

const graphics_header_bytes = 40;
const image_record_bytes = 32;
const placement_record_bytes = 56;
const placement_flag_internal_id: u32 = 1;

const ScreenIdentity = capture.ScreenIdentity;
const ImageReference = capture.ImageReference;
const PendingRelative = capture.PendingRelative;
const VirtualOrigin = capture.VirtualOrigin;
const appendBoundedPlacement = capture.appendBoundedPlacement;
const captureCanonicalPlacement = capture.captureCanonicalPlacement;
const containsImageReference = capture.containsImageReference;
const imageReference = capture.imageReference;
const sameImages = capture.sameImages;
const sameScreen = capture.sameScreen;
const validatePlacementGeometry = capture.validatePlacementGeometry;

pub const ImageFormat = capture.ImageFormat;
pub const Image = capture.Image;
pub const Placement = capture.Placement;

pub const State = struct {
    allocator: std.mem.Allocator,
    animation_tick: u64 = 0,
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
        self.animation_tick = 0;
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
        const pages = &terminal.screens.active.pages;
        const top = pages.getTopLeft(.viewport);
        const bottom = pages.getBottomRight(.viewport) orelse return error.ProjectionTooLarge;
        try self.captureRange(
            terminal,
            revision,
            cell_pixel_width,
            cell_pixel_height,
            top,
            bottom,
        );
    }

    /// Captures graphics intersecting an arbitrary retained row interval. The
    /// first selected row becomes row zero in the encoded history projection.
    pub fn captureRows(
        self: *State,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        animation_tick: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        pins: []const ghostty_vt.Pin,
    ) !void {
        if (pins.len == 0) return error.ProjectionTooLarge;
        self.animation_tick = animation_tick;
        var bottom = pins[pins.len - 1];
        bottom.x = terminal.cols - 1;
        try self.captureRange(
            terminal,
            revision,
            cell_pixel_width,
            cell_pixel_height,
            pins[0],
            bottom,
        );
    }

    fn captureRange(
        self: *State,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        top: ghostty_vt.Pin,
        bottom: ghostty_vt.Pin,
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
        var pending_relative: std.ArrayList(PendingRelative) = .empty;
        defer pending_relative.deinit(self.allocator);
        var virtual_origins: std.AutoHashMapUnmanaged(
            ghostty_vt.kitty.graphics.ImageStorage.PlacementKey,
            VirtualOrigin,
        ) = .empty;
        defer virtual_origins.deinit(self.allocator);

        // Pixel metrics may be unknown on cell-only hosts ({0,0} cell pixels).
        // Placement geometry carries pixel offsets and destination extents that
        // cannot be expressed honestly without them, so the frame must carry no
        // graphics rather than fail or reuse stale offsets.
        if (cell_pixel_width == 0 or cell_pixel_height == 0) {
            self.placements.clearRetainingCapacity();
            return;
        }
        const pages = &terminal.screens.active.pages;
        const top_screen = pages.pointFromPin(.screen, top) orelse
            return error.ProjectionTooLarge;
        const bottom_screen = pages.pointFromPin(.screen, bottom) orelse
            return error.ProjectionTooLarge;
        if (bottom_screen.screen.y < top_screen.screen.y) return error.ProjectionTooLarge;
        const top_y = top_screen.screen.y;
        const bottom_y = bottom_screen.screen.y;
        const range_rows = @as(u64, bottom_y) - top_y + 1;

        var iterator = storage.placements.iterator();
        while (iterator.next()) |entry| {
            const image = storage.images.getPtr(entry.key_ptr.image_id) orelse continue;
            const placement = entry.value_ptr;
            const origin: struct {
                pin: *const ghostty_vt.Pin,
                horizontal_offset: i32 = 0,
                vertical_offset: i32 = 0,
            } = switch (placement.location) {
                .pin => |pin| .{ .pin = pin },
                .virtual => continue,
                .relative => |relative| relative_origin: {
                    const chain = storage.resolveChain(relative) orelse continue;
                    switch (chain.root.location) {
                        .pin => |pin| break :relative_origin .{
                            .pin = pin,
                            .horizontal_offset = chain.horizontal_offset,
                            .vertical_offset = chain.vertical_offset,
                        },
                        .virtual => {
                            try pending_relative.append(self.allocator, .{
                                .horizontal_offset = chain.horizontal_offset,
                                .image_id = entry.key_ptr.image_id,
                                .key = entry.key_ptr.*,
                                .placement = placement.*,
                                .root_key = chain.root_key,
                                .vertical_offset = chain.vertical_offset,
                            });
                            continue;
                        },
                        .relative => unreachable,
                    }
                },
            };
            if (origin.pin.garbage or image.data.isPending()) continue;
            const origin_screen = pages.pointFromPin(.screen, origin.pin.*) orelse continue;
            const row = @as(i64, origin_screen.screen.y) - top_y + origin.vertical_offset;
            const column = @as(i64, origin.pin.x) + origin.horizontal_offset;
            if (try captureCanonicalPlacement(
                terminal,
                image,
                entry.key_ptr.*,
                placement,
                column,
                row,
                range_rows,
            )) |captured| {
                try appendBoundedPlacement(self.allocator, &placements, captured);
            }
        }

        var virtual_iterator = ghostty_vt.kitty.graphics.unicode.placementIterator(top, bottom);
        while (virtual_iterator.next()) |virtual| {
            const target = storage.placeholderTarget(
                virtual.image_id,
                virtual.placement_id,
            ) orelse continue;
            const image = storage.images.getPtr(virtual.image_id) orelse continue;
            if (image.data.isPending()) continue;
            const rendered = virtual.renderPlacement(
                storage,
                image,
                cell_pixel_width,
                cell_pixel_height,
            ) catch continue;
            if (rendered.dest_width == 0 or rendered.dest_height == 0 or
                rendered.source_width == 0 or rendered.source_height == 0)
            {
                continue;
            }
            const rendered_screen = pages.pointFromPin(.screen, rendered.top_left) orelse continue;
            const row_u64 = @as(u64, rendered_screen.screen.y) - top_y;
            const row = std.math.cast(i32, row_u64) orelse return error.ProjectionTooLarge;
            const column = std.math.cast(i32, rendered.top_left.x) orelse
                return error.ProjectionTooLarge;
            try validatePlacementGeometry(
                image,
                rendered.source_x,
                rendered.source_y,
                rendered.source_width,
                rendered.source_height,
                rendered.dest_width,
                rendered.dest_height,
            );
            try appendBoundedPlacement(self.allocator, &placements, .{
                .column = column,
                .destination_height = rendered.dest_height,
                .destination_width = rendered.dest_width,
                .image_id = image.id,
                .internal_id = target.key.placement_id.tag == .internal,
                .offset_x = rendered.offset_x,
                .offset_y = rendered.offset_y,
                .placement_id = target.key.placement_id.id,
                .row = row,
                .source_height = rendered.source_height,
                .source_width = rendered.source_width,
                .source_x = rendered.source_x,
                .source_y = rendered.source_y,
                .z = -1,
            });

            if (pending_relative.items.len > 0) {
                const current = VirtualOrigin{
                    .column = rendered_screen.screen.x,
                    .row = @intCast(row_u64),
                };
                const origin = try virtual_origins.getOrPut(self.allocator, target.key);
                if (!origin.found_existing) {
                    origin.value_ptr.* = current;
                } else {
                    origin.value_ptr.column = @min(origin.value_ptr.column, current.column);
                    origin.value_ptr.row = @min(origin.value_ptr.row, current.row);
                }
            }
        }

        for (pending_relative.items) |pending| {
            const origin = virtual_origins.get(pending.root_key) orelse continue;
            const image = storage.images.getPtr(pending.image_id) orelse continue;
            if (image.data.isPending()) continue;
            const column = @as(i64, origin.column) + pending.horizontal_offset;
            const row = @as(i64, origin.row) + pending.vertical_offset;
            if (try captureCanonicalPlacement(
                terminal,
                image,
                pending.key,
                &pending.placement,
                column,
                row,
                range_rows,
            )) |captured| {
                try appendBoundedPlacement(self.allocator, &placements, captured);
            }
        }

        for (placements.items) |placement| {
            if (containsImageReference(references.items, placement.image_id)) continue;
            if (references.items.len >= protocol.Limits.terminal_graphics_images) {
                return error.ProjectionTooLarge;
            }
            const image = storage.images.getPtr(placement.image_id) orelse
                return error.ProjectionTooLarge;
            try references.append(self.allocator, try imageReference(image));
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
        try writer.appendInt(u64, self.animation_tick);
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
