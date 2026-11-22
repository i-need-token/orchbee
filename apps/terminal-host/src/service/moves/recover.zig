//! `RecoverTerminal`: an import with no record, for a program the Keeper kept after its Host ended.
//! It builds at once on the creation pipeline, as the request's creation and terminal of this
//! Host's epoch, and then follows the import's claim, discard, and deadline.

const std = @import("std");
const control = @import("terminal-host-control");
const protocol = @import("terminal-protocol");
const answers_module = @import("answers.zig");
const refusal = @import("refusal.zig");
const record_import = @import("import.zig");
const host_failure = @import("../failure.zig");
const host_lifecycle = @import("../host_lifecycle.zig");

const Env = @import("env.zig").Env;
const Import = record_import.Import;
const Pending = answers_module.Pending;
const MessageKind = protocol.MessageKind;

/// `RecoverTerminal`: builds a fresh terminal for the program the Keeper kept under the request's
/// holding, as the request's creation and terminal of this Host's epoch, and answers that identity
/// once it committed; `ClaimTerminal` then claims it. The same recovery again answers as a restore
/// does, and a terminal it already claimed answers its identity.
pub fn recoverTerminal(env: Env, slot: *?*Import, to: Pending, request: control.decode.handover.RecoverRequest, admission: host_lifecycle.TerminalAdmission) void {
    var owned = request;
    const value = owned.value;
    if (slot.*) |current| {
        defer owned.deinit();
        const same = if (current.recovery) |held|
            current.creation_id.eql(value.creation_id) and std.mem.eql(u8, &held.value.holding, &value.holding) and
                std.mem.eql(u8, &held.value.mark, &value.mark)
        else
            false;
        if (!same) return env.answers.failure(to, refusal.moving);
        return switch (current.phase) {
            .restoring => env.answers.failure(to, refusal.moving),
            .restored, .claiming, .declining => record_import.answerIdentityAs(env, to, MessageKind.recover_terminal, value.creation_id, current.terminal_id),
            .receiving, .complete => unreachable,
        };
    }
    if (env.aggregate.coreRead().findTerminal(value.terminal_id)) |session| if (session.creation_id.eql(value.creation_id)) {
        defer owned.deinit();
        if (session.handover != .none) return env.answers.failure(to, refusal.moving);
        return record_import.answerIdentityAs(env, to, MessageKind.recover_terminal, value.creation_id, value.terminal_id);
    };
    if (admission == .closed) {
        owned.deinit();
        return env.answers.failure(to, refusal.draining);
    }
    const reserved = env.aggregate.reserveRecovery(value.creation_id, value.terminal_id) catch |err| {
        owned.deinit();
        return env.answers.failure(to, host_failure.fromCore(err));
    };
    const created = env.allocator.create(Import) catch {
        env.aggregate.releaseImport(reserved);
        owned.deinit();
        return env.answers.failure(to, refusal.exhausted);
    };
    var reply = record_import.encodeIdentity(env, to, MessageKind.recover_terminal, value.creation_id, value.terminal_id) catch {
        env.allocator.destroy(created);
        env.aggregate.releaseImport(reserved);
        owned.deinit();
        return env.answers.failure(to, refusal.exhausted);
    };
    created.* = .{
        .allocator = env.allocator,
        .creation_id = value.creation_id,
        .record = &.{},
        .recovery = owned,
        .deadline_at = env.now_ns +| @as(u64, value.deadline_seconds) * std.time.ns_per_s,
        .phase = .restoring,
        .terminal_id = value.terminal_id,
    };
    created.job = .init(env.allocator, value.creation_id, reserved, .{ .recovery = .{
        .identity = .{
            .creation_id = value.creation_id.bytes,
            .host_epoch = env.aggregate.hostEpoch().bytes,
            .terminal_id = value.terminal_id.bytes,
        },
        .holding = value.holding,
        .mark = value.mark,
        .geometry = value.geometry,
        .profile = value.profile,
        .scrollback_bytes = value.scrollback_bytes orelse env.aggregate.coreRead().terminalDefaults().scrollback_bytes,
    } });
    env.creations.beginRestore(env.aggregate, .{
        .connection_id = to.connection_id,
        .correlation_id = to.correlation_id,
        .host_epoch = env.aggregate.hostEpoch(),
        .reply = reply,
    }, &created.job) catch {
        reply.deinit();
        env.aggregate.releaseImport(reserved);
        created.recovery.?.deinit();
        env.allocator.destroy(created);
        return env.answers.failure(to, refusal.exhausted);
    };
    slot.* = created;
    env.answers.handOver();
}
