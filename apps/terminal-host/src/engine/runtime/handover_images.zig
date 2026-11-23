//! A moved terminal's Kitty images, which Ghostty's snapshot does not carry: the export reads each
//! screen's image storage into the record's image section, and the import puts it back on the
//! restored pages. What the section cannot carry keeps the terminal on its Host.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const image_section = @import("../handover/root.zig").image_section;
const state = @import("state.zig");

const graphics = ghostty_vt.kitty.graphics;
const GhosttyFormat = @FieldType(graphics.Image, "format");
const PlacementKey = graphics.ImageStorage.PlacementKey;

const screens = [_]struct { ghostty_vt.ScreenSet.Key, image_section.ScreenKey }{
    .{ .primary, .primary },
    .{ .alternate, .alternate },
};

/// Whether the terminal holds image state the section cannot carry: a medium being read or a
/// transmission in progress, an animation, a placement that is virtual, as Unicode placeholders
/// use, or relative to another, or more images, placements, or pixels than a screen admits.
pub fn holdsUncarried(self: *const state.State) bool {
    if (self.graphics_media.pending != null or self.graphics_media.stalled != null) return true;
    for (screens) |entry| {
        const screen = self.terminal.screens.get(entry[0]) orelse continue;
        const storage = &screen.kitty_images;
        if (storage.loading != null) return true;
        if (storage.images.count() > image_section.max_images) return true;
        if (storage.placements.count() > image_section.max_placements) return true;
        var total: usize = 0;
        var images = storage.images.valueIterator();
        while (images.next()) |image| {
            const format = formatOf(image.format) orelse return true;
            if (image.id == 0 or image.animation != null or image.compression != .none) return true;
            if (image.width == 0 or image.width > image_section.max_dimension) return true;
            if (image.height == 0 or image.height > image_section.max_dimension) return true;
            const pixels = switch (image.data) {
                .complete => |pixels| pixels,
                .pending => return true,
            };
            if (pixels.len != @as(usize, image.width) * image.height * image_section.channels(format)) return true;
            total += pixels.len;
        }
        if (total > image_section.max_screen_pixel_bytes) return true;
        var placements = storage.placements.valueIterator();
        while (placements.next()) |placement| switch (placement.location) {
            .pin => {},
            .virtual, .relative => return true,
        };
    }
    return false;
}

/// The section's screens, none when no screen holds an image. The pixels borrow the terminal's
/// storage, so the caller encodes them under the terminal lock; `holdsUncarried` was false.
pub fn exportScreens(self: *state.State, arena: std.mem.Allocator) error{OutOfMemory}![]const image_section.Screen {
    var result: std.ArrayList(image_section.Screen) = .empty;
    for (screens) |entry| {
        const screen = self.terminal.screens.get(entry[0]) orelse continue;
        const storage = &screen.kitty_images;
        if (storage.images.count() == 0) continue;
        const images = try arena.alloc(image_section.Image, storage.images.count());
        var image_iterator = storage.images.valueIterator();
        for (images) |*target| {
            const image = image_iterator.next().?;
            target.* = .{
                .id = image.id,
                .number = image.number,
                .width = image.width,
                .height = image.height,
                .format = formatOf(image.format).?,
                .transient = image.metadata.transient,
                .implicit_id = image.metadata.implicit_id,
                .pixels = image.data.complete,
            };
        }
        std.mem.sortUnstable(image_section.Image, images, {}, imageBefore);
        var placements: std.ArrayList(image_section.Placement) = .empty;
        var placement_iterator = storage.placements.iterator();
        while (placement_iterator.next()) |placement| {
            const pin = placement.value_ptr.location.pin;
            // A placement whose rows left history is garbage Ghostty has not reaped yet.
            if (pin.garbage) continue;
            const anchor = screen.pages.pointFromPin(.screen, pin.*) orelse continue;
            const value = placement.value_ptr;
            try placements.append(arena, .{
                .image_id = placement.key_ptr.image_id,
                .placement_id = placementId(placement.key_ptr.*),
                .row = anchor.screen.y,
                .column = pin.x,
                .x_offset = value.x_offset,
                .y_offset = value.y_offset,
                .source_x = value.source_x,
                .source_y = value.source_y,
                .source_width = value.source_width,
                .source_height = value.source_height,
                .columns = value.columns,
                .rows = value.rows,
                .z = value.z,
            });
        }
        std.mem.sortUnstable(image_section.Placement, placements.items, {}, placementBefore);
        try result.append(arena, .{
            .key = entry[1],
            .next_image_id = storage.next_image_id,
            .next_internal_placement_id = storage.next_internal_placement_id,
            .images = images,
            .placements = placements.items,
        });
    }
    return result.items;
}

/// Puts the section back into the restored terminal's screens, after the Host's storage limits,
/// so each storage holds the images, placements, and next ids the exporting one held. On failure
/// the terminal's own teardown frees what was added.
pub fn restore(terminal: *ghostty_vt.Terminal, section: []const image_section.Screen) error{ InvalidRecord, OutOfMemory }!void {
    const io = terminal.io();
    for (section) |recorded| {
        const key: ghostty_vt.ScreenSet.Key = switch (recorded.key) {
            .primary => .primary,
            .alternate => .alternate,
        };
        const screen = terminal.screens.get(key) orelse return error.InvalidRecord;
        const storage = &screen.kitty_images;
        for (recorded.images) |image| {
            const pixels = try screen.alloc.dupe(u8, image.pixels);
            storage.addImage(io, screen.alloc, screen, .{
                .id = image.id,
                .number = image.number,
                .width = image.width,
                .height = image.height,
                .format = ghosttyFormat(image.format),
                .data = .{ .complete = pixels },
                .metadata = .{ .transient = image.transient, .implicit_id = image.implicit_id },
            }) catch {
                screen.alloc.free(pixels);
                return error.OutOfMemory;
            };
        }
        for (recorded.placements) |placement| {
            const anchor = screen.pages.pin(.{ .screen = .{ .x = placement.column, .y = placement.row } }) orelse
                return error.InvalidRecord;
            const tracked = try screen.pages.trackPin(anchor);
            // Ghostty numbers a placement without an external id from its next internal id, so
            // each internal one keeps its id by setting that first.
            const external: u32 = switch (placement.placement_id) {
                .internal => |id| internal: {
                    storage.next_internal_placement_id = id;
                    break :internal 0;
                },
                .external => |id| id,
            };
            storage.addPlacement(io, screen.alloc, screen, placement.image_id, external, .{
                .location = .{ .pin = tracked },
                .x_offset = placement.x_offset,
                .y_offset = placement.y_offset,
                .source_x = placement.source_x,
                .source_y = placement.source_y,
                .source_width = placement.source_width,
                .source_height = placement.source_height,
                .columns = placement.columns,
                .rows = placement.rows,
                .z = placement.z,
            }) catch {
                screen.pages.untrackPin(tracked);
                return error.OutOfMemory;
            };
        }
        storage.next_image_id = recorded.next_image_id;
        storage.next_internal_placement_id = recorded.next_internal_placement_id;
    }
}

/// A complete image's decoded format; PNG never stays PNG once decoded.
fn formatOf(format: GhosttyFormat) ?image_section.Format {
    return switch (format) {
        .rgb => .rgb,
        .rgba => .rgba,
        .gray => .gray,
        .gray_alpha => .gray_alpha,
        .png => null,
    };
}

fn ghosttyFormat(format: image_section.Format) GhosttyFormat {
    return switch (format) {
        .rgb => .rgb,
        .rgba => .rgba,
        .gray => .gray,
        .gray_alpha => .gray_alpha,
    };
}

fn placementId(key: PlacementKey) image_section.PlacementId {
    return switch (key.placement_id.tag) {
        .internal => .{ .internal = key.placement_id.id },
        .external => .{ .external = key.placement_id.id },
    };
}

fn imageBefore(_: void, left: image_section.Image, right: image_section.Image) bool {
    return left.id < right.id;
}

fn placementBefore(_: void, left: image_section.Placement, right: image_section.Placement) bool {
    if (left.image_id != right.image_id) return left.image_id < right.image_id;
    const left_tag = std.meta.activeTag(left.placement_id);
    const right_tag = std.meta.activeTag(right.placement_id);
    if (left_tag != right_tag) return left_tag == .internal;
    return switch (left.placement_id) {
        inline else => |id, tag| id < @field(right.placement_id, @tagName(tag)),
    };
}
