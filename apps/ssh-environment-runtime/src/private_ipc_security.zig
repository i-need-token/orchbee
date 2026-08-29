const std = @import("std");

const c = @cImport({
    @cInclude("private_ipc_security.h");
});

pub fn prepareRuntimeDirectory(runtime_directory: []const u8) !void {
    const path = try std.posix.toPosixPath(runtime_directory);
    try requireStatus(c.ob_ssh_runtime_prepare_directory(
        &path,
        try checkedUid(std.c.geteuid()),
    ));
}

pub fn validateRuntimeDirectory(runtime_directory: []const u8) !void {
    const path = try std.posix.toPosixPath(runtime_directory);
    try requireStatus(c.ob_ssh_runtime_validate_directory(
        &path,
        try checkedUid(std.c.geteuid()),
    ));
}

pub fn acquireLifetimeLock(
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
) !std.posix.fd_t {
    const path = try std.posix.toPosixPath(runtime_directory);
    var lock_name_buffer: [40]u8 = undefined;
    const lock_name_slice = try std.fmt.bufPrint(
        &lock_name_buffer,
        "{s}.lock",
        .{runtime_instance_id},
    );
    const lock_name = try std.posix.toPosixPath(lock_name_slice);
    var descriptor: c_int = -1;
    const status = c.ob_ssh_runtime_acquire_lock(
        &path,
        &lock_name,
        try checkedUid(std.c.geteuid()),
        &descriptor,
    );
    if (status == c.OB_SSH_RUNTIME_SECURITY_ALREADY_RUNNING) return error.AlreadyRunning;
    try requireStatus(status);
    if (descriptor < 0) return error.PrivateIpcSecurityFailure;
    return descriptor;
}

pub fn secureEndpoint(endpoint_path: []const u8) !void {
    const path = try std.posix.toPosixPath(endpoint_path);
    try requireStatus(c.ob_ssh_runtime_secure_endpoint(
        &path,
        try checkedUid(std.c.geteuid()),
    ));
}

pub fn validateEndpoint(endpoint_path: []const u8) !void {
    const path = try std.posix.toPosixPath(endpoint_path);
    const status = c.ob_ssh_runtime_validate_endpoint(
        &path,
        try checkedUid(std.c.geteuid()),
    );
    if (status == c.OB_SSH_RUNTIME_SECURITY_ENDPOINT_UNAVAILABLE) return error.FileNotFound;
    try requireStatus(status);
}

pub fn requirePeerEffectiveUid(socket: std.posix.socket_t) !void {
    const status = c.ob_ssh_runtime_require_peer_uid(
        socket,
        try checkedUid(std.c.geteuid()),
    );
    return switch (status) {
        c.OB_SSH_RUNTIME_SECURITY_OK => {},
        c.OB_SSH_RUNTIME_SECURITY_PEER_MISMATCH => error.PeerIdentityMismatch,
        c.OB_SSH_RUNTIME_SECURITY_PEER_UNAVAILABLE => error.PeerIdentityUnavailable,
        else => error.PrivateIpcSecurityFailure,
    };
}

fn checkedUid(uid: std.posix.uid_t) !u32 {
    return std.math.cast(u32, uid) orelse error.PrivateIpcSecurityFailure;
}

fn requireStatus(status: c_int) !void {
    return switch (status) {
        c.OB_SSH_RUNTIME_SECURITY_OK => {},
        c.OB_SSH_RUNTIME_SECURITY_INSECURE => error.InsecurePrivateIpc,
        else => error.PrivateIpcSecurityFailure,
    };
}
