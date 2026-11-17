//! The Keeper check and the connection it yields. `AttachKeeper`'s Keeper is opened by its process
//! id and checked by its creation time; its endpoint's server must be that process; it must answer a
//! `keeper` offer of this build and the Host's `hello`. Each step waits at most the reply deadline.
//! The Keeper process handle is opened once here, with the rights a later claim pulls through, and
//! every wait watches it.

const std = @import("std");
const build_options = @import("build-options");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const pipe_module = @import("pipe_windows.zig");

const message = keeper_protocol.message;
const monotonic = platform.monotonic;
const Pipe = pipe_module.Pipe;

const windows = std.os.windows;
const HANDLE = windows.HANDLE;
const DWORD = windows.DWORD;

const SYNCHRONIZE: DWORD = 0x0010_0000;
const PROCESS_DUP_HANDLE: DWORD = 0x0040;
const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
const WAIT_OBJECT_0: DWORD = 0;

const FileTime = extern struct { low: DWORD, high: DWORD };

extern "kernel32" fn OpenProcess(access: DWORD, inherit: windows.BOOL, process_id: DWORD) callconv(.winapi) ?HANDLE;
extern "kernel32" fn GetProcessTimes(process: HANDLE, creation: *FileTime, exit: *FileTime, kernel: *FileTime, user: *FileTime) callconv(.winapi) windows.BOOL;
extern "kernel32" fn WaitForSingleObject(handle: HANDLE, milliseconds: DWORD) callconv(.winapi) DWORD;

/// The link's wake event, and what runs on the link thread each time it fires during a wait.
pub const Watch = struct {
    wake: HANDLE,
    context: *anyopaque,
    woken: *const fn (*anyopaque) void,
};

pub const Error = pipe_module.Error || error{ Unavailable, OutOfMemory };

pub const Connection = struct {
    keeper_process: HANDLE,
    pipe: ?Pipe,

    pub fn close(self: *Connection) void {
        self.closePipe();
        windows.CloseHandle(self.keeper_process);
        self.* = undefined;
    }

    pub fn closePipe(self: *Connection) void {
        if (self.pipe) |*pipe| pipe.close();
        self.pipe = null;
    }

    /// Whether the Keeper process has ended.
    pub fn keeperEnded(self: *const Connection) bool {
        return WaitForSingleObject(self.keeper_process, 0) == WAIT_OBJECT_0;
    }

    /// Sends one request and reads its reply within the reply deadline.
    pub fn request(self: *Connection, arena: std.mem.Allocator, value: message.Request, watch: Watch) Error!message.Reply {
        const timer = monotonic.Timer.start() catch return error.Closed;
        return self.requestBefore(arena, value, watch, &timer, message.reply_deadline_ns);
    }

    fn requestBefore(
        self: *Connection,
        arena: std.mem.Allocator,
        value: message.Request,
        watch: Watch,
        timer: *const monotonic.Timer,
        deadline_ns: u64,
    ) Error!message.Reply {
        const frame = keeper_protocol.encode.initiatorFrame(arena, .{ .request = value }) catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            error.PayloadTooLarge, error.InvalidMessage => return error.Closed,
        };
        const payload = try self.exchange(arena, frame, watch, timer, deadline_ns, keeper_protocol.frame.readPayload);
        return switch (keeper_protocol.decode.acceptorPayload(arena, payload) catch return error.Closed) {
            .reply => |reply| reply,
            .event => error.Closed,
        };
    }

    fn exchange(
        self: *Connection,
        arena: std.mem.Allocator,
        frame: []const u8,
        watch: Watch,
        timer: *const monotonic.Timer,
        deadline_ns: u64,
        comptime readPayload: anytype,
    ) Error![]u8 {
        const pipe = if (self.pipe) |*open_pipe| open_pipe else return error.Closed;
        const pipe_watch = self.pipeWatch(watch);
        try pipe.writeAll(frame, timer, deadline_ns, pipe_watch);
        var source: Pipe.Source(PipeWatch) = .{
            .pipe = pipe,
            .timer = timer,
            .deadline_ns = deadline_ns,
            .watch = pipe_watch,
        };
        return readPayload(arena, &source) catch |err| switch (err) {
            error.OutOfMemory => error.OutOfMemory,
            error.Deadline => error.Deadline,
            error.KeeperEnded => error.KeeperEnded,
            else => error.Closed,
        };
    }

    const PipeWatch = struct {
        keeper: HANDLE,
        wake: HANDLE,
        context: *anyopaque,
        woken: *const fn (*anyopaque) void,
    };

    fn pipeWatch(self: *const Connection, watch: Watch) PipeWatch {
        return .{ .keeper = self.keeper_process, .wake = watch.wake, .context = watch.context, .woken = watch.woken };
    }
};

/// Checks the Keeper and connects as a Host: the offer, then `hello` with request id `hello_id`,
/// all within one reply deadline. Any failure is `Unavailable`, and leaves nothing open.
pub fn open(keeper: message.Process, endpoint_id: *const [32]u8, hello_id: u32, watch: Watch, allocator: std.mem.Allocator) Error!Connection {
    const timer = monotonic.Timer.start() catch return error.Unavailable;
    const deadline_ns = message.reply_deadline_ns;
    const process = OpenProcess(SYNCHRONIZE | PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION, .FALSE, keeper.pid) orelse
        return error.Unavailable;
    var connection: Connection = .{ .keeper_process = process, .pipe = null };
    errdefer connection.close();
    const created = creationTime(process) orelse return error.Unavailable;
    if (created != keeper.created or connection.keeperEnded()) return error.Unavailable;
    connection.pipe = Pipe.open(endpoint_id, &timer, deadline_ns) catch return error.Unavailable;
    const server = connection.pipe.?.serverProcessId() orelse return error.Unavailable;
    if (server != keeper.pid) return error.Unavailable;

    var arena_state = std.heap.ArenaAllocator.init(allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const profile = handshake.registry.keeperProfile(
        build_options.host_version,
        .{ .commit = build_options.source_commit },
        build_options.environment,
    );
    const offer = handshake.encode.initiatorFrame(arena, .{ .offer = .{
        .connection = profile.connection,
        .generations = profile.generations,
        .capabilities = profile.capabilities,
        .product_version = profile.product_version,
        .build = profile.build,
        .environment = profile.environment,
        .fingerprint = profile.fingerprint,
    } }) catch return error.Unavailable;
    const answer = connection.exchange(arena, offer, watch, &timer, deadline_ns, handshake.frame.readPayload) catch
        return error.Unavailable;
    switch (handshake.decode.acceptorPayload(arena, answer) catch return error.Unavailable) {
        .answer => {},
        else => return error.Unavailable,
    }
    const hello: message.Request = .{ .id = hello_id, .body = .{ .hello = .host } };
    const reply = connection.requestBefore(arena, hello, watch, &timer, deadline_ns) catch return error.Unavailable;
    if (reply.id != hello_id or reply.body != .hello) return error.Unavailable;
    return connection;
}

fn creationTime(process: HANDLE) ?u64 {
    var creation: FileTime = undefined;
    var exit: FileTime = undefined;
    var kernel: FileTime = undefined;
    var user: FileTime = undefined;
    if (!GetProcessTimes(process, &creation, &exit, &kernel, &user).toBool()) return null;
    return (@as(u64, creation.high) << 32) | creation.low;
}
