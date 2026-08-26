const std = @import("std");
const builtin = @import("builtin");

const c = if (builtin.os.tag == .freebsd or builtin.os.tag == .linux or builtin.os.tag == .macos)
    @cImport({
        @cInclude("posix_endpoint_security.h");
    })
else
    struct {};

pub fn prepareRuntimeDirectory(io: std.Io, runtime_directory: []const u8) !std.Io.Dir {
    return prepareRuntimeDirectoryForUser(io, runtime_directory, std.c.geteuid());
}

pub fn prepareRuntimeDirectoryForUser(
    io: std.Io,
    runtime_directory: []const u8,
    expected_uid: std.posix.uid_t,
) !std.Io.Dir {
    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    std.Io.Dir.createDirAbsolute(io, runtime_directory, @enumFromInt(0o700)) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };
    var directory = try std.Io.Dir.openDirAbsolute(io, runtime_directory, .{
        .follow_symlinks = false,
    });
    errdefer directory.close(io);
    const initial_status = try directory.stat(io);
    if (initial_status.permissions.toMode() & 0o077 != 0) {
        return error.InsecureRuntimeDirectory;
    }
    try requireSecurityStatus(c.ob_posix_secure_runtime_directory(
        directory.handle,
        try checkedUid(expected_uid),
    ), error.InsecureRuntimeDirectory);
    return directory;
}

pub fn validateRuntimeDirectory(io: std.Io, runtime_directory: []const u8) !std.Io.Dir {
    return validateRuntimeDirectoryForUser(io, runtime_directory, std.c.geteuid());
}

pub fn validateRuntimeDirectoryForUser(
    io: std.Io,
    runtime_directory: []const u8,
    expected_uid: std.posix.uid_t,
) !std.Io.Dir {
    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    var directory = try std.Io.Dir.openDirAbsolute(io, runtime_directory, .{
        .follow_symlinks = false,
    });
    errdefer directory.close(io);
    try requireSecurityStatus(c.ob_posix_validate_runtime_directory(
        directory.handle,
        try checkedUid(expected_uid),
    ), error.InsecureRuntimeDirectory);
    return directory;
}

pub fn acquireLock(
    io: std.Io,
    directory: std.Io.Dir,
    lock_name: []const u8,
    expected_uid: std.posix.uid_t,
) !std.Io.File {
    const lock_name_z = try std.posix.toPosixPath(lock_name);
    var descriptor: c_int = -1;
    try requireSecurityStatus(c.ob_posix_open_lock_at(
        directory.handle,
        &lock_name_z,
        try checkedUid(expected_uid),
        &descriptor,
    ), error.InsecureLockFile);
    if (descriptor < 0) return error.PosixEndpointSecurityFailure;

    var lock_file: std.Io.File = .{
        .flags = .{ .nonblocking = false },
        .handle = descriptor,
    };
    errdefer lock_file.close(io);
    if (!try lock_file.tryLock(io, .exclusive)) return error.AlreadyRunning;
    return lock_file;
}

pub fn secureEndpoint(
    directory: std.Io.Dir,
    endpoint_name: []const u8,
    expected_uid: std.posix.uid_t,
) !void {
    const endpoint_name_z = try std.posix.toPosixPath(endpoint_name);
    try requireEndpointSecurityStatus(c.ob_posix_secure_endpoint_at(
        directory.handle,
        &endpoint_name_z,
        try checkedUid(expected_uid),
    ), error.InsecureEndpoint);
}

pub fn validateEndpoint(
    directory: std.Io.Dir,
    endpoint_name: []const u8,
    expected_uid: std.posix.uid_t,
) !void {
    const endpoint_name_z = try std.posix.toPosixPath(endpoint_name);
    try requireEndpointSecurityStatus(c.ob_posix_validate_endpoint_at(
        directory.handle,
        &endpoint_name_z,
        try checkedUid(expected_uid),
    ), error.InsecureEndpoint);
}

pub fn requirePeerEffectiveUid(socket: std.posix.socket_t, expected_uid: std.posix.uid_t) !void {
    const status = c.ob_posix_require_peer_effective_uid(socket, try checkedUid(expected_uid));
    return switch (status) {
        c.OB_POSIX_SECURITY_OK => {},
        c.OB_POSIX_SECURITY_PEER_IDENTITY_MISMATCH => error.PeerIdentityMismatch,
        c.OB_POSIX_SECURITY_PEER_IDENTITY_UNAVAILABLE => error.PeerIdentityUnavailable,
        else => error.PosixEndpointSecurityFailure,
    };
}

pub fn endpointName(allocator: std.mem.Allocator, endpoint_id: []const u8) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}.s", .{endpoint_id});
}

pub fn lockName(allocator: std.mem.Allocator, endpoint_id: []const u8) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}.l", .{endpoint_id});
}

fn checkedUid(uid: std.posix.uid_t) !u32 {
    return std.math.cast(u32, uid) orelse error.PosixEndpointSecurityFailure;
}

fn requireSecurityStatus(status: c_int, insecure_error: anyerror) !void {
    return switch (status) {
        c.OB_POSIX_SECURITY_OK => {},
        c.OB_POSIX_SECURITY_INSECURE => insecure_error,
        else => error.PosixEndpointSecurityFailure,
    };
}

fn requireEndpointSecurityStatus(status: c_int, insecure_error: anyerror) !void {
    if (status == c.OB_POSIX_SECURITY_ENDPOINT_UNAVAILABLE) return error.FileNotFound;
    return requireSecurityStatus(status, insecure_error);
}
