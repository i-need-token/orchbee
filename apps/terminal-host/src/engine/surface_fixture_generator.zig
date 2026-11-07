const std = @import("std");

const surface_fixture = @import("terminal-host-surface-fixture");

pub fn main(init: std.process.Init) !void {
    const allocator = init.gpa;
    const arguments = try init.minimal.args.toSlice(init.arena.allocator());
    if (arguments.len != 7) return error.InvalidArguments;
    for (arguments[1..]) |argument| {
        if (!std.fs.path.isAbsolute(argument)) return error.InvalidArguments;
    }

    const fixtures = try surface_fixture.create(init.io, allocator);
    defer fixtures.deinit(allocator);
    try writeFixture(init.io, arguments[1], fixtures.primary);
    try writeFixture(init.io, arguments[2], fixtures.alternate);
    try writeFixture(init.io, arguments[3], fixtures.render_row_window);
    try writeFixture(init.io, arguments[4], fixtures.graphics.placed);
    try writeFixture(init.io, arguments[5], fixtures.graphics.scrolled);
    try writeFixture(init.io, arguments[6], fixtures.graphics.deleted);
}

fn writeFixture(io: std.Io, path: []const u8, fixture: []const u8) !void {
    const output = try std.Io.Dir.createFileAbsolute(io, path, .{});
    defer output.close(io);
    try output.writeStreamingAll(io, fixture);
}
