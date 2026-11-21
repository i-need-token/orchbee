//! The build of a `RestoreTerminal`, which the creation pipeline runs as it runs a terminal's
//! spawn. On the creator thread it decodes and checks the record and builds the engine; for a
//! running terminal it then inspects the holding through the Keeper, pulls the seven duplicates,
//! checks them against the record and the Keeper's facts, packs the console, and creates the
//! threads behind the startup gate, which only the claim opens. A settled terminal needs none of
//! that. The dispatch thread commits what it built, abandons what failed, and at teardown discards
//! a built terminal without a `decline`.
//!
//! The creator thread never makes a link request itself: the dispatch thread owns the Keeper
//! attachments, so the build posts its `inspect` here and the dispatch thread hands it to the one
//! attached Keeper on its next turn.

const std = @import("std");
const builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const terminal_engine = @import("terminal-host-engine");
const refusal = @import("refusal.zig");
const host_response = @import("../response.zig");
const host_runtime_aggregate = @import("../runtime_aggregate/root.zig");

const Aggregate = host_runtime_aggregate.HostRuntimeAggregate;
const TerminalRuntime = terminal_engine.runtime.TerminalRuntime;
const Facts = terminal_engine.runtime.HandoverFacts;
const Preparation = terminal_engine.runtime.HandoverImportPreparation;
const Program = terminal_engine.runtime.HandoverImportProgram;
const Request = keeper_link.moves.Request;
const Keeper = keeper_link.Keeper;
const Details = host_response.FailureDetails;
const handles = platform.keeper.handles;
const builds = platform.keeper.builds;

pub const Outcome = enum {
    building,
    /// Committed: a running terminal is `restored` and waits for its claim.
    committed,
    /// The build failed, or teardown discarded what it built; nothing of it remains.
    ended,
};

pub const Job = struct {
    allocator: std.mem.Allocator,
    creation_id: core_module.CreationId,
    slot: host_runtime_aggregate.ImportSlot,
    /// The import's whole record, which the facts borrow.
    record: []const u8,
    mutex: platform.thread_sync.Mutex = .{},
    /// The build's `inspect`, waiting for the dispatch thread.
    posted: ?*Request = null,
    closed: bool = false,
    /// The Keeper the `inspect` went to, which the claim and a `decline` then use.
    keeper: ?Keeper = null,
    /// Why the dispatch thread could not hand the `inspect` to a Keeper.
    unsent: ?Details = null,
    failed: ?Details = null,
    facts: ?Facts = null,
    outcome: Outcome = .building,

    pub fn init(
        allocator: std.mem.Allocator,
        creation_id: core_module.CreationId,
        slot: host_runtime_aggregate.ImportSlot,
        record: []const u8,
    ) Job {
        return .{ .allocator = allocator, .creation_id = creation_id, .slot = slot, .record = record };
    }

    /// The build, on the creator thread. On error `failure` names the refusal and nothing remains.
    pub fn run(self: *Job) !*TerminalRuntime {
        var prepared = TerminalRuntime.prepareImport(self.allocator, self.record) catch |err|
            return self.fail(refusal.restored(err), err);
        const program: Program = switch (prepared.facts.terminal.inputs.lifecycle) {
            .exited, .failed => .settled,
            .running => .{ .held = self.claimCheck(&prepared.facts) catch |err| {
                prepared.abandon();
                return err;
            } },
        };
        const runtime = TerminalRuntime.startImport(&prepared, program) catch |err|
            return self.fail(startFailure(err), err);
        self.facts = prepared.facts;
        return runtime;
    }

    fn startFailure(err: anyerror) Details {
        return switch (err) {
            error.OutOfMemory, error.SystemResources => refusal.exhausted,
            else => refusal.failed("handle-failed"),
        };
    }

    /// The checks a claim needs before any of it applies: a known console build, the holding's
    /// duplicates pulled from the Keeper, and every fact of theirs matching the record's.
    fn claimCheck(self: *Job, facts: *const Facts) !platform.process.Held {
        if (comptime !platform.process.moves_supported) unreachable;
        const console = facts.console orelse return self.fail(refusal.failed("invalid-record"), error.InvalidRecord);
        const inputs = facts.terminal.inputs;
        const holding = inputs.holding_id orelse return self.fail(refusal.failed("invalid-record"), error.InvalidRecord);
        const mark = inputs.mark_id orelse return self.fail(refusal.failed("invalid-record"), error.InvalidRecord);
        const build = builds.find(console.open_console_sha256) orelse
            return self.fail(refusal.ineligible("unknown-console-build"), error.UnknownConsoleBuild);
        if (build.signal_format != console.signal_format or build.signal_format != builds.own.signal_format)
            return self.fail(refusal.ineligible("unknown-console-build"), error.UnknownConsoleBuild);
        var request: Request = .init(.{ .inspect = .{ .holding = holding, .mark = mark } });
        if (!self.post(&request)) return self.fail(refusal.failed("no-keeper"), error.KeeperUnavailable);
        const inspected = switch (request.wait()) {
            .inspected => |value| value,
            else => |result| return self.fail(self.unsent orelse refusal.keeper(result), error.KeeperUnavailable),
        };
        var duplicates = inspected.duplicates;
        const program: handles.Identity = .{ .pid = console.program.pid, .created = console.program.created };
        const host: handles.Identity = .{ .pid = console.console.pid, .created = console.console.created };
        const agreed = std.mem.eql(u8, &inspected.console_build, &console.open_console_sha256) and
            inspected.signal_format == console.signal_format and
            inspected.program.pid == program.pid and inspected.program.created == program.created and
            inspected.console.pid == host.pid and inspected.console.created == host.created;
        if (!agreed or !handles.matches(duplicates, program, host) or !handles.jobKillsOnClose(duplicates)) {
            handles.closeAll(&duplicates);
            return self.fail(refusal.failed("claim-check"), error.ClaimCheckFailed);
        }
        return .{
            .duplicates = duplicates,
            .console_pid = host.pid,
            .launch_kind = facts.terminal.launch_kind,
            .interactive_shell = facts.terminal.interactive_shell,
        };
    }

    fn fail(self: *Job, details: Details, err: anyerror) anyerror {
        self.failed = details;
        return err;
    }

    /// Posts the build's `inspect` and wakes the dispatch thread; false once teardown closed it.
    fn post(self: *Job, request: *Request) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.closed) return false;
        self.posted = request;
        platform.output_wake.signal();
        return true;
    }

    /// The dispatch thread's turn: hands a posted `inspect` to the one attached Keeper, which
    /// becomes the import's Keeper. Without exactly one, the `inspect` fails at once.
    pub fn serve(self: *Job, attachments: *keeper_link.Attachments) void {
        const request = self.take() orelse return;
        switch (attachments.count()) {
            1 => {},
            0 => {
                self.unsent = refusal.failed("no-keeper");
                return request.complete(.failed);
            },
            else => {
                self.unsent = refusal.ineligible("two-keepers");
                return request.complete(.failed);
            },
        }
        self.keeper = attachments.sole().?;
        attachments.submit(self.keeper.?, request);
    }

    /// Teardown, before the pipeline waits for the build: an `inspect` not handed over fails.
    pub fn close(self: *Job) void {
        const request = request: {
            self.mutex.lock();
            defer self.mutex.unlock();
            self.closed = true;
            const posted = self.posted;
            self.posted = null;
            break :request posted;
        };
        if (request) |value| {
            self.unsent = refusal.failed("no-keeper");
            value.complete(.failed);
        }
    }

    fn take(self: *Job) ?*Request {
        self.mutex.lock();
        defer self.mutex.unlock();
        const request = self.posted;
        self.posted = null;
        return request;
    }

    /// Commits what the build made, on the dispatch thread. A running terminal names its record's
    /// holding before any claim.
    pub fn commit(self: *Job, aggregate: *Aggregate, runtime: *TerminalRuntime) void {
        aggregate.commitImport(self.slot, self.creation_id, runtime, &self.facts.?);
        if (comptime builtin.os.tag == .windows) {
            if (self.facts.?.terminal.inputs.holding_id) |holding| platform.keeper.gate(runtime.keeperSubject()).?.name(holding);
        }
        self.outcome = .committed;
    }

    pub fn failure(self: *const Job) Details {
        return self.failed orelse refusal.failed("invalid-record");
    }

    /// Returns what the build reserved once it failed.
    pub fn abandon(self: *Job, aggregate: *Aggregate) void {
        aggregate.releaseImport(self.slot);
        self.outcome = .ended;
    }

    /// Teardown's end of a terminal built but not committed: a running one is dropped without a
    /// `decline`, so its program runs on in the Keeper's care, and a settled one is freed.
    pub fn discardBuilt(self: *Job, aggregate: *Aggregate, runtime: *TerminalRuntime) void {
        const facts = self.facts.?;
        switch (facts.terminal.inputs.lifecycle) {
            .running => runtime.dropMoved(),
            .exited, .failed => runtime.destroy(),
        }
        self.allocator.free(facts.held_attachments);
        self.facts = null;
        self.abandon(aggregate);
    }
};
