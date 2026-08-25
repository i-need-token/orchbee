const std = @import("std");
const builtin = @import("builtin");
const subject = @import("terminal-host-transport");

const security = subject.testing.posixEndpointSecurity;
const darwin_acl = if (builtin.os.tag == .macos) @cImport({
    @cInclude("posix_endpoint_security_test.h");
}) else struct {};
const posix_fixture = if (builtin.os.tag == .freebsd or
    builtin.os.tag == .linux or
    builtin.os.tag == .macos) @cImport({
    @cInclude("sys/stat.h");
}) else struct {};

const TestDirectory = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.fs.Dir,
    path_bytes: []u8,

    fn init(allocator: std.mem.Allocator) !TestDirectory {
        var random: [8]u8 = undefined;
        std.crypto.random.bytes(&random);
        const encoded = std.fmt.bytesToHex(random, .lower);
        const temporary_root = if (builtin.os.tag == .macos) "/private/tmp" else "/tmp";
        const path_bytes = try std.fmt.allocPrint(
            allocator,
            "{s}/ob-{s}",
            .{ temporary_root, encoded },
        );
        errdefer allocator.free(path_bytes);
        try std.posix.mkdir(path_bytes, 0o700);
        errdefer std.fs.deleteDirAbsolute(path_bytes) catch {};
        const directory = try std.fs.openDirAbsolute(path_bytes, .{ .no_follow = true });
        return .{
            .allocator = allocator,
            .directory = directory,
            .path_bytes = path_bytes,
        };
    }

    fn deinit(self: *TestDirectory) void {
        self.directory.close();
        std.fs.deleteTreeAbsolute(self.path_bytes) catch {};
        self.allocator.free(self.path_bytes);
        self.* = undefined;
    }
};

test "Darwin runtime preparation strips an effective inheritable everyone ACL at mode 0700" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_add_darwin_everyone_acl_fd(runtime.directory.fd, 1),
    );
    const before = try std.posix.fstat(runtime.directory.fd);
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o700), before.mode & 0o777);
    try std.testing.expectEqual(
        @as(c_int, 1),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(runtime.directory.fd),
    );

    var prepared = try security.prepareRuntimeDirectory(runtime.path_bytes);
    defer prepared.close();
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(prepared.fd),
    );
    const after = try std.posix.fstat(prepared.fd);
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o700), after.mode & 0o777);
}

test "Darwin runtime preparation removes an ACL inherited by a new child directory" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;
    var parent = try TestDirectory.init(std.testing.allocator);
    defer parent.deinit();
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_add_darwin_everyone_acl_fd(parent.directory.fd, 1),
    );
    const child_path = try std.fs.path.join(std.testing.allocator, &.{ parent.path_bytes, "child" });
    defer std.testing.allocator.free(child_path);
    try std.posix.mkdir(child_path, 0o700);
    var child = try std.fs.openDirAbsolute(child_path, .{ .no_follow = true });
    try std.testing.expectEqual(
        @as(c_int, 1),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(child.fd),
    );
    child.close();

    var prepared = try security.prepareRuntimeDirectory(child_path);
    defer prepared.close();
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(prepared.fd),
    );
}

test "POSIX runtime validation uses the opened directory identity and private mode" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();

    var validated = try security.validateRuntimeDirectory(runtime.path_bytes);
    validated.close();
    const different_uid = if (std.posix.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.posix.geteuid() - 1
    else
        std.posix.geteuid() + 1;
    try std.testing.expectError(
        error.InsecureRuntimeDirectory,
        security.validateRuntimeDirectoryForUser(runtime.path_bytes, different_uid),
    );

    try std.posix.fchmod(runtime.directory.fd, 0o750);
    try std.testing.expectError(
        error.InsecureRuntimeDirectory,
        security.validateRuntimeDirectory(runtime.path_bytes),
    );
}

test "POSIX runtime validation does not follow a replacement symlink" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const link_path = try std.fmt.allocPrint(
        std.testing.allocator,
        "{s}-link",
        .{runtime.path_bytes},
    );
    defer std.testing.allocator.free(link_path);
    try std.posix.symlink(runtime.path_bytes, link_path);
    defer std.fs.deleteFileAbsolute(link_path) catch {};

    if (security.validateRuntimeDirectory(link_path)) |directory| {
        var followed = directory;
        followed.close();
        return error.TestUnexpectedResult;
    } else |_| {}
}

test "POSIX lock acquisition rejects a pre-existing link without changing its target" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const target_name = "lock-target";
    const lock_name = "linked.l";
    const target_contents = "preserve lock target\n";

    {
        var target = try runtime.directory.createFile(target_name, .{
            .read = true,
            .mode = 0o640,
        });
        defer target.close();
        try target.writeAll(target_contents);
        try target.chmod(0o640);
    }
    try runtime.directory.symLink(target_name, lock_name, .{});

    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(runtime.directory, lock_name, std.posix.geteuid()),
    );

    const target_status = try std.posix.fstatat(
        runtime.directory.fd,
        target_name,
        std.posix.AT.SYMLINK_NOFOLLOW,
    );
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o640), target_status.mode & 0o777);
    var target = try runtime.directory.openFile(target_name, .{});
    defer target.close();
    var contents: [target_contents.len]u8 = undefined;
    try std.testing.expectEqual(contents.len, try target.readAll(&contents));
    try std.testing.expectEqualStrings(target_contents, &contents);
}

test "POSIX lock acquisition rejects non-regular entries and insecure regular metadata" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();

    try runtime.directory.makeDir("directory.l");
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(runtime.directory, "directory.l", std.posix.geteuid()),
    );
    try std.testing.expectEqual(
        @as(c_int, 0),
        posix_fixture.mkfifoat(runtime.directory.fd, "fifo.l", 0o600),
    );
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(runtime.directory, "fifo.l", std.posix.geteuid()),
    );

    {
        var wrong_mode = try runtime.directory.createFile("wrong-mode.l", .{ .mode = 0o600 });
        defer wrong_mode.close();
        try wrong_mode.chmod(0o640);
    }
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(runtime.directory, "wrong-mode.l", std.posix.geteuid()),
    );
    const wrong_mode_status = try std.posix.fstatat(
        runtime.directory.fd,
        "wrong-mode.l",
        std.posix.AT.SYMLINK_NOFOLLOW,
    );
    try std.testing.expectEqual(
        @as(std.posix.mode_t, 0o640),
        wrong_mode_status.mode & 0o777,
    );

    {
        var wrong_owner = try runtime.directory.createFile("wrong-owner.l", .{ .mode = 0o600 });
        defer wrong_owner.close();
        try wrong_owner.chmod(0o600);
    }
    const different_uid = if (std.posix.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.posix.geteuid() - 1
    else
        std.posix.geteuid() + 1;
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(runtime.directory, "wrong-owner.l", different_uid),
    );
}

test "Darwin lock acquisition rejects an existing extended ACL without changing it" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const lock_name = "acl.l";

    {
        var lock_file = try runtime.directory.createFile(lock_name, .{ .mode = 0o600 });
        defer lock_file.close();
        try lock_file.chmod(0o600);
        try std.testing.expectEqual(
            @as(c_int, 0),
            darwin_acl.ob_test_add_darwin_everyone_acl_fd(lock_file.handle, 0),
        );
    }

    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(runtime.directory, lock_name, std.posix.geteuid()),
    );
    var lock_file = try runtime.directory.openFile(lock_name, .{});
    defer lock_file.close();
    try std.testing.expectEqual(
        @as(c_int, 1),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(lock_file.handle),
    );
    const status = try std.posix.fstat(lock_file.handle);
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o600), status.mode & 0o777);
}

test "POSIX lock acquisition accepts a valid stale regular file without truncating it" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const lock_name = "stale.l";
    const stale_contents = "stale lock contents\n";

    {
        var stale = try runtime.directory.createFile(lock_name, .{
            .read = true,
            .mode = 0o600,
        });
        defer stale.close();
        try stale.writeAll(stale_contents);
        try stale.chmod(0o600);
    }
    var acquired = try security.acquireLock(
        runtime.directory,
        lock_name,
        std.posix.geteuid(),
    );
    defer acquired.close();

    var stale = try runtime.directory.openFile(lock_name, .{});
    defer stale.close();
    var contents: [stale_contents.len]u8 = undefined;
    try std.testing.expectEqual(contents.len, try stale.readAll(&contents));
    try std.testing.expectEqualStrings(stale_contents, &contents);
}

test "POSIX lock acquisition retains lifetime nonblocking exclusivity" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const lock_name = "exclusive.l";

    var acquired = try security.acquireLock(
        runtime.directory,
        lock_name,
        std.posix.geteuid(),
    );
    defer acquired.close();
    const status = try std.posix.fstat(acquired.handle);
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o600), status.mode & 0o777);
    try std.testing.expectError(
        error.AlreadyRunning,
        security.acquireLock(runtime.directory, lock_name, std.posix.geteuid()),
    );
}

test "bound POSIX endpoint validation enforces owner type mode and effective ACL" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    var directory = try security.prepareRuntimeDirectory(runtime.path_bytes);
    defer directory.close();
    const endpoint_name = "endpoint.s";

    try std.testing.expectError(
        error.FileNotFound,
        security.validateEndpoint(directory, endpoint_name, std.posix.geteuid()),
    );

    var original_directory = try std.fs.cwd().openDir(".", .{});
    defer original_directory.close();
    try std.posix.fchdir(directory.fd);
    defer std.posix.fchdir(original_directory.fd) catch {};

    const address = try std.net.Address.initUnix(endpoint_name);
    var listener = try address.listen(.{ .kernel_backlog = 1 });
    defer listener.deinit();
    defer directory.deleteFile(endpoint_name) catch {};
    try security.secureEndpoint(directory, endpoint_name, std.posix.geteuid());
    try security.validateEndpoint(directory, endpoint_name, std.posix.geteuid());

    try std.posix.fchmodat(directory.fd, endpoint_name, 0o666, 0);
    try std.testing.expectError(
        error.InsecureEndpoint,
        security.validateEndpoint(directory, endpoint_name, std.posix.geteuid()),
    );
    try security.secureEndpoint(directory, endpoint_name, std.posix.geteuid());

    const different_uid = if (std.posix.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.posix.geteuid() - 1
    else
        std.posix.geteuid() + 1;
    try std.testing.expectError(
        error.InsecureEndpoint,
        security.validateEndpoint(directory, endpoint_name, different_uid),
    );

    if (builtin.os.tag == .macos) {
        const endpoint_name_z = try std.posix.toPosixPath(endpoint_name);
        try std.testing.expectEqual(
            @as(c_int, 0),
            darwin_acl.ob_test_add_darwin_everyone_acl_link(&endpoint_name_z),
        );
        const endpoint_stat = try std.posix.fstatat(
            directory.fd,
            endpoint_name,
            std.posix.AT.SYMLINK_NOFOLLOW,
        );
        try std.testing.expectEqual(@as(std.posix.mode_t, 0o600), endpoint_stat.mode & 0o777);
        try std.testing.expectError(
            error.InsecureEndpoint,
            security.validateEndpoint(directory, endpoint_name, std.posix.geteuid()),
        );
        try security.secureEndpoint(directory, endpoint_name, std.posix.geteuid());
        try std.testing.expectEqual(
            @as(c_int, 0),
            darwin_acl.ob_test_darwin_link_has_extended_acl(&endpoint_name_z),
        );
    }

    var regular_file = try directory.createFile("regular.s", .{ .mode = 0o600 });
    regular_file.close();
    defer directory.deleteFile("regular.s") catch {};
    try std.testing.expectError(
        error.InsecureEndpoint,
        security.validateEndpoint(directory, "regular.s", std.posix.geteuid()),
    );
}

test "native POSIX peer credentials admit only the actual effective UID" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    var directory = try security.prepareRuntimeDirectory(runtime.path_bytes);
    defer directory.close();
    const endpoint_name = "peer.s";

    var original_directory = try std.fs.cwd().openDir(".", .{});
    defer original_directory.close();
    try std.posix.fchdir(directory.fd);
    defer std.posix.fchdir(original_directory.fd) catch {};
    const address = try std.net.Address.initUnix(endpoint_name);
    var listener = try address.listen(.{ .kernel_backlog = 1 });
    defer listener.deinit();
    defer directory.deleteFile(endpoint_name) catch {};
    try security.secureEndpoint(directory, endpoint_name, std.posix.geteuid());

    const client = try std.net.connectUnixSocket(endpoint_name);
    defer client.close();
    var accepted = try listener.accept();
    defer accepted.stream.close();
    try security.requirePeerEffectiveUid(client.handle, std.posix.geteuid());
    try security.requirePeerEffectiveUid(accepted.stream.handle, std.posix.geteuid());
    const different_uid = if (std.posix.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.posix.geteuid() - 1
    else
        std.posix.geteuid() + 1;
    try std.testing.expectError(
        error.PeerIdentityMismatch,
        security.requirePeerEffectiveUid(client.handle, different_uid),
    );

    const pipe = try std.posix.pipe();
    defer std.posix.close(pipe[0]);
    defer std.posix.close(pipe[1]);
    try std.testing.expectError(
        error.PeerIdentityUnavailable,
        security.requirePeerEffectiveUid(pipe[0], std.posix.geteuid()),
    );
}
