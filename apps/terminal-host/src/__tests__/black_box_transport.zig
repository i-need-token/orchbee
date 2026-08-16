const std = @import("std");
const builtin = @import("builtin");
const protocol = @import("terminal-protocol");

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};

const poll_interval_ms = 10;

pub const RuntimeDirectory = struct {
    allocator: std.mem.Allocator,
    path_bytes: []u8,

    pub fn init(allocator: std.mem.Allocator) !RuntimeDirectory {
        var random: [8]u8 = undefined;
        std.crypto.random.bytes(&random);
        const encoded = std.fmt.bytesToHex(random, .lower);
        const base = try temporaryRoot(allocator);
        defer allocator.free(base);
        const path_bytes = try std.fmt.allocPrint(
            allocator,
            "{s}{c}orchbee-terminal-host-black-box-{s}",
            .{ base, std.fs.path.sep, encoded },
        );
        errdefer allocator.free(path_bytes);
        try std.fs.makeDirAbsolute(path_bytes);
        errdefer std.fs.deleteTreeAbsolute(path_bytes) catch {};

        if (builtin.os.tag == .windows) {
            const path_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path_bytes);
            defer allocator.free(path_w);
            try requireWindowsStatus(
                windows_c.ob_windows_local_test_make_directory_private(path_w.ptr),
            );
        } else {
            var directory = try std.fs.openDirAbsolute(path_bytes, .{});
            defer directory.close();
            try directory.chmod(0o700);
        }
        return .{ .allocator = allocator, .path_bytes = path_bytes };
    }

    pub fn deinit(self: *RuntimeDirectory) void {
        std.fs.deleteTreeAbsolute(self.path_bytes) catch {};
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
    var timer = try std.time.Timer.start();
    while (true) {
        if (Client.connect(allocator, runtime_directory, endpoint_id)) |client| {
            return client;
        } else |_| {
            if (timer.read() >= timeout_ns) return error.EndpointStartupTimeout;
            std.Thread.sleep(poll_interval_ms * std.time.ns_per_ms);
        }
    }
}

pub fn readRecordBefore(
    allocator: std.mem.Allocator,
    client: *Client,
    timeout_ns: u64,
) ![]u8 {
    var timer = try std.time.Timer.start();
    var header: [protocol.transport_envelope_header_bytes]u8 = undefined;
    try readExactlyBefore(client, &header, &timer, timeout_ns);
    const total_length = try protocol.transportRecordLength(&header);
    const record = try allocator.alloc(u8, total_length);
    errdefer allocator.free(record);
    @memcpy(record[0..header.len], &header);
    try readExactlyBefore(client, record[header.len..], &timer, timeout_ns);
    return record;
}

fn temporaryRoot(allocator: std.mem.Allocator) ![]u8 {
    if (builtin.os.tag != .windows) return allocator.dupe(u8, "/tmp");
    return std.process.getEnvVarOwned(allocator, "TEMP") catch
        std.process.getEnvVarOwned(allocator, "TMP") catch
        error.MissingTemporaryDirectory;
}

fn readExactlyBefore(
    client: *Client,
    bytes: []u8,
    timer: *std.time.Timer,
    timeout_ns: u64,
) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        if (timer.read() >= timeout_ns) return error.RecordReadTimeout;
        if (!try client.waitReadable()) {
            std.Thread.sleep(poll_interval_ms * std.time.ns_per_ms);
            continue;
        }
        const length = try client.read(bytes[offset..]);
        if (length == 0) return error.ConnectionClosed;
        offset += length;
    }
}

const PosixClient = if (builtin.os.tag == .windows) void else struct {
    stream: std.net.Stream,

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
        return .{ .stream = try std.net.connectUnixSocket(path) };
    }

    pub fn close(self: *PosixClient) void {
        self.stream.close();
    }

    fn read(self: *PosixClient, bytes: []u8) !usize {
        return self.stream.read(bytes);
    }

    fn waitReadable(self: *PosixClient) !bool {
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

    pub fn writeAll(self: *PosixClient, bytes: []const u8) !void {
        try self.stream.writeAll(bytes);
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
