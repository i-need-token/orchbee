//! The Host's Keeper link against a real Keeper. The Host's service runs in this process, as in the
//! service tests, with real terminals on the pinned conpty that the runner stages beside this
//! executable, so a test can contain a terminal and read the Host's own handles. The Keeper
//! (`orchbee-terminal-host keep`) and the claimant that inspects a holding (the Keeper test's
//! stand-in) are real processes. This process is also the Keeper's Worker, admitted before it
//! registers itself as a Host.

const std = @import("std");
const harness = @import("service_harness.zig");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const support = @import("keeper-support");
const options = @import("keeper_link_test_options");

const protocol = harness.protocol;
const core_module = harness.core_module;
const platform = harness.platform_boundary;
const monotonic = platform.monotonic;
const HostService = harness.HostService;
const ConnectionState = harness.ConnectionState;
const MessageKind = harness.MessageKind;
const TerminalId = core_module.TerminalId;
const message = support.message;
const HANDLE = support.HANDLE;
const windows = std.os.windows;
const ms = std.time.ns_per_ms;

const DUPLICATE_SAME_ACCESS: windows.DWORD = 0x2;
/// Pings that outlast every test: a program that runs until its terminal ends.
const long_running = 300;

extern fn ob_windows_local_test_make_directory_private(path: [*:0]const u16) c_int;
extern "kernel32" fn GetModuleFileNameW(module: ?*anyopaque, buffer: [*]u16, size: windows.DWORD) callconv(.winapi) windows.DWORD;
extern "kernel32" fn GetModuleHandleW(name: [*:0]const u16) callconv(.winapi) ?*anyopaque;
extern "kernel32" fn GetProcAddress(module: *anyopaque, name: [*:0]const u8) callconv(.winapi) ?*const anyopaque;
extern "kernel32" fn DuplicateHandle(
    source_process: HANDLE,
    source: HANDLE,
    target_process: HANDLE,
    target: *HANDLE,
    access: windows.DWORD,
    inherit: windows.BOOL,
    flags: windows.DWORD,
) callconv(.winapi) windows.BOOL;
extern "ntdll" fn NtSuspendProcess(process: HANDLE) callconv(.winapi) windows.NTSTATUS;

const CompareObjectHandles = *const fn (first: HANDLE, second: HANDLE) callconv(.winapi) windows.BOOL;

var keepers_started: u32 = 0;
var job_assigned = false;

fn report(comptime format: []const u8, arguments: anytype) void {
    std.debug.print("keeper-link: " ++ format ++ "\n", arguments);
}

fn elapsedMs(from: u64, to: u64) f64 {
    return @as(f64, @floatFromInt(to -| from)) / ms;
}

fn expectTag(body: message.Reply.Body, comptime tag: std.meta.Tag(message.Reply.Body)) !void {
    if (body == tag) return;
    report("expected {s}, received {s}", .{ @tagName(tag), support.reasonName(body) });
    return error.UnexpectedReply;
}

/// One Keeper request and its reply; the events the Keeper sends meanwhile are skipped.
fn call(session: *support.Session, arena: std.mem.Allocator, body: message.Request.Body) !message.Reply.Body {
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

fn duplicateFrom(process: HANDLE, handle: HANDLE) !HANDLE {
    var result: HANDLE = undefined;
    if (!DuplicateHandle(process, handle, windows.GetCurrentProcess(), &result, 0, .FALSE, DUPLICATE_SAME_ACCESS).toBool())
        return error.DuplicateFailed;
    return result;
}

fn ended(handle: HANDLE) bool {
    return support.WaitForSingleObject(handle, 0) == support.WAIT_OBJECT_0;
}

fn compareObjectHandles() !CompareObjectHandles {
    const module = GetModuleHandleW(std.unicode.utf8ToUtf16LeStringLiteral("kernelbase.dll")) orelse return error.KernelBaseMissing;
    const address = GetProcAddress(module, "CompareObjectHandles") orelse return error.CompareObjectHandlesMissing;
    return @ptrCast(address);
}

/// This executable's directory, where the runner staged the pinned conpty.
fn workRoot(allocator: std.mem.Allocator, io: std.Io) ![]u8 {
    var buffer: [1024]u16 = undefined;
    const length = GetModuleFileNameW(null, &buffer, buffer.len);
    if (length == 0 or length >= buffer.len) return error.ModulePathFailed;
    const path = try std.unicode.utf16LeToUtf8Alloc(allocator, buffer[0..length]);
    defer allocator.free(path);
    const directory = std.fs.path.dirname(path) orelse return error.ModulePathFailed;
    const work = try std.fs.path.join(allocator, &.{ directory, "keeper-link-work" });
    std.Io.Dir.createDirAbsolute(io, work, @enumFromInt(0o700)) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };
    return work;
}

const Keeper = struct {
    child: std.process.Child,
    handle: HANDLE,
    identity: message.Process,
    endpoint: [32]u8,

    fn start(allocator: std.mem.Allocator, io: std.Io) !Keeper {
        keepers_started += 1;
        var random: [16]u8 = undefined;
        io.random(&random);
        const endpoint = std.fmt.bytesToHex(random, .lower);
        const work = try workRoot(allocator, io);
        defer allocator.free(work);
        const directory = try std.fmt.allocPrint(allocator, "{s}\\keeper-{d}", .{ work, keepers_started });
        defer allocator.free(directory);
        try std.Io.Dir.createDirAbsolute(io, directory, @enumFromInt(0o700));
        const directory_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, directory);
        defer allocator.free(directory_w);
        if (ob_windows_local_test_make_directory_private(directory_w.ptr) != 0) return error.PrivateDirectoryFailed;
        var child = try std.process.spawn(io, .{
            .argv = &.{ options.keeper_executable, "keep", "--runtime-directory", directory, "--endpoint-id", &endpoint },
            .stdin = .ignore,
            .stdout = .ignore,
            .stderr = .inherit,
        });
        errdefer child.kill(io);
        if (!support.endpointListening(&endpoint, 5000)) return error.KeeperNotListening;
        return .{ .child = child, .handle = child.id.?, .identity = try support.processOf(child.id.?), .endpoint = endpoint };
    }

    /// Stops the Keeper answering: it keeps its connections and duplicates but runs no thread.
    fn freeze(self: *Keeper) !void {
        if (NtSuspendProcess(self.handle) != .SUCCESS) return error.SuspendFailed;
    }

    /// Ends the Keeper by its own handle if it still runs, and releases it.
    fn end(self: *Keeper, io: std.Io) void {
        if (self.child.id == null) return;
        if (!ended(self.handle)) self.child.kill(io) else _ = self.child.wait(io) catch {};
    }
};

/// The Keeper test's stand-in Host, used only to open Keeper connections as itself.
const StandIn = struct {
    child: std.process.Child,
    identity: message.Process,
    reader: std.Io.File.Reader,
    buffer: [256]u8,
    io: std.Io,

    fn start(allocator: std.mem.Allocator, io: std.Io, endpoint: []const u8) !*StandIn {
        const self = try allocator.create(StandIn);
        errdefer allocator.destroy(self);
        self.child = try std.process.spawn(io, .{
            .argv = &.{ options.stand_in_executable, "--stand-in", endpoint },
            .stdin = .pipe,
            .stdout = .pipe,
            .stderr = .inherit,
        });
        self.identity = try support.processOf(self.child.id.?);
        self.reader = self.child.stdout.?.readerStreaming(io, &self.buffer);
        self.io = io;
        return self;
    }

    /// A connection the stand-in opened, admitted as the Host it is.
    fn session(self: *StandIn, arena: std.mem.Allocator) !support.Session {
        try self.child.stdin.?.writeStreamingAll(self.io, "connect\n");
        const answer = std.mem.trim(u8, try self.reader.interface.takeDelimiterInclusive('\n'), " \r\n");
        if (!std.mem.startsWith(u8, answer, "connected ")) return error.ConnectFailed;
        const value = try std.fmt.parseInt(u64, answer["connected ".len..], 10);
        var result: support.Session = .{ .pipe = try support.Pipe.fromHandle(try support.takeHandle(self.child.id.?, value)) };
        errdefer result.close();
        try result.offer(arena);
        try expectTag(try call(&result, arena, .{ .hello = .host }), .hello);
        return result;
    }

    fn end(self: *StandIn, allocator: std.mem.Allocator) void {
        self.child.stdin.?.writeStreamingAll(self.io, "quit\n") catch {};
        if (support.WaitForSingleObject(self.child.id.?, 5000) == support.WAIT_OBJECT_0)
            _ = self.child.wait(self.io) catch {}
        else
            self.child.kill(self.io);
        allocator.destroy(self);
    }
};

const Record = struct {
    kind: u16,
    event: bool,
    correlation: ?[16]u8,
    payload: []u8,
    at_ns: u64,
};

/// A terminal and duplicates of its program's and its console's process handles, which let the
/// test watch them end without keeping either running.
const Terminal = struct {
    id: TerminalId,
    program: HANDLE,
    console: HANDLE,
};

/// One test's Keeper, its Worker connection, and the Host's service with its Worker session.
const Fixture = struct {
    allocator: std.mem.Allocator,
    io: std.Io,
    arena_state: std.heap.ArenaAllocator,
    timer: monotonic.Timer,
    keeper: Keeper,
    worker: support.Session,
    self_process: message.Process,
    /// The system's `PING.EXE`, beside its command processor.
    ping: []u8,
    service: HostService,
    connection: ConnectionState = .awaiting_hello,
    service_live: bool = true,
    records: std.ArrayList(Record) = .empty,
    terminals: std.ArrayList(Terminal) = .empty,
    next_correlation: usize = 0,
    next_creation: usize = 0,

    fn create() !*Fixture {
        const allocator = std.testing.allocator;
        const io = std.testing.io;
        if (!job_assigned) {
            // Every process this test starts ends with it.
            try support.assignSelf(try support.killOnCloseJob());
            job_assigned = true;
        }
        const self = try allocator.create(Fixture);
        errdefer allocator.destroy(self);
        var keeper = try Keeper.start(allocator, io);
        errdefer keeper.end(io);
        const comspec = try harness.pty_process.environmentVariableOwned(allocator, "ComSpec");
        defer allocator.free(comspec);
        const ping = try std.fs.path.join(allocator, &.{ std.fs.path.dirname(comspec) orelse return error.ComSpecMissing, "PING.EXE" });
        errdefer allocator.free(ping);
        self.* = .{
            .allocator = allocator,
            .io = io,
            .arena_state = .init(std.heap.page_allocator),
            .timer = try monotonic.Timer.start(),
            .keeper = keeper,
            .worker = .{ .pipe = try support.Pipe.open(&keeper.endpoint) },
            .self_process = try support.selfProcess(),
            .ping = ping,
            .service = HostService.init(allocator),
        };
        const scratch = self.arena();
        try self.worker.offer(scratch);
        try expectTag(try call(&self.worker, scratch, .{ .hello = .worker }), .hello);
        try expectTag(try call(&self.worker, scratch, .{ .register_host = self.self_process }), .register_host);
        self.service.admitSession(true, false, false);
        try harness.testHandshake(&self.service, &self.connection, "4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b");
        return self;
    }

    fn destroy(self: *Fixture) void {
        self.endService() catch |err| report("Host teardown failed: {s}", .{@errorName(err)});
        for (self.terminals.items) |item| {
            _ = support.CloseHandle(item.program);
            _ = support.CloseHandle(item.console);
        }
        self.terminals.deinit(self.allocator);
        for (self.records.items) |record| self.allocator.free(record.payload);
        self.records.deinit(self.allocator);
        self.worker.close();
        self.keeper.end(self.io);
        self.allocator.free(self.ping);
        self.arena_state.deinit();
        self.allocator.destroy(self);
    }

    fn arena(self: *Fixture) std.mem.Allocator {
        return self.arena_state.allocator();
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
                .at_ns = self.timer.read(),
            });
        }
    }

    /// One pass of the dispatch thread's idle work: maintenance, which offers running terminals
    /// to the Keepers, and event delivery, which applies what the links reported.
    fn pump(self: *Fixture) !void {
        self.service.maintain();
        var outgoing = try self.service.pollEvents(&self.connection);
        defer outgoing.deinit();
        try self.collect(&outgoing);
    }

    fn tick(self: *Fixture) !void {
        try self.pump();
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

    fn answer(self: *Fixture, correlation: [16]u8) ?Record {
        for (self.records.items) |record| {
            const named = record.correlation orelse continue;
            if (!record.event and std.mem.eql(u8, &named, &correlation)) return record;
        }
        return null;
    }

    fn awaitAnswer(self: *Fixture, correlation: [16]u8, timeout_ms: u64) !Record {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            if (self.answer(correlation)) |record| return record;
            if (self.timer.read() > deadline) return error.RequestUnanswered;
            try self.tick();
        }
    }

    fn events(self: *Fixture, kind: u16) usize {
        var count: usize = 0;
        for (self.records.items) |record| count += @intFromBool(record.event and record.kind == kind);
        return count;
    }

    fn awaitEvent(self: *Fixture, kind: u16, count: usize, timeout_ms: u64) !Record {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            var seen: usize = 0;
            for (self.records.items) |record| if (record.event and record.kind == kind) {
                seen += 1;
                if (seen == count) return record;
            };
            if (self.timer.read() > deadline) return error.EventMissing;
            try self.tick();
        }
    }

    /// The `TerminalUnheld` event naming `id`.
    fn awaitUnheld(self: *Fixture, id: TerminalId, timeout_ms: u64) !Record {
        const named = std.fmt.bytesToHex(id.bytes, .lower);
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            for (self.records.items) |record| {
                if (record.event and record.kind == MessageKind.terminal_unheld and
                    std.mem.indexOf(u8, record.payload, &named) != null) return record;
            }
            if (self.timer.read() > deadline) return error.EventMissing;
            try self.tick();
        }
    }

    fn keeperPayload(self: *Fixture, buffer: []u8, endpoint: bool) ![]const u8 {
        const keeper = self.keeper.identity;
        if (endpoint) return std.fmt.bufPrint(buffer, "{{\"endpointId\":\"{s}\",\"keeper\":{{\"creationTime\":\"{d}\",\"pid\":{d}}}}}", .{ &self.keeper.endpoint, keeper.created, keeper.pid });
        return std.fmt.bufPrint(buffer, "{{\"keeper\":{{\"creationTime\":\"{d}\",\"pid\":{d}}}}}", .{ keeper.created, keeper.pid });
    }

    fn attach(self: *Fixture) !void {
        var buffer: [256]u8 = undefined;
        const correlation = try self.request(MessageKind.attach_keeper, try self.keeperPayload(&buffer, true));
        const record = try self.awaitAnswer(correlation, 5000);
        if (record.kind != MessageKind.attach_keeper) {
            report("AttachKeeper failed: {s}", .{record.payload});
            return error.AttachFailed;
        }
    }

    fn detach(self: *Fixture) !void {
        var buffer: [256]u8 = undefined;
        const correlation = try self.request(MessageKind.detach_keeper, try self.keeperPayload(&buffer, false));
        const record = try self.awaitAnswer(correlation, 5000);
        if (record.kind != MessageKind.detach_keeper) {
            report("DetachKeeper failed: {s}", .{record.payload});
            return error.DetachFailed;
        }
    }

    fn listTerminals(self: *Fixture) ![]const u8 {
        const correlation = try self.request(MessageKind.list_terminals, "{\"cursor\":null,\"maximumTerminals\":128}");
        return (self.answer(correlation) orelse return error.ListUnanswered).payload;
    }

    /// A terminal whose program, `ping`, sends `pings` echo requests a second apart and then
    /// exits. It starts no other process, so closing it ends the console's only client.
    fn createTerminal(self: *Fixture, pings: u16) !TerminalId {
        const creation = std.fmt.bytesToHex(harness.testIdentifier(0xc7, self.next_creation), .lower);
        self.next_creation += 1;
        var count_buffer: [8]u8 = undefined;
        const arguments = [_][]const u8{ "-n", try std.fmt.bufPrint(&count_buffer, "{d}", .{pings}), "127.0.0.1" };
        const payload = try harness.testCreatePayload(&creation, self.ping, &arguments);
        defer std.testing.allocator.free(payload);
        const correlation = try self.request(MessageKind.create_terminal, payload);
        const record = self.answer(correlation) orelse return error.CreationUnanswered;
        const Identity = struct { terminalId: []const u8 };
        const parsed = try std.json.parseFromSlice(Identity, self.allocator, record.payload, .{ .ignore_unknown_fields = true });
        defer parsed.deinit();
        var bytes: [16]u8 = undefined;
        _ = try std.fmt.hexToBytes(&bytes, parsed.value.terminalId);
        const id = TerminalId.fromBytes(bytes);
        const process = self.subject(id) orelse return error.RuntimeMissing;
        const program = try duplicateFrom(windows.GetCurrentProcess(), process.process_handle);
        errdefer _ = support.CloseHandle(program);
        const console = try duplicateFrom(windows.GetCurrentProcess(), consoleHandles(process)[2]);
        try self.terminals.append(self.allocator, .{ .id = id, .program = program, .console = console });
        return id;
    }

    fn subject(self: *Fixture, id: TerminalId) ?*platform.process.Process {
        const runtime = harness.findRuntime(&self.service, id) orelse return null;
        return runtime.keeperSubject();
    }

    fn terminal(self: *Fixture, id: TerminalId) Terminal {
        for (self.terminals.items) |item| if (std.mem.eql(u8, &item.id.bytes, &id.bytes)) return item;
        unreachable;
    }

    fn phase(self: *Fixture, id: TerminalId) core_module.TerminalPhase {
        return harness.serviceCore(&self.service).findTerminal(id).?.phase();
    }

    fn awaitRunning(self: *Fixture, ids: []const TerminalId) !void {
        const deadline = self.timer.read() + 10 * std.time.ns_per_s;
        while (true) {
            var running = true;
            for (ids) |id| running = running and self.phase(id) == .running;
            if (running) return;
            if (self.timer.read() > deadline) return error.TerminalNotRunning;
            try self.tick();
        }
    }

    /// Waits until the terminal has settled: its end has completed.
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

    /// The holdings this process owns, as the Keeper's status reports them.
    fn holdings(self: *Fixture) ![]const message.HoldingEntry {
        const arena_allocator = self.arena();
        var list: std.ArrayList(message.HoldingEntry) = .empty;
        var after: ?message.RandomId = null;
        while (true) {
            const page = switch (try call(&self.worker, arena_allocator, .{ .status = after })) {
                .status => |value| value,
                else => return error.StatusRefused,
            };
            for (page.holdings) |entry| if (entry.owner.eql(self.self_process)) try list.append(arena_allocator, entry);
            after = page.next orelse break;
        }
        return list.items;
    }

    fn awaitHeld(self: *Fixture, ids: []const TerminalId, timeout_ms: u64) ![]const message.HoldingEntry {
        const deadline = self.timer.read() + timeout_ms * ms;
        while (true) {
            const entries = try self.holdings();
            var all = true;
            for (ids) |id| {
                const entry = holdingFor(entries, id) orelse {
                    all = false;
                    break;
                };
                all = all and entry.state == .held;
            }
            if (all) return entries;
            if (self.timer.read() > deadline) return error.TerminalNotHeld;
            try self.tick();
        }
    }

    /// Waits for the terminal's end and checks that it released first and completed: the
    /// terminal settled, the console that the Keeper's duplicates would keep open ended, and the
    /// holding is gone from the Keeper.
    fn expectReleasedAndComplete(self: *Fixture, id: TerminalId, timeout_ms: u64) !void {
        try self.awaitSettled(id, timeout_ms);
        const console = self.terminal(id).console;
        if (support.WaitForSingleObject(console, @intCast(timeout_ms)) != support.WAIT_OBJECT_0) return error.ConsoleStillOpen;
        if (holdingFor(try self.holdings(), id) != null) return error.HoldingNotReleased;
    }

    /// A second connection of this Host, as a Host's own requests would use.
    fn hostSession(self: *Fixture) !support.Session {
        var result: support.Session = .{ .pipe = try support.Pipe.open(&self.keeper.endpoint) };
        errdefer result.close();
        try result.offer(self.arena());
        try expectTag(try call(&result, self.arena(), .{ .hello = .host }), .hello);
        return result;
    }

    /// Asks the Host to end the terminal: `TerminateTerminal` closes it, `ForceTerminateTerminal`
    /// forces it.
    fn end(self: *Fixture, kind: u16, id: TerminalId) !void {
        const payload = try harness.testTerminalTargetPayload(id);
        defer std.testing.allocator.free(payload);
        _ = try self.request(kind, payload);
    }
};

/// The pseudoconsole's members in the pinned conpty's layout: signal, reference, console process.
fn consoleHandles(process: *platform.process.Process) *const [3]HANDLE {
    return @ptrCast(@alignCast(process.pty.pseudoConsole()));
}

fn holdingFor(entries: []const message.HoldingEntry, id: TerminalId) ?message.HoldingEntry {
    for (entries) |entry| if (entry.facts == .terminal and std.mem.eql(u8, &entry.facts.terminal.terminal.terminal_id, &id.bytes)) return entry;
    return null;
}

fn randomId(io: std.Io) message.RandomId {
    var id: message.RandomId = undefined;
    io.random(&id);
    id[0] |= 1;
    return id;
}

test "keeper link holds every running terminal and each duplicate is the Host's handle in its role" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    const arena = fixture.arena();
    const claimant = try StandIn.start(fixture.allocator, fixture.io, &fixture.keeper.endpoint);
    defer claimant.end(fixture.allocator);
    try expectTag(try call(&fixture.worker, arena, .{ .register_host = claimant.identity }), .register_host);

    var ids: [3]TerminalId = undefined;
    for (&ids) |*id| id.* = try fixture.createTerminal(long_running);
    try fixture.awaitRunning(&ids);
    const attach_started = fixture.timer.read();
    try fixture.attach();
    const answered = fixture.timer.read();
    const entries = try fixture.awaitHeld(&ids, 5000);
    const held = fixture.timer.read();
    try std.testing.expectEqual(ids.len, entries.len);
    report("AttachKeeper answered in {d:.1} ms; all {d} running terminals held {d:.1} ms later", .{
        elapsedMs(attach_started, answered), ids.len, elapsedMs(answered, held),
    });

    const listed = try fixture.listTerminals();
    for (ids) |id| {
        const entry = holdingFor(entries, id).?;
        const item = fixture.terminal(id);
        try std.testing.expectEqual(support.GetProcessId(item.program), entry.facts.terminal.program.pid);
        try std.testing.expectEqual(support.GetProcessId(item.console), entry.facts.terminal.console.pid);
        var field_buffer: [64]u8 = undefined;
        const field = try std.fmt.bufPrint(&field_buffer, "\"holdingId\":\"{s}\"", .{&std.fmt.bytesToHex(entry.holding, .lower)});
        try std.testing.expect(std.mem.indexOf(u8, listed, field) != null);
    }

    // The claimant inspects each holding under a mark, which this Host then withdraws.
    var host_session = try fixture.hostSession();
    defer host_session.close();
    var claimant_session = try claimant.session(arena);
    defer claimant_session.close();
    const compare = try compareObjectHandles();
    for (ids) |id| {
        const entry = holdingFor(entries, id).?;
        const mark = randomId(fixture.io);
        try expectTag(try call(&host_session, arena, .{ .mark = .{ .holding = entry.holding, .target = claimant.identity, .mark = mark } }), .mark);
        const inspection = switch (try call(&claimant_session, arena, .{ .inspect = .{ .holding = entry.holding, .mark = mark } })) {
            .inspect => |value| value,
            else => |body| {
                report("inspect answered {s}", .{support.reasonName(body)});
                return error.UnexpectedReply;
            },
        };
        try expectTag(try call(&host_session, arena, .{ .@"resume" = .{ .holding = entry.holding, .mark = mark } }), .@"resume");

        const process = fixture.subject(id).?;
        const console = consoleHandles(process);
        const roles = [_][]const u8{ "job", "program", "console", "signal", "reference", "input", "output" };
        const host_handles = [_]HANDLE{ process.job_handle, process.process_handle, console[2], console[0], console[1], process.pty.input, process.pty.output };
        const values = inspection.handles;
        const keeper_values = [_]u32{ values.job, values.program, values.console, values.signal, values.reference, values.input, values.output };
        var pulled: [roles.len]HANDLE = undefined;
        var pulled_count: usize = 0;
        defer for (pulled[0..pulled_count]) |handle| {
            _ = support.CloseHandle(handle);
        };
        for (keeper_values, &pulled) |value, *local| {
            local.* = try duplicateFrom(fixture.keeper.handle, support.handleFromValue(value));
            pulled_count += 1;
        }
        for (pulled, roles, 0..) |local, role, index| for (host_handles, 0..) |host, other| {
            if (compare(local, host).toBool() != (index == other)) {
                report("the Keeper's {s} duplicate compared {s} with the Host's {s}", .{ role, if (index == other) "different" else "same", roles[other] });
                return error.RoleMismatch;
            }
        };
    }
    report("each of the 7 duplicates of {d} terminals is the same object as the Host's handle in its role and no other, by CompareObjectHandles", .{ids.len});

    try fixture.detach();
    try std.testing.expectEqual(@as(usize, 0), (try fixture.holdings()).len);
    for (ids) |id| try std.testing.expect(!ended(fixture.terminal(id).program));
    try std.testing.expect(std.mem.indexOf(u8, try fixture.listTerminals(), "holdingId") == null);
    report("DetachKeeper released every holding and left every program running", .{});
}

test "keeper link releases first at natural exit close forced termination containment and Host exit" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    // The natural end comes about four seconds after the start, well after the hold.
    const natural = try fixture.createTerminal(5);
    const closed = try fixture.createTerminal(long_running);
    const forced = try fixture.createTerminal(long_running);
    const contained = try fixture.createTerminal(long_running);
    const at_exit = [_]TerminalId{ try fixture.createTerminal(long_running), try fixture.createTerminal(long_running) };
    const all = [_]TerminalId{ natural, closed, forced, contained, at_exit[0], at_exit[1] };
    _ = try fixture.awaitHeld(&all, 3000);

    var started = fixture.timer.read();
    try fixture.end(MessageKind.terminate_terminal, closed);
    try fixture.expectReleasedAndComplete(closed, 10_000);
    report("close released first and completed in {d:.1} ms", .{elapsedMs(started, fixture.timer.read())});

    started = fixture.timer.read();
    try fixture.end(MessageKind.force_terminate_terminal, forced);
    try fixture.expectReleasedAndComplete(forced, 10_000);
    report("forced termination released first and completed in {d:.1} ms", .{elapsedMs(started, fixture.timer.read())});

    started = fixture.timer.read();
    const aggregate = harness.serviceAggregate(&fixture.service);
    aggregate.maintenance.failRuntime(&aggregate.core_state, contained, harness.findRuntime(&fixture.service, contained).?, .apply_resize);
    try fixture.expectReleasedAndComplete(contained, 10_000);
    try std.testing.expectEqual(core_module.TerminalPhase.failed, fixture.phase(contained));
    report("containment released first and completed in {d:.1} ms", .{elapsedMs(started, fixture.timer.read())});

    started = fixture.timer.read();
    try fixture.expectReleasedAndComplete(natural, 10_000);
    report("natural exit released first and completed {d:.1} ms after the other ends", .{elapsedMs(started, fixture.timer.read())});

    started = fixture.timer.read();
    try fixture.endService();
    const exit_ms = elapsedMs(started, fixture.timer.read());
    for (at_exit) |id| {
        const item = fixture.terminal(id);
        try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(item.console, 5000));
        try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(item.program, 5000));
    }
    try std.testing.expectEqual(@as(usize, 0), (try fixture.holdings()).len);
    try std.testing.expectEqual(@as(usize, 0), fixture.events(MessageKind.keeper_unresponsive));
    report("Host exit released both remaining terminals first and completed in {d:.1} ms", .{exit_ms});
}

test "keeper release the Keeper leaves unanswered holds the end until its deadline and fails the link at once" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    const first = try fixture.createTerminal(long_running);
    const second = try fixture.createTerminal(long_running);
    _ = try fixture.awaitHeld(&.{ first, second }, 3000);
    try fixture.keeper.freeze();

    const ended_at = fixture.timer.read();
    try fixture.end(MessageKind.terminate_terminal, first);
    const unresponsive = try fixture.awaitEvent(MessageKind.keeper_unresponsive, 1, 6000);
    const waited_ms = elapsedMs(ended_at, unresponsive.at_ns);
    try std.testing.expect(waited_ms >= 1900);

    // The gate every end path passes: after the miss, the link gives the second terminal up at once.
    const gate = platform.keeper.gate(fixture.subject(second).?).?;
    const pass_started = fixture.timer.read();
    gate.pass();
    const pass_ms = elapsedMs(pass_started, fixture.timer.read());
    try std.testing.expect(pass_ms < 500);
    try fixture.end(MessageKind.terminate_terminal, second);
    // Both programs end, but the duplicates the frozen Keeper kept hold both consoles open, so
    // neither end completes yet.
    for ([_]TerminalId{ first, second }) |id|
        try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(fixture.terminal(id).program, 5000));
    for (0..100) |_| try fixture.tick();
    for ([_]TerminalId{ first, second }) |id| {
        try std.testing.expect(!ended(fixture.terminal(id).console));
        try std.testing.expectEqual(core_module.TerminalPhase.finishing, fixture.phase(id));
    }

    const killed_at = fixture.timer.read();
    fixture.keeper.end(fixture.io);
    _ = try fixture.awaitEvent(MessageKind.keeper_lost, 1, 5000);
    try fixture.awaitSettled(first, 10_000);
    try fixture.awaitSettled(second, 10_000);
    for ([_]TerminalId{ first, second }) |id|
        try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(fixture.terminal(id).console, 5000));
    try std.testing.expectEqual(@as(usize, 1), fixture.events(MessageKind.keeper_unresponsive));
    report("an unanswered release held the close {d:.1} ms and yielded one KeeperUnresponsive; the next end passed in {d:.2} ms; neither end completed while the Keeper lived, and both completed {d:.1} ms after it was ended", .{
        waited_ms, pass_ms, elapsedMs(killed_at, fixture.timer.read()),
    });
}

test "keeper release a Keeper that misses its deadline never stalls the dispatch thread" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    const forced = try fixture.createTerminal(long_running);
    const contained = try fixture.createTerminal(long_running);
    _ = try fixture.awaitHeld(&.{ forced, contained }, 3000);
    try fixture.keeper.freeze();

    // A force whose console rounds once fell back to the whole Job goes to that fallback at once.
    // The test sets the mark, which reaches the fallback without a racing console holder.
    fixture.subject(forced).?.termination.whole_job.store(true, .release);
    const force_payload = try harness.testTerminalTargetPayload(forced);
    defer std.testing.allocator.free(force_payload);
    const force_started = fixture.timer.read();
    const correlation = try fixture.request(MessageKind.force_terminate_terminal, force_payload);
    const force_ms = elapsedMs(force_started, fixture.timer.read());
    try std.testing.expectEqual(MessageKind.force_terminate_terminal, (fixture.answer(correlation) orelse return error.ForceUnanswered).kind);

    const started = fixture.timer.read();
    const aggregate = harness.serviceAggregate(&fixture.service);
    aggregate.maintenance.failRuntime(&aggregate.core_state, contained, harness.findRuntime(&fixture.service, contained).?, .apply_resize);
    const contain_ms = elapsedMs(started, fixture.timer.read());
    try std.testing.expectEqual(core_module.TerminalPhase.containing_failure, fixture.phase(contained));
    try std.testing.expect(!ended(fixture.terminal(forced).program));

    // The dispatch thread keeps its pace while the link waits out the release's deadline; then
    // the forced end terminates the Job and containment completes.
    var slowest_ns: u64 = 0;
    var turns: usize = 0;
    while (fixture.phase(contained) != .failed or !ended(fixture.terminal(forced).program)) : (turns += 1) {
        if (fixture.timer.read() - started > 10 * std.time.ns_per_s) return error.EndIncomplete;
        const before = fixture.timer.read();
        try fixture.pump();
        slowest_ns = @max(slowest_ns, fixture.timer.read() - before);
        monotonic.sleep(10 * ms);
    }
    const ended_ms = elapsedMs(started, fixture.timer.read());
    try std.testing.expect(ended_ms >= 1900);
    try std.testing.expect(force_ms < 500 and contain_ms < 500);
    try std.testing.expect(elapsedMs(0, slowest_ns) < 250);
    try std.testing.expectEqual(@as(usize, 1), fixture.events(MessageKind.keeper_unresponsive));

    fixture.keeper.end(fixture.io);
    try fixture.awaitSettled(forced, 10_000);
    for ([_]TerminalId{ forced, contained }) |id|
        try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(fixture.terminal(id).console, 5000));
    report("with a Keeper missing its release deadline, a forced end's whole-Job fallback answered in {d:.1} ms and a containment returned in {d:.1} ms; over {d} dispatch turns the slowest took {d:.1} ms; both ends went on {d:.1} ms after the start, past the deadline", .{
        force_ms, contain_ms, turns, elapsedMs(0, slowest_ns), ended_ms,
    });
}

test "keeper link reports a lost Keeper and leaves every program running" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    const ids = [_]TerminalId{ try fixture.createTerminal(long_running), try fixture.createTerminal(long_running) };
    _ = try fixture.awaitHeld(&ids, 3000);

    const killed_at = fixture.timer.read();
    const keeper = fixture.keeper.identity;
    fixture.keeper.end(fixture.io);
    const lost = try fixture.awaitEvent(MessageKind.keeper_lost, 1, 5000);
    const lost_ms = elapsedMs(killed_at, lost.at_ns);
    var pid_buffer: [32]u8 = undefined;
    try std.testing.expect(std.mem.indexOf(u8, lost.payload, try std.fmt.bufPrint(&pid_buffer, "\"pid\":{d}", .{keeper.pid})) != null);
    for (0..30) |_| try fixture.tick();
    for (ids) |id| {
        try std.testing.expect(!ended(fixture.terminal(id).program));
        try std.testing.expectEqual(core_module.TerminalPhase.running, fixture.phase(id));
    }
    try std.testing.expect(std.mem.indexOf(u8, try fixture.listTerminals(), "holdingId") == null);

    const started = fixture.timer.read();
    try fixture.end(MessageKind.force_terminate_terminal, ids[0]);
    try fixture.end(MessageKind.terminate_terminal, ids[1]);
    for (ids) |id| try fixture.awaitSettled(id, 5000);
    try std.testing.expectEqual(@as(usize, 0), fixture.events(MessageKind.keeper_unresponsive));
    report("KeeperLost arrived {d:.1} ms after the Keeper was ended; both programs ran on, and their ends completed in {d:.1} ms", .{
        lost_ms, elapsedMs(started, fixture.timer.read()),
    });
}

test "keeper link keeps the handles an unanswered hold named until the Keeper ends" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    try fixture.attach();
    try fixture.keeper.freeze();
    // The program ends by itself about a second after it starts, while its hold waits.
    const id = try fixture.createTerminal(2);
    const unheld = try fixture.awaitUnheld(id, 6000);
    try std.testing.expect(std.mem.indexOf(u8, unheld.payload, "\"reason\":\"unanswered\"") != null);
    try std.testing.expectEqual(@as(usize, 1), fixture.events(MessageKind.keeper_unresponsive));
    const item = fixture.terminal(id);
    try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(item.program, 5000));
    // Only the Host's retained duplicates still name the console's reference: the Keeper never
    // read the hold.
    for (0..150) |_| try fixture.tick();
    try std.testing.expect(!ended(item.console));

    const killed_at = fixture.timer.read();
    fixture.keeper.end(fixture.io);
    try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(item.console, 5000));
    const console_ms = elapsedMs(killed_at, fixture.timer.read());
    try fixture.awaitSettled(id, 10_000);
    _ = try fixture.awaitEvent(MessageKind.keeper_lost, 1, 5000);
    report("an unanswered hold kept the console open 1.5 s after its program ended; it closed {d:.1} ms after the Keeper was ended", .{console_ms});
}

test "keeper hold the Keeper refuses leaves the terminal running unheld and reported" {
    const fixture = try Fixture.create();
    defer fixture.destroy();
    const first = try fixture.createTerminal(long_running);
    try fixture.awaitRunning(&.{first});
    try fixture.attach();
    const entries = try fixture.awaitHeld(&.{first}, 3000);
    {
        var control: support.Session = .{ .pipe = try support.Pipe.open(&fixture.keeper.endpoint) };
        defer control.close();
        switch (try control.control(fixture.arena(), .drain)) {
            .drain_result => {},
            else => return error.DrainUnanswered,
        }
    }
    const second = try fixture.createTerminal(long_running);
    const unheld = try fixture.awaitUnheld(second, 3000);
    try std.testing.expect(std.mem.indexOf(u8, unheld.payload, "\"reason\":\"draining\"") != null);
    for (0..30) |_| try fixture.tick();
    try std.testing.expectEqual(core_module.TerminalPhase.running, fixture.phase(second));
    try std.testing.expect(!ended(fixture.terminal(second).program));
    const listed = try fixture.listTerminals();
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, listed, "holdingId"));
    var field_buffer: [64]u8 = undefined;
    const field = try std.fmt.bufPrint(&field_buffer, "\"holdingId\":\"{s}\"", .{&std.fmt.bytesToHex(entries[0].holding, .lower)});
    try std.testing.expect(std.mem.indexOf(u8, listed, field) != null);

    try fixture.end(MessageKind.force_terminate_terminal, second);
    try fixture.awaitSettled(second, 5000);
    // The release empties the draining Keeper, which then ends.
    try fixture.end(MessageKind.force_terminate_terminal, first);
    try fixture.awaitSettled(first, 10_000);
    try std.testing.expectEqual(support.WAIT_OBJECT_0, support.WaitForSingleObject(fixture.terminal(first).console, 5000));
    _ = try fixture.awaitEvent(MessageKind.keeper_lost, 1, 5000);
    report("a hold the draining Keeper refused left its terminal running unheld and reported as TerminalUnheld (draining)", .{});
}
