const std = @import("std");

const surface_fixture = @import("terminal-host-surface-fixture");

pub fn main() !void {
    const allocator = std.heap.smp_allocator;
    const arguments = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, arguments);
    if (arguments.len != 4 or
        !std.fs.path.isAbsolute(arguments[1]) or
        !std.fs.path.isAbsolute(arguments[2]) or
        !std.fs.path.isAbsolute(arguments[3]))
    {
        return error.InvalidArguments;
    }

    const fixtures = try surface_fixture.create(allocator);
    defer fixtures.deinit(allocator);
    try writeFixture(arguments[1], fixtures.primary);
    try writeFixture(arguments[2], fixtures.alternate);
    try writeFixture(arguments[3], fixtures.render_row_window);
}

fn writeFixture(path: []const u8, fixture: []const u8) !void {
    const output = try std.fs.createFileAbsolute(path, .{});
    defer output.close();
    try output.writeAll(fixture);
}
