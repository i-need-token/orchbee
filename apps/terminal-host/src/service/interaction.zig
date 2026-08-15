const std = @import("std");
const target_builtin = @import("builtin");

const core_module = @import("terminal-host-core").state;
const control_encode = @import("terminal-host-control").encode;
const host_outgoing = @import("outgoing.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const protocol = @import("terminal-protocol");
const engine = @import("terminal-host-engine");
const terminal_runtime = engine.runtime;

const max_schedule_scan_per_poll = 64;
const max_scheduled_attachments_per_poll = 8;

const MessageKind = protocol.MessageKind;

const SchedulerTestingHooks = if (target_builtin.is_test) struct {
    after_cancellation_records_appended: ?*const fn (*anyopaque) void = null,
    context: ?*anyopaque = null,
} else struct {};

pub const ScheduleOutcome = host_outgoing.ScheduleOutcome;

pub const AccessError = host_runtime_aggregate.ReadableAttachmentError;
pub const ReadRowsRequest = host_runtime_aggregate.ReadRowsRequest;
pub const SelectionGestureRequest = host_runtime_aggregate.SelectionGestureRequest;
pub const SelectionCommandRequest = host_runtime_aggregate.SelectionCommandRequest;

pub const SelectionExportStatus = enum {
    cancelled,
    completed,
};

pub const Controller = struct {
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,

    pub fn init(
        aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    ) Controller {
        return .{ .aggregate = aggregate };
    }

    pub fn readableAttachment(
        self: Controller,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
    ) AccessError!ReadableAttachment {
        return self.aggregate.readableAttachment(connection_id, attachment_id);
    }
};

pub const ReadableAttachment = host_runtime_aggregate.ReadableAttachment;
pub const SelectionExportHandoff = host_runtime_aggregate.SelectionExportHandoff;

pub const Scheduler = struct {
    scan_index: usize = 0,
    testing_hooks: SchedulerTestingHooks = .{},

    fn notifyAfterCancellationRecordsAppended(
        self: *Scheduler,
        cancellation_count: usize,
    ) void {
        if (comptime target_builtin.is_test) {
            if (cancellation_count == 0) return;
            if (self.testing_hooks.after_cancellation_records_appended) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
    }

    pub fn schedule(
        self: *Scheduler,
        allocator: std.mem.Allocator,
        aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        outgoing: *host_outgoing.Outgoing,
    ) !ScheduleOutcome {
        const core = aggregate.core();
        if (core.connectionClientId(connection_id) == null) return .complete;
        const count = core.attachmentCount();
        if (count == 0) return .complete;
        const start = self.scan_index % count;
        var scanned: usize = 0;
        var scheduled: usize = 0;
        var outcome: ScheduleOutcome = .complete;
        const PendingCancellation = struct {
            prepared: terminal_runtime.PreparedSelectionExportCancellation,
            runtime: *terminal_runtime.TerminalRuntime,
        };
        const PendingBell = core_module.BellDeliveryToken;
        const PendingMutation = union(enum) {
            history: struct {
                attachment_id: core_module.AttachmentId,
                event: terminal_runtime.HistoryEvent,
                runtime: *terminal_runtime.TerminalRuntime,
            },
            selection: struct {
                attachment_id: core_module.AttachmentId,
                event: terminal_runtime.SelectionSnapshot,
                runtime: *terminal_runtime.TerminalRuntime,
            },
            selection_export_chunk: struct {
                runtime: *terminal_runtime.TerminalRuntime,
                token: terminal_runtime.SelectionExportChunkToken,
            },
        };
        var pending_cancellations: [max_scheduled_attachments_per_poll]PendingCancellation = undefined;
        var pending_cancellation_count: usize = 0;
        var pending_bells: [max_scheduled_attachments_per_poll]PendingBell = undefined;
        var pending_bell_count: usize = 0;
        var pending_mutations: [max_scheduled_attachments_per_poll * 3]PendingMutation = undefined;
        var pending_mutation_count: usize = 0;
        errdefer {
            var index = pending_mutation_count;
            while (index > 0) {
                index -= 1;
                switch (pending_mutations[index]) {
                    .history => |pending| pending.runtime.rollbackHistoryEvent(
                        pending.attachment_id,
                        pending.event,
                    ) catch unreachable,
                    .selection => |pending| pending.runtime.rollbackSelectionEvent(
                        pending.attachment_id,
                        pending.event,
                    ) catch unreachable,
                    .selection_export_chunk => |pending| pending.runtime.rollbackSelectionExportChunk(
                        pending.token,
                    ) catch unreachable,
                }
            }
        }
        scan: while (scanned < @min(count, max_schedule_scan_per_poll) and
            scheduled < max_scheduled_attachments_per_poll)
        {
            const index = (start + scanned) % count;
            scanned += 1;
            const attachment = core.attachmentValues()[index];
            if (!attachment.capabilities.read) continue;
            const runtime = aggregate.runtimeFor(attachment.terminal_id) orelse continue;
            const terminal = core.findTerminal(attachment.terminal_id) orelse continue;
            if (!terminal.hasReadableRuntimeState() or !runtime.readableStateAvailable()) continue;
            scheduled += 1;
            if (try core.prepareBellDelivery(
                connection_id,
                attachment.attachment_id,
                runtime.bellSequence(),
            )) |bell| {
                const payload = try control_encode.terminalBell(
                    allocator,
                    bell.attachment_id,
                    bell.sequence,
                );
                defer allocator.free(payload);
                switch (try outgoing.append(
                    MessageKind.terminal_bell,
                    .control_json,
                    payload,
                    .{ .event = core.epoch },
                )) {
                    .appended => {
                        std.debug.assert(pending_bell_count < pending_bells.len);
                        pending_bells[pending_bell_count] = bell;
                        pending_bell_count += 1;
                    },
                    .full => {
                        scanned -= 1;
                        outcome = .batch_full;
                        break :scan;
                    },
                }
            }
            const history_event = runtime.takeHistoryEvent(attachment.attachment_id) catch |err| switch (err) {
                error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => {
                    continue :scan;
                },
                else => return err,
            };
            if (history_event) |event| {
                var event_committed = false;
                defer if (!event_committed) {
                    runtime.rollbackHistoryEvent(attachment.attachment_id, event) catch unreachable;
                };
                const payload = try control_encode.historyChanged(allocator, event);
                defer allocator.free(payload);
                switch (try outgoing.append(
                    MessageKind.history_changed,
                    .control_json,
                    payload,
                    .{ .event = core.epoch },
                )) {
                    .appended => {
                        event_committed = true;
                        std.debug.assert(pending_mutation_count < pending_mutations.len);
                        pending_mutations[pending_mutation_count] = .{ .history = .{
                            .attachment_id = attachment.attachment_id,
                            .event = event,
                            .runtime = runtime,
                        } };
                        pending_mutation_count += 1;
                    },
                    .full => {
                        scanned -= 1;
                        outcome = .batch_full;
                        break :scan;
                    },
                }
            }
            const selection_event = runtime.takeSelectionEvent(attachment.attachment_id) catch |err| switch (err) {
                error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => {
                    continue :scan;
                },
                else => return err,
            };
            if (selection_event) |selection| {
                var event_committed = false;
                defer if (!event_committed) {
                    runtime.rollbackSelectionEvent(attachment.attachment_id, selection) catch unreachable;
                };
                switch (try appendSelectionChanged(
                    allocator,
                    outgoing,
                    attachment.attachment_id,
                    selection,
                    .{ .event = core.epoch },
                )) {
                    .appended => {
                        event_committed = true;
                        std.debug.assert(pending_mutation_count < pending_mutations.len);
                        pending_mutations[pending_mutation_count] = .{ .selection = .{
                            .attachment_id = attachment.attachment_id,
                            .event = selection,
                            .runtime = runtime,
                        } };
                        pending_mutation_count += 1;
                    },
                    .full => {
                        scanned -= 1;
                        outcome = .batch_full;
                        break :scan;
                    },
                }
            }
            const prepared_cancellation = runtime.prepareSelectionExportCancellation(
                attachment.attachment_id,
            ) catch |err| switch (err) {
                error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => {
                    continue :scan;
                },
                else => return err,
            };
            if (prepared_cancellation) |prepared| {
                const cancellation = prepared.cancellation;
                switch (try appendSelectionExportFinished(
                    allocator,
                    outgoing,
                    .{
                        .attachment_id = cancellation.attachment_id,
                        .byte_length = cancellation.byte_length,
                        .export_sequence = cancellation.export_sequence,
                    },
                    .cancelled,
                    .{ .event = core.epoch },
                )) {
                    .appended => {
                        // Commit the dequeue only after the complete batch is
                        // built. Any earlier error leaves it available to retry.
                        std.debug.assert(pending_cancellation_count < pending_cancellations.len);
                        pending_cancellations[pending_cancellation_count] = .{
                            .prepared = prepared,
                            .runtime = runtime,
                        };
                        pending_cancellation_count += 1;
                    },
                    .full => {
                        scanned -= 1;
                        outcome = .batch_full;
                        break :scan;
                    },
                }
            } else if (runtime.nextSelectionExportChunk(
                allocator,
                attachment.attachment_id,
            ) catch |err| switch (err) {
                error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => {
                    continue :scan;
                },
                else => return err,
            }) |chunk| {
                defer allocator.free(chunk.bytes);
                var chunk_committed = false;
                defer if (!chunk_committed) {
                    runtime.rollbackSelectionExportChunk(chunk.token) catch unreachable;
                };
                switch (try outgoing.append(
                    MessageKind.selection_export_chunk,
                    .raw_bytes,
                    chunk.bytes,
                    .{ .event = core.epoch },
                )) {
                    .appended => {
                        chunk_committed = true;
                        std.debug.assert(pending_mutation_count < pending_mutations.len);
                        pending_mutations[pending_mutation_count] = .{ .selection_export_chunk = .{
                            .runtime = runtime,
                            .token = chunk.token,
                        } };
                        pending_mutation_count += 1;
                    },
                    .full => {
                        scanned -= 1;
                        outcome = .batch_full;
                        break :scan;
                    },
                }
            }
        }
        self.notifyAfterCancellationRecordsAppended(pending_cancellation_count);
        // No fallible work follows these commits. Keeping them until
        // the complete interaction batch is built makes the dequeue atomic
        // with respect to every allocator failure in this scheduler.
        for (pending_cancellations[0..pending_cancellation_count]) |pending| {
            pending.runtime.commitPreparedSelectionExportCancellation(pending.prepared);
        }
        for (pending_bells[0..pending_bell_count]) |pending| {
            core.commitBellDelivery(pending);
        }
        self.scan_index = (start + scanned) % count;
        return outcome;
    }
};

pub const testing = if (target_builtin.is_test) struct {
    pub const SchedulerTestingHooksType = SchedulerTestingHooks;

    pub fn setSchedulerTestingHooks(
        scheduler: *Scheduler,
        hooks: SchedulerTestingHooks,
    ) void {
        scheduler.testing_hooks = hooks;
    }
} else struct {};

pub fn appendSelectionChanged(
    allocator: std.mem.Allocator,
    outgoing: *host_outgoing.Outgoing,
    attachment_id: core_module.AttachmentId,
    snapshot: terminal_runtime.SelectionSnapshot,
    delivery: host_outgoing.Delivery,
) !host_outgoing.AppendOutcome {
    const payload = try control_encode.selectionChanged(allocator, attachment_id, snapshot);
    defer allocator.free(payload);
    return outgoing.append(
        MessageKind.selection_changed,
        .control_json,
        payload,
        delivery,
    );
}

pub fn appendSelectionExportFinished(
    allocator: std.mem.Allocator,
    outgoing: *host_outgoing.Outgoing,
    finished: terminal_runtime.SelectionExportFinished,
    status: SelectionExportStatus,
    delivery: host_outgoing.Delivery,
) !host_outgoing.AppendOutcome {
    const payload = try control_encode.selectionExportFinished(allocator, finished, status);
    defer allocator.free(payload);
    return outgoing.append(
        MessageKind.selection_export_finished,
        .control_json,
        payload,
        delivery,
    );
}
