//! One Keeper connection on its own thread: the `OBHS` frame, `hello`, then one request at a time.
//! Every read and write happens here, outside the table's lock, and a reply the peer does not take
//! within the reply deadline ends only this connection, so a peer that stops reading stalls no
//! other connection.

const std = @import("std");
const build_options = @import("build-options");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const local_ipc = @import("terminal-host-transport");
const handles = @import("handles_windows.zig");
const state = @import("state.zig");

const monotonic = platform.monotonic;
const message = keeper_protocol.message;

/// How long a connection has for its `OBHS` frame, for `hello` after the answer, and for the rest
/// of any frame once its first byte arrived.
pub const frame_deadline_ns = handshake.frame.candidate_deadline_ns;
/// How long a connection whose final reply is written waits for its peer to close, because a pipe
/// disconnect discards a reply the peer has not read.
const reply_linger_ns = std.time.ns_per_s;
/// The longest single readiness wait; the connection's wake event ends it sooner.
const wait_slice_ms = 1000;

pub const Context = struct {
    allocator: std.mem.Allocator,
    table: *state.Table,
    transport: local_ipc.LocalServer,
    slot: usize,
};

/// Serves one accepted connection, then releases its slot and closes it.
pub fn run(context: Context) void {
    platform.fatal_record.markThread(.@"keeper-connection");
    var connection: Connection = .{
        .allocator = context.allocator,
        .table = context.table,
        .transport = context.transport,
        .slot = context.slot,
        .wake = context.table.wakeEvent(context.slot),
    };
    connection.serve() catch {};
    context.table.lock();
    context.table.releaseConnection(context.slot);
    context.table.unlock();
    connection.transport.deinit();
}

const Connection = struct {
    allocator: std.mem.Allocator,
    table: *state.Table,
    transport: local_ipc.LocalServer,
    slot: usize,
    wake: handles.Handle,
    /// The `answer` listed `recovery`, so this connection may send its requests.
    recovery: bool = false,
    /// The `answer` listed `programs`: a Host may hold background programs, and a Worker's
    /// `status` lists them.
    programs: bool = false,

    fn serve(self: *Connection) !void {
        var arena_state = std.heap.ArenaAllocator.init(self.allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        const profile = handshake.registry.keeperProfile(
            build_options.host_version,
            .{ .commit = build_options.source_commit },
            build_options.environment,
        );
        var source = self.frameSource(.within_deadline);
        const payload = handshake.frame.readPayload(arena, &source) catch |err| switch (err) {
            error.FrameTimeout => return self.finish(try handshakeReply(arena, .{ .rejection = profile.reject(.timeout) })),
            error.ForeignProtocol, error.Malformed => return self.finish(try handshakeReply(arena, .{ .rejection = profile.reject(.malformed) })),
            else => return,
        };
        const candidate = handshake.decode.initiatorPayload(arena, payload) catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            error.Encoding, error.Message => return self.finish(try handshakeReply(arena, .{ .rejection = profile.reject(.malformed) })),
        };
        self.table.lock();
        const lifecycle = self.table.lifecycle;
        const terminals = self.table.holdingCount();
        self.table.unlock();
        const decision = try profile.decide(arena, candidate, .{
            .session_slot_free = true,
            .control_slot_free = true,
            .state = processState(lifecycle),
        });
        switch (decision) {
            .accept => |answer| {
                for (answer.capabilities) |capability| {
                    if (std.mem.eql(u8, capability, handshake.registry.keeper.recovery)) self.recovery = true;
                    if (std.mem.eql(u8, capability, handshake.registry.keeper.programs)) self.programs = true;
                }
                try self.write(try handshakeReply(arena, .{ .answer = answer }));
            },
            .reject => |rejection| return self.finish(try handshakeReply(arena, .{ .rejection = rejection })),
            .unsupported => |unsupported| return self.finish(try handshakeReply(arena, .{ .unsupported = unsupported })),
            .serve => |kind| return self.control(arena, kind, terminals),
        }
        _ = arena_state.reset(.retain_capacity);
        return self.hello(arena, &arena_state);
    }

    /// Answers a control request and closes; `exit` ends the process after its reply.
    fn control(self: *Connection, arena: std.mem.Allocator, kind: handshake.message.RequestKind, terminals: u32) !void {
        switch (kind) {
            .query => {
                self.table.lock();
                const lifecycle = self.table.lifecycle;
                self.table.unlock();
                const connections = [_]handshake.message.ConnectionEntry{.{
                    .kind = handshake.registry.keeper.name,
                    .generations = handshake.registry.keeper.generations,
                    .capabilities = handshake.registry.keeper.capabilities,
                }};
                const processes = [_]handshake.message.ProcessEntry{.{
                    .role = .keeper,
                    .product_version = build_options.host_version,
                    .build = .{ .commit = build_options.source_commit },
                    .environment = build_options.environment,
                    .pid = handles.currentProcessId(),
                    .state = processState(lifecycle),
                    .terminals = terminals,
                    .connections = &connections,
                }};
                return self.finish(try handshakeReply(arena, .{ .query_result = .{
                    .processes = &processes,
                    .restart_keeps_terminals = null,
                } }));
            },
            .drain => {
                self.table.lock();
                self.table.drain();
                const lifecycle = self.table.lifecycle;
                self.table.unlock();
                return self.finish(try handshakeReply(arena, .{ .drain_result = .{ .state = processState(lifecycle) } }));
            },
            .exit => {
                // Every duplicate is closed before the reply, so `exit-result` shows the Keeper
                // holds nothing; it terminates no Job and no process itself.
                self.table.lock();
                self.table.stop();
                self.table.unlock();
                self.finish(try handshakeReply(arena, .{ .exit_result = .{} })) catch {};
                std.process.exit(0);
            },
        }
    }

    fn hello(self: *Connection, arena: std.mem.Allocator, arena_state: *std.heap.ArenaAllocator) !void {
        var source = self.frameSource(.within_deadline);
        const payload = keeper_protocol.frame.readPayload(arena, &source) catch return;
        const request = switch (keeper_protocol.decode.initiatorPayload(arena, payload) catch return) {
            .request => |request| request,
            .unknown_request => return,
        };
        const role = switch (request.body) {
            .hello => |role| role,
            else => return,
        };
        self.table.lock();
        const admission = self.table.admit(self.slot, role);
        self.table.unlock();
        switch (admission) {
            .refused => |reason| return self.finish(try reply(arena, request.id, .{ .refused = reason })),
            .admitted => |admitted| {
                var peer = admitted;
                switch (peer) {
                    .worker => |*worker| worker.programs = self.programs,
                    .host => {},
                }
                try self.write(try reply(arena, request.id, .hello));
                var used: UsedIds = .{};
                defer used.deinit(self.allocator);
                _ = try used.insert(self.allocator, request.id);
                _ = arena_state.reset(.retain_capacity);
                return self.requests(arena, arena_state, peer, &used);
            },
        }
    }

    fn requests(
        self: *Connection,
        arena: std.mem.Allocator,
        arena_state: *std.heap.ArenaAllocator,
        peer: state.Peer,
        used: *UsedIds,
    ) !void {
        while (true) {
            defer _ = arena_state.reset(.retain_capacity);
            var source = self.frameSource(.after_first_byte);
            const payload = keeper_protocol.frame.readPayload(arena, &source) catch return;
            // A frame that does not decode, or a reply sent the wrong way, ends the connection.
            const decoded = keeper_protocol.decode.initiatorPayload(arena, payload) catch return;
            const id = switch (decoded) {
                .request => |request| request.id,
                .unknown_request => |unknown| unknown.id,
            };
            if (!try used.insert(self.allocator, id)) return;
            var effects: state.Effects = .{};
            const body: message.Reply.Body = switch (decoded) {
                .unknown_request => |unknown| .{ .unsupported = unknown.request },
                .request => |request| applied: {
                    // A request added under a capability reads as unknown where it was not negotiated.
                    const kind = std.meta.activeTag(request.body);
                    if (message.Request.needsRecovery(kind) and !self.recovery) break :applied .{ .unsupported = kind.name() };
                    if (message.Request.needsPrograms(kind) and !self.programs) break :applied .{ .unsupported = kind.name() };
                    self.table.lock();
                    defer self.table.unlock();
                    if (self.table.closing(self.slot)) return;
                    break :applied try self.table.apply(arena, peer, request.body, &effects);
                },
            };
            effects.release();
            const frame = try reply(arena, id, body);
            // Bytes that arrived before this reply are a second request while one is outstanding.
            if ((self.transport.pendingBytes() catch return) > 0) return;
            self.transport.writeAllWithin(frame, message.reply_deadline_ns) catch |err| {
                if (err == error.WriteTimeout) platform.diagnostic_log.noteReplyNotTaken();
                return;
            };
        }
    }

    fn write(self: *Connection, frame: []const u8) !void {
        try self.transport.writeAllWithin(frame, frame_deadline_ns);
    }

    /// Sends a final reply and waits for the peer to close.
    fn finish(self: *Connection, frame: []const u8) !void {
        try self.write(frame);
        var timer = monotonic.Timer.start() catch return;
        while (true) {
            const elapsed = timer.read();
            if (elapsed >= reply_linger_ns) return;
            const remaining_ms: u32 = @intCast(@max(1, (reply_linger_ns - elapsed) / std.time.ns_per_ms));
            if (self.transport.waitReadableOrWake(remaining_ms, self.wake) catch return) return;
            if (self.closingNow()) return;
        }
    }

    fn closingNow(self: *Connection) bool {
        self.table.lock();
        defer self.table.unlock();
        return self.table.closing(self.slot);
    }

    fn frameSource(self: *Connection, deadline: Source.Deadline) Source {
        return .{ .connection = self, .deadline = deadline };
    }
};

/// The frame reader's byte source: it waits for bytes until the frame's deadline and ends once the
/// connection must close.
const Source = struct {
    connection: *Connection,
    deadline: Deadline,
    timer: ?monotonic.Timer = null,

    const Deadline = enum {
        /// The whole frame is due within `frame_deadline_ns` of the first read.
        within_deadline,
        /// No deadline until the first byte, then `frame_deadline_ns` for the rest.
        after_first_byte,
    };

    pub fn read(self: *Source, bytes: []u8) error{ FrameTimeout, ConnectionClosed }!usize {
        if (self.deadline == .within_deadline and self.timer == null) self.startTimer() catch return error.ConnectionClosed;
        while (true) {
            if (self.connection.closingNow()) return error.ConnectionClosed;
            var wait_ms: u32 = wait_slice_ms;
            if (self.timer) |*timer| {
                const elapsed = timer.read();
                if (elapsed >= frame_deadline_ns) return error.FrameTimeout;
                wait_ms = @intCast(@max(1, @min(wait_slice_ms, (frame_deadline_ns - elapsed) / std.time.ns_per_ms)));
            }
            const readable = self.connection.transport.waitReadableOrWake(wait_ms, self.connection.wake) catch
                return error.ConnectionClosed;
            if (!readable) continue;
            if (self.timer == null) self.startTimer() catch return error.ConnectionClosed;
            return self.connection.transport.read(bytes) catch return error.ConnectionClosed;
        }
    }

    fn startTimer(self: *Source) !void {
        self.timer = try monotonic.Timer.start();
    }
};

/// The ids a connection has used, as disjoint ascending ranges, so a requester that counts up
/// costs one range however long it lives.
const UsedIds = struct {
    ranges: std.ArrayList([2]u32) = .empty,

    fn deinit(self: *UsedIds, allocator: std.mem.Allocator) void {
        self.ranges.deinit(allocator);
    }

    /// Records `id`; false when it was already used.
    fn insert(self: *UsedIds, allocator: std.mem.Allocator, id: u32) error{OutOfMemory}!bool {
        const ranges = self.ranges.items;
        var low: usize = 0;
        var high: usize = ranges.len;
        while (low < high) {
            const middle = low + (high - low) / 2;
            if (ranges[middle][1] < id) low = middle + 1 else high = middle;
        }
        const index = low;
        if (index < ranges.len and ranges[index][0] <= id) return false;
        const joins_left = index > 0 and ranges[index - 1][1] + 1 == id;
        const joins_right = index < ranges.len and ranges[index][0] - 1 == id;
        if (joins_left and joins_right) {
            ranges[index - 1][1] = ranges[index][1];
            _ = self.ranges.orderedRemove(index);
        } else if (joins_left) {
            ranges[index - 1][1] = id;
        } else if (joins_right) {
            ranges[index][0] = id;
        } else {
            try self.ranges.insert(allocator, index, .{ id, id });
        }
        return true;
    }
};

fn processState(lifecycle: state.Lifecycle) handshake.message.ProcessState {
    return switch (lifecycle) {
        .ready => .ready,
        .draining => .draining,
        .stopping => .stopping,
    };
}

fn handshakeReply(arena: std.mem.Allocator, value: handshake.message.AcceptorMessage) ![]u8 {
    return handshake.encode.acceptorFrame(arena, value);
}

fn reply(arena: std.mem.Allocator, id: u32, body: message.Reply.Body) ![]u8 {
    return keeper_protocol.encode.acceptorFrame(arena, .{ .reply = .{ .id = id, .body = body } });
}
