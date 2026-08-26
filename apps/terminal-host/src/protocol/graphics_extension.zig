const std = @import("std");

const contract = @import("contract.zig");
const wire = @import("wire.zig");

const ProtocolError = contract.ProtocolError;
const Reader = wire.Reader;
const image_record_bytes = 32;
const placement_flag_internal_id: u32 = 1;
const placement_record_bytes = 56;

pub const UpdateKind = enum(u8) {
    snapshot = 1,
    placements = 2,
};

pub const Summary = struct {
    animation_tick: u64,
    image_count: u16,
    kind: UpdateKind,
    placement_count: u16,
    resource_revision: u64,
};

const ImageShape = struct {
    height: u32,
    id: u32,
    width: u32,
};

pub fn validate(bytes: []const u8) ProtocolError!Summary {
    if (bytes.len < contract.terminal_graphics_header_bytes) return error.Truncated;
    if (bytes.len > contract.Limits.terminal_graphics_extension_bytes) return error.LimitExceeded;
    try wire.requireMagic(bytes, contract.terminal_graphics_magic);

    var reader = Reader.init(bytes, 4);
    const major = try reader.readInt(u8);
    const minor = try reader.readInt(u8);
    const patch = try reader.readInt(u8);
    const kind_code = try reader.readInt(u8);
    const total_length = try reader.readInt(u32);
    const resource_revision = try reader.readInt(u64);
    const animation_tick = try reader.readInt(u64);
    const image_count = try reader.readInt(u16);
    const placement_count = try reader.readInt(u16);
    const declared_image_bytes = try reader.readInt(u32);
    const reserved_header = try reader.readInt(u32);

    if (major != contract.terminal_graphics_format_major or
        minor != contract.terminal_graphics_format_minor or
        patch != contract.terminal_graphics_format_patch)
    {
        return error.UnsupportedVersion;
    }
    if (total_length != bytes.len) return error.LengthMismatch;
    if (reserved_header != 0) return error.UnknownRequiredFlag;
    if (image_count > contract.Limits.terminal_graphics_images or
        placement_count > contract.Limits.terminal_graphics_placements or
        declared_image_bytes > contract.Limits.terminal_graphics_resource_bytes)
    {
        return error.LimitExceeded;
    }
    const kind: UpdateKind = switch (kind_code) {
        @intFromEnum(UpdateKind.snapshot) => .snapshot,
        @intFromEnum(UpdateKind.placements) => .placements,
        else => return error.InvalidField,
    };
    if (kind == .placements and image_count != 0) return error.InvalidField;

    var image_shapes: [contract.Limits.terminal_graphics_images]ImageShape = undefined;
    var actual_image_bytes: usize = 0;
    for (0..image_count) |index| {
        const id = try reader.readInt(u32);
        const format = try reader.readInt(u8);
        const reserved_format = try reader.readBytes(3);
        const generation = try reader.readInt(u64);
        const width = try reader.readInt(u32);
        const height = try reader.readInt(u32);
        const data_length = try reader.readInt(u32);
        const reserved_image = try reader.readInt(u32);
        if (id == 0 or generation == 0 or width == 0 or height == 0 or
            width > contract.Limits.terminal_graphics_dimension or
            height > contract.Limits.terminal_graphics_dimension)
        {
            return error.InvalidField;
        }
        if (reserved_image != 0) return error.UnknownRequiredFlag;
        for (reserved_format) |byte| if (byte != 0) return error.UnknownRequiredFlag;
        const bytes_per_pixel: usize = switch (format) {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            else => return error.InvalidField,
        };
        const pixel_count = std.math.mul(usize, width, height) catch
            return error.LimitExceeded;
        const expected_length = std.math.mul(usize, pixel_count, bytes_per_pixel) catch
            return error.LimitExceeded;
        if (data_length != expected_length) return error.LengthMismatch;
        actual_image_bytes = std.math.add(usize, actual_image_bytes, data_length) catch
            return error.LimitExceeded;
        if (actual_image_bytes > contract.Limits.terminal_graphics_resource_bytes) {
            return error.LimitExceeded;
        }
        for (image_shapes[0..index]) |shape| if (shape.id == id) return error.InvalidField;
        image_shapes[index] = .{ .height = height, .id = id, .width = width };
        _ = try reader.readBytes(data_length);
    }
    if (actual_image_bytes != declared_image_bytes) return error.LengthMismatch;
    if (image_count > 0 and resource_revision == 0) return error.InvalidField;

    for (0..placement_count) |_| {
        const image_id = try reader.readInt(u32);
        _ = try reader.readInt(u32);
        _ = try reader.readInt(u32);
        _ = try reader.readInt(u32);
        _ = try reader.readInt(u32);
        _ = try reader.readInt(u32);
        const source_x = try reader.readInt(u32);
        const source_y = try reader.readInt(u32);
        const source_width = try reader.readInt(u32);
        const source_height = try reader.readInt(u32);
        const destination_width = try reader.readInt(u32);
        const destination_height = try reader.readInt(u32);
        _ = try reader.readInt(u32); // signed z accepts the complete i32 bit pattern
        const flags = try reader.readInt(u32);
        if (image_id == 0 or source_width == 0 or source_height == 0 or
            source_width > contract.Limits.terminal_graphics_dimension or
            source_height > contract.Limits.terminal_graphics_dimension or
            destination_width == 0 or destination_height == 0 or
            destination_width > contract.Limits.terminal_graphics_destination_pixels or
            destination_height > contract.Limits.terminal_graphics_destination_pixels)
        {
            return error.InvalidField;
        }
        if (flags & ~placement_flag_internal_id != 0) return error.UnknownRequiredFlag;
        if (kind == .snapshot) {
            const image = imageById(image_shapes[0..image_count], image_id) orelse
                return error.InvalidField;
            const source_right = std.math.add(u32, source_x, source_width) catch
                return error.InvalidField;
            const source_bottom = std.math.add(u32, source_y, source_height) catch
                return error.InvalidField;
            if (source_right > image.width or source_bottom > image.height) {
                return error.InvalidField;
            }
        }
    }
    if (reader.offset != bytes.len) return error.LengthMismatch;
    const minimum_records = std.math.add(
        usize,
        contract.terminal_graphics_header_bytes,
        std.math.add(
            usize,
            std.math.mul(usize, image_count, image_record_bytes) catch
                return error.LimitExceeded,
            std.math.mul(usize, placement_count, placement_record_bytes) catch
                return error.LimitExceeded,
        ) catch return error.LimitExceeded,
    ) catch return error.LimitExceeded;
    if (minimum_records + actual_image_bytes != bytes.len) return error.LengthMismatch;

    return .{
        .animation_tick = animation_tick,
        .image_count = image_count,
        .kind = kind,
        .placement_count = placement_count,
        .resource_revision = resource_revision,
    };
}

fn imageById(images: []const ImageShape, id: u32) ?ImageShape {
    for (images) |image| if (image.id == id) return image;
    return null;
}
