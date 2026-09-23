const std = @import("std");
const builtin = @import("builtin");
const subject = @import("terminal-host-platform").testing.conpty;

test "Windows ConPTY admits absence but rejects a present incomplete or altered bundle" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const directory = try temporary.parent_dir.realPathFileAlloc(std.testing.io, &temporary.sub_path, std.testing.allocator);
    defer std.testing.allocator.free(directory);
    const absent = try std.fs.path.join(std.testing.allocator, &.{ directory, "absent" });
    defer std.testing.allocator.free(absent);
    const absent_wide = try std.unicode.utf8ToUtf16LeAllocZ(std.testing.allocator, absent);
    defer std.testing.allocator.free(absent_wide);
    var api = try subject.loadAt(absent_wide);
    defer api.deinit();
    try std.testing.expect(api.module == null);
    const wide = try std.unicode.utf8ToUtf16LeAllocZ(std.testing.allocator, directory);
    defer std.testing.allocator.free(wide);
    try std.testing.expectError(error.ConptyBundleUnavailable, subject.loadAt(wide));
    try temporary.dir.writeFile(std.testing.io, .{ .sub_path = "conpty.dll", .data = "altered" });
    try std.testing.expectError(error.ConptyBundleIntegrity, subject.loadAt(wide));
    // A failed admission releases every handle, including its deny-write lock.
    try temporary.dir.deleteFile(std.testing.io, "conpty.dll");
    try temporary.dir.createDir(std.testing.io, "conpty.dll", .default_dir);
    try std.testing.expectError(error.ConptyBundleUnavailable, subject.loadAt(wide));
}

test "Windows ConPTY hash verification owns a read lock until release" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    try temporary.dir.writeFile(std.testing.io, .{ .sub_path = "candidate", .data = "abc" });
    const file_path = try temporary.dir.realPathFileAlloc(std.testing.io, "candidate", std.testing.allocator);
    defer std.testing.allocator.free(file_path);
    const wide = try std.unicode.utf8ToUtf16LeAllocZ(std.testing.allocator, file_path);
    defer std.testing.allocator.free(wide);
    const handle = try subject.verifyFile(wide, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    {
        defer _ = std.os.windows.CloseHandle(handle);
        if (temporary.dir.createFile(std.testing.io, "candidate", .{})) |file| {
            file.close(std.testing.io);
            return error.ConptyLockDidNotProtectFile;
        } else |_| {}
    }
    try temporary.dir.writeFile(std.testing.io, .{ .sub_path = "candidate", .data = "changed" });
    try std.testing.expectError(error.ConptyBundleIntegrity, subject.verifyFile(wide, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    try temporary.dir.deleteFile(std.testing.io, "candidate");
}
