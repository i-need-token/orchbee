const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;

pub fn frameApplied(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
) core_module.CoreError!core_module.TransitionOutcome {
    return self.core_state.frameApplied(
        connection_id,
        attachment_id,
        sequence,
        revision,
        generation,
    );
}

/// Abandons the exact in-flight frame for a snapshot. When the client's decoder rejected that
/// frame, the terminal's incremental projection is discarded as well, so the snapshot is captured
/// afresh from canonical Ghostty state rather than re-encoded from the capture that the client
/// could not accept.
pub fn needSnapshot(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
    decoder_rejected: bool,
) core_module.CoreError!core_module.TransitionOutcome {
    const attachment = try self.core_state.resolveAttachment(connection_id, attachment_id, .read);
    const terminal_id = attachment.terminal_id;
    const transition = try self.core_state.needSnapshot(connection_id, attachment_id, sequence);
    if (decoder_rejected and transition == .applied) {
        if (self.runtimeFor(terminal_id)) |runtime| runtime.releaseProjectionResources();
    }
    return transition;
}

pub fn nextProjectionAt(
    self: anytype,
    terminal_id: core_module.TerminalId,
) ?i128 {
    const runtime = self.runtimeFor(terminal_id) orelse return null;
    return runtime.nextProjectionAt();
}

pub fn setNextProjectionAt(
    self: anytype,
    terminal_id: core_module.TerminalId,
    next_projection_at_ns: i128,
) bool {
    const runtime = self.runtimeFor(terminal_id) orelse return false;
    runtime.setNextProjectionAt(next_projection_at_ns);
    return true;
}

pub fn nextCloseActivitySampleAt(
    self: anytype,
    terminal_id: core_module.TerminalId,
) ?i128 {
    const runtime = self.runtimeFor(terminal_id) orelse return null;
    return runtime.nextCloseActivitySampleAt();
}

pub fn setNextCloseActivitySampleAt(
    self: anytype,
    terminal_id: core_module.TerminalId,
    next_sample_at_ns: i128,
) bool {
    const runtime = self.runtimeFor(terminal_id) orelse return false;
    runtime.setNextCloseActivitySampleAt(next_sample_at_ns);
    return true;
}

pub fn setProjectionDemand(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    demand: core_module.ProjectionDemand,
) core_module.CoreError!core_module.TransitionOutcome {
    const attachment = try self.core_state.resolveAttachment(
        connection_id,
        attachment_id,
        .projection_demand,
    );
    const terminal_id = attachment.terminal_id;
    if (attachment.projectionDemand() == demand) return .idempotent;
    // Core demand is the focus-claim visibility fact. Repair the target's
    // held input and focus claim while active demand is still authoritative;
    // the already-validated Core transition is then infallible.
    if (demand == .paused and attachment.capabilities.input) {
        if (self.runtimeFor(terminal_id)) |runtime| runtime.resetInput(attachment_id);
    }
    const transition = self.core_state.setProjectionDemand(
        connection_id,
        attachment_id,
        demand,
    ) catch unreachable;
    releaseProjectionIfIdle(self, terminal_id);
    return transition;
}

pub fn releaseProjectionIfIdle(
    self: anytype,
    terminal_id: core_module.TerminalId,
) void {
    if (self.core_state.hasActiveProjectionDemand(terminal_id)) return;
    if (target_builtin.is_test) self.testing_state.projection_release_decisions += 1;
    if (self.runtimeFor(terminal_id)) |runtime| runtime.releaseProjectionResources();
}
