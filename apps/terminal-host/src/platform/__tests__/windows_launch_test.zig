const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const Process = platform.process.Process;

test "Windows batch encoding preserves percent literals and rejects truncated commands" {
    const encode = platform.process.testing.windowsBatchCommandLineFn;
    const encoded = try encode(std.testing.allocator, "C:\\tools\\pane.cmd", &.{ "", "a&b", "%PATH%", "bang!", "a\"b", "end\\" });
    defer std.testing.allocator.free(encoded);
    try std.testing.expectEqualStrings(
        "cmd.exe /e:ON /v:OFF /d /c \"\"C:\\tools\\pane.cmd\" \"\" \"a&b\" \"%%cd:~,%PATH%%cd:~,%\" \"bang!\" \"a\"\"b\" \"end\\\\\"\"",
        encoded,
    );
    const canonical = try encode(std.testing.allocator, "\\\\?\\C:\\tools\\pane.CMD", &.{});
    defer std.testing.allocator.free(canonical);
    try std.testing.expectEqualStrings("cmd.exe /e:ON /v:OFF /d /c \"\"C:\\tools\\pane.CMD\"\"", canonical);
    for ([_][]const u8{ "a\r&echo bad", "a\nbad", "nul\x00tail" }) |invalid| {
        try std.testing.expectError(error.InvalidBatchArgument, encode(std.testing.allocator, "C:\\pane.cmd", &.{invalid}));
    }
    for ([_][]const u8{ "pane.cmd", "C:relative.cmd", "C:\\bad\"name.cmd", "\\\\?\\C:\\changed.\\pane.cmd", "\\\\.\\pipe\\pane.cmd" }) |invalid| {
        try std.testing.expectError(error.InvalidBatchPath, encode(std.testing.allocator, invalid, &.{}));
    }
    const too_long = try std.testing.allocator.alloc(u8, 8192);
    defer std.testing.allocator.free(too_long);
    @memset(too_long, 'x');
    try std.testing.expectError(error.BatchCommandTooLong, encode(std.testing.allocator, "C:\\pane.cmd", &.{too_long}));
}

test "Windows batch ConPTY launch owns scripts and literal parameters in its Job" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const script =
        "@echo off\r\n" ++
        "setlocal disableDelayedExpansion\r\n" ++
        "set \"OB_BATCH_TEXT=%~1\"\r\n" ++
        "set \"OB_BATCH_EMPTY=[%~2]\"\r\n" ++
        "set \"OB_BATCH_PERCENT=%~3\"\r\n" ++
        "set \"OB_BATCH_QUOTES=%~4\"\r\n" ++
        "set \"OB_BATCH_BANG=%~5\"\r\n" ++
        "set OB_BATCH_\r\n" ++
        "exit /B 37\r\n";
    for ([_][]const u8{ "pane %OB_REPLACE% & 中文.CMD", "pane.bat" }) |name| {
        try temporary.dir.writeFile(std.testing.io, .{ .sub_path = name, .data = script });
        const executable = try temporary.dir.realPathFileAlloc(std.testing.io, name, std.testing.allocator);
        defer std.testing.allocator.free(executable);
        var process = try Process.spawn(std.testing.allocator, .{
            .executable = executable,
            .arguments = &.{ "space & literal", "", "%OB_REPLACE%", "a\"&exit /B 99&\"", "!OB_REPLACE!" },
            .environment = &.{
                .{ .name = "OB_REPLACE", .value = "unexpected-expansion" },
                .{ .name = "ComSpec", .value = "C:\\missing-command-processor.exe" },
            },
            .current_directory = null,
            .columns = 240,
            .rows = 24,
        });
        defer process.deinit();
        var reader: Reader = .{ .process = &process };
        const thread = try std.Thread.spawn(.{}, Reader.run, .{&reader});
        var joined = false;
        defer if (!joined) {
            process.forceTerminate() catch {};
            process.abortRead() catch {};
            thread.join();
        };
        process.registerReaderThread(thread.getHandle());
        var timer = try platform.monotonic.Timer.start();
        while (!try process.processTreeContained()) {
            if (timer.read() >= 15 * std.time.ns_per_s) return error.BatchDidNotExit;
            platform.monotonic.sleep(10 * std.time.ns_per_ms);
        }
        const exit = try process.wait();
        try process.finishPtyAfterChildExit();
        thread.join();
        joined = true;
        const output = reader.output[0..try reader.result];
        try std.testing.expectEqual(platform.process.ExitResult{ .exit_code = 37 }, exit);
        for ([_][]const u8{ "OB_BATCH_TEXT=space & literal", "OB_BATCH_EMPTY=[]", "OB_BATCH_PERCENT=%OB_REPLACE%", "OB_BATCH_BANG=!OB_REPLACE!", "OB_BATCH_QUOTES=a\"\"&exit /B 99&\"\"" }) |expected| {
            if (std.mem.indexOf(u8, output, expected) == null) {
                std.debug.print("batch output missing {s}: {s}\n", .{ expected, output });
                return error.MissingLiteralParameter;
            }
        }
        try std.testing.expect(std.mem.indexOf(u8, output, "unexpected-expansion") == null);
        try std.testing.expect(try process.processTreeContained());
    }
}

const Reader = struct {
    process: *Process,
    output: [4096]u8 = undefined,
    result: anyerror!usize = error.NotRead,

    fn run(self: *@This()) void {
        var length: usize = 0;
        while (length < self.output.len) {
            const count = self.process.read(self.output[length..]) catch |err| {
                self.result = err;
                return;
            };
            if (count == 0) break;
            length += count;
        }
        self.result = length;
    }
};

test "Windows executable separator spellings preserve cmd arguments and exit" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try platform.process.environmentVariableOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    for (0..3) |spelling| {
        const path = try std.testing.allocator.dupe(u8, executable);
        defer std.testing.allocator.free(path);
        if (spelling != 0) {
            std.mem.replaceScalar(u8, path, '\\', '/');
            if (spelling == 2) {
                if (std.mem.lastIndexOfScalar(u8, path, '/')) |index| path[index] = '\\';
            }
        }
        var process = try Process.spawn(std.testing.allocator, .{
            .executable = path,
            .arguments = &.{ "/D", "/S", "/C", "<nul set /p =literal/slash /C:$value & exit /B 37" },
            .current_directory = null,
            .environment = &.{},
            .columns = 80,
            .rows = 24,
        });
        defer process.deinit();
        var reader: Reader = .{ .process = &process };
        const thread = try std.Thread.spawn(.{}, Reader.run, .{&reader});
        var joined = false;
        defer if (!joined) {
            process.forceTerminate() catch {};
            process.abortRead() catch {};
            thread.join();
        };
        process.registerReaderThread(thread.getHandle());
        var timer = try platform.monotonic.Timer.start();
        while (!try process.processTreeContained()) {
            if (timer.read() >= 15 * std.time.ns_per_s) return error.LaunchDidNotExit;
            platform.monotonic.sleep(10 * std.time.ns_per_ms);
        }
        const exit = try process.wait();
        try process.finishPtyAfterChildExit();
        thread.join();
        joined = true;
        const length = try reader.result;
        if (!std.meta.eql(platform.process.ExitResult{ .exit_code = 37 }, exit)) {
            std.debug.print("executable spelling {d}: {s}\n{s}\n", .{ spelling, path, reader.output[0..length] });
        }
        try std.testing.expectEqual(platform.process.ExitResult{ .exit_code = 37 }, exit);
        try std.testing.expect(std.mem.indexOf(u8, reader.output[0..length], "literal/slash /C:$value") != null);
    }
}
