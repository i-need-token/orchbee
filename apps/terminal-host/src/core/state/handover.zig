//! Terminals that move between Hosts, as Core records them. The old Host's quiesce makes a running
//! terminal `quiesced` and the Keeper's `claimed` makes it `moved`; a resume makes it a terminal
//! like any other again, and a drop removes it with no outcome. The new Host commits a running
//! import as `restored` until its claim or discard, and a settled one with its outcome already
//! published. Every transition runs on the dispatch thread, after the service checked the move.

const std = @import("std");
const model = @import("terminal-host-model");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const CoreError = contracts.CoreError;
const CreationId = contracts.CreationId;
const LaunchDigest = contracts.LaunchDigest;
const TerminalHandover = contracts.TerminalHandover;
const TerminalId = contracts.TerminalId;
const TerminalSession = contracts.TerminalSession;

/// Moves `terminal_id` from `from` to `to`.
pub fn commitHandover(self: anytype, terminal_id: TerminalId, from: TerminalHandover, to: TerminalHandover) void {
    const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
    std.debug.assert(terminal.handover == from);
    terminal.handover = to;
}

/// Whether a moving terminal may leave: only once no attachment names it, and while the inventory
/// generation may advance.
pub fn validateMoveRemoval(self: anytype, terminal_id: TerminalId) CoreError!void {
    const terminal = try storage.requireTerminal(self, terminal_id);
    std.debug.assert(terminal.handover != .none);
    if (self.attachmentCountForTerminal(terminal_id) != 0) return error.InvalidPhase;
    try self.terminal_registry.requireGenerationAdvance();
}

/// Removes a moving terminal without settlement: it publishes no outcome and sends no
/// `TerminalExited`. The caller frees the returned storage.
pub fn commitMoveRemoval(self: anytype, terminal_id: TerminalId) *TerminalSession {
    const terminal = self.terminal_registry.get(terminal_id).?;
    std.debug.assert(terminal.handover != .none);
    storage.removeTerminalAttachments(self, terminal_id);
    return self.terminal_registry.removeAssumePresent(terminal_id, true);
}

/// A terminal restored from a handover record, under the identity of its import.
pub const Import = struct {
    creation_id: CreationId,
    terminal_id: TerminalId,
    launch_digest: LaunchDigest,
    environment_overlay_digest: LaunchDigest,
    configuration: model.configuration.Configuration,
    configuration_revision: u64,
    theme: model.theme.State,
    start_geometry: ?model.geometry.Geometry,
    geometry: model.geometry.Geometry,
    revision: u64,
    /// Running, or settled with its outcome already published.
    lifecycle: model.lifecycle.Lifecycle,
};

/// Whether an import may take `creation_id` and `terminal_id`; the caller reserved the registry.
pub fn validateImportAdmission(self: anytype, creation_id: CreationId, terminal_id: TerminalId) CoreError!void {
    try self.terminal_registry.validateAdmission(creation_id, terminal_id);
}

/// Commits an import into `session`: a running one as `restored`, a settled one with its outcome
/// published, so it never publishes it again. The caller reserved the registry's admission.
pub fn commitImport(self: anytype, imported: Import, session: *TerminalSession) void {
    self.terminal_registry.validateAdmission(imported.creation_id, imported.terminal_id) catch unreachable;
    session.* = .{
        .configuration_state = .{ .effective = imported.configuration, .revision = imported.configuration_revision },
        .creation_id = imported.creation_id,
        .engine_observation = .{ .geometry = imported.geometry, .revision = imported.revision },
        .environment_overlay_digest = imported.environment_overlay_digest,
        .launch_digest = imported.launch_digest,
        .lifecycle = imported.lifecycle,
        .start_geometry = imported.start_geometry,
        .terminal_id = imported.terminal_id,
        .theme_state = imported.theme,
        .handover = switch (imported.lifecycle) {
            .running => .restored,
            .exited, .failed => .none,
            .awaiting_start, .finishing, .containing_failure => unreachable,
        },
    };
    std.debug.assert(session.pendingOutcome() == null);
    self.terminal_registry.commitAssumeCapacity(session);
}
