const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const engine = @import("terminal-host-engine");
const subject = engine.runtime;
const Document = platform.editor_document.Document;
const harness = @import("runtime_harness.zig");
const allocator = std.testing.allocator;
const io = std.testing.io;

fn reserveDocument(temporary: *const std.testing.TmpDir) !Document {
    const directory = try temporary.parent_dir.realPathFileAlloc(io, &temporary.sub_path, allocator);
    defer allocator.free(directory);
    return platform.editor_document.testing.reserveAt(allocator, directory);
}

fn expectContents(path: []const u8, text: []const u8) !void {
    const contents = try std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(4096));
    defer allocator.free(contents);
    try std.testing.expectEqualStrings(text, contents);
}

fn missingLaunch() platform.process.Launch {
    return .{
        .arguments = &.{},
        .columns = 80,
        .rows = 24,
        .current_directory = null,
        .environment = &.{},
        .executable = "editor-runtime-spawn-must-fail",
    };
}

test "editor document lifecycle refuses an unready owner before workers or process effects" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    var document = try reserveDocument(&temporary);
    defer document.release() catch @panic("fixture reservation release failed");
    var spawner: harness.FailingThreadSpawner = .{ .fail_at = 0 };
    try std.testing.expectError(error.InvalidLaunch, harness.createWithOptionsUsingSpawner(
        allocator,
        missingLaunch(),
        .{ .editor_document = &document },
        &spawner,
    ));
    try std.testing.expectEqual(@as(usize, 0), spawner.attempts);
    try std.testing.expectEqual(platform.editor_document.Phase.reserved, document.phase());
    try document.release();
    try std.testing.expectError(error.InvalidLaunch, harness.createWithOptionsUsingSpawner(
        allocator,
        missingLaunch(),
        .{ .editor_document = &document },
        &spawner,
    ));
    try std.testing.expectEqual(@as(usize, 0), spawner.attempts);
}

test "editor document lifecycle retains the ready caller owner after every worker startup failure" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    for (0..4) |failure_stage| {
        var document = try reserveDocument(&temporary);
        defer document.release() catch @panic("fixture document release failed");
        try document.materialize("immutable source 😀\n");
        var spawner: harness.FailingThreadSpawner = .{ .fail_at = failure_stage };
        try std.testing.expectError(error.InjectedThreadSpawnFailure, harness.createWithOptionsUsingSpawner(
            allocator,
            missingLaunch(),
            .{ .editor_document = &document },
            &spawner,
        ));
        try std.testing.expectEqual(platform.editor_document.Phase.ready, document.phase());
        try expectContents(document.absolutePath().?, "immutable source 😀\n");
    }
}

test "editor document lifecycle retains the ready caller owner after process spawn failure" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    var document = try reserveDocument(&temporary);
    defer document.release() catch @panic("fixture document release failed");
    try document.materialize("retained after failed spawn");
    var spawner: harness.FailingThreadSpawner = .{ .fail_at = 4 };
    try std.testing.expectError(error.SpawnFailed, harness.createWithOptionsUsingSpawner(
        allocator,
        missingLaunch(),
        .{ .editor_document = &document },
        &spawner,
    ));
    try std.testing.expectEqual(@as(usize, 4), spawner.attempts);
    try std.testing.expectEqual(platform.editor_document.Phase.ready, document.phase());
    try expectContents(document.absolutePath().?, "retained after failed spawn");
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

test "editor document lifecycle transfers once and joins the contained Job before deletion retry" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    var document = try reserveDocument(&temporary);
    defer document.release() catch @panic("fixture document release failed");
    try document.materialize("owned by live editor runtime");
    const path = try allocator.dupe(u8, document.absolutePath().?);
    defer allocator.free(path);
    const executable = try platform.process.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(executable);
    const runtime = try subject.TerminalRuntime.createWithOptions(allocator, .{
        .arguments = &.{ "/D", "/S", "/C", "ping -t 127.0.0.1 >nul" },
        .columns = 80,
        .rows = 24,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
    }, .{ .editor_document = &document });
    defer runtime.destroy();
    try std.testing.expectEqual(platform.editor_document.Phase.released, document.phase());
    try document.release();
    try expectContents(path, "owned by live editor runtime");
    const wide = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path);
    defer allocator.free(wide);
    {
        const blocker = CreateFileW(wide, 0x80000000, 3, null, 3, 0x80, null);
        if (blocker == std.os.windows.INVALID_HANDLE_VALUE) return error.BlockerOpenFailed;
        defer _ = std.os.windows.CloseHandle(blocker);
        try std.testing.expectError(error.EditorDocumentReleaseFailed, runtime.shutdown());
        try std.testing.expect(runtime.containmentConfirmed());
        try std.testing.expect(subject.testing.observe(runtime).threads_joined);
        try std.testing.expectEqual(subject.ShutdownCleanupPhase.release_editor_document, runtime.pendingShutdownCleanup().?);
        try expectContents(path, "owned by live editor runtime");
    }
    try runtime.shutdown();
    try std.testing.expect(runtime.pendingShutdownCleanup() == null);
    if (std.Io.Dir.cwd().openFile(io, path, .{})) |file| {
        file.close(io);
        return error.DocumentStillPresentAfterShutdown;
    } else |err| try std.testing.expectEqual(error.FileNotFound, err);
}
