const std = @import("std");

const subject = @import("terminal-host-model").geometry;

test "terminal geometry validates grid, cell, and pixel extent boundaries together" {
    try std.testing.expect(subject.isValid(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    }));
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 0,
        .rows = 24,
    }));
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 1,
        .cell_pixel_width = 1,
        .columns = 4096,
        .rows = 65,
    }));
    try std.testing.expect(subject.isValid(.{
        .cell_pixel_height = 4096,
        .cell_pixel_width = 16,
        .columns = 4095,
        .rows = 1,
    }));
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 1,
        .cell_pixel_width = 16,
        .columns = 4096,
        .rows = 1,
    }));
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 1,
        .cell_pixel_width = 0,
        .columns = 80,
        .rows = 24,
    }));
}
