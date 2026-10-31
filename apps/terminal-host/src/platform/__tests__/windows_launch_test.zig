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

// Canonical catalog paths reach the Host in their verbatim spelling. cmd.exe refuses such a
// current directory as a UNC path and silently runs in the Windows directory instead.
test "Windows ConPTY launch runs cmd in the directory a verbatim current directory names" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const directory = try temporary.parent_dir.realPathFileAlloc(std.testing.io, &temporary.sub_path, std.testing.allocator);
    defer std.testing.allocator.free(directory);
    const verbatim = try std.fmt.allocPrint(std.testing.allocator, "\\\\?\\{s}", .{directory});
    defer std.testing.allocator.free(verbatim);
    const executable = try platform.process.environmentVariableOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    var process = try Process.spawn(std.testing.allocator, .{
        .executable = executable,
        .arguments = &.{ "/D", "/C", "cd" },
        .current_directory = verbatim,
        .environment = &.{},
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
        if (timer.read() >= 15 * std.time.ns_per_s) return error.LaunchDidNotExit;
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }
    _ = try process.wait();
    try process.finishPtyAfterChildExit();
    thread.join();
    joined = true;
    // ConPTY may interleave title and cursor sequences with the printed text.
    var text: [4096]u8 = undefined;
    const output = printedText(reader.output[0..try reader.result], &text);
    const reported = try std.fmt.allocPrint(std.testing.allocator, "{s}\r\n", .{directory});
    defer std.testing.allocator.free(reported);
    if (std.mem.indexOf(u8, output, "UNC") != null or std.mem.indexOf(u8, output, reported) == null) {
        std.debug.print("cmd in {s} reported: {s}\n", .{ verbatim, output });
        return error.UnexpectedCurrentDirectory;
    }
}

/// Drops CSI and OSC sequences, keeping the characters a reader would see.
fn printedText(output: []const u8, buffer: []u8) []const u8 {
    var length: usize = 0;
    var index: usize = 0;
    while (index < output.len) {
        if (output[index] == 0x1b and index + 1 < output.len and output[index + 1] == '[') {
            index += 2;
            while (index < output.len and (output[index] < 0x40 or output[index] > 0x7e)) index += 1;
            index += 1;
        } else if (output[index] == 0x1b and index + 1 < output.len and output[index + 1] == ']') {
            index += 2;
            while (index < output.len and output[index] != 0x07 and output[index] != 0x1b) index += 1;
            index += if (index < output.len and output[index] == 0x1b) 2 else 1;
        } else {
            buffer[length] = output[index];
            length += 1;
            index += 1;
        }
    }
    return buffer[0..length];
}

test "Windows child current directory keeps a verbatim spelling it cannot shorten faithfully" {
    const child = platform.process.testing.windowsChildCurrentDirectoryFn;
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    for ([_][2][]const u8{
        .{ "C:\\work", "C:\\work" },
        .{ "\\\\?\\C:\\work\\plugin", "C:\\work\\plugin" },
        .{ "\\\\?\\C:\\", "C:\\" },
    }) |case| try std.testing.expectEqualStrings(case[1], try child(arena, case[0]));
    const long = try std.fmt.allocPrint(arena, "\\\\?\\C:\\{s}", .{"x" ** 256});
    for ([_][]const u8{
        long,
        "\\\\?\\C:\\work\\trailing.",
        "\\\\?\\C:\\work\\trailing \\plugin",
        "\\\\?\\C:\\work\\..\\plugin",
        "\\\\?\\C:\\work\\NUL",
        "\\\\?\\C:\\work\\com1.txt",
        "\\\\?\\C:/work",
        "\\\\?\\C:\\work\\\\plugin",
        "\\\\?\\GLOBALROOT\\Device",
    }) |kept| try std.testing.expectEqualStrings(kept, try child(arena, kept));
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
