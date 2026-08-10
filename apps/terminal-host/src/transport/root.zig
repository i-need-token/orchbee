const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("terminal-host-service").service;
const protocol = @import("terminal-protocol");
const posix_endpoint_security = @import("posix_endpoint_security.zig");
const posix_proxy = @import("posix_proxy.zig");

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};

const endpoint_id_bytes = 32;
const poll_interval_ms = 20;
const record_read_timeout_ns = 5 * std.time.ns_per_s;
const posix_send_flags = if (builtin.os.tag == .windows)
    0
else
    std.posix.MSG.DONTWAIT | std.posix.MSG.NOSIGNAL;

const Deadline = struct {
    timer: *std.time.Timer,
    timeout_ns: u64,

    fn init(timer: *std.time.Timer, timeout_ns: i128) !Deadline {
        if (timeout_ns < 0) return error.ConnectionClosed;
        return .{
            .timer = timer,
            .timeout_ns = std.math.cast(u64, timeout_ns) orelse return error.ConnectionClosed,
        };
    }

    fn expired(self: *Deadline) bool {
        return self.timer.read() >= self.timeout_ns;
    }

    fn remainingNs(self: *Deadline) u64 {
        const elapsed = self.timer.read();
        return self.timeout_ns -| elapsed;
    }
};

pub const LocalServer = switch (builtin.os.tag) {
    .windows => WindowsServer,
    .freebsd, .linux, .macos => PosixServer,
    else => @compileError("terminal-host has no local IPC implementation for this target"),
};

pub fn validEndpointId(endpoint_id: []const u8) bool {
    if (endpoint_id.len != endpoint_id_bytes) return false;
    for (endpoint_id) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return false,
    };
    return true;
}

pub fn preparePosixRuntimeDirectory(runtime_directory: []const u8) !void {
    if (builtin.os.tag == .windows) return error.UnsupportedPlatform;
    var directory = try posix_endpoint_security.prepareRuntimeDirectory(runtime_directory);
    directory.close();
}

pub fn runPosixClientProxy(
    allocator: std.mem.Allocator,
    runtime_directory: []const u8,
    endpoint_id: []const u8,
    status: std.fs.File,
    input: std.fs.File,
    output: std.fs.File,
) !void {
    if (builtin.os.tag == .windows) return error.UnsupportedPlatform;
    defer status.close();
    const stream = posix_endpoint_security.connectAuthenticated(
        allocator,
        runtime_directory,
        endpoint_id,
    ) catch |failure| {
        try status.writeAll(&.{posix_endpoint_security.statusForConnectError(failure)});
        return;
    };
    defer stream.close();
    try status.writeAll(&.{posix_endpoint_security.authenticated_status_ready});
    try posix_proxy.run(stream.handle, input.handle, output.handle);
}

fn serveConnection(transport: anytype, service: *host_service.HostService) !void {
    return serveConnectionWithHandshakeTimeout(
        transport,
        service,
        record_read_timeout_ns,
    );
}

fn serveConnectionWithHandshakeTimeout(
    transport: anytype,
    service: *host_service.HostService,
    handshake_timeout_ns: i128,
) !void {
    var connection: host_service.ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    var handshake_timer = std.time.Timer.start() catch return error.ConnectionClosed;
    var handshake_deadline = Deadline.init(&handshake_timer, handshake_timeout_ns) catch return error.ConnectionClosed;

    while (true) {
        if (connection.shouldClose()) return;
        if (connection.isAwaitingHello()) {
            if (handshake_deadline.expired()) return error.ConnectionClosed;
            if (!try transport.waitReadable()) {
                service.maintain();
                continue;
            }
            const record = try readRecordBeforeMaintaining(
                service.allocator,
                transport,
                &handshake_deadline,
                service,
            );
            defer service.allocator.free(record);
            var outgoing = service.handle(&connection, record) catch |err| switch (err) {
                error.InvalidTransportRecord => return error.ConnectionClosed,
                else => return err,
            };
            defer outgoing.deinit();
            try writeOutgoing(transport, outgoing.records());
            if (connection.shouldClose()) return;
            continue;
        }

        var events = try service.pollEvents(&connection);
        defer events.deinit();
        try writeOutgoing(transport, events.records());

        if (!try transport.waitReadable()) continue;
        const record = try readRecordMaintaining(service.allocator, transport, service);
        defer service.allocator.free(record);
        var outgoing = service.handle(&connection, record) catch |err| switch (err) {
            error.InvalidTransportRecord => return error.ConnectionClosed,
            else => return err,
        };
        defer outgoing.deinit();
        try writeOutgoing(transport, outgoing.records());
        if (connection.shouldClose()) return;
    }
}

fn serveAcceptedConnection(transport: anytype, service: *host_service.HostService) void {
    serveConnection(transport, service) catch |err| switch (err) {
        error.ConnectionClosed => {},
        else => std.log.warn("closing terminal Host connection after dispatch failure: {s}", .{@errorName(err)}),
    };
}

fn serveUntilIdleLoop(
    server: anytype,
    service: anytype,
    comptime recoverable_failure: fn (anyerror) bool,
) !void {
    var completed_accept = false;
    while (true) {
        if (!completed_accept) {
            server.serveOne(service) catch |err| {
                if (!recoverable_failure(err)) return err;
            };
            completed_accept = true;
            continue;
        }
        service.maintain();
        if (service.canStopWhenIdle()) return;
        const served = server.serveOneBefore(service, poll_interval_ms) catch |err| {
            if (!recoverable_failure(err)) return err;
            continue;
        };
        _ = served;
    }
}

fn writeOutgoing(transport: anytype, records: []const []const u8) !void {
    var timer = std.time.Timer.start() catch return error.ConnectionClosed;
    var deadline = try Deadline.init(&timer, record_read_timeout_ns);
    for (records) |record| {
        if (@hasDecl(@TypeOf(transport.*), "writeAllBefore")) {
            try transport.writeAllBefore(record, &deadline);
        } else {
            try transport.writeAll(record);
        }
    }
}

fn readRecord(allocator: std.mem.Allocator, transport: anytype) ![]u8 {
    return readRecordWithTimeout(allocator, transport, record_read_timeout_ns);
}

fn readRecordMaintaining(
    allocator: std.mem.Allocator,
    transport: anytype,
    service: *host_service.HostService,
) ![]u8 {
    var timer = std.time.Timer.start() catch return error.ConnectionClosed;
    var deadline = try Deadline.init(&timer, record_read_timeout_ns);
    return readRecordBeforeMaintaining(allocator, transport, &deadline, service);
}

fn readRecordWithTimeout(
    allocator: std.mem.Allocator,
    transport: anytype,
    timeout_ns: i128,
) ![]u8 {
    var timer = std.time.Timer.start() catch return error.ConnectionClosed;
    var deadline = try Deadline.init(&timer, timeout_ns);
    return readRecordBefore(allocator, transport, &deadline);
}

fn readRecordBefore(
    allocator: std.mem.Allocator,
    transport: anytype,
    deadline: *Deadline,
) ![]u8 {
    return readRecordBeforeWithMaintenance(allocator, transport, deadline, null);
}

fn readRecordBeforeMaintaining(
    allocator: std.mem.Allocator,
    transport: anytype,
    deadline: *Deadline,
    service: *host_service.HostService,
) ![]u8 {
    return readRecordBeforeWithMaintenance(allocator, transport, deadline, service);
}

fn readRecordBeforeWithMaintenance(
    allocator: std.mem.Allocator,
    transport: anytype,
    deadline: *Deadline,
    service: ?*host_service.HostService,
) ![]u8 {
    var header: [protocol.transport_envelope_header_bytes]u8 = undefined;
    try readExactlyBeforeWithMaintenance(transport, &header, deadline, service);
    const total_length = protocol.transportRecordLength(&header) catch return error.ConnectionClosed;
    const record = try allocator.alloc(u8, total_length);
    errdefer allocator.free(record);
    @memcpy(record[0..header.len], &header);
    try readExactlyBeforeWithMaintenance(transport, record[header.len..], deadline, service);
    return record;
}

fn readExactlyBefore(transport: anytype, bytes: []u8, deadline: *Deadline) !void {
    return readExactlyBeforeWithMaintenance(transport, bytes, deadline, null);
}

fn readExactlyBeforeWithMaintenance(
    transport: anytype,
    bytes: []u8,
    deadline: *Deadline,
    service: ?*host_service.HostService,
) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        if (service) |active_service| active_service.maintain();
        if (deadline.expired()) return error.ConnectionClosed;
        if (!try transport.waitReadable()) continue;
        const length = transport.read(bytes[offset..]) catch return error.ConnectionClosed;
        if (length == 0) return error.ConnectionClosed;
        offset += length;
    }
}

const PosixConnection = if (builtin.os.tag == .windows) void else struct {
    stream: std.net.Stream,

    fn read(self: *PosixConnection, bytes: []u8) !usize {
        return self.stream.read(bytes);
    }

    fn writeAll(self: *PosixConnection, bytes: []const u8) !void {
        var timer = std.time.Timer.start() catch return error.ConnectionClosed;
        var deadline = Deadline.init(&timer, record_read_timeout_ns) catch return error.ConnectionClosed;
        self.writeAllBefore(bytes, &deadline) catch return error.ConnectionClosed;
    }

    fn writeAllBefore(self: *PosixConnection, bytes: []const u8, deadline: *Deadline) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const remaining_ns = deadline.remainingNs();
            if (remaining_ns <= 0) return error.WriteTimeout;
            const timeout_ms: i32 = @intCast(@min(
                std.math.divCeil(u64, remaining_ns, @as(u64, std.time.ns_per_ms)) catch unreachable,
                @as(u64, poll_interval_ms),
            ));
            var descriptors = [_]std.posix.pollfd{.{
                .fd = self.stream.handle,
                .events = std.posix.POLL.OUT,
                .revents = 0,
            }};
            if (try std.posix.poll(&descriptors, timeout_ms) == 0) continue;
            const result = descriptors[0].revents;
            if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
                return error.ConnectionClosed;
            }
            const written = std.posix.send(
                self.stream.handle,
                bytes[offset..],
                posix_send_flags,
            ) catch |err| switch (err) {
                error.WouldBlock => continue,
                else => return error.ConnectionClosed,
            };
            if (written == 0) return error.ConnectionClosed;
            offset += written;
        }
    }

    fn waitReadable(self: *PosixConnection) !bool {
        var descriptors = [_]std.posix.pollfd{.{
            .fd = self.stream.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        }};
        if (try std.posix.poll(&descriptors, poll_interval_ms) == 0) return false;
        const result = descriptors[0].revents;
        if (result & std.posix.POLL.IN != 0) return true;
        if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return error.ConnectionClosed;
        }
        return false;
    }
};

const PosixServer = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.fs.Dir,
    listener: std.net.Server,
    lock_file: std.fs.File,
    lock_name: []u8,
    socket_name: []u8,
    socket_path: []u8,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !PosixServer {
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        if (!validEndpointId(endpoint_id)) return error.InvalidEndpointId;

        var directory = try posix_endpoint_security.prepareRuntimeDirectory(runtime_directory);
        errdefer directory.close();

        const lock_name = try posix_endpoint_security.lockName(allocator, endpoint_id);
        errdefer allocator.free(lock_name);
        var lock_file = try posix_endpoint_security.acquireLock(
            directory,
            lock_name,
            std.posix.geteuid(),
        );
        errdefer lock_file.close();

        const socket_name = try posix_endpoint_security.endpointName(allocator, endpoint_id);
        errdefer allocator.free(socket_name);
        const socket_path = try std.fs.path.join(
            allocator,
            &.{ runtime_directory, socket_name },
        );
        errdefer allocator.free(socket_path);
        directory.deleteFile(socket_name) catch |err| switch (err) {
            error.FileNotFound => {},
            else => return err,
        };
        errdefer directory.deleteFile(socket_name) catch {};

        var original_directory = try std.fs.cwd().openDir(".", .{});
        defer original_directory.close();
        try std.posix.fchdir(directory.fd);
        var restored_directory = false;
        defer if (!restored_directory) std.posix.fchdir(original_directory.fd) catch {};
        const address = std.net.Address.initUnix(socket_name) catch return error.EndpointPathTooLong;
        var listener = try address.listen(.{ .kernel_backlog = 8 });
        errdefer listener.deinit();
        try posix_endpoint_security.secureEndpoint(
            directory,
            socket_name,
            std.posix.geteuid(),
        );
        try std.posix.fchdir(original_directory.fd);
        restored_directory = true;

        return .{
            .allocator = allocator,
            .directory = directory,
            .listener = listener,
            .lock_file = lock_file,
            .lock_name = lock_name,
            .socket_name = socket_name,
            .socket_path = socket_path,
        };
    }

    pub fn deinit(self: *PosixServer) void {
        self.listener.deinit();
        self.directory.deleteFile(self.socket_name) catch {};
        self.lock_file.close();
        self.directory.close();
        self.allocator.free(self.socket_path);
        self.allocator.free(self.socket_name);
        self.allocator.free(self.lock_name);
        self.* = undefined;
    }

    pub fn endpointPath(self: *const PosixServer) []const u8 {
        return self.socket_path;
    }

    pub fn serveUntilIdle(self: *PosixServer, service: *host_service.HostService) !void {
        return serveUntilIdleLoop(self, service, struct {
            fn recoverable(err: anyerror) bool {
                return err == error.PeerIdentityMismatch;
            }
        }.recoverable);
    }

    pub fn serveOne(self: *PosixServer, service: *host_service.HostService) !void {
        try self.acceptAndServe(service);
    }

    pub fn serveOneBefore(
        self: *PosixServer,
        service: *host_service.HostService,
        timeout_ms: u32,
    ) !bool {
        var descriptors = [_]std.posix.pollfd{.{
            .fd = self.listener.stream.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        }};
        if (try std.posix.poll(&descriptors, @intCast(timeout_ms)) == 0) return false;
        const result = descriptors[0].revents;
        if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return error.ConnectionClosed;
        }
        if (result & std.posix.POLL.IN == 0) return false;
        try self.acceptAndServe(service);
        return true;
    }

    fn acceptAndServe(self: *PosixServer, service: *host_service.HostService) !void {
        var accepted = try self.listener.accept();
        defer accepted.stream.close();
        try posix_endpoint_security.requirePeerEffectiveUid(
            accepted.stream.handle,
            std.posix.geteuid(),
        );
        var connection: PosixConnection = .{ .stream = accepted.stream };
        serveAcceptedConnection(&connection, service);
    }
};

const WindowsServer = if (builtin.os.tag != .windows) void else struct {
    handle: *windows_c.ObWindowsLocalServer,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !WindowsServer {
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        if (!validEndpointId(endpoint_id)) return error.InvalidEndpointId;
        const directory_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, runtime_directory);
        defer allocator.free(directory_w);
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_server_create(
            directory_w.ptr,
            endpoint_w.ptr,
            &status,
        ) orelse return windowsStatusError(status);
        return .{ .handle = handle };
    }

    pub fn deinit(self: *WindowsServer) void {
        windows_c.ob_windows_local_server_destroy(self.handle);
        self.* = undefined;
    }

    pub fn serveUntilIdle(self: *WindowsServer, service: *host_service.HostService) !void {
        return serveUntilIdleLoop(self, service, struct {
            fn recoverable(err: anyerror) bool {
                return err == error.ConnectionClosed or err == error.PeerIdentityMismatch;
            }
        }.recoverable);
    }

    pub fn serveOne(self: *WindowsServer, service: *host_service.HostService) !void {
        try requireWindowsStatus(windows_c.ob_windows_local_server_accept(self.handle));
        defer windows_c.ob_windows_local_server_disconnect(self.handle);
        serveAcceptedConnection(self, service);
    }

    pub fn serveOneBefore(
        self: *WindowsServer,
        service: *host_service.HostService,
        timeout_ms: u32,
    ) !bool {
        const status = windows_c.ob_windows_local_server_accept_before(
            self.handle,
            timeout_ms,
        );
        if (status == windows_c.OB_WINDOWS_IPC_ACCEPT_TIMEOUT) return false;
        try requireWindowsStatus(status);
        defer windows_c.ob_windows_local_server_disconnect(self.handle);
        serveAcceptedConnection(self, service);
        return true;
    }

    fn read(self: *WindowsServer, bytes: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_read(
            self.handle,
            bytes.ptr,
            try checkedWindowsBufferLength(bytes.len),
            &read_bytes,
        ));
        return checkedWindowsTransfer(read_bytes, bytes.len);
    }

    fn writeAll(self: *WindowsServer, bytes: []const u8) !void {
        var timer = std.time.Timer.start() catch return error.ConnectionClosed;
        var deadline = Deadline.init(&timer, record_read_timeout_ns) catch return error.ConnectionClosed;
        self.writeAllBefore(bytes, &deadline) catch return error.ConnectionClosed;
    }

    fn writeAllBefore(self: *WindowsServer, bytes: []const u8, deadline: *Deadline) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const remaining_ns = deadline.remainingNs();
            if (remaining_ns <= 0) return error.WriteTimeout;
            const timeout_ms: u32 = @intCast(@min(
                std.math.divCeil(u64, remaining_ns, @as(u64, std.time.ns_per_ms)) catch unreachable,
                std.math.maxInt(u32),
            ));
            const remaining = bytes.len - offset;
            var written_bytes: u32 = 0;
            try requireWindowsStatus(windows_c.ob_windows_local_server_write_before(
                self.handle,
                bytes[offset..].ptr,
                try checkedWindowsBufferLength(remaining),
                timeout_ms,
                &written_bytes,
            ));
            offset += try checkedWindowsTransfer(written_bytes, remaining);
        }
    }

    fn waitReadable(self: *WindowsServer) !bool {
        var available: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_peek(
            self.handle,
            &available,
        ));
        if (available > 0) return true;
        std.Thread.sleep(poll_interval_ms * std.time.ns_per_ms);
        return false;
    }
};

fn windowsStatusError(status: c_int) anyerror {
    if (builtin.os.tag != .windows) unreachable;
    return switch (status) {
        windows_c.OB_WINDOWS_IPC_ALREADY_RUNNING => error.AlreadyRunning,
        windows_c.OB_WINDOWS_IPC_INVALID_RUNTIME_DIRECTORY => error.InsecureRuntimeDirectory,
        windows_c.OB_WINDOWS_IPC_PEER_IDENTITY_MISMATCH => error.PeerIdentityMismatch,
        windows_c.OB_WINDOWS_IPC_CONNECTION_CLOSED => error.ConnectionClosed,
        windows_c.OB_WINDOWS_IPC_WRITE_TIMEOUT => error.WriteTimeout,
        else => error.WindowsIpcFailure,
    };
}

fn requireWindowsStatus(status: c_int) !void {
    if (builtin.os.tag != .windows) unreachable;
    if (status != windows_c.OB_WINDOWS_IPC_OK) return windowsStatusError(status);
}

fn checkedWindowsBufferLength(length: usize) !u32 {
    return std.math.cast(u32, length) orelse error.IpcTransferTooLarge;
}

fn checkedWindowsTransfer(transferred: u32, capacity: usize) !usize {
    if (transferred == 0 or transferred > capacity) return error.ConnectionClosed;
    return transferred;
}

pub const testing = if (builtin.is_test) struct {
    pub const checkedWindowsBufferLengthFn = checkedWindowsBufferLength;
    pub const checkedWindowsTransferFn = checkedWindowsTransfer;
    pub const PosixServerType = PosixServer;
    pub const readRecordFn = readRecord;
    pub const readRecordWithTimeoutFn = readRecordWithTimeout;
    pub const requireWindowsStatusFn = requireWindowsStatus;
    pub const posixSendFlags = posix_send_flags;
    pub const posixEndpointSecurity = posix_endpoint_security;
    pub const posixProxy = posix_proxy;
    pub const serveAcceptedConnectionFn = serveAcceptedConnection;
    pub const serveConnectionFn = serveConnection;
    pub const serveConnectionWithHandshakeTimeoutFn = serveConnectionWithHandshakeTimeout;
    pub const serveUntilIdleLoopFn = serveUntilIdleLoop;
    pub const writeOutgoingFn = writeOutgoing;
    pub const WindowsServerType = WindowsServer;
    pub const windowsStatusErrorFn = windowsStatusError;
} else struct {};
