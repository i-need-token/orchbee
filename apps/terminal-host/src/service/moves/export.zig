//! The old Host's side of a move: `QuiesceTerminal` with its export, `ReadHandoverRecord`,
//! `ResumeTerminal`, `DropTerminal`, and the mark's deadline. A Host keeps at most one export. The
//! dispatch thread takes every step; a worker thread parks the reader and encodes the record, and
//! the Keeper link makes the Keeper requests, so the dispatch thread waits on neither.
//!
//! The export comes before the mark: until the mark applies, a failure discards the record and the
//! reader reads on. After it only the Keeper's `resumed` restarts the reader, unless the Keeper
//! that holds the mark is lost or missed a deadline and the Worker resumes without it; a mark or a
//! resume left unanswered leaves the terminal stopped.

const std = @import("std");
const builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const control = @import("terminal-host-control");
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const terminal_engine = @import("terminal-host-engine");
const answers_module = @import("answers.zig");
const refusal = @import("refusal.zig");
const host_failure = @import("../failure.zig");

const Env = @import("env.zig").Env;
const Pending = answers_module.Pending;
const Keeper = keeper_link.Keeper;
const Request = keeper_link.moves.Request;
const Result = keeper_link.moves.Result;
const TerminalRuntime = terminal_engine.runtime.TerminalRuntime;
const ConsoleFacts = terminal_engine.handover.ConsoleFacts;
const MessageKind = protocol.MessageKind;
const handover_encode = control.encode.handover;

pub const Phase = enum {
    /// The worker parks the reader and encodes the record.
    parking,
    /// The mark is with the Keeper link.
    marking,
    /// Marked; the record waits for its reads until the deadline.
    marked,
    /// A `resume` is with the Keeper link.
    resuming,
    /// Stopped by a mark whose fate the Host does not know: a mark or a resume went unanswered, or
    /// the deadline's resume got another answer. Only the Worker settles it.
    stopped,
    /// The Keeper answered `claimed`; the terminal waits for `DropTerminal`.
    moved,
    /// The drop waits until no link touches the terminal's gate.
    dropping,
    /// A settled terminal's record, which needs no mark.
    settled,
};

const WorkerResult = union(enum) { record: []u8, refused: refusal.Details };

pub const Export = struct {
    allocator: std.mem.Allocator,
    terminal_id: core_module.TerminalId,
    runtime: *TerminalRuntime,
    settled: bool,
    phase: Phase,
    holding: [16]u8 = @splat(0),
    mark: [16]u8 = @splat(0),
    /// The Keeper that holds the terminal and its mark; null for a settled export.
    keeper: ?Keeper = null,
    target: Keeper,
    deadline_ns: u64,
    deadline_at: ?u64 = null,
    inputs: terminal_engine.runtime.HandoverInputs = undefined,
    record: ?[]u8 = null,
    worker: ?std.Thread = null,
    mutex: platform.thread_sync.Mutex = .{},
    worker_result: ?WorkerResult = null,
    /// One Keeper request at a time; the link owns it while `requesting`.
    request: Request = undefined,
    requesting: bool = false,
    /// The `QuiesceTerminal` that waits for its answer.
    quiesce: ?Pending,
    /// The `ResumeTerminal` or `DropTerminal` that waits for its answer.
    asked: ?Pending = null,

    pub fn gate(self: *Export) *platform.keeper.Gate {
        return platform.keeper.gate(self.runtime.keeperSubject()).?;
    }

    fn freeRecord(self: *Export) void {
        if (self.record) |bytes| self.allocator.free(bytes);
        self.record = null;
    }
};

/// Quiesce steps 1, 2, and the mint of step 6, on the dispatch thread; the worker takes 3 to 6.
pub fn quiesce(env: Env, slot: *?*Export, to: Pending, request: control.decode.handover.Quiesce) void {
    if (slot.* != null) return env.answers.failure(to, refusal.moving);
    const session = env.aggregate.coreRead().findTerminal(request.terminal_id) orelse
        return env.answers.failure(to, host_failure.fromCore(error.MissingResource));
    if (session.handover != .none) return env.answers.failure(to, refusal.moving);
    const settled = switch (session.lifecycle) {
        .running => |running| if (running.termination == .none) false else null,
        .exited => |exited| if (exited.publication == .published) true else null,
        .failed => |failed| if (failed.publication == .published) true else null,
        .awaiting_start, .finishing, .containing_failure => null,
    } orelse return env.answers.failure(to, refusal.ineligible("not-running"));
    const runtime = env.aggregate.runtimeFor(request.terminal_id) orelse
        return env.answers.failure(to, refusal.ineligible("not-running"));
    const created = env.allocator.create(Export) catch return env.answers.failure(to, refusal.exhausted);
    created.* = .{
        .allocator = env.allocator,
        .terminal_id = request.terminal_id,
        .runtime = runtime,
        .settled = settled,
        .phase = if (settled) .settled else .parking,
        .target = .{ .pid = request.target.pid, .created = request.target.created },
        .deadline_ns = @as(u64, request.deadline_seconds) * std.time.ns_per_s,
        .quiesce = to,
    };
    var console: ?ConsoleFacts = null;
    if (settled) {
        // A settled terminal changes and leaves only once its record is encoded.
        env.aggregate.setHandover(request.terminal_id, .none, .exporting);
    } else if (prepareRunning(env, created, &console)) |details| {
        env.allocator.destroy(created);
        return env.answers.failure(to, details);
    }
    created.inputs = env.aggregate.exportInputs(request.terminal_id, if (settled) .{} else .{
        .holding = created.holding,
        .mark = created.mark,
    }, console);
    created.worker = std.Thread.spawn(platform.thread_stack.default_budget, work, .{created}) catch {
        rollback(env, created);
        env.allocator.destroy(created);
        return env.answers.failure(to, refusal.exhausted);
    };
    slot.* = created;
}

/// Step 1's checks and step 2: from here the terminal refuses input, resizes, terminal control,
/// and termination, and nothing ends it. Returns the refusal, with nothing changed. A macOS
/// record carries no console: the Keeper's facts name the leader the new Host checks.
fn prepareRunning(env: Env, created: *Export, console: *?ConsoleFacts) ?refusal.Details {
    const process = created.runtime.keeperSubject();
    if (comptime builtin.os.tag == .windows) {
        if (platform.keeper.hold.onSystemConsole(process)) return refusal.ineligible("system-console");
    }
    switch (env.attachments.count()) {
        0 => return refusal.ineligible("unheld"),
        1 => {},
        else => return refusal.ineligible("two-keepers"),
    }
    const gate = created.gate();
    created.keeper = env.attachments.holder(gate) orelse return refusal.ineligible("unheld");
    created.holding = gate.heldId() orelse return refusal.ineligible("unheld");
    platform.thread_sync.io().random(&created.mark);
    created.runtime.beginMove() catch return refusal.ineligible("not-running");
    if (comptime builtin.os.tag != .windows) {
        env.aggregate.setHandover(created.terminal_id, .none, .quiesced);
        return null;
    }
    // The move holds the terminal, so its handles stay open while the facts are read.
    const facts = platform.keeper.hold.facts(process) catch |err| {
        created.runtime.endMove();
        return switch (err) {
            error.SystemConsole => refusal.ineligible("system-console"),
            error.HoldFailed => refusal.failed("handle-failed"),
        };
    };
    const own = platform.keeper.builds.own;
    console.* = .{
        .conpty_version = platform.keeper.builds.own_version,
        .open_console_sha256 = own.console_sha256,
        .signal_format = own.signal_format,
        .program = .{ .pid = facts.program.pid, .created = facts.program.created },
        .console = .{ .pid = facts.console.pid, .created = facts.console.created },
    };
    env.aggregate.setHandover(created.terminal_id, .none, .quiesced);
    return null;
}

/// Quiesce steps 3 to 6, off the dispatch thread: park at ground, then encode.
fn work(self: *Export) void {
    const result: WorkerResult = result: {
        if (!self.settled) {
            const parked = self.runtime.parkForMove();
            if (parked != .parked) break :result .{ .refused = refusal.park(parked) };
        }
        const record = if (self.settled)
            self.runtime.exportSettledHandover(self.allocator, self.inputs)
        else
            self.runtime.exportHandover(self.allocator, self.inputs);
        break :result if (record) |bytes| .{ .record = bytes } else |err| .{ .refused = refusal.exported(err) };
    };
    self.mutex.lock();
    self.worker_result = result;
    self.mutex.unlock();
    platform.output_wake.signal();
}

/// The reader reads on and nothing of the quiesce remains; a settled terminal may change again.
pub fn rollback(env: Env, self: *Export) void {
    if (self.settled) return env.aggregate.setHandover(self.terminal_id, .exporting, .none);
    self.runtime.endMove();
    env.aggregate.setHandover(self.terminal_id, .quiesced, .none);
}

/// Advances the export: the worker's result, a Keeper answer, the deadline, a drop that may go
/// on, or a settled terminal that left. Frees the export once it ends.
pub fn advance(env: Env, slot: *?*Export) void {
    const self = slot.* orelse return;
    if (self.worker != null) {
        if (takeWorkerResult(self) == null) return;
        return settleWorker(env, slot, joinWorker(self));
    }
    if (self.requesting) {
        const result = self.request.poll() orelse return;
        self.requesting = false;
        return settleRequest(env, slot, result);
    }
    switch (self.phase) {
        .settled => if (env.aggregate.runtimeFor(self.terminal_id) == null or expired(env, self)) finish(env, slot),
        .marked => if (expired(env, self)) {
            // The deadline: ask the Keeper to resume.
            self.freeRecord();
            startResume(env, self, null);
        },
        .dropping => if (self.gate().idle()) drop(env, slot),
        .parking, .marking, .resuming, .stopped, .moved => {},
    }
}

pub fn settleWorker(env: Env, slot: *?*Export, result: WorkerResult) void {
    const self = slot.*.?;
    switch (result) {
        .refused => |details| {
            rollback(env, self);
            env.answers.failure(self.quiesce, details);
            finish(env, slot);
        },
        .record => |bytes| {
            self.record = bytes;
            if (self.settled) {
                env.aggregate.setHandover(self.terminal_id, .exporting, .none);
                return answerQuiesced(env, self);
            }
            // Step 7: mark the holding for the new Host.
            self.phase = .marking;
            submit(env, self, .{ .mark = .{ .holding = self.holding, .target = self.target, .mark = self.mark } });
        },
    }
}

pub fn settleRequest(env: Env, slot: *?*Export, result: Result) void {
    const self = slot.*.?;
    switch (self.phase) {
        .marking => switch (result) {
            .done => {
                self.phase = .marked;
                answerQuiesced(env, self);
            },
            // Nothing applied: discard the export and read on without the Keeper.
            .refused, .unsupported, .failed => {
                rollback(env, self);
                env.answers.failure(self.quiesce, refusal.keeper(result));
                finish(env, slot);
            },
            // Possibly applied: the terminal stays stopped.
            else => {
                self.phase = .stopped;
                self.freeRecord();
                env.answers.failure(self.quiesce, refusal.keeper(result));
                self.quiesce = null;
            },
        },
        .resuming => settleResume(env, slot, result),
        .dropping => if (self.gate().idle()) drop(env, slot),
        .parking, .marked, .stopped, .moved, .settled => unreachable,
    }
}

fn answerQuiesced(env: Env, self: *Export) void {
    self.deadline_at = env.now_ns +| self.deadline_ns;
    const bytes: u32 = @intCast(self.record.?.len);
    const payload = handover_encode.quiesced(env.allocator, self.terminal_id, bytes) catch
        return env.answers.failure(self.quiesce, refusal.exhausted);
    env.answers.success(self.quiesce, MessageKind.quiesce_terminal, .control_json, payload);
    self.quiesce = null;
}

fn expired(env: Env, self: *const Export) bool {
    const at = self.deadline_at orelse return false;
    return env.now_ns >= at;
}

/// `ReadHandoverRecord`: up to 4 MiB of the export's record from `offset`.
pub fn read(env: Env, slot: *?*Export, to: Pending, request: control.decode.handover.ReadRecord) void {
    const self = slot.* orelse return env.answers.failure(to, refusal.invalid_phase);
    if (!self.terminal_id.eql(request.terminal_id)) return env.answers.failure(to, refusal.invalid_phase);
    const record = self.record orelse return env.answers.failure(to, refusal.invalid_phase);
    if (request.offset >= record.len) return env.answers.failure(to, refusal.invalid_chunk);
    const end = @min(record.len, @as(usize, request.offset) + control.decode.handover.max_chunk_bytes);
    const payload = handover_encode.recordChunk(env.allocator, self.terminal_id.bytes, request.offset, record[request.offset..end]) catch
        return env.answers.failure(to, refusal.exhausted);
    env.answers.success(to, MessageKind.handover_record_chunk, .handover_record, payload);
}

/// `ResumeTerminal`: through the Keeper that holds the mark while it answers; without it once it
/// is lost or missed a deadline, which also resumes a moved terminal.
pub fn resumeTerminal(env: Env, slot: *?*Export, to: Pending, terminal_id: core_module.TerminalId) void {
    const self = exportOf(slot, terminal_id) orelse {
        const details = notMoving(env, terminal_id) orelse return answerResumed(env, to, terminal_id, .resumed);
        return env.answers.failure(to, details);
    };
    switch (self.phase) {
        .parking, .marking, .resuming, .dropping => env.answers.failure(to, refusal.moving),
        .settled => if (self.worker != null) env.answers.failure(to, refusal.moving) else {
            answerResumed(env, to, terminal_id, .resumed);
            finish(env, slot);
        },
        .marked, .stopped => if (keeperAnswers(env, self)) {
            self.freeRecord();
            startResume(env, self, to);
        } else resumeWithoutKeeper(env, slot, to, .quiesced),
        .moved => if (keeperAnswers(env, self))
            answerResumed(env, to, terminal_id, .claimed_elsewhere)
        else
            resumeWithoutKeeper(env, slot, to, .moved),
    }
}

fn exportOf(slot: *?*Export, terminal_id: core_module.TerminalId) ?*Export {
    const self = slot.* orelse return null;
    return if (self.terminal_id.eql(terminal_id)) self else null;
}

/// Why a terminal that is not moving refuses a resume or a drop; null when it reads already.
fn notMoving(env: Env, terminal_id: core_module.TerminalId) ?refusal.Details {
    const session = env.aggregate.coreRead().findTerminal(terminal_id) orelse
        return host_failure.fromCore(error.MissingResource);
    return if (session.handover == .none) null else refusal.invalid_phase;
}

fn answerResumed(env: Env, to: ?Pending, terminal_id: core_module.TerminalId, outcome: handover_encode.ResumeOutcome) void {
    const payload = handover_encode.resumed(env.allocator, terminal_id, outcome) catch
        return env.answers.failure(to, refusal.exhausted);
    env.answers.success(to, MessageKind.resume_terminal, .control_json, payload);
}

pub fn keeperAnswers(env: Env, self: *const Export) bool {
    return env.attachments.answers(self.keeper.?);
}

fn startResume(env: Env, self: *Export, to: ?Pending) void {
    self.phase = .resuming;
    self.asked = to;
    submit(env, self, .{ .@"resume" = .{ .gate = self.gate(), .marked = .{ .holding = self.holding, .mark = self.mark } } });
}

fn settleResume(env: Env, slot: *?*Export, result: Result) void {
    const self = slot.*.?;
    if (result == .outcome) switch (result.outcome) {
        .resumed => {
            rollback(env, self);
            answerResumed(env, self.asked, self.terminal_id, .resumed);
            return finish(env, slot);
        },
        .claimed => {
            self.phase = .moved;
            env.aggregate.setHandover(self.terminal_id, .quiesced, .moved);
            answerResumed(env, self.asked, self.terminal_id, .claimed_elsewhere);
            self.asked = null;
            return;
        },
        .declined, .unknown => {},
    };
    // Any other answer counts as unanswered.
    self.phase = .stopped;
    env.answers.failure(self.asked, refusal.keeper(if (result == .outcome) .unanswered else result));
    self.asked = null;
}

fn resumeWithoutKeeper(env: Env, slot: *?*Export, to: Pending, from: core_module.TerminalHandover) void {
    const self = slot.*.?;
    self.runtime.endMove();
    env.aggregate.setHandover(self.terminal_id, from, .none);
    answerResumed(env, to, self.terminal_id, .resumed);
    finish(env, slot);
}

/// `DropTerminal`: a moved terminal, or a quiesced one whose Keeper is lost or missed a deadline,
/// once no attachment names it.
pub fn dropTerminal(env: Env, slot: *?*Export, to: Pending, terminal_id: core_module.TerminalId) void {
    const self = exportOf(slot, terminal_id) orelse
        return env.answers.failure(to, notMoving(env, terminal_id) orelse refusal.invalid_phase);
    switch (self.phase) {
        .moved => {},
        .marked, .stopped => if (keeperAnswers(env, self)) return env.answers.failure(to, refusal.moving),
        .parking, .marking, .resuming, .dropping => return env.answers.failure(to, refusal.moving),
        .settled => return env.answers.failure(to, refusal.invalid_phase),
    }
    if (env.aggregate.coreRead().attachmentCountForTerminal(terminal_id) != 0)
        return env.answers.failure(to, refusal.invalid_phase);
    self.asked = to;
    self.freeRecord();
    self.phase = .dropping;
    // A slot still held belongs to a Keeper that missed a deadline: the link forgets it, sending
    // nothing, since a release would let the console end once this Host's handles close.
    if (self.gate().heldIndex() != null) return submit(env, self, .{ .forget = self.gate() });
    if (self.gate().idle()) drop(env, slot);
}

fn drop(env: Env, slot: *?*Export) void {
    const self = slot.*.?;
    env.aggregate.removeMoving(self.terminal_id) catch |err| {
        // An attachment came meanwhile: the terminal stays as it was, unheld.
        const moved = env.aggregate.coreRead().findTerminal(self.terminal_id).?.handover == .moved;
        self.phase = if (moved) .moved else .stopped;
        env.answers.failure(self.asked, host_failure.fromCore(err));
        self.asked = null;
        return;
    };
    const payload = handover_encode.terminalNamed(env.allocator, self.terminal_id) catch null;
    if (payload) |bytes| env.answers.success(self.asked, MessageKind.drop_terminal, .control_json, bytes) else env.answers.failure(self.asked, refusal.exhausted);
    self.asked = null;
    finish(env, slot);
}

/// Whether the Keeper `keeper` holds this export's mark, so `DetachKeeper` waits.
pub fn holdsMark(slot: *const ?*Export, keeper: Keeper) bool {
    const self = slot.* orelse return false;
    const held = self.keeper orelse return false;
    if (!held.eql(keeper)) return false;
    return switch (self.phase) {
        .parking, .marking, .marked, .resuming, .stopped => true,
        .moved, .dropping, .settled => false,
    };
}

fn submit(env: Env, self: *Export, body: keeper_link.moves.Body) void {
    self.request = .init(body);
    self.requesting = true;
    env.attachments.submit(self.keeper.?, &self.request);
}

fn takeWorkerResult(self: *Export) ?WorkerResult {
    self.mutex.lock();
    defer self.mutex.unlock();
    return self.worker_result;
}

/// Joins the worker, which returns once it has published its result.
pub fn joinWorker(self: *Export) WorkerResult {
    self.worker.?.join();
    self.worker = null;
    return takeWorkerResult(self).?;
}

pub fn finish(env: Env, slot: *?*Export) void {
    const self = slot.*.?;
    std.debug.assert(self.worker == null and !self.requesting);
    self.freeRecord();
    env.allocator.destroy(self);
    slot.* = null;
}
