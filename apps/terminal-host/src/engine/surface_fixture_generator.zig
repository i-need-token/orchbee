const std = @import("std");

const surface_fixture = @import("terminal-host-surface-fixture");

pub fn main(init: std.process.Init) !void {
    const allocator = init.gpa;
    const arguments = try init.minimal.args.toSlice(init.arena.allocator());
    if (arguments.len != 4 or
        !std.fs.path.isAbsolute(arguments[1]) or
        !std.fs.path.isAbsolute(arguments[2]) or
        !std.fs.path.isAbsolute(arguments[3]))
    {
        return error.InvalidArguments;
    }

    const fixtures = try surface_fixture.create(init.io, allocator);
    defer fixtures.deinit(allocator);
    try writeFixture(init.io, arguments[1], fixtures.primary);
    try writeFixture(init.io, arguments[2], fixtures.alternate);
    try writeFixture(init.io, arguments[3], fixtures.render_row_window);
}

fn writeFixture(io: std.Io, path: []const u8, fixture: []const u8) !void {
    const output = try std.Io.Dir.createFileAbsolute(io, path, .{});
    defer output.close(io);
    try output.writeStreamingAll(io, fixture);
}
