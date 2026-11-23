//! The Host's Keeper link on macOS against a real Keeper. The Host's service runs in this process,
//! as in the service tests, with real terminals on real PTYs. The Keeper (`orchbee-terminal-host
//! keep`) is a real process whose socket sits in this test's private folder, which is also the
//! Host's endpoint folder. This process is also the Keeper's Worker, admitted before it registers
//! itself as a Host.

const std = @import("std");
const harness = @import("service_harness.zig");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const keeper_link = @import("terminal-host-keeper-link");
const options = @import("keeper_link_test_options");

const protocol = harness.protocol;
const core_module = harness.core_module;
const platform = harness.platform_boundary;
const monotonic = platform.monotonic;
const darwin = platform.keeper.darwin;
const connect = keeper_link.testing.connect;
const HostService = harness.HostService;
const ConnectionState = harness.ConnectionState;
const MessageKind = harness.MessageKind;
const TerminalId = core_module.TerminalId;
const message = keeper_protocol.message;
const ms = std.time.ns_per_ms;

fn report(comptime format: []const u8, arguments: anytype) void {
    std.debug.print("keeper link: " ++ format ++ "\n", arguments);
}

fn processOf(pid: u32) !message.Process {
    return .{ .pid = pid, .created = darwin.processStart(pid) orelse return error.ProcessStartUnknown };
}

fn ignoreWake(_: *anyopaque) void {}

const Keeper = struct {
    child: std.process.Child,
    identity: message.Process,
    endpoint: [32]u8,
    running: bool = true,

    fn start(io: std.Io, directory: []const u8) !Keeper {
        var random: [16]u8 = undefined;
        io.random(&random);
        const endpoint = std.fmt.bytesToHex(random, .lower);
        var child = try std.process.spawn(io, .{
            .argv = &.{ options.keeper_executable, "keep", "--runtime-directory", directory, "--endpoint-id", &endpoint },
            .stdin = .ignore,
            .stdout = .ignore,
            .stderr = .inherit,
        });
        errdefer child.kill(io);
        return .{ .child = child, .identity = try processOf(@intCast(child.id.?)), .endpoint = endpoint };
    }

    /// Ends the Keeper this test started, if it still runs.
    fn end(self: *Keeper, io: std.Io) void {
        if (!self.running) return;
        self.running = false;
        self.child.kill(io);
    }
};

const Record = struct {
    kind: u16,
    event: bool,
    correlation: ?[16]u8,
    payload: []u8,
};

/// One test's Keeper, its Worker connection, and the Host's service with its Worker session.
const Fixture = struct {
    allocator: std.mem.Allocator,
    io: std.Io,
    arena_state: std.heap.ArenaAllocator,
    timer: monotonic.Timer,
    directory: []u8,
    socket_path: [:0]u8,
    keeper: Keeper,
    wake: std.posix.fd_t,
    worker: connect.Connection,
    next_request: u32 = 2,
    self_process: message.Process,
    service: HostService,
    connection: ConnectionState = .awaiting_hello,
    service_live: bool = true,
    records: std.ArrayList(Record) = .empty,
    next_correlation: usize = 0,
    next_creation: usize = 0,

    fn create() !*Fixture {
        const allocator = std.testing.allocator;
        const io = std.testing.io;
        const self = try allocator.create(Fixture);
        errdefer allocator.destroy(self);
        var random: [8]u8 = undefined;
        io.random(&random);
        const directory = try std.fmt.allocPrint(allocator, "/tmp/orchbee-keeper-link-{s}", .{std.fmt.bytesToHex(random, .lower)});
        errdefer allocator.free(directory);
        try std.Io.Dir.createDirAbsolute(io, directory, @enumFromInt(0o700));
        errdefer std.Io.Dir.cwd().deleteTree(io, directory) catch {};
        var keeper = try Keeper.start(io, directory);
        errdefer keeper.end(io);
        const socket_path = try std.fmt.allocPrintSentinel(allocator, "{s}/{s}-host.sock", .{ directory, &keeper.endpoint }, 0);
        errdefer allocator.free(socket_path);
        const wake = darwin.wake_events.create() orelse return error.WakeFailed;
        errdefer platform.posix_compat.close(wake);
        self.* = .{
            .allocator = allocator,
            .io = io,
            .arena_state = .init(std.heap.page_allocator),
            .timer = try monotonic.Timer.start(),
            .directory = directory,
            .socket_path = socket_path,
            .keeper = keeper,
            .wake = wake,
            .worker = undefined,
            .self_process = try processOf(@intCast(std.c.getpid())),
            .service = HostService.initWithOptions(allocator, .{ .endpoint_directory = directory }),
        };
        self.worker = try self.openWorker();
        errdefer self.worker.close();
        try expectTag(try self.call(.{ .register_host = self.self_process }), .register_host);
        self.service.admitSession(true, false, false);
        try harness.testHandshake(&self.service, &self.connection, "4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b");
        return self;
    }

    /// The Worker's connection, once the Keeper listens.
    fn openWorker(self: *Fixture) !connect.Connection {
        const deadline = self.timer.read() + 5000 * ms;
        while (true) {
            if (connect.open(self.keeper.identity, self.socket_path.ptr, .worker, 1, self.watch(), self.allocator)) |value| {
                if (!value.pty or !value.parent) return error.KeeperLacksPty;
                return value;
            } else |_| {}
            if (self.timer.read() > deadline) return error.KeeperNotListening;
            monotonic.sleep(20 * ms);
        }
    }

    fn destroy(self: *Fixture) void {
        self.endService() catch |err| report("Host teardown failed: {s}", .{@errorName(err)});
        for (self.records.items) |record| self.allocator.free(record.payload);
        self.records.deinit(self.allocator);
        self.worker.close();
        self.keeper.end(self.io);
        platform.posix_compat.close(self.wake);
        std.Io.Dir.cwd().deleteTree(self.io, self.directory) catch {};
        self.allocator.free(self.socket_path);
        self.allocator.free(self.directory);
        self.arena_state.deinit();
        self.allocator.destroy(self);
    }

    fn arena(self: *Fixture) std.mem.Allocator {
        return self.arena_state.allocator();
    }

    fn watch(self: *Fixture) connect.Watch {
        return .{ .wake = self.wake, .context = self, .woken = ignoreWake };
    }

    /// One Worker request and its reply.
    fn call(self: *Fixture, body: message.Request.Body) !message.Reply.Body {
        const id = self.next_request;
        self.next_request += 1;
        const reply = try self.worker.request(self.arena(), .{ .id = id, .body = body }, &.{}, self.watch());
        if (reply.id != id) return error.ReplyMismatch;
        return reply.body;
    }

    /// The Host's `exit` path: the session ends and the service tears down every terminal.
    fn endService(self: *Fixture) !void {
        if (!self.service_live) return;
        self.service_live = false;
        self.service.disconnect(&self.connection);
        try self.service.deinitChecked();
    }

    fn collect(self: *Fixture, outgoing: *const harness.Outgoing) !void {
        for (outgoing.records()) |record| {
            const envelope = try protocol.validateTransportEnvelope(record);
            const payload = try self.allocator.dupe(u8, envelope.payload);
            errdefer self.allocator.free(payload);
            try self.records.append(self.allocator, .{
                .kind = envelope.kind,
                .event = envelope.event,
                .correlation = envelope.correlation_id,
                .payload = payload,
            });
        }
    }

    /// One pass of the dispatch thread's idle work: maintenance, which offers running terminals
    /// to the Keepers, and event delivery, which applies what the links reported.
    fn tick(self: *Fixture) !void {
        self.service.maintain();
        var outgoing = try self.service.pollEvents(&self.connection);
        defer outgoing.deinit();
        try self.collect(&outgoing);
        monotonic.sleep(10 * ms);
    }

    fn request(self: *Fixture, kind: u16, payload: []const u8) ![16]u8 {
        const correlation = harness.testIdentifier(0x4b, self.next_correlation);
        self.next_correlation += 1;
        var outgoing = try harness.testRequest(&self.service, &self.connection, kind, .control_json, payload, correlation);
        defer outgoing.deinit();
        try self.collect(&outgoing);
        return correlation;
    }

    fn awaitAnswer(self: *Fixture, correlation: [16]u8, timeout_ms: u64) !Record {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            for (self.records.items) |record| {
                const named = record.correlation orelse continue;
                if (!record.event and std.mem.eql(u8, &named, &correlation)) return record;
            }
            if (self.timer.read() > deadline) return error.RequestUnanswered;
            try self.tick();
        }
    }

    /// The first event of `kind` whose payload names `text`.
    fn awaitEvent(self: *Fixture, kind: u16, text: []const u8, timeout_ms: u64) !Record {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            for (self.records.items) |record| {
                if (record.event and record.kind == kind and std.mem.indexOf(u8, record.payload, text) != null) return record;
            }
            if (self.timer.read() > deadline) return error.EventMissing;
            try self.tick();
        }
    }

    fn attach(self: *Fixture) !void {
        var buffer: [256]u8 = undefined;
        const keeper = self.keeper.identity;
        const payload = try std.fmt.bufPrint(&buffer, "{{\"endpointId\":\"{s}\",\"keeper\":{{\"creationTime\":\"{d}\",\"pid\":{d}}}}}", .{ &self.keeper.endpoint, keeper.created, keeper.pid });
        const record = try self.awaitAnswer(try self.request(MessageKind.attach_keeper, payload), 5000);
        if (record.kind != MessageKind.attach_keeper) {
            report("AttachKeeper failed: {s}", .{record.payload});
            return error.AttachFailed;
        }
    }

    fn createTerminal(self: *Fixture, executable: []const u8, arguments: []const []const u8) !TerminalId {
        const creation = std.fmt.bytesToHex(harness.testIdentifier(0xc7, self.next_creation), .lower);
        self.next_creation += 1;
        const payload = try harness.testCreatePayload(&creation, executable, arguments);
        defer self.allocator.free(payload);
        const record = try self.awaitAnswer(try self.request(MessageKind.create_terminal, payload), 10000);
        const Identity = struct { terminalId: []const u8 };
        const parsed = std.json.parseFromSlice(Identity, self.allocator, record.payload, .{ .ignore_unknown_fields = true }) catch {
            report("CreateTerminal failed: {s}", .{record.payload});
            return error.CreationFailed;
        };
        defer parsed.deinit();
        var bytes: [16]u8 = undefined;
        _ = try std.fmt.hexToBytes(&bytes, parsed.value.terminalId);
        return TerminalId.fromBytes(bytes);
    }

    fn phase(self: *Fixture, id: TerminalId) core_module.TerminalPhase {
        return harness.serviceCore(&self.service).findTerminal(id).?.phase();
    }

    fn awaitRunning(self: *Fixture, id: TerminalId) !void {
        const deadline = self.timer.read() + 10 * std.time.ns_per_s;
        while (self.phase(id) != .running) {
            if (self.timer.read() > deadline) return error.TerminalNotRunning;
            try self.tick();
        }
    }

    /// Waits until the terminal's end has completed.
    fn awaitSettled(self: *Fixture, id: TerminalId, timeout_ms: u64) !void {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            switch (self.phase(id)) {
                .exited, .failed => return,
                else => {},
            }
            if (self.timer.read() > deadline) {
                report("terminal still {s}", .{@tagName(self.phase(id))});
                return error.EndIncomplete;
            }
            try self.tick();
        }
    }

    /// The PTY holding of terminal `id` this process owns, as the Keeper's status reports it.
    fn holding(self: *Fixture, id: TerminalId) !?message.pty.PtyHolding {
        var after: ?message.RandomId = null;
        while (true) {
            const page = switch (try self.call(.{ .status = after })) {
                .status => |value| value,
                else => return error.StatusRefused,
            };
            for (page.holdings) |entry| {
                if (!entry.owner.eql(self.self_process) or entry.facts != .pty) continue;
                const facts = entry.facts.pty;
                if (std.mem.eql(u8, &facts.facts.terminal.terminal_id, &id.bytes) and entry.state == .held) return facts;
            }
            after = page.next orelse return null;
        }
    }

    fn awaitHeld(self: *Fixture, id: TerminalId) !message.pty.PtyHolding {
        const deadline = self.timer.read() + 5000 * ms;
        while (true) {
            if (try self.holding(id)) |facts| return facts;
            if (self.timer.read() > deadline) return error.TerminalNotHeld;
            try self.tick();
        }
    }

    /// Disposes the ended terminal and waits until the Keeper no longer holds it.
    fn disposeReleased(self: *Fixture, id: TerminalId) !void {
        const payload = try harness.testTerminalTargetPayload(id);
        defer self.allocator.free(payload);
        _ = try self.awaitAnswer(try self.request(MessageKind.dispose_terminal, payload), 5000);
        const deadline = self.timer.read() + 5000 * ms;
        while (try self.holding(id) != null) {
            if (self.timer.read() > deadline) return error.HoldingNotReleased;
            try self.tick();
        }
    }

    fn end(self: *Fixture, id: TerminalId) !void {
        const payload = try harness.testTerminalTargetPayload(id);
        defer self.allocator.free(payload);
        _ = try self.request(MessageKind.terminate_terminal, payload);
    }
};

fn expectTag(body: message.Reply.Body, comptime tag: std.meta.Tag(message.Reply.Body)) !void {
    if (body != tag) {
        report("expected {s}, got {s}", .{ @tagName(tag), @tagName(body) });
        return error.UnexpectedReply;
    }
}

/// Whether the leader no longer runs: no process has its id and start time.
fn leaderGone(leader: message.Process) bool {
    return darwin.processStart(leader.pid) != leader.created;
}

test "a terminal running before the Keeper attaches is held with its master, and released at its end" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    const id = try fixture.createTerminal("/bin/sleep", &.{"300"});
    try fixture.awaitRunning(id);
    try fixture.attach();
    const held = try fixture.awaitHeld(id);
    try std.testing.expect(!held.parented);
    try fixture.end(id);
    try fixture.awaitSettled(id, 10000);
    // A POSIX terminal keeps its master until it is disposed.
    try std.testing.expect(try fixture.holding(id) != null);
    try fixture.disposeReleased(id);
}

test "the attached Keeper starts a new terminal's leader, parents it, and reports its exit status" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    const id = try fixture.createTerminal("/bin/sh", &.{ "-c", "sleep 2; exit 7" });
    const held = try fixture.awaitHeld(id);
    try std.testing.expect(held.parented);
    try fixture.awaitSettled(id, 15000);
    _ = try fixture.awaitEvent(MessageKind.terminal_exited, "\"code\":7", 1000);
    // The holding is released once the terminal is disposed, after which the Keeper reaps the leader.
    try fixture.disposeReleased(id);
    const deadline = fixture.timer.read() + 5000 * ms;
    while (!leaderGone(held.facts.leader)) {
        if (fixture.timer.read() > deadline) return error.LeaderNotReaped;
        monotonic.sleep(20 * ms);
    }
}

test "a terminal whose parent Keeper is lost keeps running, and its end still completes" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    const id = try fixture.createTerminal("/bin/sleep", &.{"300"});
    const held = try fixture.awaitHeld(id);
    try std.testing.expect(held.parented);
    fixture.keeper.end(fixture.io);
    _ = try fixture.awaitEvent(MessageKind.keeper_lost, "", 5000);
    try std.testing.expectEqual(core_module.TerminalPhase.running, fixture.phase(id));
    try fixture.end(id);
    try fixture.awaitSettled(id, 10000);
    try std.testing.expect(leaderGone(held.facts.leader));
}
