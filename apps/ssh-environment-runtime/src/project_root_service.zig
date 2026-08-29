const std = @import("std");

const json_wire = @import("ssh-json-wire");
const protocol = @import("ssh-environment-protocol");

pub const maximum_concurrent_operations = 4;
pub const operation_timeout_ns = 30 * std.time.ns_per_s;

const listed_worker_output_tag = 8;
const worker_output_maximum_bytes = protocol.project_root_maximum_message_bytes + 1;

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
    listed: []const u8,
    failed: FailureReason,
};

pub const RequestKind = enum(u8) {
    inspect = 0,
    list = 1,
};

pub const Request = struct {
    type: []const u8,
    path: ?[]const u8,
};

const DirectoryEntry = struct {
    name: []const u8,
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
        kind: RequestKind,
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
        try stdin.writeStreamingAll(io, &.{@intFromEnum(kind)});
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
) !std.json.Parsed(Request) {
    if (payload.len == 0 or payload.len > protocol.project_root_maximum_message_bytes) {
        return error.InvalidRequest;
    }
    var parsed = json_wire.decode(Request, allocator, payload) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.InvalidRequest,
    };
    errdefer parsed.deinit();
    if (std.mem.eql(u8, parsed.value.type, "inspect-project-root")) {
        const path = parsed.value.path orelse return error.InvalidRequest;
        if (!validPath(path)) return error.InvalidRequest;
    } else if (std.mem.eql(u8, parsed.value.type, "list-project-directory")) {
        if (parsed.value.path) |path| if (!validPath(path)) return error.InvalidRequest;
    } else {
        return error.InvalidRequest;
    }
    return parsed;
}

pub fn requestKind(request: Request) RequestKind {
    return if (std.mem.eql(u8, request.type, "inspect-project-root")) .inspect else .list;
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
        .listed => |listing| {
            if (listing.len == 0 or
                listing.len > protocol.project_root_maximum_message_bytes or
                !std.unicode.utf8ValidateSlice(listing))
            {
                return error.InvalidDirectoryListing;
            }
            try payload.writer.writeAll(listing);
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

pub fn runWorker(io: std.Io, allocator: std.mem.Allocator) !void {
    var input_buffer: [protocol.project_root_maximum_path_bytes + 2]u8 = undefined;
    const request = readWorkerRequest(&input_buffer) catch {
        try writeWorkerFailure(.invalid_path);
        return;
    };
    if (!validPath(request.path)) {
        try writeWorkerFailure(.invalid_path);
        return;
    }
    var canonical_buffer: [std.Io.Dir.max_path_bytes]u8 = undefined;
    const canonical_length = std.Io.Dir.realPathFileAbsolute(
        io,
        request.path,
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
        .iterate = request.kind == .list,
    }) catch |err| {
        try writeWorkerFailure(mapFileSystemFailure(err));
        return;
    };
    defer directory.close(io);

    switch (request.kind) {
        .inspect => {
            var output: [protocol.project_root_maximum_path_bytes + 1]u8 = undefined;
            output[0] = 0;
            @memcpy(output[1 .. canonical_path.len + 1], canonical_path);
            try writeAll(std.Io.File.stdout().handle, output[0 .. canonical_path.len + 1]);
        },
        .list => writeDirectoryListing(io, allocator, directory, canonical_path) catch |err| {
            try writeWorkerFailure(mapFileSystemFailure(err));
        },
    }
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

fn writeDirectoryListing(
    io: std.Io,
    allocator: std.mem.Allocator,
    directory: std.Io.Dir,
    canonical_path: []const u8,
) !void {
    var entries: std.ArrayList(DirectoryEntry) = .empty;
    defer {
        for (entries.items) |entry| allocator.free(entry.name);
        entries.deinit(allocator);
    }
    var truncated = false;
    var iterator = directory.iterate();
    while (try iterator.next(io)) |entry| {
        if (!validDirectoryName(entry.name) or !canOpenDirectory(io, directory, entry.name)) continue;
        try retainBoundedDirectoryName(allocator, &entries, entry.name, &truncated);
    }
    std.mem.sort(DirectoryEntry, entries.items, {}, struct {
        fn lessThan(_: void, left: DirectoryEntry, right: DirectoryEntry) bool {
            return std.mem.order(u8, left.name, right.name) == .lt;
        }
    }.lessThan);

    const parent_path: ?[]const u8 = if (std.mem.eql(u8, canonical_path, "/"))
        null
    else
        std.fs.path.dirname(canonical_path) orelse "/";
    while (true) {
        var payload: std.Io.Writer.Allocating = .init(allocator);
        defer payload.deinit();
        try std.json.Stringify.value(.{
            .kind = "listed",
            .canonicalPath = canonical_path,
            .parentPath = parent_path,
            .directories = entries.items,
            .truncated = truncated,
        }, .{}, &payload.writer);
        if (payload.written().len <= protocol.project_root_maximum_message_bytes) {
            try writeAll(std.Io.File.stdout().handle, &.{listed_worker_output_tag});
            try writeAll(std.Io.File.stdout().handle, payload.written());
            return;
        }
        truncated = true;
        const removed = entries.pop() orelse return error.DirectoryListingTooLarge;
        allocator.free(removed.name);
    }
}

fn canOpenDirectory(io: std.Io, directory: std.Io.Dir, name: []const u8) bool {
    var child = directory.openDir(io, name, .{ .follow_symlinks = true }) catch return false;
    child.close(io);
    return true;
}

fn retainBoundedDirectoryName(
    allocator: std.mem.Allocator,
    entries: *std.ArrayList(DirectoryEntry),
    name: []const u8,
    truncated: *bool,
) !void {
    if (entries.items.len < protocol.project_root_maximum_directory_entries) {
        try entries.append(allocator, .{ .name = try allocator.dupe(u8, name) });
        return;
    }
    truncated.* = true;
    var greatest_index: usize = 0;
    for (entries.items[1..], 1..) |entry, index| {
        if (std.mem.order(u8, entries.items[greatest_index].name, entry.name) == .lt) {
            greatest_index = index;
        }
    }
    if (std.mem.order(u8, name, entries.items[greatest_index].name) != .lt) return;
    const replacement = try allocator.dupe(u8, name);
    allocator.free(entries.items[greatest_index].name);
    entries.items[greatest_index] = .{ .name = replacement };
}

fn validDirectoryName(name: []const u8) bool {
    if (name.len == 0 or name.len > protocol.project_root_maximum_directory_name_bytes or
        std.mem.eql(u8, name, ".") or std.mem.eql(u8, name, "..") or
        std.mem.indexOfScalar(u8, name, '/') != null)
    {
        return false;
    }
    const view = std.unicode.Utf8View.init(name) catch return false;
    var iterator = view.iterator();
    while (iterator.nextCodepoint()) |codepoint| {
        if (codepoint <= 0x1f or (codepoint >= 0x7f and codepoint <= 0x9f)) return false;
    }
    return true;
}

const WorkerRequest = struct {
    kind: RequestKind,
    path: []const u8,
};

fn readWorkerRequest(buffer: []u8) !WorkerRequest {
    var length: usize = 0;
    while (true) {
        if (length == buffer.len) return error.PathTooLong;
        const read_length = try std.posix.read(std.Io.File.stdin().handle, buffer[length..]);
        if (read_length == 0) break;
        length += read_length;
    }
    if (length <= 1 or length > protocol.project_root_maximum_path_bytes + 1) {
        return error.InvalidPath;
    }
    return .{
        .kind = std.enums.fromInt(RequestKind, buffer[0]) orelse return error.InvalidOperation,
        .path = buffer[1..length],
    };
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
    if (output[0] == listed_worker_output_tag) {
        const listing = output[1..];
        if (listing.len == 0 or
            listing.len > protocol.project_root_maximum_message_bytes or
            !std.unicode.utf8ValidateSlice(listing))
        {
            return error.InvalidWorkerOutput;
        }
        return .{ .listed = listing };
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
