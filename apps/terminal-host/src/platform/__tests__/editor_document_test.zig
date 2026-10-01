const std = @import("std");
const builtin = @import("builtin");
const subject = @import("terminal-host-platform").editor_document;
const allocator = std.testing.allocator;
const io = std.testing.io;

fn rootPath(temporary: *const std.testing.TmpDir) ![:0]u8 {
    return temporary.parent_dir.realPathFileAlloc(io, &temporary.sub_path, allocator);
}

fn expectContents(path: []const u8, expected: []const u8) !void {
    const contents = try std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(4096));
    defer allocator.free(contents);
    try std.testing.expectEqualStrings(expected, contents);
}

fn expectAbsent(path: []const u8) !void {
    if (std.Io.Dir.cwd().openFile(io, path, .{})) |file| {
        file.close(io);
        return error.UnexpectedEditorDocument;
    } else |err| try std.testing.expectEqual(error.FileNotFound, err);
}

test "Windows editor document reserves without effects and materializes exact immutable UTF8" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    try temporary.dir.createDir(io, "space & 中文", .default_dir);
    const directory = try temporary.dir.realPathFileAlloc(io, "space & 中文", allocator);
    defer allocator.free(directory);
    for ([_][]const u8{ "", "一行 😀 e\xcc\x81\nsecond line\n" }) |text| {
        var document = try subject.testing.reserveAt(allocator, directory);
        defer document.release() catch @panic("fixture document release failed");
        const path = try allocator.dupe(u8, document.absolutePath().?);
        defer allocator.free(path);
        try std.testing.expect(std.fs.path.isAbsolute(path));
        try std.testing.expect(std.mem.indexOf(u8, path, "space & 中文") != null);
        try std.testing.expectEqual(subject.Phase.reserved, document.phase());
        try expectAbsent(path);
        try std.testing.expectError(error.InvalidEditorDocumentText, document.materialize("invalid\xff"));
        try std.testing.expectEqual(subject.Phase.reserved, document.phase());
        try expectAbsent(path);
        try document.materialize(text);
        try std.testing.expectEqual(subject.Phase.ready, document.phase());
        try expectContents(path, text);
        try document.materialize("must not overwrite a ready snapshot");
        try expectContents(path, text);
        try document.release();
        try std.testing.expectEqual(subject.Phase.released, document.phase());
        try std.testing.expect(document.absolutePath() == null);
        try expectAbsent(path);
        try std.testing.expectError(error.EditorDocumentReleased, document.materialize(text));
    }
}

test "Windows editor document exclusive collision never owns or erases the existing file" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const directory = try rootPath(&temporary);
    defer allocator.free(directory);
    const identity = [_]u8{0x62} ** 16;
    var first = try subject.testing.reserveAtIdentity(allocator, directory, identity);
    defer first.release() catch @panic("fixture document release failed");
    try first.materialize("existing owner");
    var collision = try subject.testing.reserveAtIdentity(allocator, directory, identity);
    defer collision.release() catch @panic("fixture reservation release failed");
    try std.testing.expectError(error.PathAlreadyExists, collision.materialize("replacement"));
    try std.testing.expectEqual(subject.Phase.reserved, collision.phase());
    try collision.release();
    try expectContents(first.absolutePath().?, "existing owner");
}

test "Windows editor document handoff releases only through the destination owner" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const directory = try rootPath(&temporary);
    defer allocator.free(directory);
    var source = try subject.testing.reserveAt(allocator, directory);
    defer source.release() catch @panic("source release failed");
    try source.materialize("owned until destination cleanup");
    var destination = source.take();
    defer destination.release() catch @panic("destination release failed");
    try std.testing.expectEqual(subject.Phase.released, source.phase());
    try source.release();
    try expectContents(destination.absolutePath().?, "owned until destination cleanup");
    try destination.release();
    try destination.release();
}

extern "kernel32" fn CreateFileW(
    name: [*:0]const u16,
    access: u32,
    sharing: u32,
    security: ?*anyopaque,
    disposition: u32,
    attributes: u32,
    template: ?std.os.windows.HANDLE,
) callconv(.winapi) std.os.windows.HANDLE;

test "Windows editor document blocked deletion retains its exact owner for retry" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const directory = try rootPath(&temporary);
    defer allocator.free(directory);
    var document = try subject.testing.reserveAt(allocator, directory);
    defer document.release() catch @panic("fixture document release failed");
    try document.materialize("retained while another handle denies deletion");
    const path = try allocator.dupe(u8, document.absolutePath().?);
    defer allocator.free(path);
    const wide = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path);
    defer allocator.free(wide);
    {
        // GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, OPEN_EXISTING.
        const blocker = CreateFileW(wide, 0x80000000, 3, null, 3, 0x80, null);
        if (blocker == std.os.windows.INVALID_HANDLE_VALUE) return error.BlockerOpenFailed;
        defer _ = std.os.windows.CloseHandle(blocker);
        if (document.release()) |_| return error.DocumentReleasedBeforeBlockerClosed else |_| {}
        try std.testing.expectEqual(subject.Phase.ready, document.phase());
        try std.testing.expectEqualStrings(path, document.absolutePath().?);
        try expectContents(path, "retained while another handle denies deletion");
    }
    try document.release();
    try std.testing.expectEqual(subject.Phase.released, document.phase());
    try expectAbsent(path);
}

fn reserveWithAllocationFailures(alloc: std.mem.Allocator, directory: []const u8) !void {
    var document = try subject.testing.reserveAt(alloc, directory);
    defer document.release() catch @panic("fixture reservation release failed");
    try std.testing.expectEqual(subject.Phase.reserved, document.phase());
    try expectAbsent(document.absolutePath().?);
}

test "Windows editor document reservation rolls back all allocations and rejects nonabsolute roots" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const directory = try rootPath(&temporary);
    defer allocator.free(directory);
    try std.testing.checkAllAllocationFailures(allocator, reserveWithAllocationFailures, .{directory});
    for ([_][]const u8{ "relative", "C:relative", "C:\\root\x00suffix" }) |invalid| {
        try std.testing.expectError(error.InvalidEditorDocumentDirectory, subject.testing.reserveAt(allocator, invalid));
    }
}
