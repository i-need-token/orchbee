const std = @import("std");
const builtin = @import("builtin");
const platform_boundary = @import("terminal-host-platform");
const monotonic = platform_boundary.monotonic;
const posix_compat = platform_boundary.posix_compat;
const pty_process = platform_boundary.process;
const thread_sync = platform_boundary.thread_sync;
const protocol = @import("terminal-protocol");
const handshake = @import("terminal-host-handshake");

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};

const poll_interval_ms = 10;

pub const RuntimeDirectory = struct {
    allocator: std.mem.Allocator,
    path_bytes: []u8,

    pub fn init(allocator: std.mem.Allocator) !RuntimeDirectory {
        const io = thread_sync.io();
        var random: [8]u8 = undefined;
        io.random(&random);
        const encoded = std.fmt.bytesToHex(random, .lower);
        const base = try temporaryRoot(allocator);
        defer allocator.free(base);
        const path_bytes = try std.fmt.allocPrint(
            allocator,
            "{s}{c}orchbee-terminal-host-black-box-{s}",
            .{ base, std.fs.path.sep, encoded },
        );
        errdefer allocator.free(path_bytes);
        try std.Io.Dir.createDirAbsolute(io, path_bytes, @enumFromInt(0o700));
        errdefer std.Io.Dir.cwd().deleteTree(io, path_bytes) catch {};

        if (builtin.os.tag == .windows) {
            const path_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path_bytes);
            defer allocator.free(path_w);
            try requireWindowsStatus(
                windows_c.ob_windows_local_test_make_directory_private(path_w.ptr),
            );
        } else {
            var directory = try std.Io.Dir.openDirAbsolute(io, path_bytes, .{ .iterate = true });
            defer directory.close(io);
            try directory.setPermissions(io, @enumFromInt(0o700));
        }
        return .{ .allocator = allocator, .path_bytes = path_bytes };
    }

    pub fn deinit(self: *RuntimeDirectory) void {
        std.Io.Dir.cwd().deleteTree(thread_sync.io(), self.path_bytes) catch {};
        self.allocator.free(self.path_bytes);
        self.* = undefined;
    }

    pub fn path(self: *const RuntimeDirectory) []const u8 {
        return self.path_bytes;
    }
};

pub const Client = switch (builtin.os.tag) {
    .freebsd, .linux, .macos => PosixClient,
    .windows => WindowsClient,
    else => @compileError("terminal Host black-box transport is unavailable for this target"),
};

pub fn connectBefore(
    allocator: std.mem.Allocator,
    runtime_directory: []const u8,
    endpoint_id: []const u8,
    timeout_ns: u64,
) !Client {
    var timer = try monotonic.Timer.start();
    while (true) {
        if (Client.connect(allocator, runtime_directory, endpoint_id)) |client| {
            return client;
        } else |_| {
            if (timer.read() >= timeout_ns) return error.EndpointStartupTimeout;
            monotonic.sleep(poll_interval_ms * std.time.ns_per_ms);
        }
    }
}

pub fn readRecordBefore(
    allocator: std.mem.Allocator,
    client: *Client,
    timeout_ns: u64,
) ![]u8 {
    var timer = try monotonic.Timer.start();
    var header: [protocol.transport_envelope_header_bytes]u8 = undefined;
    try readExactlyBefore(client, &header, &timer, timeout_ns);
    const total_length = try protocol.transportRecordLength(&header);
    const record = try allocator.alloc(u8, total_length);
    errdefer allocator.free(record);
    @memcpy(record[0..header.len], &header);
    try readExactlyBefore(client, record[header.len..], &timer, timeout_ns);
    return record;
}

/// Connects and sends `offer_frame` until the Host answers it. A Host still ending the previous
/// Worker session answers `capacity` or closes before replying; both retry until the deadline.
pub fn connectWorkerBefore(
    allocator: std.mem.Allocator,
    runtime_directory: []const u8,
    endpoint_id: []const u8,
    offer_frame: []const u8,
    timeout_ns: u64,
) !Client {
    var timer = try monotonic.Timer.start();
    while (true) {
        const remaining_ns = timeout_ns -| timer.read();
        if (remaining_ns == 0) return error.EndpointStartupTimeout;
        var client = try connectBefore(allocator, runtime_directory, endpoint_id, remaining_ns);
        if (offerAnswered(allocator, &client, offer_frame, remaining_ns)) |answered| {
            if (answered) return client;
        } else |err| switch (err) {
            error.ConnectionClosed => {},
            else => {
                client.close();
                return err;
            },
        }
        client.close();
        monotonic.sleep(poll_interval_ms * std.time.ns_per_ms);
    }
}

/// Whether the Host answered the offer; `false` when it is still busy with a Worker session.
fn offerAnswered(
    allocator: std.mem.Allocator,
    client: *Client,
    offer_frame: []const u8,
    timeout_ns: u64,
) !bool {
    try client.writeAll(offer_frame);
    const payload = try readHandshakePayloadBefore(allocator, client, timeout_ns);
    defer allocator.free(payload);
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    return switch (try handshake.decode.acceptorPayload(arena.allocator(), payload)) {
        .answer => true,
        .rejection => |rejection| if (rejection.reason == .capacity) false else error.UnexpectedHandshakeRejection,
        else => error.UnexpectedHandshakeReply,
    };
}

/// Reads one `OBHS` frame's payload, the Host's reply to the connection's first frame.
fn readHandshakePayloadBefore(
    allocator: std.mem.Allocator,
    client: *Client,
    timeout_ns: u64,
) ![]u8 {
    var timer = try monotonic.Timer.start();
    var header: [handshake.frame.header_bytes]u8 = undefined;
    try readExactlyBefore(client, &header, &timer, timeout_ns);
    if (!std.mem.eql(u8, header[0..handshake.frame.magic.len], handshake.frame.magic)) {
        return error.UnexpectedHandshakeFrame;
    }
    const length = handshake.frame.payloadLength(header[handshake.frame.magic.len..].*) orelse
        return error.UnexpectedHandshakeFrame;
    const payload = try allocator.alloc(u8, length);
    errdefer allocator.free(payload);
    try readExactlyBefore(client, payload, &timer, timeout_ns);
    return payload;
}

fn temporaryRoot(allocator: std.mem.Allocator) ![]u8 {
    if (builtin.os.tag != .windows) return allocator.dupe(u8, "/tmp");
    return pty_process.environmentVariableOwned(allocator, "TEMP") catch
        pty_process.environmentVariableOwned(allocator, "TMP") catch
        error.MissingTemporaryDirectory;
}

fn readExactlyBefore(
    client: *Client,
    bytes: []u8,
    timer: *monotonic.Timer,
    timeout_ns: u64,
) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        if (timer.read() >= timeout_ns) return error.RecordReadTimeout;
        if (!try client.waitReadable()) {
            monotonic.sleep(poll_interval_ms * std.time.ns_per_ms);
            continue;
        }
        const length = try client.read(bytes[offset..]);
        if (length == 0) return error.ConnectionClosed;
        offset += length;
    }
}

const PosixClient = if (builtin.os.tag == .windows) void else struct {
    io: std.Io,
    stream: std.Io.net.Stream,

    fn connect(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !PosixClient {
        const path = try std.fmt.allocPrint(
            allocator,
            "{s}{c}{s}.s",
            .{ runtime_directory, std.fs.path.sep, endpoint_id },
        );
        defer allocator.free(path);
        const io = thread_sync.io();
        const address = try std.Io.net.UnixAddress.init(path);
        return .{ .io = io, .stream = try address.connect(io) };
    }

    pub fn close(self: *PosixClient) void {
        self.stream.close(self.io);
    }

    fn read(self: *PosixClient, bytes: []u8) !usize {
        return std.posix.read(self.stream.socket.handle, bytes);
    }

    fn waitReadable(self: *PosixClient) !bool {
        var descriptors = [_]std.posix.pollfd{.{
            .fd = self.stream.socket.handle,
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

    pub fn writeAll(self: *PosixClient, bytes: []const u8) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const written = try posix_compat.send(
                self.stream.socket.handle,
                bytes[offset..],
                std.posix.MSG.NOSIGNAL,
            );
            if (written == 0) return error.ConnectionClosed;
            offset += written;
        }
    }
};

const WindowsClient = if (builtin.os.tag != .windows) void else struct {
    handle: *anyopaque,

    fn connect(
        allocator: std.mem.Allocator,
        _: []const u8,
        endpoint_id: []const u8,
    ) !WindowsClient {
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_test_client_connect(
            endpoint_w.ptr,
            &status,
        ) orelse return windowsStatusError(status);
        return .{ .handle = handle };
    }

    pub fn close(self: *WindowsClient) void {
        windows_c.ob_windows_local_test_client_close(self.handle);
    }

    fn read(self: *WindowsClient, bytes: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_test_client_read(
            self.handle,
            bytes.ptr,
            std.math.cast(u32, bytes.len) orelse return error.IpcTransferTooLarge,
            &read_bytes,
        ));
        if (read_bytes == 0 or read_bytes > bytes.len) return error.ConnectionClosed;
        return read_bytes;
    }

    fn waitReadable(self: *WindowsClient) !bool {
        var available: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_test_client_peek(
            self.handle,
            &available,
        ));
        return available > 0;
    }

    pub fn writeAll(self: *WindowsClient, bytes: []const u8) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const remaining = bytes.len - offset;
            var written_bytes: u32 = 0;
            try requireWindowsStatus(windows_c.ob_windows_local_test_client_write(
                self.handle,
                bytes[offset..].ptr,
                std.math.cast(u32, remaining) orelse return error.IpcTransferTooLarge,
                &written_bytes,
            ));
            if (written_bytes == 0 or written_bytes > remaining) return error.ConnectionClosed;
            offset += written_bytes;
        }
    }
};

fn requireWindowsStatus(status: c_int) !void {
    if (builtin.os.tag != .windows) unreachable;
    if (status != windows_c.OB_WINDOWS_IPC_OK) return windowsStatusError(status);
}

fn windowsStatusError(status: c_int) anyerror {
    if (builtin.os.tag != .windows) unreachable;
    return switch (status) {
        windows_c.OB_WINDOWS_IPC_CONNECTION_CLOSED => error.ConnectionClosed,
        else => error.WindowsIpcFailure,
    };
}
