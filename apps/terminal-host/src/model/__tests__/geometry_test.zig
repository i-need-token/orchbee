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

test "terminal geometry admits paired-zero pixels and rejects mixed pairs" {
    // {0, 0} is the only admitted unknown representation: real cells, no pixel facts.
    const unknown: subject.Geometry = .{
        .cell_pixel_height = 0,
        .cell_pixel_width = 0,
        .columns = 80,
        .rows = 24,
    };
    try std.testing.expect(subject.isValid(unknown));
    try std.testing.expect(!subject.pixelsKnown(unknown));
    // Cell bounds still apply when pixels are unknown.
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 0,
        .cell_pixel_width = 0,
        .columns = 0,
        .rows = 24,
    }));
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 0,
        .cell_pixel_width = 0,
        .columns = 80,
        .rows = 0,
    }));
    // A mixed zero/positive pair is invalid in either direction.
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 0,
        .columns = 80,
        .rows = 24,
    }));
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 0,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    }));
    const known: subject.Geometry = .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    };
    try std.testing.expect(subject.isValid(known));
    try std.testing.expect(subject.pixelsKnown(known));
    // The pixel dimension cap still applies only to measured pairs.
    try std.testing.expect(!subject.isValid(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 4097,
        .columns = 80,
        .rows = 24,
    }));
}
