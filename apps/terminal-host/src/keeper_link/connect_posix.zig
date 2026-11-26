//! The Keeper check and the connection it yields on macOS and Linux. `AttachKeeper`'s Keeper is
//! checked by its process id and start time; the server behind its Unix socket must be that
//! process; it must answer a `keeper` offer of this build and the `hello`. Each step waits at most
//! the reply deadline. A request's descriptors leave with its frame, and the `exited` events the
//! Keeper sends between replies go to the link as they arrive.

const std = @import("std");
const build_options = @import("build-options");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

const message = keeper_protocol.message;
const monotonic = platform.monotonic;
const native = platform.keeper.native;
const posix = std.posix;

/// How long one wait lasts before it checks the deadline and the Keeper process again.
const wait_slice_ms = 100;

/// The link's wake, and what runs on the link thread each time it fires during a wait.
pub const Watch = struct {
    wake: posix.fd_t,
    context: *anyopaque,
    woken: *const fn (*anyopaque) void,
};

/// Where each `exited` the Keeper sends goes, on the link thread.
pub const Events = struct {
    context: *anyopaque,
    exited: *const fn (*anyopaque, message.pty.Exited) void,
};

/// `Unencodable`: the request does not fit the wire, so nothing was sent.
pub const Error = error{ Closed, Deadline, KeeperEnded, OutOfMemory, Unencodable };

pub const Connection = struct {
    keeper: message.Process,
    stream: ?native.Stream,
    events: ?Events = null,
    /// The Keeper's answer listed `pty`, so it takes `hold-pty`.
    pty: bool = false,
    /// The Keeper's answer listed `parent`, so it takes `spawn` and `reap` and sends `exited`.
    parent: bool = false,
    /// The Keeper's answer listed `programs`, so it takes `hold-pipes` and `spawn-program`.
    programs: bool = false,
    /// The descriptors an `inspect` reply carried, in order, until the request's maker takes them:
    /// a PTY holding's master, or each captured stream's read end.
    received: [2]?posix.fd_t = .{ null, null },

    pub fn close(self: *Connection) void {
        self.closeStream();
        closeAll(self.takeReceived());
        self.* = undefined;
    }

    /// The descriptors the last reply carried, which the caller then owns.
    pub fn takeReceived(self: *Connection) [2]?posix.fd_t {
        const descriptors = self.received;
        self.received = .{ null, null };
        return descriptors;
    }

    pub fn closeStream(self: *Connection) void {
        if (self.stream) |*stream| stream.deinit();
        self.stream = null;
    }

    /// Whether the Keeper process has ended: no process runs under its id with its start time.
    pub fn keeperEnded(self: *const Connection) bool {
        const started = native.processStart(self.keeper.pid) orelse return true;
        return started != self.keeper.created;
    }

    /// Sends one request with `descriptors` and reads its reply within the reply deadline.
    pub fn request(
        self: *Connection,
        arena: std.mem.Allocator,
        value: message.Request,
        descriptors: []const posix.fd_t,
        watch: Watch,
    ) Error!message.Reply {
        const timer = monotonic.Timer.start() catch return error.Closed;
        return self.requestBefore(arena, value, descriptors, watch, &timer);
    }

    fn requestBefore(
        self: *Connection,
        arena: std.mem.Allocator,
        value: message.Request,
        descriptors: []const posix.fd_t,
        watch: Watch,
        timer: *const monotonic.Timer,
    ) Error!message.Reply {
        const frame = keeper_protocol.encode.initiatorFrame(arena, .{ .request = value }) catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            error.PayloadTooLarge, error.InvalidMessage => return error.Unencodable,
        };
        try self.write(frame, descriptors, timer);
        while (true) {
            const payload = try self.readFrame(arena, watch, timer, keeper_protocol.frame.readPayload);
            switch (keeper_protocol.decode.acceptorPayload(arena, payload) catch return error.Closed) {
                .reply => |reply| {
                    // Only an `inspect` carries descriptors; a reply whose descriptors did not all
                    // arrive leaves none to take.
                    if (self.stream) |*stream| {
                        var arrived: [2]posix.fd_t = undefined;
                        const count = reply.body.descriptorCount();
                        if (count > 0 and count <= arrived.len and stream.takeDescriptors(arrived[0..count])) {
                            for (arrived[0..count], 0..) |descriptor, index| self.received[index] = descriptor;
                        } else stream.discardDescriptors();
                    }
                    return reply;
                },
                .exited => |exited| self.deliver(exited),
                .event => {},
            }
        }
    }

    /// Reads the events the Keeper sent while no request was outstanding, once the socket reads as
    /// readable; the rest of a frame that began is due within the reply deadline.
    pub fn readEvents(self: *Connection, arena: std.mem.Allocator, watch: Watch) Error!void {
        const stream = if (self.stream) |*value| value else return error.Closed;
        while ((stream.pendingBytes() catch return error.Closed) > 0 or !(try self.waitedQuiet())) {
            const timer = monotonic.Timer.start() catch return error.Closed;
            const payload = try self.readFrame(arena, watch, &timer, keeper_protocol.frame.readPayload);
            switch (keeper_protocol.decode.acceptorPayload(arena, payload) catch return error.Closed) {
                // A reply nobody asked for breaks the exchange.
                .reply => return error.Closed,
                .exited => |exited| self.deliver(exited),
                .event => {},
            }
            stream.discardDescriptors();
        }
    }

    /// Whether the socket holds no frame: false when it is readable, including at its close.
    fn waitedQuiet(self: *Connection) Error!bool {
        const stream = if (self.stream) |*value| value else return error.Closed;
        var ready = [_]posix.pollfd{.{ .fd = stream.socket, .events = posix.POLL.IN, .revents = 0 }};
        const count = posix.poll(&ready, 0) catch return error.Closed;
        return count == 0;
    }

    /// Waits up to `timeout_ms` for the socket or the wake; true when the socket is readable or
    /// closed. A wake runs `watch.woken`.
    pub fn waitIdle(self: *Connection, watch: Watch, timeout_ms: i32) bool {
        var ready = [_]posix.pollfd{
            .{ .fd = watch.wake, .events = posix.POLL.IN, .revents = 0 },
            .{ .fd = if (self.stream) |stream| stream.socket else -1, .events = posix.POLL.IN, .revents = 0 },
        };
        const count = posix.poll(&ready, timeout_ms) catch return false;
        if (count == 0) return false;
        if (ready[0].revents != 0) {
            native.wake_events.consume(watch.wake);
            watch.woken(watch.context);
        }
        return self.stream != null and ready[1].revents != 0;
    }

    fn deliver(self: *Connection, exited: message.pty.Exited) void {
        if (self.events) |events| events.exited(events.context, exited);
    }

    fn write(self: *Connection, frame: []const u8, descriptors: []const posix.fd_t, timer: *const monotonic.Timer) Error!void {
        const stream = if (self.stream) |*value| value else return error.Closed;
        const elapsed = timer.read();
        if (elapsed >= message.reply_deadline_ns) return error.Deadline;
        stream.writeWithDescriptors(frame, descriptors, message.reply_deadline_ns - elapsed) catch |err| switch (err) {
            error.WriteTimeout => return error.Deadline,
            error.ConnectionClosed => return if (self.keeperEnded()) error.KeeperEnded else error.Closed,
        };
    }

    fn readFrame(
        self: *Connection,
        arena: std.mem.Allocator,
        watch: Watch,
        timer: *const monotonic.Timer,
        comptime readPayload: anytype,
    ) Error![]u8 {
        var source: Source = .{ .connection = self, .watch = watch, .timer = timer };
        return readPayload(arena, &source) catch |err| switch (err) {
            error.OutOfMemory => error.OutOfMemory,
            error.Deadline => error.Deadline,
            error.KeeperEnded => error.KeeperEnded,
            else => error.Closed,
        };
    }

    const Source = struct {
        connection: *Connection,
        watch: Watch,
        timer: *const monotonic.Timer,

        pub fn read(self: *Source, bytes: []u8) error{ Deadline, KeeperEnded, Closed }!usize {
            const connection = self.connection;
            const stream = if (connection.stream) |*value| value else return error.Closed;
            while (true) {
                const elapsed = self.timer.read();
                if (elapsed >= message.reply_deadline_ns) return error.Deadline;
                const remaining_ms = (message.reply_deadline_ns - elapsed) / std.time.ns_per_ms;
                const slice: i32 = @intCast(@max(1, @min(wait_slice_ms, remaining_ms)));
                if (connection.waitIdle(self.watch, slice)) {
                    const received = stream.read(bytes) catch
                        return if (connection.keeperEnded()) error.KeeperEnded else error.Closed;
                    if (received > 0) return received;
                } else if (connection.keeperEnded()) return error.KeeperEnded;
            }
        }
    };
};

/// Checks the Keeper and connects as `role` through the socket at `socket_path`: the offer, then
/// `hello` with request id `hello_id`, all within one reply deadline. Any failure is `Unavailable`,
/// and leaves nothing open.
pub fn open(
    keeper: message.Process,
    socket_path: [*:0]const u8,
    role: message.Role,
    hello_id: u32,
    watch: Watch,
    allocator: std.mem.Allocator,
) error{Unavailable}!Connection {
    const timer = monotonic.Timer.start() catch return error.Unavailable;
    var connection: Connection = .{ .keeper = keeper, .stream = null };
    errdefer connection.close();
    if (connection.keeperEnded()) return error.Unavailable;
    const socket = native.connectLocal(socket_path) orelse return error.Unavailable;
    connection.stream = native.Stream.init(socket);
    const server = native.peerProcessId(socket) orelse return error.Unavailable;
    if (server != keeper.pid or connection.keeperEnded()) return error.Unavailable;

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
    connection.write(offer, &.{}, &timer) catch return error.Unavailable;
    const answer = connection.readFrame(arena, watch, &timer, handshake.frame.readPayload) catch
        return error.Unavailable;
    switch (handshake.decode.acceptorPayload(arena, answer) catch return error.Unavailable) {
        .answer => |accepted| for (accepted.capabilities) |capability| {
            if (std.mem.eql(u8, capability, handshake.registry.keeper.pty)) connection.pty = true;
            if (std.mem.eql(u8, capability, handshake.registry.keeper.parent)) connection.parent = true;
            if (std.mem.eql(u8, capability, handshake.registry.keeper.programs)) connection.programs = true;
        },
        else => return error.Unavailable,
    }
    const hello: message.Request = .{ .id = hello_id, .body = .{ .hello = role } };
    const reply = connection.requestBefore(arena, hello, &.{}, watch, &timer) catch return error.Unavailable;
    if (reply.id != hello_id or reply.body != .hello) return error.Unavailable;
    return connection;
}

/// Closes each descriptor a reply carried.
pub fn closeAll(descriptors: [2]?posix.fd_t) void {
    for (descriptors) |named| if (named) |descriptor| platform.posix_compat.close(descriptor);
}
