const std = @import("std");

const json_wire = @import("ssh-json-wire");
const protocol = @import("ssh-environment-protocol");

pub const maximum_concurrent_inspections = 4;
pub const inspection_timeout_ns = 30 * std.time.ns_per_s;

const worker_output_maximum_bytes = protocol.project_root_maximum_path_bytes + 1;

pub const FailureReason = enum(u8) {
    invalid_path = 1,
    not_found = 2,
    not_directory = 3,
    permission_denied = 4,
    unavailable = 5,
    timed_out = 6,
    busy = 7,
};

pub const Outcome = union(enum) {
    resolved: []const u8,
    failed: FailureReason,
};

pub const InspectionRequest = struct {
    type: []const u8,
    path: []const u8,
};

pub const Worker = struct {
    attachment_generation: u64,
    channel_id: u32,
    child: std.process.Child,
    output: [worker_output_maximum_bytes + 1]u8 = undefined,
    output_length: usize = 0,
    started_at: std.Io.Timestamp,

    pub fn spawn(
        io: std.Io,
        executable_path: []const u8,
        attachment_generation: u64,
        channel_id: u32,
        path: []const u8,
    ) !Worker {
        var child = try std.process.spawn(io, .{
            .argv = &.{ executable_path, "project-root-worker" },
            .stdin = .pipe,
            .stdout = .pipe,
            .stderr = .ignore,
        });
        errdefer child.kill(io);
        const stdin = child.stdin orelse return error.WorkerPipeUnavailable;
        try stdin.writeStreamingAll(io, path);
        stdin.close(io);
        child.stdin = null;
        if (child.stdout == null) return error.WorkerPipeUnavailable;
        return .{
            .attachment_generation = attachment_generation,
            .channel_id = channel_id,
            .child = child,
            .started_at = .now(io, .awake),
        };
    }

    pub fn outputDescriptor(self: *const Worker) std.posix.fd_t {
        return self.child.stdout.?.handle;
    }

    pub fn readAvailable(self: *Worker) !bool {
        const stdout = self.child.stdout orelse return error.WorkerPipeUnavailable;
        if (self.output_length == self.output.len) return error.WorkerOutputTooLarge;
        const length = try std.posix.read(stdout.handle, self.output[self.output_length..]);
        if (length == 0) return true;
        self.output_length += length;
        if (self.output_length > worker_output_maximum_bytes) return error.WorkerOutputTooLarge;
        return false;
    }

    pub fn drainAfterHangup(self: *Worker) !void {
        while (!try self.readAvailable()) {}
    }

    pub fn finish(self: *Worker, io: std.Io) !Outcome {
        const term = try self.child.wait(io);
        if (term != .exited or term.exited != 0) return error.WorkerFailed;
        return decodeWorkerOutput(self.output[0..self.output_length]);
    }

    pub fn terminate(self: *Worker, io: std.Io) void {
        self.child.kill(io);
    }
};

pub fn decodeRequest(
    allocator: std.mem.Allocator,
    payload: []const u8,
) !std.json.Parsed(InspectionRequest) {
    if (payload.len == 0 or payload.len > protocol.project_root_maximum_message_bytes) {
        return error.InvalidRequest;
    }
    var parsed = json_wire.decode(InspectionRequest, allocator, payload) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.InvalidRequest,
    };
    errdefer parsed.deinit();
    if (!std.mem.eql(u8, parsed.value.type, "inspect-project-root") or
        !validPath(parsed.value.path))
    {
        return error.InvalidRequest;
    }
    return parsed;
}

pub fn writeResult(
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
    channel_id: u32,
    outcome: Outcome,
) !void {
    var payload: std.Io.Writer.Allocating = .init(allocator);
    defer payload.deinit();
    switch (outcome) {
        .resolved => |canonical_path| {
            if (!validPath(canonical_path)) return error.InvalidCanonicalPath;
            try std.json.Stringify.value(.{
                .kind = "resolved",
                .canonicalPath = canonical_path,
            }, .{}, &payload.writer);
        },
        .failed => |reason| try std.json.Stringify.value(.{
            .kind = "failed",
            .reason = failureReasonName(reason),
        }, .{}, &payload.writer),
    }
    if (payload.written().len > protocol.project_root_maximum_message_bytes) {
        return error.ResultTooLarge;
    }
    try protocol.writeChannelData(allocator, socket, channel_id, payload.written());
}

pub fn runWorker(io: std.Io) !void {
    var path_buffer: [protocol.project_root_maximum_path_bytes + 1]u8 = undefined;
    const path = readWorkerPath(&path_buffer) catch {
        try writeWorkerFailure(.invalid_path);
        return;
    };
    if (!validPath(path)) {
        try writeWorkerFailure(.invalid_path);
        return;
    }
    var canonical_buffer: [std.Io.Dir.max_path_bytes]u8 = undefined;
    const canonical_length = std.Io.Dir.realPathFileAbsolute(
        io,
        path,
        &canonical_buffer,
    ) catch |err| {
        try writeWorkerFailure(mapFileSystemFailure(err));
        return;
    };
    const canonical_path = canonical_buffer[0..canonical_length];
    if (!validPath(canonical_path)) {
        try writeWorkerFailure(.unavailable);
        return;
    }
    var directory = std.Io.Dir.openDirAbsolute(io, canonical_path, .{
        .follow_symlinks = true,
    }) catch |err| {
        try writeWorkerFailure(mapFileSystemFailure(err));
        return;
    };
    directory.close(io);

    var output: [worker_output_maximum_bytes]u8 = undefined;
    output[0] = 0;
    @memcpy(output[1 .. canonical_path.len + 1], canonical_path);
    try writeAll(std.Io.File.stdout().handle, output[0 .. canonical_path.len + 1]);
}

pub fn validPath(path: []const u8) bool {
    if (path.len == 0 or path.len > protocol.project_root_maximum_path_bytes or path[0] != '/') {
        return false;
    }
    const view = std.unicode.Utf8View.init(path) catch return false;
    var iterator = view.iterator();
    while (iterator.nextCodepoint()) |codepoint| {
        if (codepoint <= 0x1f or (codepoint >= 0x7f and codepoint <= 0x9f)) return false;
    }
    return true;
}

fn readWorkerPath(buffer: []u8) ![]const u8 {
    var length: usize = 0;
    while (true) {
        if (length == buffer.len) return error.PathTooLong;
        const read_length = try std.posix.read(std.Io.File.stdin().handle, buffer[length..]);
        if (read_length == 0) break;
        length += read_length;
    }
    if (length == 0 or length > protocol.project_root_maximum_path_bytes) {
        return error.InvalidPath;
    }
    return buffer[0..length];
}

fn writeWorkerFailure(reason: FailureReason) !void {
    try writeAll(std.Io.File.stdout().handle, &.{@intFromEnum(reason)});
}

fn decodeWorkerOutput(output: []const u8) !Outcome {
    if (output.len == 0) return error.InvalidWorkerOutput;
    if (output[0] == 0) {
        const canonical_path = output[1..];
        if (!validPath(canonical_path)) return error.InvalidWorkerOutput;
        return .{ .resolved = canonical_path };
    }
    if (output.len != 1) return error.InvalidWorkerOutput;
    const reason = std.enums.fromInt(FailureReason, output[0]) orelse
        return error.InvalidWorkerOutput;
    return .{ .failed = reason };
}

fn mapFileSystemFailure(err: anyerror) FailureReason {
    return switch (err) {
        error.FileNotFound => .not_found,
        error.NotDir => .not_directory,
        error.AccessDenied, error.PermissionDenied => .permission_denied,
        else => .unavailable,
    };
}

fn failureReasonName(reason: FailureReason) []const u8 {
    return switch (reason) {
        .invalid_path => "invalid-path",
        .not_found => "not-found",
        .not_directory => "not-directory",
        .permission_denied => "permission-denied",
        .unavailable => "unavailable",
        .timed_out => "timed-out",
        .busy => "busy",
    };
}

fn writeAll(descriptor: std.posix.fd_t, bytes: []const u8) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        const written = std.posix.system.write(
            descriptor,
            bytes[offset..].ptr,
            bytes.len - offset,
        );
        switch (std.posix.errno(written)) {
            .SUCCESS => offset += @intCast(written),
            .INTR => continue,
            .PIPE => return error.ConnectionClosed,
            else => |err| return std.posix.unexpectedErrno(err),
        }
    }
}
