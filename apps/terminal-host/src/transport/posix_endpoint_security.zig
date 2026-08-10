const std = @import("std");
const builtin = @import("builtin");

const c = if (builtin.os.tag == .freebsd or builtin.os.tag == .linux or builtin.os.tag == .macos)
    @cImport({
        @cInclude("posix_endpoint_security.h");
    })
else
    struct {};

pub const authenticated_status_ready: u8 = 1;
pub const authenticated_status_endpoint_unavailable: u8 = 2;
pub const authenticated_status_identity_mismatch: u8 = 3;
pub const authenticated_status_identity_unavailable: u8 = 4;
pub const authenticated_status_insecure_endpoint: u8 = 5;
pub const authenticated_status_system_error: u8 = 6;

pub fn prepareRuntimeDirectory(runtime_directory: []const u8) !std.fs.Dir {
    return prepareRuntimeDirectoryForUser(runtime_directory, std.posix.geteuid());
}

pub fn prepareRuntimeDirectoryForUser(
    runtime_directory: []const u8,
    expected_uid: std.posix.uid_t,
) !std.fs.Dir {
    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    std.posix.mkdir(runtime_directory, 0o700) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };
    var directory = try std.fs.openDirAbsolute(runtime_directory, .{ .no_follow = true });
    errdefer directory.close();
    const initial_status = try std.posix.fstat(directory.fd);
    if (initial_status.uid != expected_uid or initial_status.mode & 0o077 != 0) {
        return error.InsecureRuntimeDirectory;
    }
    try requireSecurityStatus(c.ob_posix_secure_runtime_directory(
        directory.fd,
        try checkedUid(expected_uid),
    ), error.InsecureRuntimeDirectory);
    return directory;
}

pub fn validateRuntimeDirectory(runtime_directory: []const u8) !std.fs.Dir {
    return validateRuntimeDirectoryForUser(runtime_directory, std.posix.geteuid());
}

pub fn validateRuntimeDirectoryForUser(
    runtime_directory: []const u8,
    expected_uid: std.posix.uid_t,
) !std.fs.Dir {
    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    var directory = try std.fs.openDirAbsolute(runtime_directory, .{ .no_follow = true });
    errdefer directory.close();
    try requireSecurityStatus(c.ob_posix_validate_runtime_directory(
        directory.fd,
        try checkedUid(expected_uid),
    ), error.InsecureRuntimeDirectory);
    return directory;
}

pub fn acquireLock(
    directory: std.fs.Dir,
    lock_name: []const u8,
    expected_uid: std.posix.uid_t,
) !std.fs.File {
    const lock_name_z = try std.posix.toPosixPath(lock_name);
    var descriptor: c_int = -1;
    try requireSecurityStatus(c.ob_posix_open_lock_at(
        directory.fd,
        &lock_name_z,
        try checkedUid(expected_uid),
        &descriptor,
    ), error.InsecureLockFile);
    if (descriptor < 0) return error.PosixEndpointSecurityFailure;

    var lock_file: std.fs.File = .{ .handle = descriptor };
    errdefer lock_file.close();
    if (!try lock_file.tryLock(.exclusive)) return error.AlreadyRunning;
    return lock_file;
}

pub fn secureEndpoint(
    directory: std.fs.Dir,
    endpoint_name: []const u8,
    expected_uid: std.posix.uid_t,
) !void {
    const endpoint_name_z = try std.posix.toPosixPath(endpoint_name);
    try requireEndpointSecurityStatus(c.ob_posix_secure_endpoint_at(
        directory.fd,
        &endpoint_name_z,
        try checkedUid(expected_uid),
    ), error.InsecureEndpoint);
}

pub fn validateEndpoint(
    directory: std.fs.Dir,
    endpoint_name: []const u8,
    expected_uid: std.posix.uid_t,
) !void {
    const endpoint_name_z = try std.posix.toPosixPath(endpoint_name);
    try requireEndpointSecurityStatus(c.ob_posix_validate_endpoint_at(
        directory.fd,
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

pub fn connectAuthenticated(
    allocator: std.mem.Allocator,
    runtime_directory: []const u8,
    endpoint_id: []const u8,
) !std.net.Stream {
    return connectAuthenticatedForUser(
        allocator,
        runtime_directory,
        endpoint_id,
        std.posix.geteuid(),
    );
}

pub fn connectAuthenticatedForUser(
    allocator: std.mem.Allocator,
    runtime_directory: []const u8,
    endpoint_id: []const u8,
    expected_uid: std.posix.uid_t,
) !std.net.Stream {
    var directory = try validateRuntimeDirectoryForUser(runtime_directory, expected_uid);
    defer directory.close();
    const endpoint_name = try endpointName(allocator, endpoint_id);
    defer allocator.free(endpoint_name);

    var original_directory = try std.fs.cwd().openDir(".", .{});
    defer original_directory.close();
    try std.posix.fchdir(directory.fd);
    var restored = false;
    defer if (!restored) std.posix.fchdir(original_directory.fd) catch {};

    try validateEndpoint(directory, endpoint_name, expected_uid);
    const stream = try std.net.connectUnixSocket(endpoint_name);
    errdefer stream.close();
    try std.posix.fchdir(original_directory.fd);
    restored = true;
    try requirePeerEffectiveUid(stream.handle, expected_uid);
    return stream;
}

pub fn endpointName(allocator: std.mem.Allocator, endpoint_id: []const u8) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}.s", .{endpoint_id});
}

pub fn lockName(allocator: std.mem.Allocator, endpoint_id: []const u8) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}.l", .{endpoint_id});
}

pub fn statusForConnectError(failure: anyerror) u8 {
    return switch (failure) {
        error.FileNotFound,
        error.ConnectionRefused,
        error.ConnectionResetByPeer,
        => authenticated_status_endpoint_unavailable,
        error.PeerIdentityMismatch => authenticated_status_identity_mismatch,
        error.PeerIdentityUnavailable => authenticated_status_identity_unavailable,
        error.InsecureEndpoint,
        error.InsecureRuntimeDirectory,
        => authenticated_status_insecure_endpoint,
        else => authenticated_status_system_error,
    };
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
