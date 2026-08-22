const attachment_projection = @import("../attachment_projection.zig");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const AttachmentId = contracts.AttachmentId;
const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const InFlightFrame = contracts.InFlightFrame;
const ProjectionBlock = contracts.ProjectionBlock;
const ProjectionBlockCommit = contracts.ProjectionBlockCommit;
const ProjectionDemand = contracts.ProjectionDemand;
const ProjectionFrameCommit = contracts.ProjectionFrameCommit;
const ProjectionFrameKind = contracts.ProjectionFrameKind;
const TransitionOutcome = contracts.TransitionOutcome;

pub fn setProjectionDemand(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    demand: ProjectionDemand,
) CoreError!TransitionOutcome {
    const attachment = try storage.resolveMutableAttachment(self, connection_id, attachment_id, .read);
    if (attachment.projectionDemand() == demand) return .idempotent;
    const broker = if (self.broker) |*active| active else unreachable;
    broker.applyProjectionDemand(attachment_id, demand);
    return .applied;
}

/// Invalidates every read presentation for one retained terminal without
/// changing canonical terminal state or attachment sequencing. Ready readers
/// require a snapshot, in-flight readers force one after acknowledgement, and
/// paused readers remain paused.
pub fn invalidateTerminalPresentations(
    self: anytype,
    terminal_id: contracts.TerminalId,
) CoreError!void {
    _ = try storage.requireTerminal(self, terminal_id);
    const broker = if (self.broker) |*active| active else return;
    broker.invalidateTerminalPresentation(terminal_id);
}

pub fn commitFrame(
    self: anytype,
    attachment_id: AttachmentId,
    base_revision: ?u64,
    revision: u64,
    generation: u64,
    kind: ProjectionFrameKind,
) CoreError!InFlightFrame {
    return (try self.commitFrameTransaction(
        attachment_id,
        base_revision,
        revision,
        generation,
        kind,
    )).flight;
}

pub fn commitFrameTransaction(
    self: anytype,
    attachment_id: AttachmentId,
    base_revision: ?u64,
    revision: u64,
    generation: u64,
    kind: ProjectionFrameKind,
) CoreError!ProjectionFrameCommit {
    const attachment = storage.mutableAttachment(self, attachment_id) orelse return error.MissingResource;
    if (!attachment.capabilities.read) return error.InvalidPhase;
    return attachment.projection.commitFrame(
        attachment_id,
        base_revision,
        revision,
        generation,
        kind,
    );
}

pub fn rollbackFrameCommit(
    self: anytype,
    commit: ProjectionFrameCommit,
) CoreError!void {
    const attachment = storage.rawAttachment(self, commit.attachment_id) orelse
        return error.MissingResource;
    try attachment.projection.rollbackFrame(commit);
}

pub fn frameApplied(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
) CoreError!TransitionOutcome {
    const attachment = try storage.resolveMutableAttachment(self, connection_id, attachment_id, .read);
    const terminal = try storage.requireTerminal(self, attachment.terminal_id);
    return projectionTransition(try attachment.projection.frameApplied(
        sequence,
        revision,
        generation,
        terminal.observedGeometryRevision(),
    ));
}

pub fn needSnapshot(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    sequence: u64,
) CoreError!TransitionOutcome {
    const attachment = try storage.resolveMutableAttachment(self, connection_id, attachment_id, .read);
    return projectionTransition(try attachment.projection.needSnapshot(sequence));
}

pub fn blockProjection(
    self: anytype,
    attachment_id: AttachmentId,
    reason: ProjectionBlock,
    failed_revision: u64,
) CoreError!void {
    _ = try self.blockProjectionTransaction(
        attachment_id,
        reason,
        failed_revision,
    );
}

pub fn blockProjectionTransaction(
    self: anytype,
    attachment_id: AttachmentId,
    reason: ProjectionBlock,
    failed_revision: u64,
) CoreError!ProjectionBlockCommit {
    const attachment = storage.rawAttachment(self, attachment_id) orelse return error.MissingResource;
    const terminal = try storage.requireTerminal(self, attachment.terminal_id);
    if (failed_revision > terminal.observedRevision()) return error.InvalidPhase;
    storage.refreshAttachmentProjectionState(self, attachment);
    return attachment.projection.block(
        attachment_id,
        reason,
        failed_revision,
    );
}

pub fn rollbackProjectionBlock(
    self: anytype,
    commit: ProjectionBlockCommit,
) CoreError!void {
    const attachment = storage.rawAttachment(self, commit.attachment_id) orelse
        return error.MissingResource;
    try attachment.projection.rollbackBlock(commit);
}

pub fn projectionCaughtUp(
    self: anytype,
    attachment_id: AttachmentId,
) CoreError!void {
    const attachment = storage.mutableAttachment(self, attachment_id) orelse return error.MissingResource;
    const terminal = try storage.requireTerminal(self, attachment.terminal_id);
    try attachment.projection.requireCaughtUp(terminal.observedRevision());
}

fn projectionTransition(value: attachment_projection.Transition) TransitionOutcome {
    return switch (value) {
        .applied => .applied,
        .idempotent => .idempotent,
    };
}
