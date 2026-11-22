//! The aggregate's side of a move between Hosts, on the dispatch thread: the facts a quiesce
//! exports beside a terminal's engine, the commit of a restored import, and the removal of a
//! terminal that moved away or whose import was discarded, which ends nothing.

const std = @import("std");
const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const model = @import("terminal-host-model");
const host_retained_terminal = @import("../retained_terminal.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const TerminalRuntime = terminal_engine.runtime.TerminalRuntime;
const HandoverInputs = terminal_engine.runtime.HandoverInputs;
const HandoverFacts = terminal_engine.runtime.HandoverFacts;
const ConsoleFacts = terminal_engine.handover.ConsoleFacts;
const RecordLifecycle = terminal_engine.handover.host_sections.Lifecycle;

/// What a record names beside the terminal's own facts.
pub const RecordIds = struct {
    holding: ?[16]u8 = null,
    mark: ?[16]u8 = null,
};

/// The facts a record of `terminal_id` carries that its engine does not hold.
pub fn exportInputs(self: anytype, terminal_id: core_module.TerminalId, ids: RecordIds, console: ?ConsoleFacts) HandoverInputs {
    const session = self.core_state.findTerminal(terminal_id).?;
    const lifecycle: RecordLifecycle = switch (session.lifecycle) {
        .running => |running| .{ .running = running.termination },
        .exited => |exited| .{ .exited = .{
            .final_revision = exited.final_revision,
            .result = exited.result,
            .termination = exited.termination,
        } },
        .failed => |failed| .{ .failed = .{
            .final_revision = failed.final_revision,
            .failure = failed.failure,
            .process = failed.process,
        } },
        .awaiting_start, .finishing, .containing_failure => unreachable,
    };
    return .{
        .terminal = .{
            .holding_id = ids.holding,
            .mark_id = ids.mark,
            .identity = .{
                .creation_id = session.creation_id.bytes,
                .host_epoch = self.hostEpoch().bytes,
                .terminal_id = session.terminal_id.bytes,
            },
            .lifecycle = lifecycle,
            .start_geometry = session.start_geometry,
            .launch_fingerprint = session.launch_digest,
            .environment_overlay_digest = session.environment_overlay_digest,
        },
        .configuration_revision = session.configuration_state.revision,
        .theme = session.theme_state,
        .console = console,
    };
}

/// Room for one restored terminal: the registry's capacity and its storage. The commit then cannot
/// fail; `release` returns both when the import ends before it.
pub const ImportSlot = struct {
    terminal_id: core_module.TerminalId,
    retained: *RetainedTerminal,
};

pub fn reserveImport(self: anytype, creation_id: core_module.CreationId) !ImportSlot {
    try self.core_state.reserveTerminal();
    var terminal_id = model.identifiers.TerminalId.random();
    while (self.core_state.findTerminal(terminal_id) != null) terminal_id = model.identifiers.TerminalId.random();
    try self.core_state.validateImportAdmission(creation_id, terminal_id);
    const retained = try self.allocator.create(RetainedTerminal);
    return .{ .terminal_id = terminal_id, .retained = retained };
}

/// Room for a recovered terminal under the terminal id it ran as, which no terminal of this Host
/// may hold.
pub fn reserveRecovery(self: anytype, creation_id: core_module.CreationId, terminal_id: core_module.TerminalId) !ImportSlot {
    try self.core_state.reserveTerminal();
    try self.core_state.validateImportAdmission(creation_id, terminal_id);
    const retained = try self.allocator.create(RetainedTerminal);
    return .{ .terminal_id = terminal_id, .retained = retained };
}

pub fn releaseImport(self: anytype, slot: ImportSlot) void {
    self.allocator.destroy(slot.retained);
}

/// Commits a restored import: a running one as `restored`, which reads nothing until its claim,
/// and a settled one with its outcome already published.
pub fn commitImport(
    self: anytype,
    slot: ImportSlot,
    creation_id: core_module.CreationId,
    runtime: *TerminalRuntime,
    facts: *const HandoverFacts,
) void {
    const terminal = facts.terminal;
    const inputs = terminal.inputs;
    const lifecycle: model.lifecycle.Lifecycle = switch (inputs.lifecycle) {
        .running => |termination| .{ .running = .{ .termination = termination } },
        .exited => |exited| .{ .exited = .{
            .final_revision = exited.final_revision,
            .publication = .published,
            .result = exited.result,
            .termination = exited.termination,
        } },
        .failed => |failed| .{ .failed = .{
            .failure = failed.failure,
            .final_revision = failed.final_revision,
            .process = failed.process,
            .publication = .published,
        } },
    };
    slot.retained.* = .{ .runtime = runtime, .session = undefined, .start_preparation = null };
    self.core_state.commitImport(.{
        .creation_id = creation_id,
        .terminal_id = slot.terminal_id,
        .launch_digest = inputs.launch_fingerprint,
        .environment_overlay_digest = inputs.environment_overlay_digest,
        .configuration = facts.configuration.configuration,
        .configuration_revision = facts.configuration.revision,
        .theme = facts.configuration.theme,
        .start_geometry = inputs.start_geometry,
        .geometry = terminal.geometry,
        .revision = terminal.revision,
        .lifecycle = lifecycle,
    }, &slot.retained.session);
}

/// Removes a terminal that moved away or whose import was discarded, at a Worker's request: only
/// once no attachment names it. Its threads are joined and only this Host's handles close, so
/// its program and console run on.
pub fn removeMoving(self: anytype, terminal_id: core_module.TerminalId) core_module.CoreError!void {
    try self.core_state.validateMoveRemoval(terminal_id);
    const session = self.core_state.commitMoveRemoval(terminal_id);
    release(self, session);
}

/// Host teardown's removal of a moving terminal, whatever still names it.
pub fn removeMovingAtTeardown(self: anytype, terminal_id: core_module.TerminalId) void {
    release(self, self.core_state.takeTerminalForHostShutdown(terminal_id));
}

fn release(self: anytype, session: *core_module.TerminalSession) void {
    const retained = RetainedTerminal.fromMutableSession(session);
    if (retained.runtime) |runtime| runtime.dropMoved();
    retained.deinitStartPreparation(self.allocator);
    self.allocator.destroy(retained);
}

/// Moves a terminal between its move states, after the moves owner checked the move.
pub fn setHandover(
    self: anytype,
    terminal_id: core_module.TerminalId,
    from: core_module.TerminalHandover,
    to: core_module.TerminalHandover,
) void {
    self.core_state.commitHandover(terminal_id, from, to);
}
