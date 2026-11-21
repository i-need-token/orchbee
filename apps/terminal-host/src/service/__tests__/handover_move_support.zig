//! The fixture of the moves between two Hosts. Host B is the built Host executable that the runner
//! stages beside this executable, with real terminals on the pinned conpty staged there, and the
//! Keeper is that executable's `keep`. Host A is either a Host service in this process, as in the
//! Keeper link witnesses, so a witness can read its screens and feed its parser, or, for the
//! black-box run, a second process of the built Host. The test is the Worker of the Keeper and of
//! both Hosts, so it reads every answer and event of both.

const std = @import("std");
const harness = @import("service_harness.zig");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const support = @import("keeper-support");
const transport = @import("black-box-transport");
const handshake = @import("terminal-host-handshake");
const build_options = @import("build-options");
const ghostty_vt = @import("ghostty-vt");

pub const protocol = harness.protocol;
pub const core_module = harness.core_module;
pub const MessageKind = harness.MessageKind;
pub const TerminalId = core_module.TerminalId;
pub const message = support.message;
const platform = harness.platform_boundary;
const monotonic = platform.monotonic;
const HostService = harness.HostService;
const ConnectionState = harness.ConnectionState;
const HANDLE = support.HANDLE;
const windows = std.os.windows;
pub const ms = std.time.ns_per_ms;

extern fn ob_windows_local_test_make_directory_private(path: [*:0]const u16) c_int;
extern "kernel32" fn GetModuleFileNameW(module: ?*anyopaque, buffer: [*]u16, size: windows.DWORD) callconv(.winapi) windows.DWORD;
extern "kernel32" fn GetProcessHandleCount(process: HANDLE, count: *windows.DWORD) callconv(.winapi) windows.BOOL;
extern "kernel32" fn OpenProcess(access: windows.DWORD, inherit: windows.BOOL, process_id: windows.DWORD) callconv(.winapi) ?HANDLE;
extern "ntdll" fn NtSuspendProcess(process: HANDLE) callconv(.winapi) windows.NTSTATUS;
extern "ntdll" fn NtResumeProcess(process: HANDLE) callconv(.winapi) windows.NTSTATUS;

const SYNCHRONIZE: windows.DWORD = 0x0010_0000;
const PROCESS_QUERY_LIMITED_INFORMATION: windows.DWORD = 0x1000;
const record_header_bytes = 32;
const max_chunk_bytes = 4 * 1024 * 1024;

var job_assigned = false;
var directories_made: u32 = 0;

pub fn report(comptime format: []const u8, arguments: anytype) void {
    std.debug.print("handover: " ++ format ++ "\n", arguments);
}

pub const Side = enum {
    a,
    b,

    pub fn other(self: Side) Side {
        return if (self == .a) .b else .a;
    }
};

/// Where Host A runs: a Host service in this process, or a process of the built Host.
pub const HostKind = enum { service, process };

/// A terminal's launch beyond its program: its size and its scrollback bound.
pub const Launch = struct {
    columns: u16 = 80,
    rows: u16 = 24,
    scrollback_bytes: ?usize = null,
};

pub const Record = struct {
    side: Side,
    kind: u16,
    event: bool,
    correlation: ?[16]u8,
    payload: []u8,
};

/// A terminal and the two processes the test watches: its program and its OpenConsole. The test
/// holds them only to wait on them, so it keeps neither running.
pub const Terminal = struct {
    id: TerminalId,
    program: HANDLE,
    console: HANDLE,
    holding: message.RandomId,
};

pub const Counts = struct {
    a: u32,
    b: u32,
    keeper: u32,

    pub fn all(self: Counts) Baseline {
        return .{ .a = self.a, .b = self.b, .keeper = self.keeper };
    }
};

/// The handle counts a witness requires back; a null count is not compared.
pub const Baseline = struct { a: ?u32 = null, b: ?u32 = null, keeper: ?u32 = null };

/// A terminal's inventory summary on one Host.
pub const Summary = struct {
    phase: []const u8,
    handover: ?[]const u8,
    holding: ?[]const u8,
};

const Keeper = struct {
    child: std.process.Child,
    handle: HANDLE,
    identity: message.Process,
    endpoint: [32]u8,
    worker: support.Session,
};

/// A Host that runs as a process of the built Host, reached as its Worker.
const HostProcess = struct {
    child: std.process.Child,
    handle: HANDLE,
    identity: message.Process,
    endpoint: [32]u8,
    client: transport.Client,
    epoch: [16]u8,
    pending: std.ArrayList(u8) = .empty,
    /// False once the connection closed, as it does when the Host ends.
    open: bool = true,

    fn close(self: *HostProcess) void {
        if (!self.open) return;
        self.open = false;
        self.client.close();
    }
};

pub const Fixture = struct {
    allocator: std.mem.Allocator,
    io: std.Io,
    arena_state: std.heap.ArenaAllocator,
    timer: monotonic.Timer,
    self_process: message.Process,
    /// The staged Host executable, which is also the Keeper.
    host_path: []u8,
    /// The system's command processor.
    comspec: []u8,
    /// The system's `PING.EXE`, beside its command processor.
    ping: []u8,
    keeper: ?Keeper = null,
    a_kind: HostKind,
    /// Host A while it runs in this process; never initialized when Host A is a process.
    service: HostService = undefined,
    connection: ConnectionState = .awaiting_hello,
    /// Whether the Host service in this process still runs; false when Host A is a process.
    a_live: bool,
    /// The Hosts that run as processes, by side.
    hosts: [2]?HostProcess = .{ null, null },
    records: std.ArrayList(Record) = .empty,
    terminals: std.ArrayList(Terminal) = .empty,
    /// A restored copy's identity, which names the processes of the terminal it was moved from.
    aliases: std.ArrayList(struct { id: TerminalId, of: TerminalId }) = .empty,
    next_correlation: usize = 0,
    next_creation: usize = 0,
    /// Whether a wait polls without sleeping, so that a timed step is not rounded up to the
    /// system timer's tick, about 16 ms on Windows.
    spin: bool = false,

    pub fn create() !*Fixture {
        return createWith(.service);
    }

    pub fn createWith(a_kind: HostKind) !*Fixture {
        const self = try allocate(a_kind);
        errdefer self.destroy();
        if (a_kind == .service) {
            self.service.admitSession(true, true);
            try harness.testHandshake(&self.service, &self.connection, "4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d");
        }
        try self.startKeeper();
        switch (a_kind) {
            .service => try self.attach(.a),
            .process => try self.startHost(.a),
        }
        try self.startHost(.b);
        try self.warmUp(&.{ .a, .b });
        const baseline = try self.counts();
        report("handle baselines after warm-up: A {d}, B {d}, Keeper {d}", .{ baseline.a, baseline.b, baseline.keeper });
        return self;
    }

    /// Runs and disposes one short terminal on each of the Hosts, so the handles a Host or the
    /// Keeper opens once, on its first terminal, are in every baseline a witness takes.
    pub fn warmUp(self: *Fixture, sides: []const Side) !void {
        var ids: [2]TerminalId = undefined;
        for (sides, 0..) |side, index| ids[index] = try self.createTerminal(side, 2);
        for (sides, 0..) |side, index| {
            _ = try self.awaitExitCode(side, ids[index], 10_000);
            try self.awaitEnded(ids[index], 10_000);
            try self.dispose(side, ids[index]);
            self.unwatch(ids[index]);
        }
    }

    fn allocate(a_kind: HostKind) !*Fixture {
        const allocator = std.testing.allocator;
        if (!job_assigned) {
            // Every process this test starts ends with it.
            try support.assignSelf(try support.killOnCloseJob());
            job_assigned = true;
        }
        const timer = try monotonic.Timer.start();
        const self_process = try support.selfProcess();
        const comspec = try harness.pty_process.environmentVariableOwned(allocator, "ComSpec");
        errdefer allocator.free(comspec);
        const ping = try std.fs.path.join(allocator, &.{ std.fs.path.dirname(comspec) orelse return error.ComSpecMissing, "PING.EXE" });
        errdefer allocator.free(ping);
        const host_path = try besideSelf(allocator, "orchbee-terminal-host.exe");
        errdefer allocator.free(host_path);
        const self = try allocator.create(Fixture);
        self.* = .{
            .allocator = allocator,
            .io = std.testing.io,
            .arena_state = .init(std.heap.page_allocator),
            .timer = timer,
            .self_process = self_process,
            .host_path = host_path,
            .comspec = comspec,
            .ping = ping,
            .a_kind = a_kind,
            .a_live = a_kind == .service,
        };
        if (a_kind == .service) self.service = HostService.init(allocator);
        return self;
    }

    pub fn destroy(self: *Fixture) void {
        report("witness ran {d} ms", .{self.timer.read() / ms});
        self.endA() catch |err| report("Host A teardown failed: {s}", .{@errorName(err)});
        self.killHost(.a);
        self.killHost(.b);
        self.endKeeper();
        for (self.terminals.items) |item| {
            _ = support.CloseHandle(item.program);
            _ = support.CloseHandle(item.console);
        }
        self.terminals.deinit(self.allocator);
        self.aliases.deinit(self.allocator);
        for (self.records.items) |record| self.allocator.free(record.payload);
        self.records.deinit(self.allocator);
        self.allocator.free(self.ping);
        self.allocator.free(self.comspec);
        self.allocator.free(self.host_path);
        self.arena_state.deinit();
        self.allocator.destroy(self);
    }

    pub fn arena(self: *Fixture) std.mem.Allocator {
        return self.arena_state.allocator();
    }

    /// The `exit` of Host A in this process: its session ends and its service tears down every
    /// terminal. A process Host ends by `exitHost` or `killHost`.
    pub fn endA(self: *Fixture) !void {
        if (!self.a_live) return;
        self.a_live = false;
        self.service.disconnect(&self.connection);
        try self.service.deinitChecked();
    }

    // ---- processes ----

    /// A private directory under this executable's work directory.
    fn privateDirectory(self: *Fixture, comptime role: []const u8) ![]u8 {
        directories_made += 1;
        const work = try besideSelf(self.allocator, "handover-work");
        defer self.allocator.free(work);
        std.Io.Dir.createDirAbsolute(self.io, work, @enumFromInt(0o700)) catch |err| switch (err) {
            error.PathAlreadyExists => {},
            else => return err,
        };
        const directory = try std.fmt.allocPrint(self.allocator, "{s}\\" ++ role ++ "-{d}", .{ work, directories_made });
        errdefer self.allocator.free(directory);
        try std.Io.Dir.createDirAbsolute(self.io, directory, @enumFromInt(0o700));
        const directory_w = try std.unicode.utf8ToUtf16LeAllocZ(self.allocator, directory);
        defer self.allocator.free(directory_w);
        if (ob_windows_local_test_make_directory_private(directory_w.ptr) != 0) return error.PrivateDirectoryFailed;
        return directory;
    }

    /// Starts a Keeper and its Worker connection, which registers both Hosts. The Hosts attach it
    /// only when the witness asks.
    pub fn startKeeper(self: *Fixture) !void {
        var random: [16]u8 = undefined;
        self.io.random(&random);
        const endpoint = std.fmt.bytesToHex(random, .lower);
        const directory = try self.privateDirectory("keeper");
        defer self.allocator.free(directory);
        var child = try std.process.spawn(self.io, .{
            .argv = &.{ self.host_path, "keep", "--runtime-directory", directory, "--endpoint-id", &endpoint },
            .stdin = .ignore,
            .stdout = .ignore,
            .stderr = .inherit,
        });
        errdefer child.kill(self.io);
        if (!support.endpointListening(&endpoint, 5000)) return error.KeeperNotListening;
        var worker: support.Session = .{ .pipe = try support.Pipe.open(&endpoint) };
        errdefer worker.close();
        const scratch = self.arena();
        try worker.offer(scratch);
        try expectTag(try keeperCall(&worker, scratch, .{ .hello = .worker }), .hello);
        if (self.a_kind == .service) try expectTag(try keeperCall(&worker, scratch, .{ .register_host = self.self_process }), .register_host);
        for (self.hosts) |slot| if (slot) |host| try expectTag(try keeperCall(&worker, scratch, .{ .register_host = host.identity }), .register_host);
        self.keeper = .{
            .child = child,
            .handle = child.id.?,
            .identity = try support.processOf(child.id.?),
            .endpoint = endpoint,
            .worker = worker,
        };
    }

    /// Ends the Keeper by its own handle if it still runs, and releases it.
    pub fn endKeeper(self: *Fixture) void {
        var keeper = self.keeper orelse return;
        self.keeper = null;
        keeper.worker.close();
        if (support.WaitForSingleObject(keeper.handle, 0) != support.WAIT_OBJECT_0) keeper.child.kill(self.io) else _ = keeper.child.wait(self.io) catch {};
    }

    /// Stops the Keeper answering: it keeps its connections and duplicates but runs no thread.
    pub fn freezeKeeper(self: *Fixture) !void {
        if (NtSuspendProcess(self.keeper.?.handle) != .SUCCESS) return error.SuspendFailed;
    }

    pub fn thawKeeper(self: *Fixture) !void {
        if (NtResumeProcess(self.keeper.?.handle) != .SUCCESS) return error.ResumeFailed;
    }

    /// Ends the Keeper, waits until each live Host reports it lost, then attaches a new one.
    pub fn replaceKeeper(self: *Fixture) !void {
        try self.loseKeeper();
        try self.startKeeper();
        if (self.live(.a)) try self.attach(.a);
        if (self.live(.b)) try self.attach(.b);
    }

    /// Ends the Keeper and waits until each live Host reports it lost.
    pub fn loseKeeper(self: *Fixture) !void {
        const lost_a = self.events(.a, MessageKind.keeper_lost);
        const lost_b = self.events(.b, MessageKind.keeper_lost);
        self.endKeeper();
        if (self.live(.a)) _ = try self.awaitEvent(.a, MessageKind.keeper_lost, lost_a + 1, 10_000);
        if (self.live(.b)) _ = try self.awaitEvent(.b, MessageKind.keeper_lost, lost_b + 1, 10_000);
    }

    /// Starts a Host process as the Worker would, admitted by the Keeper, and attaches the Keeper
    /// there. Host A runs as a process only in a fixture made for it.
    pub fn startHost(self: *Fixture, side: Side) !void {
        std.debug.assert(side == .b or self.a_kind == .process);
        std.debug.assert(self.hosts[@intFromEnum(side)] == null);
        var random: [16]u8 = undefined;
        self.io.random(&random);
        const endpoint = std.fmt.bytesToHex(random, .lower);
        const directory = try self.privateDirectory("host");
        defer self.allocator.free(directory);
        var child = try std.process.spawn(self.io, .{
            .argv = &.{ self.host_path, "serve", "--runtime-directory", directory, "--endpoint-id", &endpoint },
            .stdin = .ignore,
            .stdout = .ignore,
            .stderr = .inherit,
        });
        errdefer child.kill(self.io);
        const process = try support.processOf(child.id.?);
        try expectTag(try keeperCall(&self.keeper.?.worker, self.arena(), .{ .register_host = process }), .register_host);
        var client = try connectWorker(self.allocator, directory, &endpoint);
        errdefer client.close();
        const epoch = try hello(self.allocator, &client);
        self.hosts[@intFromEnum(side)] = .{ .child = child, .handle = child.id.?, .identity = process, .endpoint = endpoint, .client = client, .epoch = epoch };
        try self.attach(side);
    }

    /// Whether the Host still runs: the service while it serves, a process while its connection
    /// is open.
    pub fn live(self: *const Fixture, side: Side) bool {
        if (side == .a and self.a_kind == .service) return self.a_live;
        const host = self.hosts[@intFromEnum(side)] orelse return false;
        return host.open;
    }

    /// A process Host's fatal end: the process is terminated, so no teardown of its own runs.
    pub fn killHost(self: *Fixture, side: Side) void {
        const slot = &self.hosts[@intFromEnum(side)];
        if (slot.*) |*host| {
            host.close();
            if (support.WaitForSingleObject(host.handle, 0) != support.WAIT_OBJECT_0) host.child.kill(self.io) else _ = host.child.wait(self.io) catch {};
            host.pending.deinit(self.allocator);
        }
        slot.* = null;
    }

    /// A process Host's `exit` on a control connection; returns the Host's exit code.
    pub fn exitHost(self: *Fixture, side: Side) !u32 {
        const host = &self.hosts[@intFromEnum(side)].?;
        var control = try transport.connectBefore(self.allocator, "", &host.endpoint, 5 * std.time.ns_per_s);
        defer control.close();
        const frame = try handshake.encode.initiatorFrame(self.arena(), .{ .request = .{
            .kind = .exit,
            .environment = build_options.environment,
        } });
        try control.writeAll(frame);
        const deadline = self.timer.read() + 15 * std.time.ns_per_s;
        while (support.WaitForSingleObject(host.handle, 0) != support.WAIT_OBJECT_0) {
            if (self.timer.read() > deadline) return error.HostStillRunning;
            try self.tick();
        }
        const code = support.exitCode(host.handle) orelse return error.HostStillRunning;
        self.killHost(side);
        return code;
    }

    pub fn identity(self: *const Fixture, side: Side) message.Process {
        if (side == .a and self.a_kind == .service) return self.self_process;
        return self.hosts[@intFromEnum(side)].?.identity;
    }

    pub fn attach(self: *Fixture, side: Side) !void {
        const keeper = self.keeper.?;
        var buffer: [256]u8 = undefined;
        const payload = try std.fmt.bufPrint(&buffer, "{{\"endpointId\":\"{s}\",\"keeper\":{{\"creationTime\":\"{d}\",\"pid\":{d}}}}}", .{ &keeper.endpoint, keeper.identity.created, keeper.identity.pid });
        try expectKind(try self.call(side, MessageKind.attach_keeper, .control_json, payload, 10_000), MessageKind.attach_keeper);
    }

    // ---- records ----

    fn collect(self: *Fixture, side: Side, records: []const []const u8) !void {
        for (records) |record| {
            const envelope = try protocol.validateTransportEnvelope(record);
            const payload = try self.allocator.dupe(u8, envelope.payload);
            errdefer self.allocator.free(payload);
            try self.records.append(self.allocator, .{
                .side = side,
                .kind = envelope.kind,
                .event = envelope.event,
                .correlation = envelope.correlation_id,
                .payload = payload,
            });
        }
    }

    /// One pass of each Host's work: the dispatch-thread turn of Host A in this process, as its
    /// session loop takes it, and whatever each process Host has sent.
    pub fn pump(self: *Fixture) !void {
        if (self.a_live) {
            self.service.maintain();
            if (self.service.settleCreation(&self.connection)) |value| {
                var reply = value;
                defer reply.deinit();
                switch (reply) {
                    .records => |*records| try self.collect(.a, records.records()),
                    .failure => |*failure| try self.collect(.a, &.{failure.bytes()}),
                }
            }
            while (self.service.takeReleasedRequest()) |record| {
                defer self.allocator.free(record);
                var outgoing = try self.service.handleReleased(&self.connection, record);
                defer outgoing.deinit();
                try self.collect(.a, outgoing.records());
            }
            var outgoing = try self.service.pollEvents(&self.connection);
            defer outgoing.deinit();
            try self.collect(.a, outgoing.records());
        }
        for (&self.hosts, [_]Side{ .a, .b }) |*slot, side| if (slot.*) |*host| try self.readHost(side, host);
    }

    fn readHost(self: *Fixture, side: Side, host: *HostProcess) !void {
        if (!host.open) return;
        while (true) {
            const readable = host.client.waitReadable() catch return host.close();
            if (!readable) break;
            var buffer: [64 * 1024]u8 = undefined;
            const length = host.client.read(&buffer) catch return host.close();
            if (length == 0) return host.close();
            try host.pending.appendSlice(self.allocator, buffer[0..length]);
        }
        while (host.pending.items.len >= protocol.transport_envelope_header_bytes) {
            const total = try protocol.transportRecordLength(host.pending.items[0..protocol.transport_envelope_header_bytes]);
            if (host.pending.items.len < total) break;
            try self.collect(side, &.{host.pending.items[0..total]});
            const rest = host.pending.items.len - total;
            std.mem.copyForwards(u8, host.pending.items[0..rest], host.pending.items[total..]);
            host.pending.shrinkRetainingCapacity(rest);
        }
    }

    pub fn tick(self: *Fixture) !void {
        try self.pump();
        if (self.spin) std.Thread.yield() catch {} else monotonic.sleep(5 * ms);
    }

    /// Waits `duration_ms` while both Hosts go on working.
    pub fn idle(self: *Fixture, duration_ms: u64) !void {
        const until = self.timer.read() + duration_ms * ms;
        while (self.timer.read() < until) try self.tick();
    }

    pub fn request(self: *Fixture, side: Side, kind: u16, encoding: protocol.PayloadEncoding, payload: []const u8) ![16]u8 {
        const correlation = harness.testIdentifier(0x4d, self.next_correlation);
        self.next_correlation += 1;
        if (side == .a and self.a_kind == .service) {
            // As the session loop does, read nothing while the held-request limit is reached.
            const deadline = self.timer.read() + 20 * std.time.ns_per_s;
            while (!self.service.acceptsRequests()) {
                if (self.timer.read() > deadline) return error.CreationNeverSettled;
                try self.tick();
            }
            const record = try protocol.encodeTransportEnvelope(self.allocator, kind, encoding, payload, correlation, harness.serviceCore(&self.service).epoch.bytes, false, false);
            defer self.allocator.free(record);
            var outgoing = try self.service.handle(&self.connection, record);
            defer outgoing.deinit();
            try self.collect(.a, outgoing.records());
        } else {
            const host = &self.hosts[@intFromEnum(side)].?;
            const record = try protocol.encodeTransportEnvelope(self.allocator, kind, encoding, payload, correlation, host.epoch, false, false);
            defer self.allocator.free(record);
            try host.client.writeAll(record);
        }
        return correlation;
    }

    pub fn answer(self: *const Fixture, side: Side, correlation: [16]u8) ?Record {
        for (self.records.items) |record| {
            if (record.side != side or record.event) continue;
            const named = record.correlation orelse continue;
            if (std.mem.eql(u8, &named, &correlation)) return record;
        }
        return null;
    }

    pub fn awaitAnswer(self: *Fixture, side: Side, correlation: [16]u8, timeout_ms: u64) !Record {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            if (self.answer(side, correlation)) |record| return record;
            if (self.timer.read() > deadline) return error.RequestUnanswered;
            try self.tick();
        }
    }

    pub fn call(self: *Fixture, side: Side, kind: u16, encoding: protocol.PayloadEncoding, payload: []const u8, timeout_ms: u64) !Record {
        return self.awaitAnswer(side, try self.request(side, kind, encoding, payload), timeout_ms);
    }

    pub fn events(self: *const Fixture, side: Side, kind: u16) usize {
        var count: usize = 0;
        for (self.records.items) |record| count += @intFromBool(record.side == side and record.event and record.kind == kind);
        return count;
    }

    pub fn awaitEvent(self: *Fixture, side: Side, kind: u16, count: usize, timeout_ms: u64) !Record {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            var seen: usize = 0;
            for (self.records.items) |record| if (record.side == side and record.event and record.kind == kind) {
                seen += 1;
                if (seen == count) return record;
            };
            if (self.timer.read() > deadline) return error.EventMissing;
            try self.tick();
        }
    }

    /// The events of `kind` from `side` that name the terminal.
    pub fn eventsNaming(self: *const Fixture, side: Side, kind: u16, id: TerminalId) usize {
        const named = std.fmt.bytesToHex(id.bytes, .lower);
        var count: usize = 0;
        for (self.records.items) |record| {
            if (record.side == side and record.event and record.kind == kind and std.mem.indexOf(u8, record.payload, &named) != null) count += 1;
        }
        return count;
    }

    /// Waits for the terminal's `TerminalExited` on `side` and returns its exit code.
    pub fn awaitExitCode(self: *Fixture, side: Side, id: TerminalId, timeout_ms: u64) !?u32 {
        const named = std.fmt.bytesToHex(id.bytes, .lower);
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            for (self.records.items) |record| {
                if (record.side != side or !record.event or record.kind != MessageKind.terminal_exited) continue;
                if (std.mem.indexOf(u8, record.payload, &named) == null) continue;
                const Exited = struct { exit: struct { kind: []const u8, code: ?u32 = null } };
                const parsed = try std.json.parseFromSlice(Exited, self.allocator, record.payload, .{ .ignore_unknown_fields = true });
                defer parsed.deinit();
                return parsed.value.exit.code;
            }
            if (self.timer.read() > deadline) return error.TerminalNotExited;
            try self.tick();
        }
    }

    // ---- terminals ----

    /// A terminal whose program, `ping`, sends `pings` echo requests a second apart and then exits,
    /// held by the Keeper once it runs. Each reply line names `TTL=` in every display language.
    pub fn createTerminal(self: *Fixture, side: Side, pings: u16) !TerminalId {
        var count_buffer: [8]u8 = undefined;
        const arguments = [_][]const u8{ "-n", try std.fmt.bufPrint(&count_buffer, "{d}", .{pings}), "127.0.0.1" };
        return self.createTerminalWith(side, self.ping, &arguments);
    }

    /// A terminal running `executable`, held by the Keeper once it runs.
    pub fn createTerminalWith(self: *Fixture, side: Side, executable: []const u8, arguments: []const []const u8) !TerminalId {
        return self.createTerminalLaunched(side, executable, arguments, .{});
    }

    /// A terminal running `executable` with `launch`'s size and scrollback bound, held by the
    /// Keeper once it runs.
    pub fn createTerminalLaunched(self: *Fixture, side: Side, executable: []const u8, arguments: []const []const u8, launch: Launch) !TerminalId {
        const creation = std.fmt.bytesToHex(harness.testIdentifier(0xc8, self.next_creation), .lower);
        self.next_creation += 1;
        const configuration: ?harness.TerminalConfigurationWire = if (launch.scrollback_bytes) |bytes| .{ .scrollbackBytes = bytes } else null;
        const payload = try harness.testCreatePayloadSized(&creation, executable, arguments, configuration, .{ .columns = launch.columns, .rows = launch.rows });
        defer std.testing.allocator.free(payload);
        const record = try self.call(side, MessageKind.create_terminal, .control_json, payload, 20_000);
        if (record.kind == MessageKind.typed_failure) {
            report("CreateTerminal failed: {s}", .{record.payload});
            return error.CreationFailed;
        }
        const id = try terminalIdOf(self.allocator, record.payload);
        const entry = try self.awaitHolding(id, self.identity(side), 10_000);
        const program = try openProcess(entry.facts.program);
        errdefer _ = support.CloseHandle(program);
        const console = try openProcess(entry.facts.console);
        try self.terminals.append(self.allocator, .{ .id = id, .program = program, .console = console, .holding = entry.holding });
        return id;
    }

    /// The identity under which the terminal was created, for any of its Hosts' identities.
    fn watched(self: *const Fixture, id: TerminalId) TerminalId {
        for (self.aliases.items) |alias| if (alias.id.eql(id)) return alias.of;
        return id;
    }

    pub fn terminal(self: *const Fixture, id: TerminalId) Terminal {
        const created = self.watched(id);
        for (self.terminals.items) |item| if (item.id.eql(created)) return item;
        unreachable;
    }

    /// Stops watching the terminal's processes, so the test holds none of their handles.
    pub fn unwatch(self: *Fixture, id: TerminalId) void {
        const created = self.watched(id);
        var index: usize = 0;
        while (index < self.aliases.items.len) {
            if (self.aliases.items[index].of.eql(created)) _ = self.aliases.swapRemove(index) else index += 1;
        }
        for (self.terminals.items, 0..) |item, position| if (item.id.eql(created)) {
            _ = support.CloseHandle(item.program);
            _ = support.CloseHandle(item.console);
            _ = self.terminals.swapRemove(position);
            return;
        };
    }

    /// Whether the terminal's program and OpenConsole still run.
    pub fn running(self: *const Fixture, id: TerminalId) bool {
        const item = self.terminal(id);
        return support.WaitForSingleObject(item.program, 0) == support.WAIT_TIMEOUT and
            support.WaitForSingleObject(item.console, 0) == support.WAIT_TIMEOUT;
    }

    /// Waits until both the program and OpenConsole have ended.
    pub fn awaitEnded(self: *Fixture, id: TerminalId, timeout_ms: u64) !void {
        const item = self.terminal(id);
        const deadline = self.timer.read() + timeout_ms * ms;
        while (support.WaitForSingleObject(item.program, 0) != support.WAIT_OBJECT_0 or
            support.WaitForSingleObject(item.console, 0) != support.WAIT_OBJECT_0)
        {
            if (self.timer.read() > deadline) return error.TerminalStillRunning;
            try self.tick();
        }
    }

    pub fn targetPayload(self: *Fixture, id: TerminalId) ![]const u8 {
        return std.fmt.allocPrint(self.arena(), "{{\"terminalId\":\"{s}\"}}", .{&std.fmt.bytesToHex(id.bytes, .lower)});
    }

    pub fn terminate(self: *Fixture, side: Side, id: TerminalId) !Record {
        return self.call(side, MessageKind.terminate_terminal, .control_json, try self.targetPayload(id), 10_000);
    }

    pub fn dispose(self: *Fixture, side: Side, id: TerminalId) !void {
        const record = try self.call(side, MessageKind.dispose_terminal, .control_json, try self.targetPayload(id), 10_000);
        try expectKind(record, MessageKind.dispose_terminal);
    }

    const Listed = struct {
        identity: struct { terminalId: []const u8 },
        phase: []const u8,
        handoverState: ?[]const u8 = null,
        holdingId: ?[]const u8 = null,
    };

    /// Every terminal in `side`'s inventory.
    pub fn inventory(self: *Fixture, side: Side) ![]const Listed {
        const record = try self.call(side, MessageKind.list_terminals, .control_json, "{\"cursor\":null,\"maximumTerminals\":128}", 10_000);
        try expectKind(record, MessageKind.list_terminals);
        const List = struct { terminals: []const Listed };
        const parsed = try std.json.parseFromSliceLeaky(List, self.arena(), record.payload, .{ .ignore_unknown_fields = true });
        return parsed.terminals;
    }

    /// The terminal's summary in `side`'s inventory, or null when the Host lists no such terminal.
    pub fn summary(self: *Fixture, side: Side, id: TerminalId) !?Summary {
        const named = std.fmt.bytesToHex(id.bytes, .lower);
        for (try self.inventory(side)) |entry| {
            if (!std.mem.eql(u8, entry.identity.terminalId, &named)) continue;
            return .{ .phase = entry.phase, .handover = entry.handoverState, .holding = entry.holdingId };
        }
        return null;
    }

    /// Waits until the terminal's `handoverState` on `side` is `state`, or absent for null.
    pub fn awaitHandoverState(self: *Fixture, side: Side, id: TerminalId, state: ?[]const u8, timeout_ms: u64) !void {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            const found = (try self.summary(side, id)) orelse return error.TerminalMissing;
            if (optionalEql(found.handover, state)) return;
            if (self.timer.read() > deadline) {
                report("handoverState is {s}, expected {s}", .{ found.handover orelse "absent", state orelse "absent" });
                return error.HandoverStateMismatch;
            }
            try self.idle(20);
        }
    }

    /// Waits until `side` lists no such terminal.
    pub fn awaitGone(self: *Fixture, side: Side, id: TerminalId, timeout_ms: u64) !void {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (try self.summary(side, id) != null) {
            if (self.timer.read() > deadline) return error.TerminalStillListed;
            try self.idle(20);
        }
    }

    // ---- moves ----

    pub fn quiesce(self: *Fixture, from: Side, id: TerminalId, target: message.Process, deadline_seconds: u8) !Record {
        const payload = try std.fmt.allocPrint(self.arena(), "{{\"deadlineSeconds\":{d},\"target\":{{\"creationTime\":\"{d}\",\"pid\":{d}}},\"terminalId\":\"{s}\"}}", .{ deadline_seconds, target.created, target.pid, &std.fmt.bytesToHex(id.bytes, .lower) });
        return self.call(from, MessageKind.quiesce_terminal, .control_json, payload, 15_000);
    }

    /// Quiesces the terminal for `target` and returns its record's size; any refusal fails.
    pub fn quiesced(self: *Fixture, from: Side, id: TerminalId, target: Side, deadline_seconds: u8) !u32 {
        const record = try self.quiesce(from, id, self.identity(target), deadline_seconds);
        try expectKind(record, MessageKind.quiesce_terminal);
        const Quiesced = struct { recordBytes: u32 };
        const parsed = try std.json.parseFromSliceLeaky(Quiesced, self.arena(), record.payload, .{ .ignore_unknown_fields = true });
        return parsed.recordBytes;
    }

    pub fn readPayload(self: *Fixture, id: TerminalId, offset: u32) ![]const u8 {
        return std.fmt.allocPrint(self.arena(), "{{\"offset\":{d},\"terminalId\":\"{s}\"}}", .{ offset, &std.fmt.bytesToHex(id.bytes, .lower) });
    }

    /// Reads the export's whole record from `from`, in chunks.
    pub fn readRecord(self: *Fixture, from: Side, id: TerminalId, bytes: u32) ![]u8 {
        const record = try self.arena().alloc(u8, bytes);
        var offset: u32 = 0;
        while (offset < bytes) {
            const chunk = try self.call(from, MessageKind.read_handover_record, .control_json, try self.readPayload(id, offset), 10_000);
            try expectKind(chunk, MessageKind.handover_record_chunk);
            if (chunk.payload.len <= record_header_bytes or !std.mem.eql(u8, chunk.payload[0..16], &id.bytes)) return error.InvalidChunk;
            const chunk_offset = std.mem.readInt(u64, chunk.payload[16..24], .little);
            const length = std.mem.readInt(u32, chunk.payload[24..28], .little);
            if (chunk_offset != offset or length != chunk.payload.len - record_header_bytes or offset + length > bytes) return error.InvalidChunk;
            @memcpy(record[offset..][0..length], chunk.payload[record_header_bytes..]);
            offset += length;
        }
        return record;
    }

    pub fn creationId(self: *Fixture) [16]u8 {
        defer self.next_creation += 1;
        return harness.testIdentifier(0xd9, self.next_creation);
    }

    pub fn importPayload(self: *Fixture, creation: [16]u8, bytes: usize, deadline_seconds: u8) ![]const u8 {
        return std.fmt.allocPrint(self.arena(), "{{\"creationId\":\"{s}\",\"deadlineSeconds\":{d},\"recordBytes\":{d}}}", .{ &std.fmt.bytesToHex(creation, .lower), deadline_seconds, bytes });
    }

    pub fn chunkPayload(self: *Fixture, creation: [16]u8, offset: usize, chunk: []const u8) ![]const u8 {
        const payload = try self.arena().alloc(u8, record_header_bytes + chunk.len);
        @memcpy(payload[0..16], &creation);
        std.mem.writeInt(u64, payload[16..24], offset, .little);
        std.mem.writeInt(u32, payload[24..28], @intCast(chunk.len), .little);
        std.mem.writeInt(u32, payload[28..32], 0, .little);
        @memcpy(payload[record_header_bytes..], chunk);
        return payload;
    }

    /// `ImportTerminal` and every chunk of `record` on `to`; any refusal fails.
    pub fn importRecord(self: *Fixture, to: Side, creation: [16]u8, record: []const u8, deadline_seconds: u8) !void {
        const imported = try self.call(to, MessageKind.import_terminal, .control_json, try self.importPayload(creation, record.len, deadline_seconds), 10_000);
        try expectKind(imported, MessageKind.import_terminal);
        var offset: usize = 0;
        while (offset < record.len) {
            const end = @min(record.len, offset + max_chunk_bytes);
            const written = try self.call(to, MessageKind.write_handover_record, .handover_record, try self.chunkPayload(creation, offset, record[offset..end]), 10_000);
            try expectKind(written, MessageKind.handover_record_written);
            offset = end;
        }
    }

    pub fn creationPayload(self: *Fixture, creation: [16]u8) ![]const u8 {
        return std.fmt.allocPrint(self.arena(), "{{\"creationId\":\"{s}\"}}", .{&std.fmt.bytesToHex(creation, .lower)});
    }

    pub fn restore(self: *Fixture, to: Side, creation: [16]u8) !Record {
        return self.call(to, MessageKind.restore_terminal, .control_json, try self.creationPayload(creation), 20_000);
    }

    /// Exports the quiesced terminal from `from` and restores it on `to`; any refusal fails.
    /// Returns the terminal's identity on `to`, a new one, which names the same watched processes.
    pub fn restored(self: *Fixture, from: Side, to: Side, id: TerminalId, bytes: u32, deadline_seconds: u8) !TerminalId {
        const creation = self.creationId();
        try self.importRecord(to, creation, try self.readRecord(from, id, bytes), deadline_seconds);
        const identity_record = try self.restore(to, creation);
        try expectKind(identity_record, MessageKind.restore_terminal);
        return self.copyOf(id, identity_record.payload);
    }

    /// Watches the restored terminal that a `TerminalIdentity` names as the copy of `id`.
    pub fn copyOf(self: *Fixture, id: TerminalId, identity_payload: []const u8) !TerminalId {
        const copy = try terminalIdOf(self.allocator, identity_payload);
        if (copy.eql(id)) return error.RestoredIdentityReused;
        try self.aliases.append(self.allocator, .{ .id = copy, .of = self.watched(id) });
        return copy;
    }

    pub fn claim(self: *Fixture, to: Side, id: TerminalId) !Record {
        return self.call(to, MessageKind.claim_terminal, .control_json, try self.targetPayload(id), 15_000);
    }

    pub fn resumeOn(self: *Fixture, side: Side, id: TerminalId) !Record {
        return self.call(side, MessageKind.resume_terminal, .control_json, try self.targetPayload(id), 15_000);
    }

    /// Resumes the terminal on `side` and requires the answer `outcome`.
    pub fn resumed(self: *Fixture, side: Side, id: TerminalId, outcome: []const u8) !void {
        const record = try self.resumeOn(side, id);
        try expectKind(record, MessageKind.resume_terminal);
        try expectOutcome(self.allocator, record.payload, outcome);
    }

    pub fn dropOn(self: *Fixture, side: Side, id: TerminalId) !Record {
        return self.call(side, MessageKind.drop_terminal, .control_json, try self.targetPayload(id), 15_000);
    }

    pub fn discard(self: *Fixture, side: Side, payload: []const u8) !Record {
        return self.call(side, MessageKind.discard_import, .control_json, payload, 15_000);
    }

    /// A whole move from `from` to `to`, through the claim and the old Host's `claimed-elsewhere`,
    /// stopping before the drop. Returns the terminal's identity on `to`.
    pub fn moveUntilDrop(self: *Fixture, from: Side, to: Side, id: TerminalId) !TerminalId {
        const bytes = try self.quiesced(from, id, to, 30);
        const copy = try self.restored(from, to, id, bytes, 30);
        try expectKind(try self.claim(to, copy), MessageKind.claim_terminal);
        try self.resumed(from, id, "claimed-elsewhere");
        return copy;
    }

    // ---- the Keeper's view ----

    /// The holdings the current Keeper reports, of any owner.
    pub fn holdings(self: *Fixture) ![]const message.HoldingEntry {
        const scratch = self.arena();
        const worker = &self.keeper.?.worker;
        var list: std.ArrayList(message.HoldingEntry) = .empty;
        var after: ?message.RandomId = null;
        while (true) {
            const page = switch (try keeperCall(worker, scratch, .{ .status = after })) {
                .status => |value| value,
                else => return error.StatusRefused,
            };
            try list.appendSlice(scratch, page.holdings);
            after = page.next orelse break;
        }
        return list.items;
    }

    pub fn holdingOf(self: *Fixture, id: TerminalId) !?message.HoldingEntry {
        for (try self.holdings()) |entry| if (std.mem.eql(u8, &entry.facts.terminal.terminal_id, &id.bytes)) return entry;
        return null;
    }

    /// Waits until the Keeper holds the terminal for `owner` and no mark is current.
    pub fn awaitHolding(self: *Fixture, id: TerminalId, owner: message.Process, timeout_ms: u64) !message.HoldingEntry {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            if (try self.holdingOf(id)) |entry| if (entry.owner.eql(owner) and entry.state == .held) return entry;
            if (self.timer.read() > deadline) return error.TerminalNotHeld;
            try self.idle(20);
        }
    }

    // ---- handle counts ----

    /// A Host in this process shares it with the test, so its count leaves out the two handles the
    /// test holds for each terminal it watches. A Host or Keeper that is not running counts 0.
    pub fn counts(self: *const Fixture) !Counts {
        const watching: u32 = @intCast(2 * self.terminals.items.len);
        return .{
            .a = if (self.a_kind == .service) try handleCount(windows.GetCurrentProcess()) - watching else try self.processCount(.a),
            .b = try self.processCount(.b),
            .keeper = if (self.keeper) |keeper| try handleCount(keeper.handle) else 0,
        };
    }

    fn processCount(self: *const Fixture, side: Side) !u32 {
        const host = self.hosts[@intFromEnum(side)] orelse return 0;
        return handleCount(host.handle);
    }

    /// Waits until each handle count that `expected` names is back at its baseline.
    pub fn expectCounts(self: *Fixture, expected: Baseline, timeout_ms: u64) !void {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            const actual = try self.counts();
            if (matches(actual.a, expected.a) and matches(actual.b, expected.b) and matches(actual.keeper, expected.keeper)) return;
            if (self.timer.read() > deadline) {
                report("handle counts A {d} B {d} Keeper {d}, baseline A {?d} B {?d} Keeper {?d}", .{ actual.a, actual.b, actual.keeper, expected.a, expected.b, expected.keeper });
                return error.HandleCountsChanged;
            }
            try self.idle(50);
        }
    }

    fn matches(actual: u32, expected: ?u32) bool {
        return actual == expected orelse return true;
    }

    // ---- Host A's screen ----

    /// Host A's screen of the terminal as plain text, read under the terminal lock.
    pub fn screenText(self: *Fixture, id: TerminalId) ![]const u8 {
        const runtime = harness.findRuntime(&self.service, id) orelse return error.RuntimeMissing;
        const state = harness.runtimeState(runtime);
        state.terminal_mutex.lock();
        defer state.terminal_mutex.unlock();
        while (state.reader_commit_in_progress) state.terminal_condition.wait(&state.terminal_mutex);
        var writer: std.Io.Writer.Allocating = .init(self.arena());
        var formatter: ghostty_vt.formatter.ScreenFormatter = .init(state.terminal.screens.active, .{ .emit = .plain, .trim = true, .unwrap = true });
        try formatter.format(&writer.writer);
        return writer.written();
    }

    /// Requires that Host A's screen of the terminal shows every one of `pings` replies once.
    pub fn expectReplies(self: *Fixture, id: TerminalId, pings: usize) !void {
        const text = try self.screenText(id);
        const replies = std.mem.count(u8, text, "TTL=");
        if (replies == pings) return;
        report("{d} replies on the screen, expected {d}:\n{s}", .{ replies, pings, text });
        return error.RepliesLostOrRepeated;
    }

    /// Feeds bytes to Host A's parser of the terminal as if its reader had read them.
    pub fn feed(self: *Fixture, id: TerminalId, bytes: []const u8) !void {
        const runtime = harness.findRuntime(&self.service, id) orelse return error.RuntimeMissing;
        harness.parseTerminalInput(runtime, bytes);
    }
};

// ---- helpers ----

/// A path beside this executable, where the runner staged the Host and the pinned conpty.
fn besideSelf(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    var buffer: [1024]u16 = undefined;
    const length = GetModuleFileNameW(null, &buffer, buffer.len);
    if (length == 0 or length >= buffer.len) return error.ModulePathFailed;
    const path = try std.unicode.utf16LeToUtf8Alloc(allocator, buffer[0..length]);
    defer allocator.free(path);
    const directory = std.fs.path.dirname(path) orelse return error.ModulePathFailed;
    return std.fs.path.join(allocator, &.{ directory, name });
}

fn handleCount(process: HANDLE) !u32 {
    var count: windows.DWORD = 0;
    if (!GetProcessHandleCount(process, &count).toBool()) return error.HandleCountFailed;
    return count;
}

/// A handle that can only wait on the process, after checking it is the one named.
fn openProcess(process: message.Process) !HANDLE {
    const handle = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, .FALSE, process.pid) orelse return error.ProcessGone;
    errdefer _ = support.CloseHandle(handle);
    if (!(try support.processOf(handle)).eql(process)) return error.ProcessGone;
    return handle;
}

fn optionalEql(actual: ?[]const u8, expected: ?[]const u8) bool {
    const value = actual orelse return expected == null;
    return std.mem.eql(u8, value, expected orelse return false);
}

pub fn expectTag(body: message.Reply.Body, comptime tag: std.meta.Tag(message.Reply.Body)) !void {
    if (body == tag) return;
    report("expected {s}, received {s}", .{ @tagName(tag), support.reasonName(body) });
    return error.UnexpectedReply;
}

/// One Keeper request and its reply; the events the Keeper sends meanwhile are skipped.
fn keeperCall(session: *support.Session, arena: std.mem.Allocator, body: message.Request.Body) !message.Reply.Body {
    const id = session.id();
    const encoded = try keeper_protocol.encode.initiatorFrame(arena, .{ .request = .{ .id = id, .body = body } });
    try session.pipe.writeAll(encoded, 3000);
    while (true) {
        const received = switch (try session.pipe.readFrame(arena, 4000)) {
            .frame => |value| value,
            .closed => return error.ConnectionClosed,
            .timeout => return error.ReplyTimeout,
        };
        switch (try keeper_protocol.decode.acceptorPayload(arena, received.payload)) {
            .reply => |reply| {
                if (reply.id != id) return error.ReplyIdMismatch;
                return reply.body;
            },
            .event => {},
        }
    }
}

/// Connects as a Worker that offers `keeper` and `handover`, so Host B negotiates both.
fn connectWorker(allocator: std.mem.Allocator, directory: []const u8, endpoint: []const u8) !transport.Client {
    const host = handshake.registry.workerHostProfile(build_options.host_version, .{ .commit = build_options.source_commit }, build_options.environment, true);
    const frame = try handshake.encode.initiatorFrame(allocator, .{ .offer = .{
        .connection = host.connection,
        .generations = host.generations,
        .capabilities = host.capabilities,
        .product_version = host.product_version,
        .build = host.build,
        .environment = host.environment,
        .fingerprint = host.fingerprint,
    } });
    defer allocator.free(frame);
    return transport.connectWorkerBefore(allocator, directory, endpoint, frame, 10 * std.time.ns_per_s);
}

/// The session's `Hello`; returns the Host epoch.
fn hello(allocator: std.mem.Allocator, client: *transport.Client) ![16]u8 {
    const payload = try std.fmt.allocPrint(allocator, "{{\"clientId\":\"22222222222222222222222222222222\",\"frameBytes\":1048576,\"inputBytes\":2048,\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}", .{ protocol.current_protocol_major, protocol.current_protocol_minor, protocol.current_protocol_patch });
    defer allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(allocator, MessageKind.hello, .control_json, payload, null, null, false, false);
    defer allocator.free(record);
    try client.writeAll(record);
    const response = try transport.readRecordBefore(allocator, client, 5 * std.time.ns_per_s);
    defer allocator.free(response);
    const summary_value = try protocol.validateTransportEnvelope(response);
    if (summary_value.kind != MessageKind.hello_accepted) return error.UnexpectedHelloResponse;
    return summary_value.host_epoch orelse error.UnexpectedHelloResponse;
}

pub fn terminalIdOf(allocator: std.mem.Allocator, payload: []const u8) !TerminalId {
    const Identity = struct { terminalId: []const u8 };
    const parsed = try std.json.parseFromSlice(Identity, allocator, payload, .{ .ignore_unknown_fields = true });
    defer parsed.deinit();
    var bytes: [16]u8 = undefined;
    _ = try std.fmt.hexToBytes(&bytes, parsed.value.terminalId);
    return TerminalId.fromBytes(bytes);
}

pub fn expectKind(record: Record, kind: u16) !void {
    if (record.kind == kind) return;
    report("expected kind {d}, received {d}: {s}", .{ kind, record.kind, record.payload });
    return error.UnexpectedAnswer;
}

/// Requires a typed failure with `code`, and with `reason` when one is named.
pub fn expectFailure(record: Record, code: []const u8, reason: ?[]const u8) !void {
    if (record.kind == MessageKind.typed_failure) {
        const Failure = struct { code: []const u8, reason: ?[]const u8 = null };
        const parsed = try std.json.parseFromSlice(Failure, std.testing.allocator, record.payload, .{ .ignore_unknown_fields = true });
        defer parsed.deinit();
        if (std.mem.eql(u8, parsed.value.code, code) and (reason == null or optionalEql(parsed.value.reason, reason))) return;
    }
    report("expected failure {s} ({s}), received kind {d}: {s}", .{ code, reason orelse "any reason", record.kind, record.payload });
    return error.UnexpectedAnswer;
}

/// Requires the answer's `outcome`.
pub fn expectOutcome(allocator: std.mem.Allocator, payload: []const u8, outcome: []const u8) !void {
    const Wire = struct { outcome: []const u8 };
    const parsed = try std.json.parseFromSlice(Wire, allocator, payload, .{ .ignore_unknown_fields = true });
    defer parsed.deinit();
    if (std.mem.eql(u8, parsed.value.outcome, outcome)) return;
    report("expected outcome {s}: {s}", .{ outcome, payload });
    return error.UnexpectedAnswer;
}
