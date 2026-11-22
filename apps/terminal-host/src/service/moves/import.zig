//! The new Host's side of a move: `ImportTerminal`, `WriteHandoverRecord`, `RestoreTerminal`,
//! `ClaimTerminal`, `DiscardImport`, and the import's deadline. A Host keeps at most one import.
//! The record arrives in order, the restore builds on the creation pipeline's creator thread, and
//! the claim and `decline` go through the Keeper link, so the dispatch thread waits on neither.
//! `recover.zig` starts an import with no record, for a program the Keeper kept after its Host
//! ended, which then follows the same claim, discard, and deadline.
//!
//! The claim is the only commit: until the Keeper applies it the restored terminal reads nothing.
//! Whichever answer first shows this Host `claimed`, to `claim` or to `decline`, completes the
//! claim; any other end drops the terminal, closing only this Host's handles, so its program runs
//! on in the Keeper's care.

const std = @import("std");
const core_module = @import("terminal-host-core").state;
const control = @import("terminal-host-control");
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const terminal_engine = @import("terminal-host-engine");
const answers_module = @import("answers.zig");
const refusal = @import("refusal.zig");
const restore_build = @import("restore_build.zig");
const host_failure = @import("../failure.zig");
const host_lifecycle = @import("../host_lifecycle.zig");
const host_outgoing = @import("../outgoing.zig");
const host_response = @import("../response.zig");

const Env = @import("env.zig").Env;
const Pending = answers_module.Pending;
const Request = keeper_link.moves.Request;
const Result = keeper_link.moves.Result;
const Keeper = keeper_link.Keeper;
const TerminalRuntime = terminal_engine.runtime.TerminalRuntime;
const MessageKind = protocol.MessageKind;
const handover_encode = control.encode.handover;

pub const Phase = enum {
    /// Chunks arrive in order from offset 0.
    receiving,
    /// The whole record arrived.
    complete,
    /// The creation pipeline builds the terminal.
    restoring,
    /// Restored and committed: it reads nothing until its claim.
    restored,
    claiming,
    declining,
};

/// The request a pending claim or `decline` answers once it settles.
const Asker = enum { claim, discard, deadline };

pub const Import = struct {
    allocator: std.mem.Allocator,
    creation_id: core_module.CreationId,
    record: []u8,
    /// A recovery's request, which its build reads; null for a move's import.
    recovery: ?control.decode.handover.RecoverRequest = null,
    received: u32 = 0,
    deadline_at: u64,
    phase: Phase = .receiving,
    /// The restore's build, which the creation pipeline references from `restoring` until it
    /// settles.
    job: restore_build.Job = undefined,
    terminal_id: core_module.TerminalId = undefined,
    /// From `restored`: what the claim needs.
    runtime: *TerminalRuntime = undefined,
    claim: keeper_link.moves.Body = undefined,
    keeper: Keeper = undefined,
    held_attachments: []core_module.AttachmentId = &.{},
    request: Request = undefined,
    requesting: bool = false,
    asker: Asker = .deadline,
    claim_refusal: refusal.Details = undefined,
    /// The `ClaimTerminal` or `DiscardImport` that waits for its answer.
    asked: ?Pending = null,
    /// A `DiscardImport` or the deadline came while the restore built: the import is discarded
    /// once the build ends.
    discard_held: bool = false,
};

/// `ImportTerminal`: room for the record; the same creation again answers without effect.
pub fn importTerminal(env: Env, slot: *?*Import, to: Pending, request: control.decode.handover.Import, admission: host_lifecycle.TerminalAdmission) void {
    if (slot.*) |current| {
        if (!current.creation_id.eql(request.creation_id)) return env.answers.failure(to, refusal.moving);
        return answerImported(env, to, request.creation_id);
    }
    if (admission == .closed) return env.answers.failure(to, refusal.draining);
    const record = env.allocator.alloc(u8, request.record_bytes) catch return env.answers.failure(to, refusal.exhausted);
    const created = env.allocator.create(Import) catch {
        env.allocator.free(record);
        return env.answers.failure(to, refusal.exhausted);
    };
    created.* = .{
        .allocator = env.allocator,
        .creation_id = request.creation_id,
        .record = record,
        .deadline_at = env.now_ns +| @as(u64, request.deadline_seconds) * std.time.ns_per_s,
    };
    slot.* = created;
    answerImported(env, to, request.creation_id);
}

fn answerImported(env: Env, to: Pending, creation_id: core_module.CreationId) void {
    const payload = handover_encode.imported(env.allocator, creation_id) catch return env.answers.failure(to, refusal.exhausted);
    env.answers.success(to, MessageKind.import_terminal, .control_json, payload);
}

/// `WriteHandoverRecord`: the next chunk in order; a chunk already written answers again.
pub fn writeRecord(env: Env, slot: *?*Import, to: Pending, chunk: control.decode.handover.RecordChunk) void {
    const self = slot.* orelse return env.answers.failure(to, refusal.missing_import);
    if (!std.mem.eql(u8, &chunk.id, &self.creation_id.bytes)) return env.answers.failure(to, refusal.missing_import);
    // From the restore on, the build reads the record and then it goes.
    if (self.phase != .receiving and self.phase != .complete) return env.answers.failure(to, refusal.invalid_phase);
    const end = @as(usize, chunk.offset) + chunk.chunk.len;
    const repeated = end <= self.received and std.mem.eql(u8, self.record[chunk.offset..end], chunk.chunk);
    if (!repeated) {
        if (self.phase != .receiving or chunk.offset != self.received or end > self.record.len)
            return env.answers.failure(to, refusal.invalid_chunk);
        @memcpy(self.record[chunk.offset..end], chunk.chunk);
        self.received = @intCast(end);
        if (end == self.record.len) self.phase = .complete;
    }
    const payload = handover_encode.recordWritten(env.allocator, self.creation_id, self.received) catch
        return env.answers.failure(to, refusal.exhausted);
    env.answers.success(to, MessageKind.handover_record_written, .control_json, payload);
}

/// `RestoreTerminal`: hands the whole record to the creation pipeline, which answers the new
/// terminal's identity once it committed the restore. A restore that already happened answers
/// that identity again.
pub fn restore(env: Env, slot: *?*Import, to: Pending, creation_id: core_module.CreationId, admission: host_lifecycle.TerminalAdmission) void {
    const self = slot.* orelse return replayRestore(env, to, creation_id);
    if (!self.creation_id.eql(creation_id)) return replayRestore(env, to, creation_id);
    switch (self.phase) {
        .receiving => return env.answers.failure(to, refusal.incomplete_record),
        .restoring => return env.answers.failure(to, refusal.moving),
        .restored, .claiming, .declining => return answerIdentity(env, to, creation_id, self.terminal_id),
        .complete => {},
    }
    if (admission == .closed) return env.answers.failure(to, refusal.draining);
    const reserved = env.aggregate.reserveImport(creation_id) catch |err|
        return env.answers.failure(to, host_failure.fromCore(err));
    var reply = encodeIdentity(env, to, MessageKind.restore_terminal, creation_id, reserved.terminal_id) catch {
        env.aggregate.releaseImport(reserved);
        return env.answers.failure(to, refusal.exhausted);
    };
    self.job = .init(env.allocator, creation_id, reserved, .{ .record = self.record });
    self.terminal_id = reserved.terminal_id;
    env.creations.beginRestore(env.aggregate, .{
        .connection_id = to.connection_id,
        .correlation_id = to.correlation_id,
        .host_epoch = env.aggregate.hostEpoch(),
        .reply = reply,
    }, &self.job) catch {
        reply.deinit();
        env.aggregate.releaseImport(reserved);
        return env.answers.failure(to, refusal.exhausted);
    };
    self.phase = .restoring;
    env.answers.handOver();
}

pub fn encodeIdentity(env: Env, to: Pending, kind: u16, creation_id: core_module.CreationId, terminal_id: core_module.TerminalId) !host_outgoing.Outgoing {
    const epoch = env.aggregate.hostEpoch();
    const payload = try control.encode.terminalIdentity(env.allocator, creation_id, epoch, terminal_id);
    defer env.allocator.free(payload);
    var reply = host_outgoing.Outgoing.init(env.allocator);
    errdefer reply.deinit();
    _ = try host_response.appendControlSuccess(&reply, .{ .kind = kind }, payload, host_response.responseDelivery(to.correlation_id, epoch));
    return reply;
}

fn answerIdentity(env: Env, to: Pending, creation_id: core_module.CreationId, terminal_id: core_module.TerminalId) void {
    answerIdentityAs(env, to, MessageKind.restore_terminal, creation_id, terminal_id);
}

pub fn answerIdentityAs(env: Env, to: Pending, kind: u16, creation_id: core_module.CreationId, terminal_id: core_module.TerminalId) void {
    const payload = control.encode.terminalIdentity(env.allocator, creation_id, env.aggregate.hostEpoch(), terminal_id) catch
        return env.answers.failure(to, refusal.exhausted);
    env.answers.success(to, kind, .control_json, payload);
}

/// A restore whose import ended answers the terminal its creation committed.
fn replayRestore(env: Env, to: Pending, creation_id: core_module.CreationId) void {
    for (env.aggregate.coreRead().terminalValues()) |session| {
        if (session.creation_id.eql(creation_id)) return answerIdentity(env, to, creation_id, session.terminal_id);
    }
    env.answers.failure(to, refusal.missing_import);
}

/// Advances the import: a restore that settled, a Keeper answer, or the deadline. Frees the
/// import once it ends.
pub fn advance(env: Env, slot: *?*Import) void {
    const self = slot.* orelse return;
    if (self.requesting) {
        const result = self.request.poll() orelse return;
        self.requesting = false;
        return settleRequest(env, slot, result);
    }
    switch (self.phase) {
        .restoring => {
            self.job.serve(env.attachments);
            switch (self.job.outcome) {
                // The deadline, like a discard, waits for the build to end.
                .building => if (!self.discard_held and env.now_ns >= self.deadline_at) {
                    self.discard_held = true;
                    self.asker = .deadline;
                },
                .committed => committed(env, slot),
                // A failed restore discards the import.
                .ended => {
                    if (self.discard_held and self.asker == .discard) answerDiscarded(env, self.asked, .discarded);
                    self.asked = null;
                    finish(env, slot);
                },
            }
        },
        .receiving, .complete => if (env.now_ns >= self.deadline_at) finish(env, slot),
        .restored => if (env.now_ns >= self.deadline_at) startDecline(env, self, .deadline, null),
        .claiming, .declining => {},
    }
}

/// A committed restore: a settled terminal is this Host's at once, and a running one keeps what
/// its claim needs, so the record goes.
fn committed(env: Env, slot: *?*Import) void {
    const self = slot.*.?;
    const facts = self.job.facts.?;
    self.job.facts = null;
    self.held_attachments = facts.held_attachments;
    if (facts.console) |console| {
        const inputs = facts.terminal.inputs;
        self.runtime = env.aggregate.runtimeFor(self.terminal_id).?;
        self.keeper = self.job.keeper.?;
        self.claim = .{ .claim = .{
            .gate = platform.keeper.gate(self.runtime.keeperSubject()).?,
            .claim = .{
                .holding = inputs.holding_id.?,
                .mark = inputs.mark_id.?,
                .program = .{ .pid = console.program.pid, .created = console.program.created },
                .terminal = .{
                    .creation_id = self.creation_id.bytes,
                    .host_epoch = env.aggregate.hostEpoch().bytes,
                    .terminal_id = self.terminal_id.bytes,
                },
            },
        } };
    }
    env.allocator.free(self.record);
    self.record = &.{};
    if (facts.terminal.inputs.lifecycle != .running) {
        // A settled import is this Host's at once, so a discard held through its build finds it
        // complete, as one that came after it would.
        if (self.discard_held and self.asker == .discard) answerDiscarded(env, self.asked, .claimed);
        self.asked = null;
        return finish(env, slot);
    }
    self.phase = .restored;
    if (self.discard_held) startDecline(env, self, self.asker, null);
}

/// `ClaimTerminal`: the claim goes to the Keeper the restore inspected. A terminal this Host
/// already reads answers at once; a settled import refuses.
pub fn claimTerminal(env: Env, slot: *?*Import, to: Pending, terminal_id: core_module.TerminalId) void {
    const self = restoredImport(slot, terminal_id) orelse {
        const session = env.aggregate.coreRead().findTerminal(terminal_id) orelse
            return env.answers.failure(to, host_failure.fromCore(error.MissingResource));
        if (session.handover != .none) return env.answers.failure(to, refusal.moving);
        if (session.lifecycle != .running) return env.answers.failure(to, refusal.ineligible("not-running"));
        return answerNamed(env, to, MessageKind.claim_terminal, terminal_id);
    };
    switch (self.phase) {
        .restored => {
            self.asked = to;
            self.asker = .claim;
            self.phase = .claiming;
            submit(env, self, self.claim);
        },
        .restoring, .claiming, .declining => env.answers.failure(to, refusal.moving),
        .receiving, .complete => unreachable,
    }
}

/// `DiscardImport`: drops an import and its restored terminal, after a `decline` that may show the
/// claim completed instead.
pub fn discardImport(env: Env, slot: *?*Import, to: Pending, target: control.decode.handover.Discard) void {
    const self: *Import = switch (target) {
        .import => |creation_id| found: {
            const current = slot.* orelse return answerDiscarded(env, to, .discarded);
            if (!current.creation_id.eql(creation_id)) return answerDiscarded(env, to, .discarded);
            break :found current;
        },
        .terminal => |terminal_id| restoredImport(slot, terminal_id) orelse {
            // A terminal this Host reads already was claimed; an unknown one has nothing to discard.
            const session = env.aggregate.coreRead().findTerminal(terminal_id) orelse
                return answerDiscarded(env, to, .discarded);
            if (session.handover != .none) return env.answers.failure(to, refusal.moving);
            return answerDiscarded(env, to, .claimed);
        },
    };
    switch (self.phase) {
        .receiving, .complete => {
            finish(env, slot);
            answerDiscarded(env, to, .discarded);
        },
        .restored => startDecline(env, self, .discard, to),
        // Held until the build ends, which a discard does not cancel.
        .restoring => if (self.asked != null) env.answers.failure(to, refusal.moving) else {
            self.discard_held = true;
            self.asker = .discard;
            self.asked = to;
        },
        .claiming, .declining => env.answers.failure(to, refusal.moving),
    }
}

fn restoredImport(slot: *?*Import, terminal_id: core_module.TerminalId) ?*Import {
    const self = slot.* orelse return null;
    return switch (self.phase) {
        .receiving, .complete => null,
        .restoring, .restored, .claiming, .declining => if (self.terminal_id.eql(terminal_id)) self else null,
    };
}

fn startDecline(env: Env, self: *Import, asker: Asker, to: ?Pending) void {
    self.asker = asker;
    if (to) |pending| self.asked = pending;
    self.phase = .declining;
    const claim = self.claim.claim;
    submit(env, self, .{ .decline = .{ .gate = claim.gate, .marked = .{ .holding = claim.claim.holding, .mark = claim.claim.mark } } });
}

fn settleRequest(env: Env, slot: *?*Import, result: Result) void {
    const self = slot.*.?;
    switch (self.phase) {
        .claiming => switch (result) {
            .done => completeClaim(env, slot),
            // Nothing applied: the `decline` that follows may still show an earlier claim applied.
            .refused, .unsupported => {
                self.claim_refusal = refusal.keeper(result);
                startDecline(env, self, .claim, null);
            },
            // Possibly applied: the terminal stays restored and reads nothing.
            else => {
                self.phase = .restored;
                env.answers.failure(self.asked, refusal.keeper(result));
                self.asked = null;
            },
        },
        .declining => if (result == .outcome and result.outcome == .claimed) completeClaim(env, slot) else discardRestored(env, slot),
        .receiving, .complete, .restoring, .restored => unreachable,
    }
}

/// The Keeper names this Host owner: the gate opens and the terminal reads where the old Host's
/// reader parked.
fn completeClaim(env: Env, slot: *?*Import) void {
    const self = slot.*.?;
    self.runtime.claimRestored();
    for (self.held_attachments) |attachment_id| self.runtime.detachInput(attachment_id);
    env.aggregate.setHandover(self.terminal_id, .restored, .none);
    // A recovered terminal starts empty: a console resize makes the console repaint its screen and
    // a full-screen program redraw. A failed nudge leaves the terminal as it is.
    if (self.recovery != null) self.runtime.nudgeConsole() catch |err|
        std.log.warn("terminal Host could not resize a recovered console: {s}", .{@errorName(err)});
    switch (self.asker) {
        .claim => answerNamed(env, self.asked, MessageKind.claim_terminal, self.terminal_id),
        .discard => answerDiscarded(env, self.asked, .claimed),
        .deadline => {},
    }
    self.asked = null;
    finish(env, slot);
}

fn discardRestored(env: Env, slot: *?*Import) void {
    const self = slot.*.?;
    env.aggregate.removeMoving(self.terminal_id) catch {
        std.log.err("terminal Host found an attachment on a restored terminal it discards", .{});
        env.aggregate.removeMovingAtTeardown(self.terminal_id);
    };
    switch (self.asker) {
        .claim => env.answers.failure(self.asked, self.claim_refusal),
        .discard => answerDiscarded(env, self.asked, .discarded),
        .deadline => {},
    }
    self.asked = null;
    finish(env, slot);
}

fn answerNamed(env: Env, to: ?Pending, kind: u16, terminal_id: core_module.TerminalId) void {
    const payload = handover_encode.terminalNamed(env.allocator, terminal_id) catch return env.answers.failure(to, refusal.exhausted);
    env.answers.success(to, kind, .control_json, payload);
}

fn answerDiscarded(env: Env, to: ?Pending, outcome: handover_encode.DiscardOutcome) void {
    env.answers.successStatic(to, MessageKind.discard_import, handover_encode.discarded(outcome));
}

/// Whether a restored import came from `keeper`, so `DetachKeeper` waits.
pub fn heldBy(slot: *const ?*Import, keeper: Keeper) bool {
    const self = slot.* orelse return false;
    return switch (self.phase) {
        .receiving, .complete => false,
        .restoring => if (self.job.keeper) |used| used.eql(keeper) else false,
        .restored, .claiming, .declining => self.keeper.eql(keeper),
    };
}

fn submit(env: Env, self: *Import, body: keeper_link.moves.Body) void {
    self.request = .init(body);
    self.requesting = true;
    env.attachments.submit(self.keeper, &self.request);
}

/// Host teardown, once the dispatch loop has ended and the creation pipeline settled the restore:
/// a claim the Keeper applied completes, so the terminal ends as this Host's own; any other import
/// is discarded without a `decline`, closing only this Host's handles.
pub fn teardown(env: Env, slot: *?*Import) void {
    const self = slot.* orelse return;
    self.asked = null;
    if (self.requesting) {
        const result = self.request.wait();
        self.requesting = false;
        const claimed = switch (result) {
            .done => self.phase == .claiming,
            .outcome => |outcome| outcome == .claimed,
            else => false,
        };
        if (claimed) return completeClaim(env, slot);
        self.phase = .restored;
    }
    switch (self.phase) {
        .receiving, .complete => {},
        .restoring => switch (self.job.outcome) {
            // The pipeline abandoned a build that never returned, and its thread still reads the
            // job and the record, so both are kept.
            .building => {
                slot.* = null;
                return;
            },
            .committed => {
                const facts = self.job.facts.?;
                self.job.facts = null;
                self.held_attachments = facts.held_attachments;
                if (facts.terminal.inputs.lifecycle == .running) env.aggregate.removeMovingAtTeardown(self.terminal_id);
            },
            .ended => {},
        },
        .restored => env.aggregate.removeMovingAtTeardown(self.terminal_id),
        .claiming, .declining => unreachable,
    }
    finish(env, slot);
}

/// Teardown, before the creation pipeline waits for the build: an `inspect` not handed over fails.
pub fn closeRestore(slot: *?*Import) void {
    const self = slot.* orelse return;
    if (self.phase == .restoring) self.job.close();
}

pub fn finish(env: Env, slot: *?*Import) void {
    const self = slot.*.?;
    std.debug.assert(!self.requesting);
    if (self.phase == .restoring) std.debug.assert(self.job.outcome != .building);
    env.allocator.free(self.held_attachments);
    env.allocator.free(self.record);
    if (self.recovery) |*request| request.deinit();
    env.allocator.destroy(self);
    slot.* = null;
}
