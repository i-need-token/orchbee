const std = @import("std");
const builtin = @import("builtin");
const platform_boundary = @import("terminal-host-platform");
const monotonic = platform_boundary.monotonic;
const posix_compat = platform_boundary.posix_compat;
const subject = platform_boundary.testing.pty;

const Pty = subject.Pty;

test "PTY opens with bounded dimensions and resizes" {
    var instance = try Pty.open(.{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    try instance.setSize(.{ .rows = 40, .columns = 120 });
}

test "Windows PTY rejects resize after pseudoconsole close" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var instance = try Pty.open(.{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    instance.closeConsole();
    try std.testing.expectError(
        error.PtyClosed,
        instance.setSize(.{ .rows = 40, .columns = 120 }),
    );
}

test "POSIX child setup preserves standard streams when openpty reuses low descriptors" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const posix = std.posix;
    const report = try posix_compat.pipe2(.{ .CLOEXEC = true });
    const child_id = try posix_compat.fork();
    if (child_id == 0) {
        _ = posix.system.close(report[0]);
        _ = posix.system.close(posix.STDIN_FILENO);
        _ = posix.system.close(posix.STDOUT_FILENO);
        _ = posix.system.close(posix.STDERR_FILENO);
        const instance = Pty.open(.{ .rows = 24, .columns = 80 }) catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        // A real parent retains the master while its child installs the slave.
        // Keep that reference above stdio until process exit so Linux does not
        // hang up this fixture before its result has been reported.
        _ = posix_compat.fcntl(
            instance.master,
            posix.F.DUPFD_CLOEXEC,
            posix.STDERR_FILENO + 1,
        ) catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        instance.childSetup() catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        for ([_]posix.fd_t{
            posix.STDIN_FILENO,
            posix.STDOUT_FILENO,
            posix.STDERR_FILENO,
        }) |descriptor| {
            _ = posix_compat.fcntl(descriptor, posix.F.GETFD, 0) catch {
                _ = posix_compat.write(report[1], &.{0}) catch 0;
                posix_compat.exit(1);
            };
        }
        _ = posix_compat.write(report[1], &.{1}) catch 0;
        posix_compat.exit(0);
    }
    _ = posix.system.close(report[1]);
    var marker: [1]u8 = undefined;
    const marker_length = try posix.read(report[0], &marker);
    _ = posix.system.close(report[0]);
    const result = posix_compat.waitpid(child_id, 0);
    try std.testing.expectEqual(@as(usize, 1), marker_length);
    try std.testing.expectEqual(@as(u8, 1), marker[0]);
    try std.testing.expect(posix.W.IFEXITED(result.status));
    try std.testing.expectEqual(@as(u8, 0), posix.W.EXITSTATUS(result.status));
}

test "PTY abort wakes a reader blocked without child output" {
    const Reader = struct {
        pty: *Pty,
        result: ?usize = null,
        failure: ?anyerror = null,

        fn run(self: *@This()) void {
            var buffer: [32]u8 = undefined;
            const result = self.pty.read(&buffer) catch |err| {
                self.failure = err;
                return;
            };
            self.result = result;
        }
    };

    var instance = try Pty.open(.{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    var reader: Reader = .{ .pty = &instance };
    var thread = try std.Thread.spawn(.{}, Reader.run, .{&reader});
    instance.registerReaderThread(thread.getHandle());
    monotonic.sleep(10 * std.time.ns_per_ms);
    try instance.abortRead();
    thread.join();
    try std.testing.expect(reader.failure == null);
    try std.testing.expectEqual(@as(usize, 0), reader.result.?);
}
