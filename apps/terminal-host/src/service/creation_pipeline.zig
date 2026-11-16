//! Builds a terminal's process and engine on a creator thread, so a slow start never holds the
//! dispatch thread, and holds the requests that depend on that creation until it settles.
//!
//! A creation passes three stages. The dispatch thread admits it, takes the launch it was
//! admitted with, and encodes its reply; a creator thread builds the runtime, which spawns the
//! child; the dispatch thread then commits the runtime and sends the reply, or sends the build's
//! failure. One creation is in progress at a time. Until it settles, every request that names
//! the terminal it creates or starts, every other creation, and every request that changes what
//! a creation is admitted with is held in arrival order; each is dispatched once nothing before
//! it holds it, against the state the creation left, so no request acts on a half-built
//! terminal. A requester that disconnects leaves its creation to commit unanswered, as a start
//! on the dispatch thread would have, and drops the requests it had held. Host teardown cancels
//! the creation, which releases only what the creation itself acquired: the runtime its build
//! made and, for a new terminal, the launch and record admitted for it. A later stage that
//! imports a terminal instead of spawning one joins at the build and keeps the same commit and
//! cancellation contract.
const std = @import("std");
const target_builtin = @import("builtin");

const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const platform = @import("terminal-host-platform");
const fatal_record = platform.fatal_record;
const monotonic = platform.monotonic;
const output_wake = platform.output_wake;
const pty_process = platform.process;
const thread_sync = platform.thread_sync;
const terminal_runtime = @import("terminal-host-engine").runtime;
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const host_outgoing = @import("outgoing.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");

const Aggregate = host_runtime_aggregate.HostRuntimeAggregate;
const Outgoing = host_outgoing.Outgoing;
const TerminalRuntime = terminal_runtime.TerminalRuntime;
const WireIdentifier = identifiers.IdentifierBytes;

/// Requests held behind a creation before the session stops reading more of them.
pub const held_request_limit = 16;

/// Teardown retries containing a cancelled build's process tree for about one second.
const cancel_cleanup_attempts = 20;
const cancel_cleanup_backoff_ns = 50 * std.time.ns_per_ms;

const failure_record_bytes = protocol.transport_envelope_header_bytes + 512;

pub const BeginError = error{
    CreationInProgress,
    CreatorUnavailable,
    OutOfMemory,
    RuntimeAllocationOutOfMemory,
    TerminalRuntimeMissing,
};

pub const HoldError = error{ HeldRequestLimit, OutOfMemory };

/// The admitted request a creation answers.
pub const Request = struct {
    connection_id: core_module.ConnectionId,
    correlation_id: WireIdentifier,
    host_epoch: identifiers.HostEpoch,
    /// The success response and `TerminalStarted`. Ownership moves only when the begin succeeds.
    reply: Outgoing,
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

/// A complete typed-failure record, encoded without allocation.
pub const FailureRecord = struct {
    storage: [failure_record_bytes]u8,
    len: usize,

    pub fn bytes(self: *const FailureRecord) []const u8 {
        return self.storage[0..self.len];
    }
};

const Work = union(enum) {
    create: host_runtime_aggregate.CreateBuild,
    start: host_runtime_aggregate.StartBuild,

    fn terminalId(self: *const Work) core_module.TerminalId {
        return switch (self.*) {
            .create => |*build| build.pending.terminal_id,
            .start => |*build| build.pending.identity.terminal_id,
        };
    }
};

const BuildOutcome = union(enum) {
    built: *TerminalRuntime,
    failed: anyerror,
};

const BuildFailure = enum { allocation, spawn, missing_launch };

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

/// What a request depends on, from its kind and the terminal its payload names.
const Dependence = struct {
    /// It creates or starts a terminal, or changes what a later creation is admitted with.
    creation: bool = false,
    terminal: ?core_module.TerminalId = null,
};

const HeldRequest = struct {
    record: []u8,
    dependence: Dependence,
};

/// Owned by the dispatch thread, like the rest of the Host service.
pub const Pipeline = struct {
    in_progress: ?*Creation = null,
    held: std.ArrayList(HeldRequest) = .empty,
    build_gate: if (target_builtin.is_test) ?*BuildGate else void =
        if (target_builtin.is_test) null else {},

    pub fn creating(self: *const Pipeline) bool {
        return self.in_progress != null;
    }

    /// Whether the session may read another request: false while the held-request limit is
    /// reached, until the creation they wait for settles.
    pub fn acceptsRequests(self: *const Pipeline) bool {
        return self.held.items.len < held_request_limit;
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
        self.launchCreator(creation) catch {
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
        self.launchCreator(creation) catch {
            preparation.* = .{ .pending = creation.work.start.pending };
            return error.CreatorUnavailable;
        };
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
        if (self.in_progress == null and self.held.items.len == 0) return false;
        const dependence = dependenceOf(allocator, record);
        if (!self.dependsOnEarlier(dependence, self.held.items)) return false;
        if (self.held.items.len >= held_request_limit) return error.HeldRequestLimit;
        try self.held.ensureUnusedCapacity(allocator, 1);
        const copy = try allocator.dupe(u8, record);
        self.held.appendAssumeCapacity(.{ .record = copy, .dependence = dependence });
        return true;
    }

    /// The first held request that nothing before it holds any longer. The caller owns the
    /// record and dispatches it without holding it again.
    pub fn takeReleased(self: *Pipeline) ?[]u8 {
        for (self.held.items, 0..) |held, index| {
            if (self.dependsOnEarlier(held.dependence, self.held.items[0..index])) continue;
            _ = self.held.orderedRemove(index);
            return held.record;
        }
        return null;
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
        self.dropHeld(allocator);
    }

    /// Host teardown: waits for a build in progress to return and cancels its creation.
    pub fn shutdown(self: *Pipeline, aggregate: *Aggregate) void {
        self.dropHeld(aggregate.allocator);
        self.held.deinit(aggregate.allocator);
        self.held = .empty;
        const creation = self.in_progress orelse return;
        creation.creator.join();
        self.in_progress = null;
        defer aggregate.allocator.destroy(creation);
        creation.reply.deinit();
        switch (creation.outcome.?) {
            .built => |runtime| cancelRuntime(runtime),
            .failed => {},
        }
        switch (creation.work) {
            .create => |*build| aggregate.abandonCreateBuild(build),
            .start => {},
        }
    }

    fn launchCreator(self: *Pipeline, creation: *Creation) !void {
        if (comptime target_builtin.is_test) creation.build_gate = self.build_gate;
        creation.creator = try std.Thread.spawn(.{}, runBuild, .{creation});
    }

    fn dependsOnEarlier(
        self: *const Pipeline,
        dependence: Dependence,
        earlier: []const HeldRequest,
    ) bool {
        if (self.in_progress) |creation| {
            if (dependence.creation) return true;
            if (dependence.terminal) |terminal_id| {
                if (terminal_id.eql(creation.work.terminalId())) return true;
            }
        }
        for (earlier) |held| {
            if (dependence.creation and held.dependence.creation) return true;
            const terminal_id = dependence.terminal orelse continue;
            const held_terminal_id = held.dependence.terminal orelse continue;
            if (terminal_id.eql(held_terminal_id)) return true;
        }
        return false;
    }

    fn dropHeld(self: *Pipeline, allocator: std.mem.Allocator) void {
        for (self.held.items) |held| allocator.free(held.record);
        self.held.clearRetainingCapacity();
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
    const outcome: BuildOutcome = if (TerminalRuntime.createWithOptions(
        creation.allocator,
        creation.launch,
        creation.options,
    )) |runtime| .{ .built = runtime } else |err| .{ .failed = err };
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
                    return failureReply(creation, .missing_launch);
                }
            },
        },
        .failed => |err| {
            std.log.warn(
                "terminal creation failed to build its process and engine: {s}",
                .{@errorName(err)},
            );
            switch (creation.work) {
                .create => |*create_build| aggregate.abandonCreateBuild(create_build),
                .start => {},
            }
            creation.reply.deinit();
            return failureReply(creation, if (err == error.OutOfMemory) .allocation else .spawn);
        },
    }
}

fn failureReply(creation: *const Creation, failure: BuildFailure) Reply {
    const code: []const u8, const message: []const u8, const retryable = switch (failure) {
        .allocation => .{ "resource-exhausted", "Host could not allocate terminal runtime state", true },
        .spawn => .{ "spawn-failed", "PTY or child creation failed", true },
        .missing_launch => .{ "internal-error", "deferred terminal launch state is missing", false },
    };
    var record: FailureRecord = .{ .storage = undefined, .len = 0 };
    var payload: std.Io.Writer = .fixed(record.storage[protocol.transport_envelope_header_bytes..]);
    // The Host's own fixed failures always fit the record.
    control.encode.writeTypedFailure(&payload, code, message, retryable) catch unreachable;
    record.len = (protocol.finalizeTransportEnvelope(
        &record.storage,
        protocol.MessageKind.typed_failure,
        .control_json,
        payload.buffered().len,
        creation.correlation_id,
        creation.host_epoch.bytes,
        true,
        false,
    ) catch unreachable).len;
    return .{ .failure = record };
}

/// Contains and frees a cancelled build's runtime. A process tree that stays unconfirmed after
/// the retries is logged and its runtime retained, never freed under its workers.
fn cancelRuntime(runtime: *TerminalRuntime) void {
    var attempt: usize = 1;
    while (true) : (attempt += 1) {
        runtime.shutdown() catch |err| {
            if (attempt < cancel_cleanup_attempts) {
                monotonic.sleep(cancel_cleanup_backoff_ns);
                continue;
            }
            std.log.err(
                "retaining a cancelled terminal build whose process tree is not contained: {s}",
                .{@errorName(err)},
            );
            return;
        };
        runtime.destroyAfterShutdown();
        return;
    }
}

fn dependenceOf(allocator: std.mem.Allocator, record: []const u8) Dependence {
    const envelope = protocol.validateTransportEnvelope(record) catch return .{};
    if (envelope.event or envelope.response) return .{};
    const contract = protocol.messageContract(envelope.kind) orelse return .{};
    const request = contract.request orelse return .{};
    return switch (request.operation) {
        .create_terminal, .stage_terminal_output, .set_terminal_defaults => .{ .creation = true },
        .start_terminal => .{
            .creation = true,
            .terminal = control.decode.namedTerminal(allocator, envelope.payload),
        },
        .terminate_terminal,
        .force_terminate_terminal,
        .dispose_terminal,
        .attach_terminal,
        .update_terminal_configuration,
        .apply_terminal_theme,
        => .{ .terminal = control.decode.namedTerminal(allocator, envelope.payload) },
        // Attachment-scoped requests cannot name a creation: no attachment admits a terminal
        // that is not yet running.
        else => .{},
    };
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
