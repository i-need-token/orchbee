const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const Process = platform.process.Process;

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
