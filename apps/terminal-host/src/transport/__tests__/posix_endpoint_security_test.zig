const std = @import("std");
const builtin = @import("builtin");
const subject = @import("terminal-host-transport");

const security = subject.testing.posixEndpointSecurity;
const posix_compat = subject.testing.posixCompat;
const darwin_acl = if (builtin.os.tag == .macos) @cImport({
    @cInclude("posix_endpoint_security_test.h");
}) else struct {};
const posix_fixture = if (builtin.os.tag == .freebsd or
    builtin.os.tag == .linux or
    builtin.os.tag == .macos) struct {
    extern "c" fn mkfifoat(
        directory_fd: c_int,
        path: [*:0]const u8,
        mode: std.posix.mode_t,
    ) c_int;
} else struct {};

const TestDirectory = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.Io.Dir,
    path_bytes: []u8,

    fn init(allocator: std.mem.Allocator) !TestDirectory {
        var random: [8]u8 = undefined;
        std.testing.io.random(&random);
        const encoded = std.fmt.bytesToHex(random, .lower);
        const temporary_root = if (builtin.os.tag == .macos) "/private/tmp" else "/tmp";
        const path_bytes = try std.fmt.allocPrint(
            allocator,
            "{s}/ob-{s}",
            .{ temporary_root, encoded },
        );
        errdefer allocator.free(path_bytes);
        try std.Io.Dir.createDirAbsolute(std.testing.io, path_bytes, @enumFromInt(0o700));
        errdefer std.Io.Dir.cwd().deleteDir(std.testing.io, path_bytes) catch {};
        const directory = try std.Io.Dir.openDirAbsolute(std.testing.io, path_bytes, .{
            .follow_symlinks = false,
        });
        return .{
            .allocator = allocator,
            .directory = directory,
            .path_bytes = path_bytes,
        };
    }

    fn deinit(self: *TestDirectory) void {
        self.directory.close(std.testing.io);
        std.Io.Dir.cwd().deleteTree(std.testing.io, self.path_bytes) catch {};
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
        darwin_acl.ob_test_add_darwin_everyone_acl_fd(runtime.directory.handle, 1),
    );
    const before = try runtime.directory.stat(std.testing.io);
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o700), before.permissions.toMode() & 0o777);
    try std.testing.expectEqual(
        @as(c_int, 1),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(runtime.directory.handle),
    );

    const prepared = try security.prepareRuntimeDirectory(std.testing.io, runtime.path_bytes);
    defer prepared.close(std.testing.io);
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(prepared.handle),
    );
    const after = try prepared.stat(std.testing.io);
    try std.testing.expectEqual(@as(std.posix.mode_t, 0o700), after.permissions.toMode() & 0o777);
}

test "Darwin runtime preparation removes an ACL inherited by a new child directory" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;
    var parent = try TestDirectory.init(std.testing.allocator);
    defer parent.deinit();
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_add_darwin_everyone_acl_fd(parent.directory.handle, 1),
    );
    const child_path = try std.fs.path.join(std.testing.allocator, &.{ parent.path_bytes, "child" });
    defer std.testing.allocator.free(child_path);
    try std.Io.Dir.createDirAbsolute(std.testing.io, child_path, @enumFromInt(0o700));
    const child = try std.Io.Dir.openDirAbsolute(std.testing.io, child_path, .{
        .follow_symlinks = false,
    });
    try std.testing.expectEqual(
        @as(c_int, 1),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(child.handle),
    );
    child.close(std.testing.io);

    const prepared = try security.prepareRuntimeDirectory(std.testing.io, child_path);
    defer prepared.close(std.testing.io);
    try std.testing.expectEqual(
        @as(c_int, 0),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(prepared.handle),
    );
}

test "POSIX runtime validation uses the opened directory identity and private mode" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();

    const validated = try security.validateRuntimeDirectory(std.testing.io, runtime.path_bytes);
    validated.close(std.testing.io);
    const different_uid = if (std.c.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.c.geteuid() - 1
    else
        std.c.geteuid() + 1;
    try std.testing.expectError(
        error.InsecureRuntimeDirectory,
        security.validateRuntimeDirectoryForUser(std.testing.io, runtime.path_bytes, different_uid),
    );

    try runtime.directory.setPermissions(std.testing.io, @enumFromInt(0o750));
    try std.testing.expectError(
        error.InsecureRuntimeDirectory,
        security.validateRuntimeDirectory(std.testing.io, runtime.path_bytes),
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
    try std.Io.Dir.symLinkAbsolute(std.testing.io, runtime.path_bytes, link_path, .{});
    defer std.Io.Dir.cwd().deleteFile(std.testing.io, link_path) catch {};

    if (security.validateRuntimeDirectory(std.testing.io, link_path)) |directory| {
        directory.close(std.testing.io);
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
        const target = try runtime.directory.createFile(std.testing.io, target_name, .{
            .read = true,
            .permissions = @enumFromInt(0o640),
        });
        defer target.close(std.testing.io);
        try target.writeStreamingAll(std.testing.io, target_contents);
        try target.setPermissions(std.testing.io, @enumFromInt(0o640));
    }
    try runtime.directory.symLink(std.testing.io, target_name, lock_name, .{});

    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(std.testing.io, runtime.directory, lock_name, std.c.geteuid()),
    );

    const target = try runtime.directory.openFile(std.testing.io, target_name, .{
        .follow_symlinks = false,
    });
    defer target.close(std.testing.io);
    const target_status = try target.stat(std.testing.io);
    try std.testing.expectEqual(
        @as(std.posix.mode_t, 0o640),
        target_status.permissions.toMode() & 0o777,
    );
    var contents: [target_contents.len]u8 = undefined;
    try std.testing.expectEqual(contents.len, try target.readPositionalAll(std.testing.io, &contents, 0));
    try std.testing.expectEqualStrings(target_contents, &contents);
}

test "POSIX lock acquisition rejects non-regular entries and insecure regular metadata" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();

    try runtime.directory.createDir(std.testing.io, "directory.l", @enumFromInt(0o700));
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(std.testing.io, runtime.directory, "directory.l", std.c.geteuid()),
    );
    try std.testing.expectEqual(
        @as(c_int, 0),
        posix_fixture.mkfifoat(runtime.directory.handle, "fifo.l", 0o600),
    );
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(std.testing.io, runtime.directory, "fifo.l", std.c.geteuid()),
    );

    {
        const wrong_mode = try runtime.directory.createFile(std.testing.io, "wrong-mode.l", .{
            .permissions = @enumFromInt(0o600),
        });
        defer wrong_mode.close(std.testing.io);
        try wrong_mode.setPermissions(std.testing.io, @enumFromInt(0o640));
    }
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(std.testing.io, runtime.directory, "wrong-mode.l", std.c.geteuid()),
    );
    const wrong_mode_file = try runtime.directory.openFile(std.testing.io, "wrong-mode.l", .{
        .follow_symlinks = false,
    });
    defer wrong_mode_file.close(std.testing.io);
    const wrong_mode_status = try wrong_mode_file.stat(std.testing.io);
    try std.testing.expectEqual(
        @as(std.posix.mode_t, 0o640),
        wrong_mode_status.permissions.toMode() & 0o777,
    );

    {
        const wrong_owner = try runtime.directory.createFile(std.testing.io, "wrong-owner.l", .{
            .permissions = @enumFromInt(0o600),
        });
        defer wrong_owner.close(std.testing.io);
        try wrong_owner.setPermissions(std.testing.io, @enumFromInt(0o600));
    }
    const different_uid = if (std.c.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.c.geteuid() - 1
    else
        std.c.geteuid() + 1;
    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(std.testing.io, runtime.directory, "wrong-owner.l", different_uid),
    );
}

test "Darwin lock acquisition rejects an existing extended ACL without changing it" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const lock_name = "acl.l";

    {
        const lock_file = try runtime.directory.createFile(std.testing.io, lock_name, .{
            .permissions = @enumFromInt(0o600),
        });
        defer lock_file.close(std.testing.io);
        try lock_file.setPermissions(std.testing.io, @enumFromInt(0o600));
        try std.testing.expectEqual(
            @as(c_int, 0),
            darwin_acl.ob_test_add_darwin_everyone_acl_fd(lock_file.handle, 0),
        );
    }

    try std.testing.expectError(
        error.InsecureLockFile,
        security.acquireLock(std.testing.io, runtime.directory, lock_name, std.c.geteuid()),
    );
    const lock_file = try runtime.directory.openFile(std.testing.io, lock_name, .{});
    defer lock_file.close(std.testing.io);
    try std.testing.expectEqual(
        @as(c_int, 1),
        darwin_acl.ob_test_darwin_fd_has_extended_acl(lock_file.handle),
    );
    const status = try lock_file.stat(std.testing.io);
    try std.testing.expectEqual(
        @as(std.posix.mode_t, 0o600),
        status.permissions.toMode() & 0o777,
    );
}

test "POSIX lock acquisition accepts a valid stale regular file without truncating it" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const lock_name = "stale.l";
    const stale_contents = "stale lock contents\n";

    {
        const stale = try runtime.directory.createFile(std.testing.io, lock_name, .{
            .read = true,
            .permissions = @enumFromInt(0o600),
        });
        defer stale.close(std.testing.io);
        try stale.writeStreamingAll(std.testing.io, stale_contents);
        try stale.setPermissions(std.testing.io, @enumFromInt(0o600));
    }
    const acquired = try security.acquireLock(
        std.testing.io,
        runtime.directory,
        lock_name,
        std.c.geteuid(),
    );
    defer acquired.close(std.testing.io);

    const stale = try runtime.directory.openFile(std.testing.io, lock_name, .{});
    defer stale.close(std.testing.io);
    var contents: [stale_contents.len]u8 = undefined;
    try std.testing.expectEqual(contents.len, try stale.readPositionalAll(std.testing.io, &contents, 0));
    try std.testing.expectEqualStrings(stale_contents, &contents);
}

test "POSIX lock acquisition retains lifetime nonblocking exclusivity" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const lock_name = "exclusive.l";

    const acquired = try security.acquireLock(
        std.testing.io,
        runtime.directory,
        lock_name,
        std.c.geteuid(),
    );
    defer acquired.close(std.testing.io);
    const status = try acquired.stat(std.testing.io);
    try std.testing.expectEqual(
        @as(std.posix.mode_t, 0o600),
        status.permissions.toMode() & 0o777,
    );
    try std.testing.expectError(
        error.AlreadyRunning,
        security.acquireLock(std.testing.io, runtime.directory, lock_name, std.c.geteuid()),
    );
}

test "bound POSIX endpoint validation enforces owner type mode and effective ACL" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const directory = try security.prepareRuntimeDirectory(std.testing.io, runtime.path_bytes);
    defer directory.close(std.testing.io);
    const endpoint_name = "endpoint.s";

    try std.testing.expectError(
        error.FileNotFound,
        security.validateEndpoint(directory, endpoint_name, std.c.geteuid()),
    );

    const original_directory = try std.Io.Dir.cwd().openDir(std.testing.io, ".", .{});
    defer original_directory.close(std.testing.io);
    try posix_compat.fchdir(directory.handle);
    defer posix_compat.fchdir(original_directory.handle) catch {};

    const address = try std.Io.net.UnixAddress.init(endpoint_name);
    var listener = try address.listen(std.testing.io, .{ .kernel_backlog = 1 });
    defer listener.deinit(std.testing.io);
    defer directory.deleteFile(std.testing.io, endpoint_name) catch {};
    try security.secureEndpoint(directory, endpoint_name, std.c.geteuid());
    try security.validateEndpoint(directory, endpoint_name, std.c.geteuid());

    try directory.setFilePermissions(
        std.testing.io,
        endpoint_name,
        @enumFromInt(0o666),
        .{ .follow_symlinks = false },
    );
    try std.testing.expectError(
        error.InsecureEndpoint,
        security.validateEndpoint(directory, endpoint_name, std.c.geteuid()),
    );
    try security.secureEndpoint(directory, endpoint_name, std.c.geteuid());

    const different_uid = if (std.c.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.c.geteuid() - 1
    else
        std.c.geteuid() + 1;
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
        const endpoint_stat = try directory.statFile(
            std.testing.io,
            endpoint_name,
            .{ .follow_symlinks = false },
        );
        try std.testing.expectEqual(
            @as(std.posix.mode_t, 0o600),
            endpoint_stat.permissions.toMode() & 0o777,
        );
        try std.testing.expectError(
            error.InsecureEndpoint,
            security.validateEndpoint(directory, endpoint_name, std.c.geteuid()),
        );
        try security.secureEndpoint(directory, endpoint_name, std.c.geteuid());
        try std.testing.expectEqual(
            @as(c_int, 0),
            darwin_acl.ob_test_darwin_link_has_extended_acl(&endpoint_name_z),
        );
    }

    const regular_file = try directory.createFile(std.testing.io, "regular.s", .{
        .permissions = @enumFromInt(0o600),
    });
    regular_file.close(std.testing.io);
    defer directory.deleteFile(std.testing.io, "regular.s") catch {};
    try std.testing.expectError(
        error.InsecureEndpoint,
        security.validateEndpoint(directory, "regular.s", std.c.geteuid()),
    );
}

test "native POSIX peer credentials admit only the actual effective UID" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const directory = try security.prepareRuntimeDirectory(std.testing.io, runtime.path_bytes);
    defer directory.close(std.testing.io);
    const endpoint_name = "peer.s";

    const original_directory = try std.Io.Dir.cwd().openDir(std.testing.io, ".", .{});
    defer original_directory.close(std.testing.io);
    try posix_compat.fchdir(directory.handle);
    defer posix_compat.fchdir(original_directory.handle) catch {};
    const address = try std.Io.net.UnixAddress.init(endpoint_name);
    var listener = try address.listen(std.testing.io, .{ .kernel_backlog = 1 });
    defer listener.deinit(std.testing.io);
    defer directory.deleteFile(std.testing.io, endpoint_name) catch {};
    try security.secureEndpoint(directory, endpoint_name, std.c.geteuid());

    const client = try address.connect(std.testing.io);
    defer client.close(std.testing.io);
    const accepted = try listener.accept(std.testing.io);
    defer accepted.close(std.testing.io);
    try security.requirePeerEffectiveUid(client.socket.handle, std.c.geteuid());
    try security.requirePeerEffectiveUid(accepted.socket.handle, std.c.geteuid());
    const different_uid = if (std.c.geteuid() == std.math.maxInt(std.posix.uid_t))
        std.c.geteuid() - 1
    else
        std.c.geteuid() + 1;
    try std.testing.expectError(
        error.PeerIdentityMismatch,
        security.requirePeerEffectiveUid(client.socket.handle, different_uid),
    );

    const pipe = try posix_compat.pipe2(.{});
    defer _ = std.posix.system.close(pipe[0]);
    defer _ = std.posix.system.close(pipe[1]);
    try std.testing.expectError(
        error.PeerIdentityUnavailable,
        security.requirePeerEffectiveUid(pipe[0], std.c.geteuid()),
    );
}
