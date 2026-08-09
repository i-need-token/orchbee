const std = @import("std");
const target_builtin = @import("builtin");

const core_module = @import("terminal-host-core").state;
const control_encode = @import("terminal-host-control").encode;
const host_outgoing = @import("outgoing.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const protocol = @import("terminal-protocol");
const terminal_runtime = @import("terminal-host-engine").runtime;

const projection_interval_ns = 16 * std.time.ns_per_ms;
const max_schedule_scan_per_poll = 64;
const max_scheduled_attachments_per_poll = 8;

const MessageKind = protocol.MessageKind;

const ProjectionAppendOutcome = union(enum) {
    appended: core_module.ProjectionFrameCommit,
    batch_full,
};

pub const Scheduler = struct {
    scan_index: usize = 0,

    pub fn schedule(
        self: *Scheduler,
        allocator: std.mem.Allocator,
        aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        maximum_frame_bytes: usize,
        outgoing: *host_outgoing.Outgoing,
    ) !host_outgoing.ScheduleOutcome {
        const core = aggregate.core();
        if (core.connectionClientId(connection_id) == null) return .complete;
        const count = core.attachmentCount();
        if (count == 0) return .complete;
        const now = std.time.nanoTimestamp();
        const start = self.scan_index % count;
        var scanned: usize = 0;
        var scheduled: usize = 0;
        var outcome: host_outgoing.ScheduleOutcome = .complete;
        var used_terminal_ids: [max_scheduled_attachments_per_poll]core_module.TerminalId = undefined;
        var used_terminal_count: usize = 0;
        const PendingMutation = union(enum) {
            block: core_module.ProjectionBlockCommit,
            frame: core_module.ProjectionFrameCommit,
        };
        var pending_mutations: [max_scheduled_attachments_per_poll]PendingMutation = undefined;
        var pending_mutation_count: usize = 0;
        errdefer {
            var index = pending_mutation_count;
            while (index > 0) {
                index -= 1;
                switch (pending_mutations[index]) {
                    .block => |commit| core.rollbackProjectionBlock(commit) catch unreachable,
                    .frame => |commit| core.rollbackFrameCommit(commit) catch unreachable,
                }
            }
        }
        scan: while (scanned < @min(count, max_schedule_scan_per_poll) and
            scheduled < max_scheduled_attachments_per_poll)
        {
            const index = (start + scanned) % count;
            scanned += 1;
            const attachment_id = core.attachmentKeys()[index];
            // findAttachment performs lazy revision and geometry invalidation
            // before delta versus snapshot is selected. Reading the backing
            // map value directly would miss a geometry change since the last
            // request.
            const attachment = core.findAttachment(attachment_id) orelse continue;
            if (!attachment.capabilities.read) continue;
            const ready = attachment.projectionReady() orelse continue;
            const runtime = aggregate.runtimeFor(attachment.terminal_id) orelse continue;
            const terminal = core.findTerminal(attachment.terminal_id) orelse continue;
            if (!terminal.hasReadableRuntimeState() or !runtime.readableStateAvailable()) continue;
            const ready_base = ready.base();
            const revision_dirty = ready_base == null or
                terminal.observedRevision() > ready_base.?.revision;
            if (!ready.forceSnapshot() and !revision_dirty) continue;
            if (now < aggregate.nextProjectionAt(attachment.terminal_id).?) continue;
            var already_used = false;
            for (used_terminal_ids[0..used_terminal_count]) |used_terminal_id| {
                already_used = already_used or used_terminal_id.eql(attachment.terminal_id);
            }
            if (already_used) continue;
            std.debug.assert(used_terminal_count < used_terminal_ids.len);
            used_terminal_ids[used_terminal_count] = attachment.terminal_id;
            used_terminal_count += 1;
            scheduled += 1;
            const geometry = runtime.geometry();
            _ = core.synchronizeRevision(terminal.terminal_id, geometry.revision) catch unreachable;
            const force_snapshot = ready.forceSnapshot() or ready_base == null or
                ready_base.?.generation != geometry.generation or
                terminal.observedGeometryRevision() > ready_base.?.revision;
            if (!force_snapshot and ready_base.?.revision >= geometry.revision) {
                core.projectionCaughtUp(attachment.attachment_id) catch unreachable;
                continue;
            }
            var encoded = runtime.encodeProjectionFrameOwned(
                outgoing.ownedPayloadAllocator(),
                host_outgoing.owned_payload_prefix_bytes,
                .{
                    .attachment_id = attachment.attachment_id,
                    .base_generation = if (ready_base) |base| base.generation else null,
                    .base_revision = if (ready_base) |base| base.revision else null,
                    .force_snapshot = force_snapshot,
                    .host_epoch = core.epoch,
                    .maximum_bytes = maximum_frame_bytes,
                    .sequence = attachment.nextProjectionSequence(),
                },
            ) catch |err| {
                if (err == error.TerminalResizeFailed or
                    err == error.TerminalRuntimeUnavailable)
                {
                    used_terminal_count -= 1;
                    scheduled -= 1;
                    continue :scan;
                }
                const reason: core_module.ProjectionBlock = switch (err) {
                    error.ProjectionTooLarge => .projection_too_large,
                    else => .capture_allocation_failed,
                };
                const blocked_geometry = runtime.geometry();
                _ = core.synchronizeRevision(
                    attachment.terminal_id,
                    blocked_geometry.revision,
                ) catch unreachable;
                const append_outcome = try appendProjectionBlocked(
                    allocator,
                    core.epoch,
                    outgoing,
                    attachment.attachment_id,
                    reason,
                    blocked_geometry.revision,
                );
                const append_token = switch (append_outcome) {
                    .appended => |token| token,
                    .full => {
                        scanned -= 1;
                        used_terminal_count -= 1;
                        outcome = .batch_full;
                        break :scan;
                    },
                };
                const block_commit = core.blockProjectionTransaction(
                    attachment.attachment_id,
                    reason,
                ) catch |block_err| {
                    outgoing.rollback(append_token) catch unreachable;
                    return block_err;
                };
                std.debug.assert(pending_mutation_count < pending_mutations.len);
                pending_mutations[pending_mutation_count] = .{ .block = block_commit };
                pending_mutation_count += 1;
                continue;
            };
            defer if (encoded.allocation.len != 0) {
                outgoing.ownedPayloadAllocator().free(encoded.allocation);
            };
            _ = core.synchronizeRevision(attachment.terminal_id, encoded.revision) catch unreachable;
            const message_kind = switch (encoded.kind) {
                .snapshot => MessageKind.snapshot_frame,
                .delta => MessageKind.delta_frame,
            };
            switch (try appendProjectionFrame(
                aggregate,
                outgoing,
                attachment.attachment_id,
                message_kind,
                &encoded,
            )) {
                .appended => |commit| {
                    std.debug.assert(pending_mutation_count < pending_mutations.len);
                    pending_mutations[pending_mutation_count] = .{ .frame = commit };
                    pending_mutation_count += 1;
                },
                .batch_full => {
                    scanned -= 1;
                    used_terminal_count -= 1;
                    outcome = .batch_full;
                    break :scan;
                },
            }
        }
        self.scan_index = (start + scanned) % count;
        for (used_terminal_ids[0..used_terminal_count]) |terminal_id| {
            _ = aggregate.setNextProjectionAt(
                terminal_id,
                now + projection_interval_ns,
            );
        }
        return outcome;
    }
};

fn appendProjectionFrame(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    outgoing: *host_outgoing.Outgoing,
    attachment_id: core_module.AttachmentId,
    message_kind: u16,
    encoded: *terminal_runtime.ProjectionFrame,
) !ProjectionAppendOutcome {
    const core = aggregate.core();
    const append_token = switch (try outgoing.adoptPayload(
        message_kind,
        .terminal_frame,
        encoded.allocation,
        encoded.bytes,
        .{ .event = core.epoch },
    )) {
        .appended => |token| token,
        .full => return .batch_full,
    };
    encoded.allocation = &.{};
    encoded.bytes = &.{};
    const commit = core.commitFrameTransaction(
        attachment_id,
        encoded.base_revision,
        encoded.revision,
        encoded.generation,
        switch (encoded.kind) {
            .snapshot => .snapshot,
            .delta => .delta,
        },
    ) catch |err| {
        outgoing.rollback(append_token) catch unreachable;
        return err;
    };
    std.debug.assert(commit.flight.sequence == encoded.sequence);
    return .{ .appended = commit };
}

fn appendProjectionBlocked(
    allocator: std.mem.Allocator,
    host_epoch: core_module.HostEpoch,
    outgoing: *host_outgoing.Outgoing,
    attachment_id: core_module.AttachmentId,
    reason: core_module.ProjectionBlock,
    revision: u64,
) !host_outgoing.AppendOutcome {
    const payload = try control_encode.projectionBlocked(
        allocator,
        attachment_id,
        reason,
        revision,
    );
    defer allocator.free(payload);
    return outgoing.append(
        MessageKind.projection_blocked,
        .control_json,
        payload,
        .{ .event = host_epoch },
    );
}

pub const testing = if (target_builtin.is_test) struct {
    pub const appendProjectionFrameFn = appendProjectionFrame;
} else struct {};
