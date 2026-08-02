const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("host_service.zig");
const protocol = @import("terminal-protocol");

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};

pub const endpoint_id_bytes = 32;
pub const poll_interval_ms = 20;

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

fn serveConnection(transport: anytype, service: *host_service.HostService) !void {
    var connection: host_service.ConnectionState = .{};
    defer service.disconnect(&connection);

    while (true) {
        var events = try service.pollEvents(&connection);
        defer events.deinit();
        try writeOutgoing(transport, events.records.items);

        if (!try transport.waitReadable()) continue;
        const record = try readRecord(service.allocator, transport);
        defer service.allocator.free(record);
        var outgoing = service.handle(&connection, record) catch |err| switch (err) {
            error.InvalidTransportRecord => return error.ConnectionClosed,
            else => return err,
        };
        defer outgoing.deinit();
        try writeOutgoing(transport, outgoing.records.items);
        if (connection.close_after_response) return;
    }
}

fn writeOutgoing(transport: anytype, records: []const []u8) !void {
    for (records) |record| try transport.writeAll(record);
}

fn readRecord(allocator: std.mem.Allocator, transport: anytype) ![]u8 {
    var header: [protocol.transport_envelope_header_bytes]u8 = undefined;
    try readExactly(transport, &header);
    const total_length = protocol.transportRecordLength(&header) catch return error.ConnectionClosed;
    const record = try allocator.alloc(u8, total_length);
    errdefer allocator.free(record);
    @memcpy(record[0..header.len], &header);
    try readExactly(transport, record[header.len..]);
    return record;
}

fn readExactly(transport: anytype, bytes: []u8) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
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
        self.stream.writeAll(bytes) catch return error.ConnectionClosed;
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

        var directory = try std.fs.openDirAbsolute(runtime_directory, .{ .no_follow = true });
        errdefer directory.close();
        const directory_stat = try std.posix.fstat(directory.fd);
        if (directory_stat.uid != std.posix.geteuid() or directory_stat.mode & 0o077 != 0) {
            return error.InsecureRuntimeDirectory;
        }

        const lock_name = try std.fmt.allocPrint(
            allocator,
            "terminal-host-{s}.lock",
            .{endpoint_id},
        );
        errdefer allocator.free(lock_name);
        var lock_file = directory.createFile(lock_name, .{
            .lock = .exclusive,
            .lock_nonblocking = true,
            .mode = 0o600,
            .truncate = false,
        }) catch |err| switch (err) {
            error.WouldBlock => return error.AlreadyRunning,
            else => return err,
        };
        errdefer lock_file.close();
        try lock_file.chmod(0o600);

        const socket_name = try std.fmt.allocPrint(
            allocator,
            "terminal-host-{s}.sock",
            .{endpoint_id},
        );
        errdefer allocator.free(socket_name);
        const socket_path = try std.fs.path.join(
            allocator,
            &.{ runtime_directory, socket_name },
        );
        errdefer allocator.free(socket_path);
        const address = std.net.Address.initUnix(socket_path) catch return error.EndpointPathTooLong;

        directory.deleteFile(socket_name) catch |err| switch (err) {
            error.FileNotFound => {},
            else => return err,
        };
        errdefer directory.deleteFile(socket_name) catch {};
        var listener = try address.listen(.{ .kernel_backlog = 8 });
        errdefer listener.deinit();
        try std.posix.fchmodat(directory.fd, socket_name, 0o600, 0);
        const socket_stat = try directory.statFile(socket_name);
        if (socket_stat.kind != .unix_domain_socket or socket_stat.mode & 0o077 != 0) {
            return error.InsecureEndpoint;
        }

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

    pub fn serveForever(self: *PosixServer, service: *host_service.HostService) !void {
        while (true) {
            self.serveOne(service) catch |err| switch (err) {
                error.ConnectionClosed, error.PeerIdentityMismatch => continue,
                else => return err,
            };
        }
    }

    pub fn serveOne(self: *PosixServer, service: *host_service.HostService) !void {
        var accepted = try self.listener.accept();
        defer accepted.stream.close();
        if (!try sameUserPeer(accepted.stream.handle)) return error.PeerIdentityMismatch;
        var connection: PosixConnection = .{ .stream = accepted.stream };
        serveConnection(&connection, service) catch |err| switch (err) {
            error.ConnectionClosed => {},
            else => return err,
        };
    }

    fn sameUserPeer(socket: std.posix.socket_t) !bool {
        return switch (builtin.os.tag) {
            .linux => linuxSameUserPeer(socket),
            .freebsd, .macos => bsdSameUserPeer(socket),
            else => unreachable,
        };
    }

    fn linuxSameUserPeer(socket: std.posix.socket_t) !bool {
        const Credentials = extern struct {
            pid: c_int,
            uid: std.posix.uid_t,
            gid: std.posix.gid_t,
        };
        var credentials: Credentials = undefined;
        try std.posix.getsockopt(
            socket,
            std.posix.SOL.SOCKET,
            std.posix.SO.PEERCRED,
            std.mem.asBytes(&credentials),
        );
        return credentials.uid == std.posix.geteuid();
    }

    fn bsdSameUserPeer(socket: std.posix.socket_t) !bool {
        var user: std.posix.uid_t = undefined;
        var group: std.posix.gid_t = undefined;
        if (getpeereid(socket, &user, &group) != 0) return error.PeerIdentityUnavailable;
        return user == std.posix.geteuid();
    }

    extern "c" fn getpeereid(
        socket: c_int,
        effective_user: *std.posix.uid_t,
        effective_group: *std.posix.gid_t,
    ) c_int;
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

    pub fn serveForever(self: *WindowsServer, service: *host_service.HostService) !void {
        while (true) {
            self.serveOne(service) catch |err| switch (err) {
                error.ConnectionClosed, error.PeerIdentityMismatch => continue,
                else => return err,
            };
        }
    }

    pub fn serveOne(self: *WindowsServer, service: *host_service.HostService) !void {
        try requireWindowsStatus(windows_c.ob_windows_local_server_accept(self.handle));
        defer windows_c.ob_windows_local_server_disconnect(self.handle);
        serveConnection(self, service) catch |err| switch (err) {
            error.ConnectionClosed => {},
            else => return err,
        };
    }

    fn read(self: *WindowsServer, bytes: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_read(
            self.handle,
            bytes.ptr,
            @intCast(bytes.len),
            &read_bytes,
        ));
        return read_bytes;
    }

    fn writeAll(self: *WindowsServer, bytes: []const u8) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            var written_bytes: u32 = 0;
            try requireWindowsStatus(windows_c.ob_windows_local_server_write(
                self.handle,
                bytes[offset..].ptr,
                @intCast(bytes.len - offset),
                &written_bytes,
            ));
            offset += written_bytes;
        }
    }

    fn waitReadable(self: *WindowsServer) !bool {
        var available: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_peek(
            self.handle,
            &available,
        ));
        if (available >= protocol.transport_envelope_header_bytes) return true;
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
        else => error.WindowsIpcFailure,
    };
}

fn requireWindowsStatus(status: c_int) !void {
    if (builtin.os.tag != .windows) unreachable;
    if (status != windows_c.OB_WINDOWS_IPC_OK) return windowsStatusError(status);
}

const TestClient = switch (builtin.os.tag) {
    .windows => WindowsTestClient,
    .freebsd, .linux, .macos => PosixTestClient,
    else => void,
};

const TestRuntimeDirectory = switch (builtin.os.tag) {
    .windows => WindowsTestRuntimeDirectory,
    .freebsd, .linux, .macos => PosixTestRuntimeDirectory,
    else => void,
};

const PosixTestRuntimeDirectory = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.fs.Dir,
    path_bytes: []u8,

    fn init(allocator: std.mem.Allocator) !PosixTestRuntimeDirectory {
        var random: [8]u8 = undefined;
        std.crypto.random.bytes(&random);
        const encoded = std.fmt.bytesToHex(random, .lower);
        const path_bytes = try std.fmt.allocPrint(allocator, "/tmp/ob-{s}", .{encoded});
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

    fn deinit(self: *PosixTestRuntimeDirectory) void {
        self.directory.close();
        std.fs.deleteTreeAbsolute(self.path_bytes) catch {};
        self.allocator.free(self.path_bytes);
        self.* = undefined;
    }

    fn path(self: *const PosixTestRuntimeDirectory) []const u8 {
        return self.path_bytes;
    }

    fn chmod(self: *PosixTestRuntimeDirectory, mode: std.fs.File.Mode) !void {
        try self.directory.chmod(mode);
    }
};

const WindowsTestRuntimeDirectory = if (builtin.os.tag != .windows) void else struct {
    allocator: std.mem.Allocator,
    path_bytes: []u8,
    temporary: std.testing.TmpDir,

    fn init(allocator: std.mem.Allocator) !WindowsTestRuntimeDirectory {
        var temporary = std.testing.tmpDir(.{});
        errdefer temporary.cleanup();
        const path_bytes = try temporary.parent_dir.realpathAlloc(
            allocator,
            &temporary.sub_path,
        );
        errdefer allocator.free(path_bytes);
        const path_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path_bytes);
        defer allocator.free(path_w);
        try requireWindowsStatus(windows_c.ob_windows_local_test_make_directory_private(path_w.ptr));
        return .{
            .allocator = allocator,
            .path_bytes = path_bytes,
            .temporary = temporary,
        };
    }

    fn deinit(self: *WindowsTestRuntimeDirectory) void {
        self.allocator.free(self.path_bytes);
        self.temporary.cleanup();
        self.* = undefined;
    }

    fn path(self: *const WindowsTestRuntimeDirectory) []const u8 {
        return self.path_bytes;
    }
};

const PosixTestClient = if (builtin.os.tag == .windows) void else struct {
    stream: std.net.Stream,

    fn connect(server: *const PosixServer, _: std.mem.Allocator, _: []const u8) !PosixTestClient {
        return .{ .stream = try std.net.connectUnixSocket(server.endpointPath()) };
    }

    fn close(self: *PosixTestClient) void {
        self.stream.close();
    }

    fn read(self: *PosixTestClient, bytes: []u8) !usize {
        return self.stream.read(bytes);
    }

    fn writeAll(self: *PosixTestClient, bytes: []const u8) !void {
        try self.stream.writeAll(bytes);
    }
};

const WindowsTestClient = if (builtin.os.tag != .windows) void else struct {
    handle: *anyopaque,

    fn connect(_: *const WindowsServer, allocator: std.mem.Allocator, endpoint_id: []const u8) !WindowsTestClient {
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_test_client_connect(
            endpoint_w.ptr,
            &status,
        ) orelse return windowsStatusError(status);
        return .{ .handle = handle };
    }

    fn close(self: *WindowsTestClient) void {
        windows_c.ob_windows_local_test_client_close(self.handle);
    }

    fn read(self: *WindowsTestClient, bytes: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_test_client_read(
            self.handle,
            bytes.ptr,
            @intCast(bytes.len),
            &read_bytes,
        ));
        return read_bytes;
    }

    fn writeAll(self: *WindowsTestClient, bytes: []const u8) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            var written_bytes: u32 = 0;
            try requireWindowsStatus(windows_c.ob_windows_local_test_client_write(
                self.handle,
                bytes[offset..].ptr,
                @intCast(bytes.len - offset),
                &written_bytes,
            ));
            offset += written_bytes;
        }
    }
};

test "endpoint id is exactly lowercase hexadecimal" {
    try std.testing.expect(validEndpointId("0123456789abcdef0123456789abcdef"));
    try std.testing.expect(!validEndpointId("0123456789ABCDEF0123456789ABCDEF"));
    try std.testing.expect(!validEndpointId("short"));
}

test "private local endpoint owns one lifetime lock and serves a handshake" {
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const runtime_directory = runtime.path();
    const endpoint_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    var server = try LocalServer.init(
        std.testing.allocator,
        runtime_directory,
        endpoint_id,
    );
    defer server.deinit();
    if (builtin.os.tag != .windows) {
        try std.testing.expectEqual(std.posix.AF.UNIX, server.listener.listen_address.any.family);
    }
    try std.testing.expectError(
        error.AlreadyRunning,
        LocalServer.init(std.testing.allocator, runtime_directory, endpoint_id),
    );

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var thread_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const server_thread = try std.Thread.spawn(.{}, serveTestConnection, .{&thread_context});

    var client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    try expectHandshakeAccepted(&client);
    client.close();
    server_thread.join();
    if (thread_context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), service.core.connections.count());
}

test "malformed local record closes only that connection" {
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const endpoint_id = "cccccccccccccccccccccccccccccccc";
    var server = try LocalServer.init(
        std.testing.allocator,
        runtime.path(),
        endpoint_id,
    );
    defer server.deinit();
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();

    var malformed_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const malformed_thread = try std.Thread.spawn(.{}, serveTestConnection, .{&malformed_context});
    var malformed_client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var malformed = [_]u8{0} ** protocol.transport_envelope_header_bytes;
    @memcpy(malformed[0..4], "OBTP");
    std.mem.writeInt(u16, malformed[4..6], protocol.transport_envelope_header_bytes, .little);
    std.mem.writeInt(u32, malformed[8..12], std.math.maxInt(u32), .little);
    try malformed_client.writeAll(&malformed);
    malformed_client.close();
    malformed_thread.join();
    if (malformed_context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), service.core.terminalCount());
    try std.testing.expectEqual(@as(usize, 0), service.core.connections.count());

    var valid_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const valid_thread = try std.Thread.spawn(.{}, serveTestConnection, .{&valid_context});
    var valid_client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    try expectHandshakeAccepted(&valid_client);
    valid_client.close();
    valid_thread.join();
    if (valid_context.failure) |failure| return failure;
}

fn expectHandshakeAccepted(client: *TestClient) !void {
    const hello_payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":1024,"protocol":{"major":1,"maximumMinor":0,"minimumMinor":0},"rowWindowBytes":1024,"supportedOptionalFeatures":[]}
    ;
    const hello = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        1,
        .control_json,
        hello_payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(hello);
    try client.writeAll(hello);
    const response_record = try readRecord(std.testing.allocator, client);
    defer std.testing.allocator.free(response_record);
    const response = try protocol.validateTransportEnvelope(response_record);
    try std.testing.expectEqual(@as(u16, 2), response.kind);
}

const ServeThreadContext = struct {
    server: *LocalServer,
    service: *host_service.HostService,
    failure: ?anyerror = null,
};

fn serveTestConnection(context: *ServeThreadContext) void {
    context.server.serveOne(context.service) catch |err| {
        context.failure = err;
    };
}

test "POSIX endpoint rejects a directory visible to another account" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    try runtime.chmod(0o755);
    const runtime_directory = runtime.path();
    try std.testing.expectError(
        error.InsecureRuntimeDirectory,
        LocalServer.init(
            std.testing.allocator,
            runtime_directory,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ),
    );
}
