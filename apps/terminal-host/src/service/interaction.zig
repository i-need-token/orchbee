const std = @import("std");

const core_module = @import("terminal-host-core").state;
const control_encode = @import("terminal-host-control").encode;
const host_outgoing = @import("outgoing.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const protocol = @import("terminal-protocol");
const engine = @import("terminal-host-engine");
const terminal_history = engine.history;
const terminal_runtime = engine.runtime;

const max_schedule_scan_per_poll = 64;
const max_scheduled_attachments_per_poll = 8;

const MessageKind = protocol.MessageKind;

pub const ScheduleOutcome = host_outgoing.ScheduleOutcome;

pub const AccessError = error{
    ReadableAttachmentUnavailable,
    TerminalRuntimeMissing,
};

pub const ReadRowsRequest = struct {
    detail: terminal_history.Detail,
    generation: u64,
    host_epoch: core_module.HostEpoch,
    maximum_bytes: usize,
    origin: terminal_history.Origin,
    purpose: terminal_history.Purpose,
    request_sequence: u64,
    revision: u64,
    row_count: u16,
};

pub const SelectionGestureRequest = struct {
    action: terminal_history.SelectionGestureAction,
    click_count: u2,
    generation: u64,
    point: terminal_history.SelectionPoint,
    rectangle: bool,
    revision: u64,
};

pub const SelectionCommandRequest = struct {
    command: terminal_history.SelectionCommand,
    generation: u64,
    revision: u64,
};

pub const SelectionExportStatus = enum {
    cancelled,
    completed,
};

const ReadableAttachmentState = struct {
    attachment_id: core_module.AttachmentId,
    runtime: *terminal_runtime.TerminalRuntime,
};

const SelectionExportHandoffState = union(enum) {
    provisional: struct {
        result: terminal_runtime.SelectionExportStartResult,
        runtime: *terminal_runtime.TerminalRuntime,
    },
    committed,
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
        const attachment = self.aggregate.core().resolveAttachment(
            connection_id,
            attachment_id,
            .read,
        ) catch return error.ReadableAttachmentUnavailable;
        const terminal = self.aggregate.core().findTerminal(attachment.terminal_id) orelse
            return error.ReadableAttachmentUnavailable;
        const runtime = self.aggregate.runtimeFor(attachment.terminal_id) orelse
            return error.TerminalRuntimeMissing;
        if (!terminal.hasReadableRuntimeState() or !runtime.readableStateAvailable()) {
            return error.ReadableAttachmentUnavailable;
        }
        return .{
            .state = .{
                .attachment_id = attachment_id,
                .runtime = runtime,
            },
        };
    }
};

/// A request-scoped capability produced only after the physical connection
/// and read permission resolve to a retained runtime. HostService does not
/// retain it across another dispatch or attachment transition.
pub const ReadableAttachment = struct {
    state: ReadableAttachmentState,

    pub fn readRows(
        self: ReadableAttachment,
        allocator: std.mem.Allocator,
        request: ReadRowsRequest,
    ) !terminal_runtime.HistoryRows {
        return self.state.runtime.readHistoryRows(allocator, .{
            .attachment_id = self.state.attachment_id,
            .detail = request.detail,
            .generation = request.generation,
            .host_epoch = request.host_epoch,
            .maximum_bytes = request.maximum_bytes,
            .origin = request.origin,
            .purpose = request.purpose,
            .request_sequence = request.request_sequence,
            .revision = request.revision,
            .row_count = request.row_count,
        });
    }

    pub fn readRowsInto(
        self: ReadableAttachment,
        scratch_allocator: std.mem.Allocator,
        destination: []u8,
        request: ReadRowsRequest,
    ) !terminal_runtime.HistoryRows {
        return self.state.runtime.readHistoryRowsInto(scratch_allocator, destination, .{
            .attachment_id = self.state.attachment_id,
            .detail = request.detail,
            .generation = request.generation,
            .host_epoch = request.host_epoch,
            .maximum_bytes = request.maximum_bytes,
            .origin = request.origin,
            .purpose = request.purpose,
            .request_sequence = request.request_sequence,
            .revision = request.revision,
            .row_count = request.row_count,
        });
    }

    pub fn rowsApplied(
        self: ReadableAttachment,
        request_sequence: u64,
        revision: u64,
        generation: u64,
    ) !void {
        try self.state.runtime.historyRowsApplied(
            self.state.attachment_id,
            request_sequence,
            revision,
            generation,
        );
    }

    pub fn setHistoryDemand(
        self: ReadableAttachment,
        demand: terminal_runtime.HistoryDemand,
    ) !void {
        try self.state.runtime.setHistoryDemand(self.state.attachment_id, demand);
    }

    pub fn historyObserved(
        self: ReadableAttachment,
        revision: u64,
        generation: u64,
    ) !void {
        try self.state.runtime.historyObserved(self.state.attachment_id, revision, generation);
    }

    pub fn applySelectionGesture(
        self: ReadableAttachment,
        request: SelectionGestureRequest,
    ) !terminal_runtime.SelectionSnapshot {
        return self.state.runtime.applySelectionGesture(.{
            .action = request.action,
            .attachment_id = self.state.attachment_id,
            .click_count = request.click_count,
            .generation = request.generation,
            .point = request.point,
            .rectangle = request.rectangle,
            .revision = request.revision,
        });
    }

    pub fn applySelectionCommand(
        self: ReadableAttachment,
        request: SelectionCommandRequest,
    ) !terminal_runtime.SelectionSnapshot {
        return self.state.runtime.applySelectionCommand(.{
            .attachment_id = self.state.attachment_id,
            .command = request.command,
            .generation = request.generation,
            .revision = request.revision,
        });
    }

    pub fn startSelectionExport(
        self: ReadableAttachment,
        export_sequence: u64,
        format: terminal_runtime.SelectionExportFormat,
    ) !SelectionExportHandoff {
        return .{
            .state = .{ .provisional = .{
                .result = try self.state.runtime.startSelectionExport(
                    self.state.attachment_id,
                    export_sequence,
                    format,
                ),
                .runtime = self.state.runtime,
            } },
        };
    }

    pub fn selectionExportChunkApplied(
        self: ReadableAttachment,
        export_sequence: u64,
        chunk_sequence: u32,
    ) !?terminal_runtime.SelectionExportFinished {
        return self.state.runtime.selectionExportChunkApplied(
            self.state.attachment_id,
            export_sequence,
            chunk_sequence,
        );
    }

    pub fn cancelSelectionExport(
        self: ReadableAttachment,
        export_sequence: u64,
    ) !terminal_runtime.SelectionExportFinished {
        return self.state.runtime.cancelSelectionExport(
            self.state.attachment_id,
            export_sequence,
        );
    }
};

pub const SelectionExportHandoff = struct {
    state: SelectionExportHandoffState,

    pub fn result(self: *const SelectionExportHandoff) terminal_runtime.SelectionExportStartResult {
        return switch (self.state) {
            .provisional => |provisional| provisional.result,
            .committed => unreachable,
        };
    }

    pub fn commit(self: *SelectionExportHandoff) void {
        switch (self.state) {
            .provisional => self.state = .committed,
            .committed => unreachable,
        }
    }

    pub fn deinit(self: *SelectionExportHandoff) void {
        switch (self.state) {
            .provisional => |provisional| provisional.runtime.rollbackSelectionExportStart(
                provisional.result,
            ),
            .committed => {},
        }
        self.* = undefined;
    }
};

pub const Scheduler = struct {
    scan_index: usize = 0,

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
            attachment_id: core_module.AttachmentId,
            cancellation: terminal_runtime.SelectionExportCancellation,
            runtime: *terminal_runtime.TerminalRuntime,
        };
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
            const pending_cancellation = runtime.peekSelectionExportCancellation(
                attachment.attachment_id,
            ) catch |err| switch (err) {
                error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => {
                    continue :scan;
                },
                else => return err,
            };
            if (pending_cancellation) |cancellation| {
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
                            .attachment_id = attachment.attachment_id,
                            .cancellation = cancellation,
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
        // No fallible work follows these acknowledgements. Keeping them until
        // the complete interaction batch is built makes the dequeue atomic
        // with respect to every allocator failure in this scheduler.
        for (pending_cancellations[0..pending_cancellation_count]) |pending| {
            pending.runtime.acknowledgeSelectionExportCancellation(
                pending.attachment_id,
                pending.cancellation,
            ) catch unreachable;
        }
        self.scan_index = (start + scanned) % count;
        return outcome;
    }
};

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
