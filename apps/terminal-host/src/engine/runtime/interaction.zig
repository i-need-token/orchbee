const std = @import("std");
const monotonic = @import("terminal-host-platform").monotonic;
const identifiers = @import("terminal-host-model").identifiers;
const terminal_history = @import("../history/root.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const failure_ops = @import("failure.zig");

pub fn setHistoryDemand(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    demand: contracts.HistoryDemand,
) !void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    try self.history.setDemand(&self.terminal, attachment_id, demand);
}

pub fn readHistoryRows(
    self: *state.State,
    output_allocator: std.mem.Allocator,
    request: contracts.HistoryReadRequest,
) !contracts.HistoryRows {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    self.history.animation_tick = self.animation_tick;
    return self.history.readRows(
        output_allocator,
        &self.terminal,
        self.revision.load(.monotonic),
        self.generation,
        self.cell_pixel_width,
        self.cell_pixel_height,
        request,
    );
}

pub fn readHistoryRowsInto(
    self: *state.State,
    scratch_allocator: std.mem.Allocator,
    destination: []u8,
    request: contracts.HistoryReadRequest,
) !contracts.HistoryRows {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    self.history.animation_tick = self.animation_tick;
    return self.history.readRowsInto(
        scratch_allocator,
        destination,
        &self.terminal,
        self.revision.load(.monotonic),
        self.generation,
        self.cell_pixel_width,
        self.cell_pixel_height,
        request,
    );
}

pub fn historyRowsApplied(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    request_sequence: u64,
    revision: u64,
    generation: u64,
) !void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    try self.history.rowsApplied(
        &self.terminal,
        attachment_id,
        request_sequence,
        revision,
        generation,
    );
}

pub fn takeHistoryEvent(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
) !?contracts.HistoryEvent {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.takeHistoryEvent(
        &self.terminal,
        attachment_id,
        self.revision.load(.monotonic),
        self.generation,
    );
}

pub fn rollbackHistoryEvent(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    event: contracts.HistoryEvent,
) !void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    try self.history.rollbackHistoryEvent(attachment_id, event);
}

pub fn historyObserved(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
) !void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    try self.history.historyObserved(attachment_id, sequence, revision, generation);
}

pub fn takeSelectionEvent(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
) !?contracts.SelectionSnapshot {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.takeSelectionEvent(attachment_id);
}

pub fn rollbackSelectionEvent(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    event: contracts.SelectionSnapshot,
) !void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    try self.history.rollbackSelectionEvent(attachment_id, event);
}

pub fn takeSelectionExportCancellation(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
) !?contracts.SelectionExportCancellation {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.takeSelectionExportCancellation(attachment_id);
}

pub fn prepareSelectionExportCancellation(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
) !?contracts.PreparedSelectionExportCancellation {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.prepareSelectionExportCancellation(attachment_id);
}

pub fn commitPreparedSelectionExportCancellation(
    self: *state.State,
    prepared: contracts.PreparedSelectionExportCancellation,
) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    self.history.commitPreparedSelectionExportCancellation(prepared);
}

pub fn applySelectionGesture(
    self: *state.State,
    request: contracts.SelectionGesture,
) !contracts.SelectionSnapshot {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.applyGesture(
        &self.terminal,
        self.generation,
        request,
    );
}

pub fn applySelectionCommand(
    self: *state.State,
    request: contracts.SelectionCommandRequest,
) !contracts.SelectionSnapshot {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.applySelectionCommand(
        &self.terminal,
        self.revision.load(.monotonic),
        self.generation,
        request,
    );
}

pub fn startSelectionExport(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    export_sequence: u64,
    format: contracts.SelectionExportFormat,
) !contracts.SelectionExportStartResult {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    var lock_timer = monotonic.Timer.start() catch null;
    // The reader can invalidate an export between IPC dispatches. Keep
    // cancellation extraction and the replacement start in one state transition;
    // HostService serializes the returned cancellation records before the start response.
    const invalidation_history = try self.history.snapshotInvalidatedExportHistory(attachment_id);
    var cancellations = std.mem.zeroes(
        [terminal_history.max_pending_export_cancellations]contracts.SelectionExportCancellation,
    );
    var cancellation_count: usize = 0;
    while (true) {
        const cancellation = (try self.history.takeSelectionExportCancellation(attachment_id)) orelse break;
        std.debug.assert(
            cancellation_count < terminal_history.max_pending_export_cancellations,
        );
        cancellations[cancellation_count] = cancellation;
        cancellation_count += 1;
    }
    const start_transaction = self.history.startExportTransaction(
        &self.terminal,
        attachment_id,
        export_sequence,
        format,
    ) catch |err| {
        self.history.restoreInvalidatedExportHistory(attachment_id, invalidation_history);
        self.history.restoreSelectionExportCancellations(
            attachment_id,
            cancellations[0..cancellation_count],
        ) catch |restore_err| return restore_err;
        return err;
    };
    const elapsed = if (lock_timer) |*timer| timer.read() else 0;
    return .{
        .cancellation_count = cancellation_count,
        .cancellations = cancellations,
        .invalidation_history = invalidation_history,
        .lock_hold_nanoseconds = elapsed,
        .previous_completed_export_sequence = start_transaction.previous_completed_export_sequence,
        .previous_last_export_sequence = start_transaction.previous_last_export_sequence,
        .started = start_transaction.started,
    };
}

pub fn rollbackSelectionExportStart(
    self: *state.State,
    result: contracts.SelectionExportStartResult,
) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    self.history.rollbackExportStart(
        result.started.attachment_id,
        result.started.export_sequence,
        result.previous_completed_export_sequence,
        result.previous_last_export_sequence,
        result.cancellations[0..result.cancellation_count],
        result.invalidation_history,
    );
}

pub fn nextSelectionExportChunk(
    self: *state.State,
    output_allocator: std.mem.Allocator,
    attachment_id: identifiers.AttachmentId,
) !?contracts.SelectionExportChunk {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.encodeNextExportChunk(output_allocator, attachment_id);
}

pub fn rollbackSelectionExportChunk(
    self: *state.State,
    token: contracts.SelectionExportChunkToken,
) !void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    try self.history.rollbackExportChunk(token);
}

pub fn selectionExportChunkApplied(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    export_sequence: u64,
    chunk_sequence: u32,
) !?contracts.SelectionExportFinished {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.exportChunkApplied(
        attachment_id,
        export_sequence,
        chunk_sequence,
    );
}

pub fn cancelSelectionExport(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    export_sequence: u64,
) !contracts.SelectionExportFinished {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.history.cancelExport(attachment_id, export_sequence);
}
