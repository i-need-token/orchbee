const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const terminal_history = terminal_engine.history;
const terminal_runtime = terminal_engine.runtime;

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
    event: terminal_history.SelectionGestureEvent,
    generation: u64,
    revision: u64,
};

pub const SelectionCommandRequest = struct {
    command: terminal_history.SelectionCommand,
    generation: u64,
    revision: u64,
};

const SelectionExportHandoffState = union(enum) {
    provisional: struct {
        result: terminal_runtime.SelectionExportStartResult,
        runtime: *terminal_runtime.TerminalRuntime,
    },
    committed,
};

/// A request-scoped interaction capability produced only after the aggregate
/// has resolved a physical connection, read authority, and readable runtime.
/// It owns no Core authority and must not outlive the request dispatch.
pub const ReadableAttachment = struct {
    attachment_id: core_module.AttachmentId,
    runtime: *terminal_runtime.TerminalRuntime,

    pub fn init(
        attachment_id: core_module.AttachmentId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) ReadableAttachment {
        return .{ .attachment_id = attachment_id, .runtime = runtime };
    }

    pub fn readRows(
        self: ReadableAttachment,
        allocator: std.mem.Allocator,
        request: ReadRowsRequest,
    ) !terminal_runtime.HistoryRows {
        return self.runtime.readHistoryRows(allocator, .{
            .attachment_id = self.attachment_id,
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
        return self.runtime.readHistoryRowsInto(scratch_allocator, destination, .{
            .attachment_id = self.attachment_id,
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
        try self.runtime.historyRowsApplied(
            self.attachment_id,
            request_sequence,
            revision,
            generation,
        );
    }

    pub fn setHistoryDemand(
        self: ReadableAttachment,
        demand: terminal_runtime.HistoryDemand,
    ) !void {
        try self.runtime.setHistoryDemand(self.attachment_id, demand);
    }

    pub fn historyObserved(
        self: ReadableAttachment,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) !void {
        try self.runtime.historyObserved(
            self.attachment_id,
            sequence,
            revision,
            generation,
        );
    }

    pub fn applySelectionGesture(
        self: ReadableAttachment,
        request: SelectionGestureRequest,
    ) !terminal_runtime.SelectionSnapshot {
        return self.runtime.applySelectionGesture(.{
            .attachment_id = self.attachment_id,
            .event = request.event,
            .generation = request.generation,
            .revision = request.revision,
        });
    }

    pub fn applySelectionCommand(
        self: ReadableAttachment,
        request: SelectionCommandRequest,
    ) !terminal_runtime.SelectionSnapshot {
        return self.runtime.applySelectionCommand(.{
            .attachment_id = self.attachment_id,
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
                .result = try self.runtime.startSelectionExport(
                    self.attachment_id,
                    export_sequence,
                    format,
                ),
                .runtime = self.runtime,
            } },
        };
    }

    pub fn selectionExportChunkApplied(
        self: ReadableAttachment,
        export_sequence: u64,
        chunk_sequence: u32,
    ) !?terminal_runtime.SelectionExportFinished {
        return self.runtime.selectionExportChunkApplied(
            self.attachment_id,
            export_sequence,
            chunk_sequence,
        );
    }

    pub fn cancelSelectionExport(
        self: ReadableAttachment,
        export_sequence: u64,
    ) !terminal_runtime.SelectionExportFinished {
        return self.runtime.cancelSelectionExport(
            self.attachment_id,
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
