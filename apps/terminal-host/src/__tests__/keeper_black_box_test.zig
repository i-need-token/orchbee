//! The Keeper's real-process test. A driver starts real Keepers with `orchbee-terminal-host keep`
//! and stand-in Hosts that hold real programs on the pinned conpty in kill-on-close Jobs, then
//! checks every sequence fixture of `schemas/keeper/v1`, the identity checks, the checks behind a
//! blocked synchronous read, a peer that stops reading, the claim and resume race, the invalid
//! fixtures on real connections, the bounds and the reply time at capacity under full processor
//! load, the endpoint's protection against other identities, `drain`, `exit`, and the empty exit.
//! The driver itself is the Worker, a process the Keeper never registers.

const std = @import("std");
const conpty_options = @import("windows-conpty-options");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const support = @import("keeper_black_box_support.zig");
const stand_in = @import("keeper_stand_in.zig");

const message = support.message;
const monotonic = support.monotonic;
const Session = support.Session;
const HANDLE = support.HANDLE;
const ms = std.time.ns_per_ms;

extern fn ob_windows_local_test_make_directory_private(path: [*:0]const u16) c_int;

/// Every `hello` uses the highest id, which no fixture step uses.
const hello_id: u32 = std.math.maxInt(u32);
const race_rounds = 1000;
const load_rounds = 8;
const status_passes = 8;

pub fn main(init: std.process.Init) !void {
    const arguments = try init.minimal.args.toSlice(init.arena.allocator());
    if (arguments.len == 3 and std.mem.eql(u8, arguments[1], "--stand-in"))
        return stand_in.run(std.heap.page_allocator, init.io, arguments[2]);
    if (arguments.len == 3 and std.mem.eql(u8, arguments[1], "--low-probe"))
        std.process.exit(stand_in.lowProbe(arguments[2]));
    if (arguments.len == 2 and std.mem.eql(u8, arguments[1], "--suspended")) return;
    if (arguments.len != 7 or
        !std.mem.eql(u8, arguments[1], "--keeper") or
        !std.mem.eql(u8, arguments[3], "--fixtures") or
        !std.mem.eql(u8, arguments[5], "--work") or
        !std.fs.path.isAbsolute(arguments[0])) return error.InvalidArguments;
    // Every process the driver starts joins this Job, so none outlives the driver.
    const job = try support.killOnCloseJob();
    try support.assignSelf(job);
    var long_lived = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer long_lived.deinit();
    var driver: Driver = .{
        .allocator = long_lived.allocator(),
        .io = init.io,
        .self_path = arguments[0],
        .keeper_path = arguments[2],
        .fixtures = arguments[4],
        .work = arguments[6],
    };
    _ = try std.fmt.hexToBytes(&driver.console_build, conpty_options.console_sha256);
    try driver.run();
}

fn report(comptime format: []const u8, arguments: anytype) void {
    std.debug.print("keeper-test: " ++ format ++ "\n", arguments);
}

fn fail(comptime format: []const u8, arguments: anytype) error{WitnessFailed} {
    report("FAILED: " ++ format, arguments);
    return error.WitnessFailed;
}

fn expectTag(context: []const u8, body: message.Reply.Body, comptime tag: std.meta.Tag(message.Reply.Body)) !void {
    if (body != tag) return fail("{s}: expected {s}, received {s}", .{ context, @tagName(tag), support.reasonName(body) });
}

fn expectRefused(context: []const u8, body: message.Reply.Body, reason: message.Reason) !void {
    if (!support.refusedWith(body, reason))
        return fail("{s}: expected refused {s}, received {s}", .{ context, reason.name(), support.reasonName(body) });
}

/// The offer, then `hello` as `role`; the reply's body.
fn admit(session: *Session, arena: std.mem.Allocator, role: message.Role) !message.Reply.Body {
    try session.offer(arena);
    return switch (try session.send(arena, .{ .id = hello_id, .body = .{ .hello = role } })) {
        .reply => |received| (try support.decodeReply(arena, received.payload)).body,
        .closed => error.ConnectionClosed,
        .timeout => error.ReplyTimeout,
    };
}

const Keeper = struct {
    child: std.process.Child,
    process: HANDLE,
    endpoint: [32]u8,
    runtime_directory: []const u8,
    io: std.Io,

    fn endpointId(self: *const Keeper) []const u8 {
        return &self.endpoint;
    }

    fn running(self: *const Keeper) bool {
        return self.child.id != null and support.WaitForSingleObject(self.process, 0) == support.WAIT_TIMEOUT;
    }

    /// Whether the Keeper ended by itself within `timeout_ms`.
    fn ended(self: *const Keeper, timeout_ms: u32) bool {
        if (self.child.id == null) return true;
        return support.WaitForSingleObject(self.process, timeout_ms) == support.WAIT_OBJECT_0;
    }

    /// Ends the Keeper by its own handle if it still runs, and releases it.
    fn stop(self: *Keeper) void {
        if (self.child.id == null) return;
        if (self.running()) self.child.kill(self.io) else _ = self.child.wait(self.io) catch {};
    }
};

const Terminal = struct {
    index: u32,
    handles: message.Handles,
    facts: message.Facts,
};

/// One stand-in Host process, driven over its standard input and output.
const StandIn = struct {
    child: std.process.Child,
    handle: HANDLE,
    process: message.Process,
    reader: std.Io.File.Reader,
    buffer: [1024]u8,
    io: std.Io,
    terminals: u32,
    ended: bool,

    fn start(driver: *Driver, endpoint: []const u8) !*StandIn {
        const self = try driver.allocator.create(StandIn);
        self.child = try std.process.spawn(driver.io, .{
            .argv = &.{ driver.self_path, "--stand-in", endpoint },
            .stdin = .pipe,
            .stdout = .pipe,
            .stderr = .inherit,
        });
        self.handle = self.child.id.?;
        self.process = try support.processOf(self.handle);
        self.reader = self.child.stdout.?.readerStreaming(driver.io, &self.buffer);
        self.io = driver.io;
        self.terminals = 0;
        self.ended = false;
        return self;
    }

    fn command(self: *StandIn, line: []const u8) ![]const u8 {
        try self.child.stdin.?.writeStreamingAll(self.io, line);
        try self.child.stdin.?.writeStreamingAll(self.io, "\n");
        const answer = try self.reader.interface.takeDelimiterInclusive('\n');
        return std.mem.trim(u8, answer, " \r\n");
    }

    fn terminal(self: *StandIn, driver: *Driver) !Terminal {
        const answer = try self.command("terminal");
        var fields = std.mem.tokenizeScalar(u8, answer, ' ');
        if (!std.mem.eql(u8, fields.next() orelse "", "terminal")) return error.TerminalFailed;
        var values: [11]u64 = undefined;
        for (&values) |*value| value.* = try std.fmt.parseInt(u64, fields.next() orelse return error.TerminalFailed, 10);
        const launch = try driver.allocator.dupe(u8, fields.next() orelse return error.TerminalFailed);
        defer self.terminals += 1;
        return .{
            .index = self.terminals,
            .handles = .{
                .job = @intCast(values[0]),
                .program = @intCast(values[1]),
                .console = @intCast(values[2]),
                .signal = @intCast(values[3]),
                .reference = @intCast(values[4]),
                .input = @intCast(values[5]),
                .output = @intCast(values[6]),
            },
            .facts = .{
                .terminal = driver.terminalIdentity(),
                .program = .{ .pid = @intCast(values[7]), .created = values[8] },
                .console = .{ .pid = @intCast(values[9]), .created = values[10] },
                .console_build = driver.console_build,
                .signal_format = 1,
                .launch = launch,
            },
        };
    }

    /// Opens a Keeper connection as this Host and takes it over.
    fn connect(self: *StandIn) !Session {
        const answer = try self.command("connect");
        if (!std.mem.startsWith(u8, answer, "connected ")) return error.ConnectFailed;
        const value = try std.fmt.parseInt(u64, answer["connected ".len..], 10);
        return .{ .pipe = try support.Pipe.fromHandle(try support.takeHandle(self.handle, value)) };
    }

    /// A connection this Host opened and the Keeper admitted.
    fn hostSession(self: *StandIn, arena: std.mem.Allocator) !Session {
        var session = try self.connect();
        errdefer session.close();
        try expectTag("hello host", try admit(&session, arena, .host), .hello);
        return session;
    }

    /// Ends the stand-in, and with it every handle it holds.
    fn end(self: *StandIn) void {
        if (self.ended) return;
        self.ended = true;
        self.child.stdin.?.writeStreamingAll(self.io, "quit\n") catch {};
        if (support.WaitForSingleObject(self.handle, 5000) == support.WAIT_OBJECT_0)
            _ = self.child.wait(self.io) catch {}
        else
            self.child.kill(self.io);
    }
};

const SequenceHost = struct {
    name: []const u8,
    stand_in: *StandIn,
    session: Session,
    terminal: ?Terminal = null,
};
const ProcessPair = struct { fake: message.Process, real: message.Process };
const TextPair = struct { fake: []const u8, real: []const u8 };
const HeldProgram = struct { holding: []const u8, program: message.Process };

const Snapshot = struct {
    holdings: []const message.HoldingEntry,
    hosts: []const message.HostEntry,
};

/// The Worker's view of the whole table, page by page.
fn statusAll(arena: std.mem.Allocator, worker: *Session) !Snapshot {
    var holdings: std.ArrayList(message.HoldingEntry) = .empty;
    var hosts: []const message.HostEntry = &.{};
    var after: ?message.RandomId = null;
    while (true) {
        const page = switch (try worker.call(arena, .{ .status = after })) {
            .status => |page| page,
            else => return error.StatusRefused,
        };
        if (page.hosts) |listed| hosts = listed;
        try holdings.appendSlice(arena, page.holdings);
        after = page.next orelse break;
    }
    return .{ .holdings = holdings.items, .hosts = hosts };
}

const Watch = struct {
    keeper: Keeper,
    exit: *ExitRecord,
};

/// The monotonic reading at which a Keeper process ends, on the clock the Keeper counts its 30 s
/// with, so no FILETIME conversion can add skew. A thread blocked on the process handle takes the
/// reading: it is never earlier than the end, and later only by that thread's wake-up.
const ExitRecord = struct {
    clock: monotonic.Timer,
    process: HANDLE,
    ended: bool = false,
    ended_ns: u64 = 0,
    thread: std.Thread = undefined,

    /// Starts the thread that records the end on `clock`; `finish` joins it and frees the record.
    fn start(allocator: std.mem.Allocator, clock: monotonic.Timer, process: HANDLE) !*ExitRecord {
        const record = try allocator.create(ExitRecord);
        errdefer allocator.destroy(record);
        record.* = .{ .clock = clock, .process = process };
        record.thread = try std.Thread.spawn(.{ .stack_size = 256 * 1024 }, wait, .{record});
        return record;
    }

    /// Bounded past the 45 s the test allows, so the thread ends even when the Keeper never does.
    fn wait(self: *ExitRecord) void {
        if (support.WaitForSingleObject(self.process, 60_000) != support.WAIT_OBJECT_0) return;
        self.ended_ns = self.clock.read();
        self.ended = true;
    }

    /// Joins the thread and frees the record; the reading of the end, or null when there was none.
    fn finish(self: *ExitRecord, allocator: std.mem.Allocator) ?u64 {
        self.thread.join();
        defer allocator.destroy(self);
        return if (self.ended) self.ended_ns else null;
    }
};

const Driver = struct {
    allocator: std.mem.Allocator,
    io: std.Io,
    self_path: []const u8,
    keeper_path: []const u8,
    fixtures: []const u8,
    work: []const u8,
    console_build: [32]u8 = undefined,
    keepers: u32 = 0,

    fn run(self: *Driver) !void {
        var from_start = try self.watchEmptyFromStart();
        var after_use = try self.watchEmptyAfterUse();
        try self.sequences();
        try self.identities();
        try self.unreadReplies();
        try self.races();
        try self.invalidFixtures();
        try self.connectionBounds();
        try self.otherPeers();
        try self.drainExit();
        try self.capacity();
        try self.finishEmpty(&from_start, "since it started");
        try self.finishEmpty(&after_use, "since its last Host was unregistered and its Worker left");
        report("every witness passed", .{});
    }

    fn randomId(self: *Driver) message.RandomId {
        var id: message.RandomId = undefined;
        self.io.random(&id);
        id[0] |= 1;
        return id;
    }

    fn terminalIdentity(self: *Driver) message.Terminal {
        return .{ .creation_id = self.randomId(), .host_epoch = self.randomId(), .terminal_id = self.randomId() };
    }

    fn startKeeper(self: *Driver) !Keeper {
        self.keepers += 1;
        var random: [16]u8 = undefined;
        self.io.random(&random);
        const endpoint = std.fmt.bytesToHex(random, .lower);
        const directory = try std.fmt.allocPrint(self.allocator, "{s}\\keeper-{d}", .{ self.work, self.keepers });
        try std.Io.Dir.createDirAbsolute(self.io, directory, @enumFromInt(0o700));
        const directory_w = try std.unicode.utf8ToUtf16LeAllocZ(self.allocator, directory);
        if (ob_windows_local_test_make_directory_private(directory_w.ptr) != 0) return error.PrivateDirectoryFailed;
        var child = try std.process.spawn(self.io, .{
            .argv = &.{ self.keeper_path, "keep", "--runtime-directory", directory, "--endpoint-id", &endpoint },
            .stdin = .ignore,
            .stdout = .ignore,
            .stderr = .inherit,
        });
        errdefer child.kill(self.io);
        if (!support.endpointListening(&endpoint, 5000)) return error.KeeperNotListening;
        return .{ .child = child, .process = child.id.?, .endpoint = endpoint, .runtime_directory = directory, .io = self.io };
    }

    fn workerSession(self: *Driver, arena: std.mem.Allocator, keeper: *const Keeper) !Session {
        _ = self;
        var session: Session = .{ .pipe = try support.Pipe.open(keeper.endpointId()) };
        errdefer session.close();
        try expectTag("hello worker", try admit(&session, arena, .worker), .hello);
        return session;
    }

    fn readFixture(self: *Driver, arena: std.mem.Allocator, name: []const u8) !std.json.Value {
        const path = try std.fs.path.join(arena, &.{ self.fixtures, name });
        const bytes = try std.Io.Dir.cwd().readFileAlloc(self.io, path, arena, .limited(16 * 1024 * 1024));
        return std.json.parseFromSliceLeaky(std.json.Value, arena, bytes, .{});
    }

    // --- Sequence fixtures ---

    fn sequences(self: *Driver) !void {
        var count: usize = 0;
        for ([_][]const u8{ "owner-sequences.json", "claimant-sequences.json" }) |file| {
            var document_arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
            defer document_arena.deinit();
            var document = try self.readFixture(document_arena.allocator(), file);
            for (document.object.getPtr("sequences").?.array.items) |*sequence| {
                var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
                defer arena.deinit();
                self.runSequence(arena.allocator(), sequence) catch |err| {
                    report("sequence {s} failed: {s}", .{ sequence.object.get("name").?.string, @errorName(err) });
                    return err;
                };
                count += 1;
            }
        }
        report("{d} sequence fixtures passed with real Hosts and real terminals", .{count});
    }

    fn runSequence(self: *Driver, arena: std.mem.Allocator, sequence: *std.json.Value) !void {
        const name = sequence.object.get("name").?.string;
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        var hosts: std.ArrayList(SequenceHost) = .empty;
        defer for (hosts.items) |*host| {
            host.session.close();
            host.stand_in.end();
        };
        var processes: std.ArrayList(ProcessPair) = .empty;
        var texts: std.ArrayList(TextPair) = .empty;
        var held: std.ArrayList(HeldProgram) = .empty;
        for (sequence.object.get("hosts").?.array.items) |entry| {
            const stand = try StandIn.start(self, keeper.endpointId());
            errdefer stand.end();
            try processes.append(arena, .{ .fake = try parseProcess(entry.object.get("process").?), .real = stand.process });
            try expectTag("register-host", try worker.call(arena, .{ .register_host = stand.process }), .register_host);
            var session = try stand.hostSession(arena);
            errdefer session.close();
            try hosts.append(arena, .{ .name = entry.object.get("name").?.string, .stand_in = stand, .session = session });
        }
        var exited = false;
        for (sequence.object.getPtr("steps").?.array.items) |*step| {
            if (step.object.get("ends")) |ended| {
                const host = findHost(hosts.items, ended.string) orelse return error.UnknownHost;
                host.stand_in.end();
                try waitHostEnded(arena, &worker, host.stand_in.process);
                continue;
            }
            if (step.object.get("exit") != null) {
                try exitKeeper(arena, &keeper);
                exited = true;
                // `exit` ends no program: each stand-in still holds its own handles.
                for (hosts.items) |host| if (host.terminal) |terminal| if (!support.alive(terminal.facts.program))
                    return fail("{s}: a program ended with the Keeper's exit", .{name});
                continue;
            }
            const host = findHost(hosts.items, step.object.get("host").?.string) orelse return error.UnknownHost;
            const request = step.object.getPtr("request").?;
            const reply = step.object.getPtr("reply").?;
            if (std.mem.eql(u8, request.object.get("type").?.string, "hold")) {
                const terminal = host.terminal orelse try host.stand_in.terminal(self);
                host.terminal = terminal;
                try realHold(arena, request, terminal, &processes, &texts);
            } else try substitute(arena, request, processes.items, texts.items);
            try substitute(arena, reply, processes.items, texts.items);
            const payload = try std.json.Stringify.valueAlloc(arena, request.*, .{});
            try compareReply(arena, name, payload, try host.session.sendPayload(arena, payload), reply.*);
            if (std.mem.eql(u8, reply.object.get("type").?.string, "hold-result"))
                try held.append(arena, .{ .holding = request.object.get("holding").?.string, .program = host.terminal.?.facts.program });
        }
        if (exited) return;
        const expect = sequence.object.get("expect").?.array.items;
        try checkExpect(arena, name, &worker, expect, hosts.items);
        // A holding the Keeper holds keeps its program running, even once its Host ended.
        for (expect) |entry| {
            const holding = entry.object.get("holding").?.string;
            for (held.items) |item| if (std.mem.eql(u8, item.holding, holding) and !support.alive(item.program))
                return fail("{s}: the program of held {s} ended", .{ name, holding });
        }
    }

    // --- Identities and the blocked read ---

    fn identities(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        const host = try StandIn.start(self, keeper.endpointId());
        defer host.end();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = host.process }), .register_host);
        var wrong_host = host.process;
        wrong_host.created += 1;
        try expectRefused("register-host naming another creation time", try worker.call(arena, .{ .register_host = wrong_host }), .identity_mismatch);
        var session = try host.hostSession(arena);
        defer session.close();
        const first = try host.terminal(self);
        const second = try host.terminal(self);

        var other_program = first.facts;
        other_program.program = second.facts.program;
        var other_console = first.facts;
        other_console.console = second.facts.console;
        var later_program = first.facts;
        later_program.program.created += 1;
        var program_handle = first.handles;
        program_handle.program = second.handles.program;
        var foreign_job = first.handles;
        foreign_job.job = second.handles.job;
        var swapped = first.handles;
        swapped.input = first.handles.output;
        swapped.output = first.handles.input;
        var job_as_signal = first.handles;
        job_as_signal.signal = first.handles.job;
        const cases = [_]struct { name: []const u8, handles: message.Handles, facts: message.Facts }{
            .{ .name = "facts naming another program", .handles = first.handles, .facts = other_program },
            .{ .name = "facts naming another console", .handles = first.handles, .facts = other_console },
            .{ .name = "facts naming the program with another creation time", .handles = first.handles, .facts = later_program },
            .{ .name = "the program handle of another program", .handles = program_handle, .facts = first.facts },
            .{ .name = "a Job the program is not in", .handles = foreign_job, .facts = first.facts },
            .{ .name = "input and output swapped", .handles = swapped, .facts = first.facts },
            .{ .name = "the Job as the signal pipe", .handles = job_as_signal, .facts = first.facts },
        };
        for (cases) |case| try expectRefused(case.name, try session.call(arena, .{ .hold = .{
            .holding = self.randomId(),
            .handles = case.handles,
            .facts = case.facts,
        } }), .identity_mismatch);
        if ((try statusAll(arena, &worker)).holdings.len != 0) return fail("a refused hold left a holding", .{});
        report("{d} mismatched identities refused (identity-mismatch), holding nothing", .{cases.len + 1});

        // The pull and every check complete while the stand-in's synchronous read of the output
        // pipe is pending; a pipe query behind that read shows it blocks the pipe throughout.
        const probe = try host.command(try std.fmt.allocPrint(arena, "probe {d}", .{first.index}));
        if (!std.mem.eql(u8, probe, "probe blocked")) return fail("the pipe query behind the pending read completed: {s}", .{probe});
        var timer = try monotonic.Timer.start();
        try expectTag("hold behind a blocked read", try session.call(arena, .{ .hold = .{
            .holding = self.randomId(),
            .handles = first.handles,
            .facts = first.facts,
        } }), .hold);
        const elapsed_ns = timer.read();
        const still = try host.command(try std.fmt.allocPrint(arena, "probe-state {d}", .{first.index}));
        if (!std.mem.eql(u8, still, "probe blocked")) return fail("the read completed during the hold, which then proves nothing", .{});
        report("hold behind another process's blocked synchronous read: hold-result in {d:.1} ms while a pipe query stayed blocked", .{nsToMs(elapsed_ns)});
    }

    // --- A peer that stops reading ---

    fn unreadReplies(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        const host = try StandIn.start(self, keeper.endpointId());
        defer host.end();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = host.process }), .register_host);
        var session = try host.hostSession(arena);
        defer session.close();
        const terminal = try host.terminal(self);
        var first_holding: message.RandomId = undefined;
        for (0..message.maximum_status_entries) |index| {
            const holding = self.randomId();
            if (index == 0) first_holding = holding;
            try expectTag("hold", try session.call(arena, .{ .hold = .{ .holding = holding, .handles = terminal.handles, .facts = terminal.facts } }), .hold);
        }
        // The Worker asks for full status pages and reads none of them, until the Keeper's write
        // of a reply cannot finish.
        var timer = try monotonic.Timer.start();
        var sent: u32 = 0;
        while (sent < 8) : (sent += 1) {
            const frame = try keeper_protocol.encode.initiatorFrame(arena, .{ .request = .{ .id = worker.id(), .body = .{ .status = null } } });
            worker.pipe.writeAll(frame, 1000) catch break;
            monotonic.sleep(30 * ms);
        }
        // Meanwhile the Host's connection is answered at once, again and again.
        var answered: u32 = 0;
        var slowest_ns: u64 = 0;
        while (timer.read() < 2500 * ms) {
            const answer = try session.send(arena, .{ .id = session.id(), .body = .{ .hold = .{ .holding = first_holding, .handles = terminal.handles, .facts = terminal.facts } } });
            const received = switch (answer) {
                .reply => |received| received,
                else => return fail("the Host's connection stalled behind an unread reply", .{}),
            };
            try expectTag("repeated hold", (try support.decodeReply(arena, received.payload)).body, .hold);
            slowest_ns = @max(slowest_ns, received.elapsed_ns);
            answered += 1;
            monotonic.sleep(10 * ms);
        }
        // The Worker's connection ends once its reply has waited out the reply deadline.
        const drained = worker.pipe.drain(4000);
        const closed_after_ns = timer.read();
        worker.close();
        switch (drained) {
            .closed => {},
            else => return fail("the connection that stopped reading was not closed", .{}),
        }
        var next_worker = try self.workerSession(arena, &keeper);
        defer next_worker.close();
        _ = try statusAll(arena, &next_worker);
        const diagnostics_path = try std.fs.path.join(arena, &.{ keeper.runtime_directory, "keeper-diagnostics.jsonl" });
        monotonic.sleep(300 * ms);
        const diagnostics = std.Io.Dir.cwd().readFileAlloc(self.io, diagnostics_path, arena, .limited(64 * 1024)) catch "";
        if (std.mem.indexOf(u8, diagnostics, "\"reply_not_taken\":1") == null)
            return fail("keeper-diagnostics.jsonl did not count the reply not taken: {s}", .{diagnostics});
        report("a Worker that stopped reading {d} status pages was closed {d:.0} ms after its first unread request; {d} Host requests meanwhile, slowest {d:.1} ms", .{
            sent, nsToMs(closed_after_ns), answered, nsToMs(slowest_ns),
        });
    }

    // --- The claim and resume race ---

    fn races(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        const first = try StandIn.start(self, keeper.endpointId());
        defer first.end();
        const second = try StandIn.start(self, keeper.endpointId());
        defer second.end();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = first.process }), .register_host);
        try expectTag("register-host", try worker.call(arena, .{ .register_host = second.process }), .register_host);
        var sessions = [2]Session{ try first.hostSession(arena), try second.hostSession(arena) };
        defer for (&sessions) |*session| session.close();
        const processes = [2]message.Process{ first.process, second.process };
        const terminal = try first.terminal(self);
        const holding = self.randomId();
        try expectTag("hold", try sessions[0].call(arena, .{ .hold = .{ .holding = holding, .handles = terminal.handles, .facts = terminal.facts } }), .hold);
        var owner: usize = 0;
        var resumed_first: u32 = 0;
        var claimed_first: u32 = 0;
        var side_arenas = [2]std.heap.ArenaAllocator{ .init(std.heap.page_allocator), .init(std.heap.page_allocator) };
        defer for (&side_arenas) |*side| side.deinit();
        for (0..race_rounds) |_| {
            defer _ = arena_state.reset(.retain_capacity);
            defer for (&side_arenas) |*side| {
                _ = side.reset(.retain_capacity);
            };
            const target = 1 - owner;
            const mark = self.randomId();
            try expectTag("mark", try sessions[owner].call(arena, .{ .mark = .{ .holding = holding, .target = processes[target], .mark = mark } }), .mark);
            var go = std.atomic.Value(bool).init(false);
            var resume_side: RaceSide = .{
                .session = &sessions[owner],
                .frame = try keeper_protocol.encode.initiatorFrame(arena, .{ .request = .{ .id = sessions[owner].id(), .body = .{ .@"resume" = .{ .holding = holding, .mark = mark } } } }),
                .go = &go,
                .arena = &side_arenas[0],
            };
            var claim_side: RaceSide = .{
                .session = &sessions[target],
                .frame = try keeper_protocol.encode.initiatorFrame(arena, .{ .request = .{ .id = sessions[target].id(), .body = .{ .claim = .{
                    .holding = holding,
                    .mark = mark,
                    .program = terminal.facts.program,
                    .terminal = self.terminalIdentity(),
                } } } }),
                .go = &go,
                .arena = &side_arenas[1],
            };
            const resume_thread = try std.Thread.spawn(.{ .stack_size = 256 * 1024 }, RaceSide.run, .{&resume_side});
            const claim_thread = try std.Thread.spawn(.{ .stack_size = 256 * 1024 }, RaceSide.run, .{&claim_side});
            go.store(true, .release);
            resume_thread.join();
            claim_thread.join();
            const resume_body = try resume_side.body(arena);
            const claim_body = try claim_side.body(arena);
            const resumed = switch (resume_body) {
                .@"resume" => |outcome| outcome == .resumed,
                else => false,
            };
            const claimed = claim_body == .claim;
            if (resumed == claimed) return fail("claim and resume of one mark: resume {s}, claim {s}", .{ outcomeName(resume_body), support.reasonName(claim_body) });
            if (claimed) {
                const after_claim = switch (resume_body) {
                    .@"resume" => |outcome| outcome == .claimed,
                    else => false,
                };
                if (!after_claim) return fail("a resume after the claim answered {s}", .{outcomeName(resume_body)});
                claimed_first += 1;
                owner = target;
            } else {
                try expectRefused("a claim after the resume", claim_body, .stale_mark);
                resumed_first += 1;
            }
        }
        report("{d} claim and resume pairs of one mark sent at once: never both succeeded (resume won {d}, claim won {d})", .{
            race_rounds, resumed_first, claimed_first,
        });
    }

    // --- Invalid fixtures on real connections ---

    fn invalidFixtures(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        const host = try StandIn.start(self, keeper.endpointId());
        defer host.end();
        {
            var worker = try self.workerSession(arena, &keeper);
            defer worker.close();
            try expectTag("register-host", try worker.call(arena, .{ .register_host = host.process }), .register_host);
        }
        var bystander = try host.hostSession(arena);
        defer bystander.close();
        var sent: usize = 0;
        var slowest_close_ns: u64 = 0;
        for ([_][]const u8{ "owner.json", "claimant.json", "worker.json" }) |file| {
            const document = try self.readFixture(arena, file);
            const role: message.Role = if (std.mem.eql(u8, file, "worker.json")) .worker else .host;
            for (document.object.get("invalid").?.array.items) |case| {
                const name = case.object.get("name").?.string;
                const bytes = if (case.object.get("frameHex")) |hex| frame: {
                    const decoded = try arena.alloc(u8, hex.string.len / 2);
                    break :frame try std.fmt.hexToBytes(decoded, hex.string);
                } else frame: {
                    const payload = case.object.get("payload").?.string;
                    // An invalid reply that is a valid request in the Keeper's reading direction is
                    // answered as that request.
                    if (keeper_protocol.decode.initiatorPayload(arena, payload)) |_| {
                        var session = try host.hostSession(arena);
                        defer session.close();
                        switch (try session.sendPayload(arena, payload)) {
                            .reply => {},
                            else => return fail("{s}: a valid request in the Keeper's direction was not answered", .{name}),
                        }
                        report("{s} ({s}) is a valid request in the Keeper's reading direction and was answered", .{ name, file });
                        continue;
                    } else |_| {}
                    break :frame try keeper_protocol.frame.encode(arena, payload);
                };
                var session = if (role == .host) try host.hostSession(arena) else try self.workerSession(arena, &keeper);
                defer session.close();
                var timer = try monotonic.Timer.start();
                session.pipe.writeAll(bytes, 2000) catch {};
                switch (session.pipe.drain(4000)) {
                    .closed => |count| if (count != 0) return fail("{s} ({s}): {d} reply bytes before the close", .{ name, file, count }),
                    .timeout => return fail("{s} ({s}): the connection stayed open", .{ name, file }),
                    .frame => unreachable,
                }
                slowest_close_ns = @max(slowest_close_ns, timer.read());
                try expectTag("the bystander's release", try bystander.call(arena, .{ .release = self.randomId() }), .release);
                sent += 1;
            }
        }
        if (!keeper.running()) return fail("the Keeper ended during the invalid fixtures", .{});
        report("{d} invalid fixtures on real connections each closed only their connection without a reply (slowest close {d:.0} ms); the bystander was answered after each", .{
            sent, nsToMs(slowest_close_ns),
        });
    }

    // --- Connection bounds ---

    fn connectionBounds(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var pending: [8]support.Pipe = undefined;
        var opened = try monotonic.Timer.start();
        for (&pending) |*pipe| pipe.* = try support.Pipe.open(keeper.endpointId());
        monotonic.sleep(200 * ms);
        var beyond = try support.Pipe.open(keeper.endpointId());
        var timer = try monotonic.Timer.start();
        const outcome = beyond.drain(1500);
        const closed_ns = timer.read();
        beyond.close();
        switch (outcome) {
            .closed => |count| if (count != 0) return fail("the ninth unadmitted connection received {d} bytes", .{count}),
            else => return fail("the ninth unadmitted connection stayed open", .{}),
        }
        // The eight still wait for their first frame and each closes at its deadline. A peer that
        // sent no byte was never identified, so nothing is written to it.
        for (&pending) |*pipe| {
            defer pipe.close();
            switch (pipe.drain(4000)) {
                .closed => |count| if (count != 0) return fail("a silent unadmitted connection received {d} bytes", .{count}),
                else => return fail("a silent unadmitted connection outlived its deadline", .{}),
            }
        }
        const silent_closed_ns = opened.read();
        if (silent_closed_ns < 1900 * ms) return fail("silent connections closed after {d:.0} ms, before their deadline", .{nsToMs(silent_closed_ns)});
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        var second: Session = .{ .pipe = try support.Pipe.open(keeper.endpointId()) };
        defer second.close();
        try expectRefused("a second Worker connection", try admit(&second, arena, .worker), .capacity);
        switch (second.pipe.drain(2000)) {
            .closed => {},
            else => return fail("a refused hello left its connection open", .{}),
        }
        _ = try statusAll(arena, &worker);
        report("a ninth unadmitted connection closed in {d:.1} ms; the eight silent ones closed at their 2 s deadline (all closed {d:.0} ms after opening); a second Worker was refused capacity", .{
            nsToMs(closed_ns), nsToMs(silent_closed_ns),
        });
    }

    // --- Peers the Keeper does not serve ---

    fn otherPeers(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        const host = try StandIn.start(self, keeper.endpointId());
        defer host.end();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = host.process }), .register_host);
        var bystander = try host.hostSession(arena);
        defer bystander.close();

        // The driver is no registered Host.
        var unregistered: Session = .{ .pipe = try support.Pipe.open(keeper.endpointId()) };
        defer unregistered.close();
        try expectRefused("hello host from an unregistered process", try admit(&unregistered, arena, .host), .not_registered);
        switch (unregistered.pipe.drain(2000)) {
            .closed => {},
            else => return fail("the unregistered peer's connection stayed open", .{}),
        }
        try expectTag("the bystander's release", try bystander.call(arena, .{ .release = self.randomId() }), .release);
        _ = try statusAll(arena, &worker);

        const anonymous_write = try support.openAsAnonymous(keeper.endpointId(), support.GENERIC_READ | support.GENERIC_WRITE);
        const anonymous_read = try support.openAsAnonymous(keeper.endpointId(), support.GENERIC_READ);
        if (anonymous_write != support.ERROR_ACCESS_DENIED or anonymous_read != support.ERROR_ACCESS_DENIED)
            return fail("the anonymous token opened the endpoint: {d} {d}", .{ anonymous_write, anonymous_read });

        const command_line = try std.fmt.allocPrint(arena, "keeper-low-probe --low-probe {s}", .{keeper.endpointId()});
        const low = try support.spawnLowIntegrity(arena, self.self_path, command_line);
        defer _ = support.CloseHandle(low);
        if (support.WaitForSingleObject(low, 10_000) != support.WAIT_OBJECT_0) return fail("the Low-integrity probe did not end", .{});
        const low_result = support.exitCode(low) orelse return fail("the Low-integrity probe has no exit code", .{});
        if (low_result & 0x30 != 0 or low_result & 0x1 == 0 or low_result & 0x6 == 0)
            return fail("the Low-integrity probe reached the endpoint: result {x}", .{low_result});
        try expectTag("the bystander's release", try bystander.call(arena, .{ .release = self.randomId() }), .release);
        _ = try statusAll(arena, &worker);
        report("an unregistered peer was refused not-registered and closed; the anonymous token was denied ({d}/{d}); a Low-integrity child sent no frame (read-write open denied, {s})", .{
            anonymous_write,
            anonymous_read,
            if (low_result & 0x2 != 0) "read-only open denied" else "write through a read-only open failed",
        });
    }

    // --- drain ---

    fn drainExit(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        const host = try StandIn.start(self, keeper.endpointId());
        defer host.end();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = host.process }), .register_host);
        var session = try host.hostSession(arena);
        defer session.close();
        const terminal = try host.terminal(self);
        const holding = self.randomId();
        try expectTag("hold", try session.call(arena, .{ .hold = .{ .holding = holding, .handles = terminal.handles, .facts = terminal.facts } }), .hold);
        {
            var control: Session = .{ .pipe = try support.Pipe.open(keeper.endpointId()) };
            defer control.close();
            switch (try control.control(arena, .drain)) {
                .drain_result => |result| if (result.state != .draining) return fail("drain answered {s}", .{@tagName(result.state)}),
                else => return fail("drain was not answered", .{}),
            }
        }
        try expectRefused("register-host while draining", try worker.call(arena, .{ .register_host = try support.selfProcess() }), .draining);
        try expectRefused("hold while draining", try session.call(arena, .{ .hold = .{ .holding = self.randomId(), .handles = terminal.handles, .facts = terminal.facts } }), .draining);
        if (!keeper.running()) return fail("a draining Keeper that holds a terminal ended", .{});
        try expectTag("release", try session.call(arena, .{ .release = holding }), .release);
        if (!keeper.ended(2000)) return fail("a drained Keeper that holds nothing did not end", .{});
        if (support.exitCode(keeper.process) != 0) return fail("the drained Keeper's exit code is not 0", .{});
        if (!support.alive(terminal.facts.program)) return fail("the drained Keeper's end ended a program", .{});
        report("drain refused registration and holds (draining) and the Keeper ended once it held nothing", .{});
    }

    // --- Capacity and the reply time under full processor load ---

    fn capacity(self: *Driver) !void {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        defer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        defer worker.close();
        const host = try StandIn.start(self, keeper.endpointId());
        defer host.end();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = host.process }), .register_host);
        // The other 255 Hosts are processes that never run, in a Job that ends them afterwards.
        const job = try support.killOnCloseJob();
        defer _ = support.CloseHandle(job);
        for (1..message.maximum_hosts) |_| {
            const process = try support.spawnSuspended(arena, self.self_path, job);
            defer _ = support.CloseHandle(process);
            try expectTag("register-host", try worker.call(arena, .{ .register_host = try support.processOf(process) }), .register_host);
        }
        {
            const process = try support.spawnSuspended(arena, self.self_path, job);
            defer _ = support.CloseHandle(process);
            try expectRefused("a 257th registered Host", try worker.call(arena, .{ .register_host = try support.processOf(process) }), .capacity);
        }
        var sessions = try arena.alloc(Session, 256);
        var opened: usize = 0;
        defer for (sessions[0..opened]) |*session| session.close();
        while (opened < sessions.len) : (opened += 1) sessions[opened] = try host.hostSession(arena);
        {
            var beyond = try host.connect();
            defer beyond.close();
            try expectRefused("a 257th Host connection", try admit(&beyond, arena, .host), .capacity);
        }
        const terminal = try host.terminal(self);
        const holdings = try arena.alloc(message.RandomId, message.maximum_holdings);
        for (holdings, 0..) |*holding, index| {
            holding.* = self.randomId();
            try expectTag("hold", try sessions[index % sessions.len].call(arena, .{ .hold = .{ .holding = holding.*, .handles = terminal.handles, .facts = terminal.facts } }), .hold);
        }
        try expectRefused("a 1,025th holding", try sessions[0].call(arena, .{ .hold = .{ .holding = self.randomId(), .handles = terminal.handles, .facts = terminal.facts } }), .capacity);

        // Every logical processor spins while every connection asks at once.
        const processors = try std.Thread.getCpuCount();
        var stop = std.atomic.Value(bool).init(false);
        const spinners = try arena.alloc(std.Thread, processors);
        for (spinners) |*spinner| spinner.* = try std.Thread.spawn(.{ .stack_size = 64 * 1024 }, spin, .{&stop});
        defer {
            stop.store(true, .release);
            for (spinners) |spinner| spinner.join();
        }
        monotonic.sleep(200 * ms);
        const start = try support.manualEvent();
        defer _ = support.CloseHandle(start);
        var failed = std.atomic.Value(bool).init(false);
        const host_samples = try arena.alloc(u64, sessions.len * load_rounds);
        const status_samples = try arena.alloc(u64, status_passes * (message.maximum_holdings / message.maximum_status_entries));
        const threads = try arena.alloc(std.Thread, sessions.len);
        for (threads, 0..) |*thread, index| thread.* = try std.Thread.spawn(.{ .stack_size = 256 * 1024 }, loadConnection, .{LoadContext{
            .session = &sessions[index],
            .holding = holdings[index],
            .terminal = &terminal,
            .samples = host_samples[index * load_rounds ..][0..load_rounds],
            .failed = &failed,
            .start = start,
        }});
        const status_thread = try std.Thread.spawn(.{ .stack_size = 256 * 1024 }, loadStatus, .{StatusContext{
            .session = &worker,
            .samples = status_samples,
            .failed = &failed,
            .start = start,
        }});
        support.setEvent(start);
        for (threads) |thread| thread.join();
        status_thread.join();
        if (failed.load(.acquire)) return fail("a request at capacity under load was not answered", .{});
        std.mem.sort(u64, host_samples, {}, std.sort.asc(u64));
        std.mem.sort(u64, status_samples, {}, std.sort.asc(u64));
        const host_max = host_samples[host_samples.len - 1];
        const status_max = status_samples[status_samples.len - 1];
        if (host_max >= message.reply_deadline_ns or status_max >= message.reply_deadline_ns)
            return fail("a reply at capacity took {d:.0} ms", .{nsToMs(@max(host_max, status_max))});
        report("capacity (256 Hosts, 256 Host connections, 1,024 holdings) under full load ({d} spinning threads): {d} Host requests p50 {d:.1} ms, p99 {d:.1} ms, max {d:.1} ms; {d} status pages of 256 p50 {d:.1} ms, max {d:.1} ms", .{
            processors,
            host_samples.len,
            nsToMs(host_samples[host_samples.len / 2]),
            nsToMs(host_samples[(host_samples.len * 99) / 100]),
            nsToMs(host_max),
            status_samples.len,
            nsToMs(status_samples[status_samples.len / 2]),
            nsToMs(status_max),
        });
        report("bounds refused with capacity: a 257th Host registration, a 257th Host connection, a 1,025th holding", .{});
    }

    // --- The empty exit ---

    /// The clock starts before the Keeper process exists, so the span holds its whole 30 s.
    fn watchEmptyFromStart(self: *Driver) !Watch {
        const clock = try monotonic.Timer.start();
        var keeper = try self.startKeeper();
        errdefer keeper.stop();
        return .{ .keeper = keeper, .exit = try ExitRecord.start(self.allocator, clock, keeper.process) };
    }

    fn watchEmptyAfterUse(self: *Driver) !Watch {
        var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var keeper = try self.startKeeper();
        errdefer keeper.stop();
        var worker = try self.workerSession(arena, &keeper);
        const driver_process = try support.selfProcess();
        try expectTag("register-host", try worker.call(arena, .{ .register_host = driver_process }), .register_host);
        try expectTag("unregister-host", try worker.call(arena, .{ .unregister_host = driver_process }), .unregister_host);
        // The clock starts before the close that makes the Worker leave. The Keeper's 30 s begins only
        // after that departure, so the span measured from here contains all of it.
        const clock = try monotonic.Timer.start();
        const record = try ExitRecord.start(self.allocator, clock, keeper.process);
        worker.close();
        return .{ .keeper = keeper, .exit = record };
    }

    fn finishEmpty(self: *Driver, watch: *Watch, label: []const u8) !void {
        // The exit code is read, and the record joined, before `stop` releases the process handle.
        const ended = watch.keeper.ended(45_000);
        const exit_code = support.exitCode(watch.keeper.process);
        const ended_ns = watch.exit.finish(self.allocator);
        watch.keeper.stop();
        if (!ended) return fail("an empty Keeper still runs 45 s {s}", .{label});
        if (exit_code != 0) return fail("the empty Keeper's exit code is not 0", .{});
        const end = ended_ns orelse return fail("the empty Keeper's end was not recorded", .{});
        const elapsed_ms = end / std.time.ns_per_ms;
        if (elapsed_ms < 30_000 or elapsed_ms > 32_000) return fail("an empty Keeper ended {d} ms {s}", .{ elapsed_ms, label });
        report("an empty Keeper ended {d} ms {s}", .{ elapsed_ms, label });
    }
};

const RaceSide = struct {
    session: *Session,
    frame: []const u8,
    go: *std.atomic.Value(bool),
    arena: *std.heap.ArenaAllocator,
    answer: ?support.Answer = null,

    fn run(self: *RaceSide) void {
        while (!self.go.load(.acquire)) std.atomic.spinLoopHint();
        self.answer = self.session.sendFrame(self.arena.allocator(), self.frame) catch null;
    }

    fn body(self: *RaceSide, arena: std.mem.Allocator) !message.Reply.Body {
        const answer = self.answer orelse return fail("a racing request failed", .{});
        return switch (answer) {
            .reply => |received| (try support.decodeReply(arena, received.payload)).body,
            else => fail("a racing request got no reply ({s})", .{@tagName(answer)}),
        };
    }
};

const LoadContext = struct {
    session: *Session,
    holding: message.RandomId,
    terminal: *const Terminal,
    samples: []u64,
    failed: *std.atomic.Value(bool),
    start: HANDLE,
};

/// One Host connection repeating its hold, a lookup that answers `hold-result`.
fn loadConnection(context: LoadContext) void {
    var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena_state.deinit();
    _ = support.WaitForSingleObject(context.start, support.INFINITE);
    for (context.samples) |*sample| {
        defer _ = arena_state.reset(.retain_capacity);
        const arena = arena_state.allocator();
        const answer = context.session.send(arena, .{ .id = context.session.id(), .body = .{ .hold = .{
            .holding = context.holding,
            .handles = context.terminal.handles,
            .facts = context.terminal.facts,
        } } }) catch return context.failed.store(true, .release);
        switch (answer) {
            .reply => |received| {
                sample.* = received.elapsed_ns;
                const reply = support.decodeReply(arena, received.payload) catch return context.failed.store(true, .release);
                if (reply.body != .hold) return context.failed.store(true, .release);
            },
            else => return context.failed.store(true, .release),
        }
    }
}

const StatusContext = struct {
    session: *Session,
    samples: []u64,
    failed: *std.atomic.Value(bool),
    start: HANDLE,
};

/// The Worker paging through all 1,024 holdings, again and again.
fn loadStatus(context: StatusContext) void {
    var arena_state = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena_state.deinit();
    _ = support.WaitForSingleObject(context.start, support.INFINITE);
    var after: ?message.RandomId = null;
    for (context.samples) |*sample| {
        defer _ = arena_state.reset(.retain_capacity);
        const arena = arena_state.allocator();
        const answer = context.session.send(arena, .{ .id = context.session.id(), .body = .{ .status = after } }) catch
            return context.failed.store(true, .release);
        switch (answer) {
            .reply => |received| {
                sample.* = received.elapsed_ns;
                const reply = support.decodeReply(arena, received.payload) catch return context.failed.store(true, .release);
                const page = switch (reply.body) {
                    .status => |page| page,
                    else => return context.failed.store(true, .release),
                };
                if (page.holdings.len != message.maximum_status_entries) return context.failed.store(true, .release);
                after = page.next;
            },
            else => return context.failed.store(true, .release),
        }
    }
}

fn spin(stop: *std.atomic.Value(bool)) void {
    var counter: u64 = 0;
    while (!stop.load(.monotonic)) {
        counter +%= 1;
        std.mem.doNotOptimizeAway(counter);
    }
}

fn nsToMs(nanoseconds: u64) f64 {
    return @as(f64, @floatFromInt(nanoseconds)) / @as(f64, @floatFromInt(ms));
}

fn outcomeName(body: message.Reply.Body) []const u8 {
    return switch (body) {
        .@"resume", .decline => |outcome| outcome.name(),
        else => support.reasonName(body),
    };
}

fn findHost(hosts: []SequenceHost, name: []const u8) ?*SequenceHost {
    for (hosts) |*host| if (std.mem.eql(u8, host.name, name)) return host;
    return null;
}

fn hostProcess(hosts: []const SequenceHost, name: []const u8) ?message.Process {
    for (hosts) |host| if (std.mem.eql(u8, host.name, name)) return host.stand_in.process;
    return null;
}

fn parseProcess(value: std.json.Value) !message.Process {
    return .{
        .pid = @intCast(value.object.get("pid").?.integer),
        .created = try std.fmt.parseInt(u64, value.object.get("created").?.string, 16),
    };
}

fn hex16(arena: std.mem.Allocator, value: u64) ![]const u8 {
    return std.fmt.allocPrint(arena, "{x:0>16}", .{value});
}

fn setProcess(arena: std.mem.Allocator, value: *std.json.Value, process: message.Process) !void {
    value.object.getPtr("pid").?.* = .{ .integer = process.pid };
    value.object.getPtr("created").?.* = .{ .string = try hex16(arena, process.created) };
}

fn rememberProcess(arena: std.mem.Allocator, pairs: *std.ArrayList(ProcessPair), fake: message.Process, real: message.Process) !void {
    for (pairs.items) |pair| if (pair.fake.eql(fake)) return;
    try pairs.append(arena, .{ .fake = fake, .real = real });
}

fn rememberText(arena: std.mem.Allocator, pairs: *std.ArrayList(TextPair), fake: []const u8, real: []const u8) !void {
    for (pairs.items) |pair| if (std.mem.eql(u8, pair.fake, fake)) return;
    try pairs.append(arena, .{ .fake = fake, .real = real });
}

/// Replaces a fixture hold's handles and facts with a real terminal's; the fixture's terminal
/// identities, which name nothing real, stay. Later steps see the real values for the fake ones.
fn realHold(
    arena: std.mem.Allocator,
    request: *std.json.Value,
    terminal: Terminal,
    processes: *std.ArrayList(ProcessPair),
    texts: *std.ArrayList(TextPair),
) !void {
    const facts = request.object.getPtr("facts").?;
    const program = facts.object.getPtr("program").?;
    const console = facts.object.getPtr("console").?;
    try rememberProcess(arena, processes, try parseProcess(program.*), terminal.facts.program);
    try rememberProcess(arena, processes, try parseProcess(console.*), terminal.facts.console);
    const build = try arena.dupe(u8, &std.fmt.bytesToHex(terminal.facts.console_build, .lower));
    try rememberText(arena, texts, facts.object.get("consoleBuild").?.string, build);
    try rememberText(arena, texts, facts.object.get("launch").?.string, terminal.facts.launch);
    try setProcess(arena, program, terminal.facts.program);
    try setProcess(arena, console, terminal.facts.console);
    facts.object.getPtr("consoleBuild").?.* = .{ .string = build };
    facts.object.getPtr("launch").?.* = .{ .string = terminal.facts.launch };
    const handles = request.object.getPtr("handles").?;
    inline for (message.Handles.roles) |role|
        handles.object.getPtr(role).?.* = .{ .string = try hex16(arena, @field(terminal.handles, role)) };
}

/// Replaces every fake process and text value the fixture used with its real counterpart; values
/// no step made real, such as a process no Host is, stay as the fixture wrote them.
fn substitute(arena: std.mem.Allocator, value: *std.json.Value, processes: []const ProcessPair, texts: []const TextPair) !void {
    switch (value.*) {
        .object => |*object| {
            if (object.count() == 2 and object.get("pid") != null and object.get("created") != null) {
                const fake = parseProcess(value.*) catch return;
                for (processes) |pair| if (pair.fake.eql(fake)) return setProcess(arena, value, pair.real);
                return;
            }
            var iterator = object.iterator();
            while (iterator.next()) |entry| try substitute(arena, entry.value_ptr, processes, texts);
        },
        .array => |*array| for (array.items) |*item| try substitute(arena, item, processes, texts),
        .string => |text| for (texts) |pair| if (std.mem.eql(u8, pair.fake, text)) {
            value.* = .{ .string = pair.real };
            return;
        },
        else => {},
    }
}

fn compareReply(arena: std.mem.Allocator, name: []const u8, request: []const u8, answer: support.Answer, expected: std.json.Value) !void {
    const payload = switch (answer) {
        .reply => |received| received.payload,
        .closed, .timeout => return fail("{s}: {s} got no reply ({s})", .{ name, request, @tagName(answer) }),
    };
    const expected_payload = try std.json.Stringify.valueAlloc(arena, expected, .{});
    var actual = try support.decodeReply(arena, payload);
    var wanted = try support.decodeReply(arena, expected_payload);
    // Handle values are the Keeper's own, so an inspection is compared without them.
    const placeholder: message.Handles = .{ .job = 1, .program = 2, .console = 3, .signal = 4, .reference = 5, .input = 6, .output = 7 };
    switch (actual.body) {
        .inspect => |*inspection| inspection.handles = placeholder,
        else => {},
    }
    switch (wanted.body) {
        .inspect => |*inspection| inspection.handles = placeholder,
        else => {},
    }
    if (!std.mem.eql(u8, try support.encodeReply(arena, actual), try support.encodeReply(arena, wanted)))
        return fail("{s}: {s}\n  expected {s}\n  received {s}", .{ name, request, expected_payload, payload });
}

fn checkExpect(arena: std.mem.Allocator, name: []const u8, worker: *Session, expect: []const std.json.Value, hosts: []const SequenceHost) !void {
    const snapshot = try statusAll(arena, worker);
    if (snapshot.holdings.len != expect.len) return fail("{s}: {d} holdings, expected {d}", .{ name, snapshot.holdings.len, expect.len });
    for (expect) |entry| {
        var id: message.RandomId = undefined;
        _ = try std.fmt.hexToBytes(&id, entry.object.get("holding").?.string);
        const actual = for (snapshot.holdings) |holding| {
            if (std.mem.eql(u8, &holding.holding, &id)) break holding;
        } else return fail("{s}: holding {s} is missing", .{ name, entry.object.get("holding").?.string });
        const owner = hostProcess(hosts, entry.object.get("owner").?.string) orelse return error.UnknownHost;
        if (!actual.owner.eql(owner)) return fail("{s}: the holding's owner differs", .{name});
        const state = message.HoldingState.fromName(entry.object.get("state").?.string);
        if (actual.state != state) return fail("{s}: the holding is {s}, expected {s}", .{ name, actual.state.name(), state.name() });
        if (entry.object.get("for")) |target_name| {
            const target = hostProcess(hosts, target_name.string) orelse return error.UnknownHost;
            if (actual.target == null or !actual.target.?.eql(target)) return fail("{s}: the holding's mark target differs", .{name});
        } else if (actual.target != null) return fail("{s}: the holding names a mark target", .{name});
    }
}

fn waitHostEnded(arena: std.mem.Allocator, worker: *Session, process: message.Process) !void {
    var timer = try monotonic.Timer.start();
    while (timer.read() < 5 * std.time.ns_per_s) {
        const page = switch (try worker.call(arena, .{ .status = null })) {
            .status => |page| page,
            else => return error.StatusRefused,
        };
        const entry = for (page.hosts orelse &.{}) |host| {
            if (host.process.eql(process)) break host;
        } else null;
        if (entry == null or entry.?.ended) return;
        monotonic.sleep(20 * ms);
    }
    return fail("the Keeper did not see a Host end", .{});
}

fn exitKeeper(arena: std.mem.Allocator, keeper: *Keeper) !void {
    var session: Session = .{ .pipe = try support.Pipe.open(keeper.endpointId()) };
    defer session.close();
    switch (try session.control(arena, .exit)) {
        .exit_result => {},
        else => return fail("exit was not answered", .{}),
    }
    if (!keeper.ended(3000)) return fail("the Keeper did not end after exit", .{});
    if (support.exitCode(keeper.process) != 0) return fail("the Keeper's exit code after exit is not 0", .{});
}
