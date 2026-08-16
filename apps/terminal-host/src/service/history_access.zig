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

/// A request-scoped history capability produced only after the aggregate has
/// resolved a physical connection, read authority, and readable runtime.
pub const HistoryAccess = struct {
    attachment_id: core_module.AttachmentId,
    runtime: *terminal_runtime.TerminalRuntime,

    pub fn init(
        attachment_id: core_module.AttachmentId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) HistoryAccess {
        return .{ .attachment_id = attachment_id, .runtime = runtime };
    }

    pub fn readRows(
        self: HistoryAccess,
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
        self: HistoryAccess,
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
        self: HistoryAccess,
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
        self: HistoryAccess,
        demand: terminal_runtime.HistoryDemand,
    ) !void {
        try self.runtime.setHistoryDemand(self.attachment_id, demand);
    }

    pub fn historyObserved(
        self: HistoryAccess,
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
};
