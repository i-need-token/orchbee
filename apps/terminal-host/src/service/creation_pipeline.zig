//! Builds a terminal's process and engine on a creator thread, so a slow start never holds the
//! dispatch thread, and holds the requests that depend on that creation until it settles.
//!
//! A creation passes three stages. The dispatch thread admits it, takes the launch it was
//! admitted with, and encodes its reply; a creator thread builds the runtime, which spawns the
//! child; the dispatch thread then commits the runtime and sends the reply, or sends the build's
//! failure. One creation is in progress at a time, and the requests that depend on it wait in
//! `creation_holds.zig` until it settles, so no request acts on a half-built terminal. A
//! requester that disconnects leaves its creation to commit unanswered, as a start on the
//! dispatch thread would have, and drops the requests it had held.
//!
//! Host teardown waits a bounded time for a build in progress. A runtime it built is committed
//! unanswered, so the aggregate's teardown contains it with every other terminal and keeps it
//! under THDD-001 while its process tree stays unconfirmed; a failed build releases what its
//! creation acquired. A build that never returns is abandoned with everything it reads, and
//! teardown then fails. A restore builds a terminal from a handover record instead of spawning
//! one: its job, which the moves owner keeps, decides its commit, its failure, and its teardown,
//! where a runtime it built is discarded rather than contained, since its program belongs to the
//! Host that holds it.
const std = @import("std");
const target_builtin = @import("builtin");

const core_module = @import("terminal-host-core").state;
const platform = @import("terminal-host-platform");
const fatal_record = platform.fatal_record;
const monotonic = platform.monotonic;
const output_wake = platform.output_wake;
const pty_process = platform.process;
const thread_sync = platform.thread_sync;
const terminal_runtime = @import("terminal-host-engine").runtime;
const identifiers = @import("terminal-host-model").identifiers;
const keeper_link = @import("terminal-host-keeper-link");
const creation_failure = @import("creation_failure.zig");
const creation_leader = @import("creation_leader.zig");
const creation_holds = @import("creation_holds.zig");
const restore_build = @import("moves/restore_build.zig");
const host_outgoing = @import("outgoing.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");

const Aggregate = host_runtime_aggregate.HostRuntimeAggregate;
const Outgoing = host_outgoing.Outgoing;
const TerminalRuntime = terminal_runtime.TerminalRuntime;
const WireIdentifier = identifiers.IdentifierBytes;

/// How long Host teardown waits for a build in progress before it abandons the build.
const teardown_build_wait_ns: u64 = 5 * std.time.ns_per_s;

pub const BeginError = error{
    CreationInProgress,
    CreatorUnavailable,
    OutOfMemory,
    RuntimeAllocationOutOfMemory,
    TerminalRuntimeMissing,
};

pub const HoldError = creation_holds.HoldError;

/// The admitted request a creation answers.
pub const Request = struct {
    connection_id: core_module.ConnectionId,
    correlation_id: WireIdentifier,
    host_epoch: identifiers.HostEpoch,
    /// The success response and `TerminalStarted`. Ownership moves only when the begin succeeds.
    reply: Outgoing,
    /// The Keeper attachments: on macOS an attached Keeper that parents leaders starts the new
    /// terminal's leader.
    keepers: ?*keeper_link.Attachments = null,
};

/// What a settled creation sends its requester.
pub const Reply = union(enum) {
    /// The committed creation's response and its `TerminalStarted` event.
    records: Outgoing,
    /// The typed failure of a build that produced no runtime.
    failure: FailureRecord,

    pub fn deinit(self: *Reply) void {
        switch (self.*) {
            .records => |*outgoing| outgoing.deinit(),
            .failure => {},
        }
        self.* = undefined;
    }
};

pub const FailureRecord = creation_failure.FailureRecord;

const Work = union(enum) {
    create: host_runtime_aggregate.CreateBuild,
    start: host_runtime_aggregate.StartBuild,
    restore: *restore_build.Job,

    /// The identity a new or started terminal runs under.
    fn identity(self: *const Work) host_runtime_aggregate.TerminalIdentity {
        return switch (self.*) {
            .create => |*build| build.identity(),
            .start => |*build| build.pending.identity,
            .restore => unreachable,
        };
    }

    fn terminalId(self: *const Work) core_module.TerminalId {
        return switch (self.*) {
            .create => |*build| build.pending.terminal_id,
            .start => |*build| build.pending.identity.terminal_id,
            .restore => |job| job.slot.terminal_id,
        };
    }
};

const BuildOutcome = union(enum) {
    built: *TerminalRuntime,
    failed: anyerror,
};

const BuildGate = if (target_builtin.is_test) struct {
    mutex: thread_sync.Mutex = .{},
    condition: thread_sync.Condition = .{},
    arrived: bool = false,
    open: bool = false,

    fn pass(self: *@This()) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.arrived = true;
        self.condition.broadcast();
        while (!self.open) self.condition.wait(&self.mutex);
    }

    pub fn awaitArrival(self: *@This()) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (!self.arrived) self.condition.wait(&self.mutex);
    }

    pub fn release(self: *@This()) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.open = true;
        self.condition.broadcast();
    }
} else void;

/// One creation in progress. The dispatch thread owns every field except `outcome`, which the
/// creator writes once under `mutex` and the dispatch thread reads after.
const Creation = struct {
    allocator: std.mem.Allocator,
    connection_id: core_module.ConnectionId,
    correlation_id: WireIdentifier,
    host_epoch: identifiers.HostEpoch,
    /// The requester's session is still open, so the settled result is answered on it.
    answerable: bool = true,
    reply: Outgoing,
    work: Work,
    launch: pty_process.Launch = undefined,
    options: terminal_runtime.Options = undefined,
    leader_start: creation_leader.LeaderStart = .{},
    creator: std.Thread = undefined,
    mutex: thread_sync.Mutex = .{},
    finished: thread_sync.Condition = .{},
    outcome: ?BuildOutcome = null,
    build_gate: if (target_builtin.is_test) ?*BuildGate else void =
        if (target_builtin.is_test) null else {},

    fn takeOutcome(self: *Creation) ?BuildOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.outcome;
    }
};

/// Owned by the dispatch thread, like the rest of the Host service.
pub const Pipeline = struct {
    in_progress: ?*Creation = null,
    holds: creation_holds.Holds = .{},
    /// Teardown abandoned a build that never returned.
    abandoned_build: bool = false,
    build_gate: if (target_builtin.is_test) ?*BuildGate else void =
        if (target_builtin.is_test) null else {},

    pub fn creating(self: *const Pipeline) bool {
        return self.in_progress != null;
    }

    /// Whether the session may read another request: false while the held-request limit is
    /// reached, until the creation they wait for settles.
    pub fn acceptsRequests(self: *const Pipeline) bool {
        return self.holds.acceptsRequests();
    }

    /// Starts building the terminal `preparation` creates with its geometry. On error the
    /// preparation is unchanged and the caller still owns `request.reply`.
    pub fn beginCreate(
        self: *Pipeline,
        aggregate: *Aggregate,
        request: Request,
        preparation: *host_runtime_aggregate.CreatePreparation,
    ) BeginError!void {
        if (self.in_progress != null) return error.CreationInProgress;
        const creation = try aggregate.allocator.create(Creation);
        errdefer aggregate.allocator.destroy(creation);
        const build = try aggregate.beginCreateBuild(preparation);
        creation.* = initCreation(aggregate.allocator, request, .{ .create = build });
        creation.launch = creation.work.create.processLaunch();
        creation.options = creation.work.create.runtimeOptions();
        creation.leader_start = .pin(request.keepers, creation.work.identity(), request.host_epoch);
        self.launchCreator(creation) catch {
            creation.leader_start.unpin();
            aggregate.allocator.destroy(creation.work.create.retained);
            preparation.* = .{ .pending = creation.work.create.pending };
            return error.CreatorUnavailable;
        };
        self.in_progress = creation;
    }

    /// Starts building the awaiting terminal `preparation` starts. On error the preparation is
    /// unchanged and the caller still owns `request.reply`.
    pub fn beginStart(
        self: *Pipeline,
        aggregate: *Aggregate,
        request: Request,
        preparation: *host_runtime_aggregate.StartPreparation,
    ) BeginError!void {
        if (self.in_progress != null) return error.CreationInProgress;
        const creation = try aggregate.allocator.create(Creation);
        errdefer aggregate.allocator.destroy(creation);
        const build = try aggregate.beginStartBuild(preparation);
        creation.* = initCreation(aggregate.allocator, request, .{ .start = build });
        creation.launch = creation.work.start.processLaunch();
        creation.options = creation.work.start.runtimeOptions();
        creation.leader_start = .pin(request.keepers, creation.work.identity(), request.host_epoch);
        self.launchCreator(creation) catch {
            creation.leader_start.unpin();
            preparation.* = .{ .pending = creation.work.start.pending };
            return error.CreatorUnavailable;
        };
        self.in_progress = creation;
    }

    /// Starts restoring the terminal `job` builds from a handover record. On error the job is
    /// unchanged and the caller still owns `request.reply`.
    pub fn beginRestore(self: *Pipeline, aggregate: *Aggregate, request: Request, job: *restore_build.Job) BeginError!void {
        if (self.in_progress != null) return error.CreationInProgress;
        const creation = try aggregate.allocator.create(Creation);
        errdefer aggregate.allocator.destroy(creation);
        creation.* = initCreation(aggregate.allocator, request, .{ .restore = job });
        self.launchCreator(creation) catch return error.CreatorUnavailable;
        self.in_progress = creation;
    }

    /// Settles the creation once its build has returned. `session` is the active connection: a
    /// creation it requested is answered there, and one whose requester left settles unanswered
    /// from any caller, maintenance without a session included.
    pub fn settle(
        self: *Pipeline,
        aggregate: *Aggregate,
        session: ?core_module.ConnectionId,
    ) ?Reply {
        const creation = self.in_progress orelse return null;
        const answer = creation.answerable and session != null and session.? == creation.connection_id;
        if (creation.answerable and !answer) return null;
        const outcome = creation.takeOutcome() orelse return null;
        creation.creator.join();
        self.in_progress = null;
        defer aggregate.allocator.destroy(creation);
        var reply = settleOutcome(aggregate, creation, outcome);
        if (answer) return reply;
        reply.deinit();
        return null;
    }

    /// Waits up to `timeout_ns` for the build in progress to return.
    pub fn awaitBuild(self: *Pipeline, timeout_ns: u64) void {
        const creation = self.in_progress orelse return monotonic.sleep(timeout_ns);
        creation.mutex.lock();
        defer creation.mutex.unlock();
        if (creation.outcome == null) creation.finished.timedWait(&creation.mutex, timeout_ns) catch {};
    }

    /// Holds `record` when it depends on the creation in progress or on a request already held,
    /// and returns whether it did.
    pub fn holdIfDependent(
        self: *Pipeline,
        allocator: std.mem.Allocator,
        record: []const u8,
    ) HoldError!bool {
        return self.holds.holdIfDependent(allocator, self.creatingTerminal(), record);
    }

    /// The first held request that nothing before it holds any longer. The caller owns the
    /// record and dispatches it without holding it again.
    pub fn takeReleased(self: *Pipeline) ?[]u8 {
        return self.holds.takeReleased(self.creatingTerminal());
    }

    /// The session of `connection_id` ended: its creation settles unanswered, and the requests it
    /// held are dropped unanswered.
    pub fn detach(
        self: *Pipeline,
        allocator: std.mem.Allocator,
        connection_id: core_module.ConnectionId,
    ) void {
        if (self.in_progress) |creation| {
            if (creation.connection_id == connection_id) creation.answerable = false;
        }
        self.holds.drop(allocator);
    }

    /// Host teardown, before the aggregate's: waits up to `teardown_build_wait_ns` for a build in
    /// progress. A runtime it built is committed unanswered, so the aggregate's teardown contains
    /// it, and keeps it under THDD-001 while its process tree stays unconfirmed; a failed build
    /// releases what its creation acquired. A build that never returns is abandoned.
    pub fn shutdown(self: *Pipeline, aggregate: *Aggregate) void {
        self.holds.deinit(aggregate.allocator);
        const creation = self.in_progress orelse return;
        self.in_progress = null;
        creation.reply.deinit();
        const outcome = awaitOutcome(creation, teardown_build_wait_ns) orelse
            return self.abandon(creation);
        creation.creator.join();
        defer aggregate.allocator.destroy(creation);
        switch (outcome) {
            .built => |runtime| commitAtTeardown(aggregate, creation, runtime),
            .failed => switch (creation.work) {
                .create => |*build| aggregate.abandonCreateBuild(build),
                .start => {},
                .restore => |job| job.abandon(aggregate),
            },
        }
    }

    /// THDD-001: a build that never returned may own a child the Host cannot reach, and its thread
    /// still reads the creation and the launch it was admitted with. The thread is detached and
    /// keeps both, never freed: a new terminal's launch stays with its creation, and a start's
    /// launch leaves its awaiting record, which the aggregate's teardown then frees without it.
    /// The Host's teardown fails once every other terminal is contained.
    fn abandon(self: *Pipeline, creation: *Creation) void {
        creation.creator.detach();
        switch (creation.work) {
            .create, .restore => {},
            .start => |*build| build.retained.start_preparation = null,
        }
        self.abandoned_build = true;
        std.log.err(
            "terminal Host abandoned a terminal creation whose build did not return within {d} s; its process may outlive the Host",
            .{teardown_build_wait_ns / std.time.ns_per_s},
        );
    }

    fn creatingTerminal(self: *const Pipeline) ?core_module.TerminalId {
        const creation = self.in_progress orelse return null;
        return creation.work.terminalId();
    }

    fn launchCreator(self: *Pipeline, creation: *Creation) !void {
        if (comptime target_builtin.is_test) creation.build_gate = self.build_gate;
        creation.creator = try std.Thread.spawn(.{}, runBuild, .{creation});
    }
};

fn initCreation(allocator: std.mem.Allocator, request: Request, work: Work) Creation {
    return .{
        .allocator = allocator,
        .connection_id = request.connection_id,
        .correlation_id = request.correlation_id,
        .host_epoch = request.host_epoch,
        .reply = request.reply,
        .work = work,
    };
}

/// The creator thread. It holds no Host lock: the build spawns the child, and on POSIX nothing
/// this thread holds may be held across that fork.
fn runBuild(creation: *Creation) void {
    fatal_record.markThread(.@"terminal-creation");
    if (comptime target_builtin.is_test) {
        if (creation.build_gate) |gate| gate.pass();
    }
    creation.leader_start.apply(&creation.launch);
    const built = switch (creation.work) {
        .restore => |job| job.run(),
        .create, .start => TerminalRuntime.createWithOptions(creation.allocator, creation.launch, creation.options),
    };
    creation.leader_start.unpin();
    const outcome: BuildOutcome = if (built) |runtime| .{ .built = runtime } else |err| .{ .failed = err };
    creation.mutex.lock();
    creation.outcome = outcome;
    creation.finished.broadcast();
    creation.mutex.unlock();
    output_wake.signal();
}

fn settleOutcome(aggregate: *Aggregate, creation: *Creation, outcome: BuildOutcome) Reply {
    switch (outcome) {
        .built => |runtime| switch (creation.work) {
            .create => |*create_build| {
                _ = aggregate.commitCreateBuild(create_build, runtime);
                return .{ .records = creation.reply };
            },
            .start => |*start_build| {
                if (aggregate.commitStartBuild(start_build, runtime)) |_| {
                    return .{ .records = creation.reply };
                } else |_| {
                    // Requests naming the terminal were held, so its launch cannot have gone;
                    // answered rather than asserted, since nothing is half-built yet.
                    std.log.err("terminal start found its awaiting terminal gone after building", .{});
                    runtime.destroy();
                    creation.reply.deinit();
                    return failureReply(creation, creation_failure.details(.missing_launch));
                }
            },
            .restore => |job| {
                job.commit(aggregate, runtime);
                return .{ .records = creation.reply };
            },
        },
        .failed => |err| {
            creation.reply.deinit();
            switch (creation.work) {
                .restore => |job| {
                    const failure = job.failure();
                    job.abandon(aggregate);
                    return failureReply(creation, failure);
                },
                .create => |*create_build| aggregate.abandonCreateBuild(create_build),
                .start => {},
            }
            std.log.warn(
                "terminal creation failed to build its process and engine: {s}",
                .{@errorName(err)},
            );
            return failureReply(creation, creation_failure.details(if (err == error.OutOfMemory) .allocation else .spawn));
        },
    }
}

fn failureReply(creation: *const Creation, failure: host_response.FailureDetails) Reply {
    return .{ .failure = creation_failure.encode(failure, creation.correlation_id, creation.host_epoch) };
}

/// The build's outcome once it returns within `timeout_ns`, or null.
fn awaitOutcome(creation: *Creation, timeout_ns: u64) ?BuildOutcome {
    var timer = monotonic.Timer.start() catch null;
    var waited_ns: u64 = 0;
    creation.mutex.lock();
    defer creation.mutex.unlock();
    while (creation.outcome == null) {
        const elapsed_ns = if (timer) |*value| value.read() else waited_ns;
        if (elapsed_ns >= timeout_ns) return null;
        const slice_ns = @min(std.time.ns_per_s, timeout_ns - elapsed_ns);
        creation.finished.timedWait(&creation.mutex, slice_ns) catch {};
        waited_ns += slice_ns;
    }
    return creation.outcome;
}

/// Commits a build that teardown waited for, unanswered, so the aggregate's teardown owns its
/// runtime like every other terminal's.
fn commitAtTeardown(aggregate: *Aggregate, creation: *Creation, runtime: *TerminalRuntime) void {
    switch (creation.work) {
        .create => |*build| _ = aggregate.commitCreateBuild(build, runtime),
        .start => |*build| _ = aggregate.commitStartBuild(build, runtime) catch {
            // Requests naming the terminal were held, so its record cannot have gone.
            std.log.err("terminal start found its awaiting terminal gone at teardown", .{});
            runtime.destroy();
        },
        .restore => |job| job.discardBuilt(aggregate, runtime),
    }
}

pub const testing = if (target_builtin.is_test) struct {
    pub const Gate = BuildGate;

    /// Makes every later build wait at `gate` before it spawns anything.
    pub fn gateBuilds(pipeline: *Pipeline, gate: ?*BuildGate) void {
        pipeline.build_gate = gate;
    }

    /// Waits for the build in progress, then settles it as `settle` would.
    pub fn settleWhenBuilt(
        pipeline: *Pipeline,
        aggregate: *Aggregate,
        session: ?core_module.ConnectionId,
    ) ?Reply {
        const creation = pipeline.in_progress orelse return null;
        while (creation.takeOutcome() == null) pipeline.awaitBuild(std.time.ns_per_s);
        return pipeline.settle(aggregate, session);
    }
} else struct {};
