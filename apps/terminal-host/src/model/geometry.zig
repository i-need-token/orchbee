const std = @import("std");
const protocol = @import("terminal-protocol");

pub const Geometry = struct {
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    columns: u16,
    rows: u16,
};

pub fn isValid(geometry: Geometry) bool {
    return geometry.columns >= 1 and geometry.columns <= protocol.Limits.frame_columns and
        geometry.rows >= 1 and geometry.rows <= protocol.Limits.frame_rows and
        @as(u32, geometry.columns) * geometry.rows <= protocol.Limits.frame_cells and
        geometry.cell_pixel_width >= 1 and
        geometry.cell_pixel_width <= protocol.Limits.cell_pixel_dimension and
        geometry.cell_pixel_height >= 1 and
        geometry.cell_pixel_height <= protocol.Limits.cell_pixel_dimension and
        @as(u32, geometry.columns) * geometry.cell_pixel_width <= std.math.maxInt(u16) and
        @as(u32, geometry.rows) * geometry.cell_pixel_height <= std.math.maxInt(u16);
}

pub fn eql(left: Geometry, right: Geometry) bool {
    return left.columns == right.columns and left.rows == right.rows and
        left.cell_pixel_width == right.cell_pixel_width and
        left.cell_pixel_height == right.cell_pixel_height;
}
