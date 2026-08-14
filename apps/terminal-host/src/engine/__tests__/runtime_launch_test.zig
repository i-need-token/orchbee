const std = @import("std");

const subject = @import("terminal-host-engine").testing.runtime_launch;

test "runtime launch preparation resolves one canonical pixel geometry" {
    const checked = try subject.prepare(std.testing.allocator, .{
        .arguments = &.{},
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "probe",
        .rows = 24,
        .stdin_fd = 42,
    }, .{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
    });

    try std.testing.expectEqual(@as(u16, 18), checked.process_launch.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 9), checked.process_launch.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 18), checked.runtime_options.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 9), checked.runtime_options.cell_pixel_width);
    try std.testing.expectEqual(@as(?isize, 42), checked.process_launch.stdin_fd);
}

test "runtime launch preparation rejects conflicting pixel authorities" {
    try std.testing.expectError(error.InvalidLaunch, subject.prepare(std.testing.allocator, .{
        .arguments = &.{},
        .cell_pixel_width = 8,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "probe",
        .rows = 24,
    }, .{
        .cell_pixel_width = 9,
    }));
}
