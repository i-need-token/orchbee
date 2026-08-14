const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const max_export_bytes = protocol.Limits.selection_export_bytes;
const max_chunk_bytes = protocol.Limits.selection_export_chunk_bytes;
const chunk_header_bytes = 32;

// One queued cancellation may precede a newer export; invalidating that newer
// export produces the only second entry before the queue must be drained.
pub const max_pending_cancellations = 2;

pub const AttachmentId = identifiers.AttachmentId;

pub const Format = enum {
    html,
    plain,
    vt,
};

pub const Started = struct {
    attachment_id: AttachmentId,
    byte_length: usize,
    export_sequence: u64,
    format: Format,
};

pub const StartTransaction = struct {
    previous_completed_export_sequence: ?u64,
    previous_last_export_sequence: u64,
    started: Started,
};

pub const Finished = struct {
    attachment_id: AttachmentId,
    byte_length: usize,
    export_sequence: u64,
};

pub const Cancellation = struct {
    attachment_id: AttachmentId,
    byte_length: usize,
    export_sequence: u64,
};

pub const PreparedCancellation = struct {
    cancellation: Cancellation,
};

pub const ChunkToken = struct {
    attachment_id: AttachmentId,
    chunk_sequence: u32,
    export_sequence: u64,
};

pub const EncodedChunk = struct {
    bytes: []u8,
    token: ChunkToken,
};

pub const InvalidationSnapshot = struct {
    count: usize,
    entries: [max_pending_cancellations]Cancellation,
};

pub const Error = error{
    ExportConflict,
    ExportTooLarge,
    InvalidAcknowledgement,
    InvalidRequest,
};

pub const Active = struct {
    attachment_id: AttachmentId,
    bytes_length: usize,
    bytes_offset: usize,
    chunk_in_flight: bool = false,
    chunk_length: usize = 0,
    chunk_sequence: u32 = 0,
    export_sequence: u64,
    format: Format,
    next_offset: usize = 0,
    storage: []u8,
};

/// Attachment-local selection export protocol state. It owns formatted bytes,
/// monotonic export and chunk identities, invalidation history, and the bounded
/// cancellation FIFO. Selection validity remains the history manager's fact.
pub const State = struct {
    active: ?Active = null,
    invalidated: [max_pending_cancellations]Cancellation = undefined,
    invalidated_count: usize = 0,
    last_completed_sequence: ?u64 = null,
    last_sequence: u64 = 0,
    pending_cancellations: [max_pending_cancellations]Cancellation = undefined,
    pending_cancellation_count: usize = 0,

    pub fn deinit(self: *State, allocator: std.mem.Allocator) void {
        self.releaseActive(allocator);
        self.* = undefined;
    }

    pub fn validateStart(self: *const State, export_sequence: u64) Error!void {
        if (export_sequence == 0 or export_sequence <= self.last_sequence) {
            return error.InvalidRequest;
        }
        if (self.pending_cancellation_count == max_pending_cancellations or
            self.active != null)
        {
            return error.ExportConflict;
        }
    }

    pub fn start(
        self: *State,
        allocator: std.mem.Allocator,
        terminal: *ghostty_vt.Terminal,
        selection: ghostty_vt.Selection,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: Format,
    ) (Error || std.mem.Allocator.Error)!Started {
        try self.validateStart(export_sequence);

        const storage = try allocator.alloc(u8, max_export_bytes + 1);
        errdefer allocator.free(storage);
        var fixed: std.heap.FixedBufferAllocator = .init(storage);
        var writer: std.Io.Writer.Allocating = .init(fixed.allocator());
        defer writer.deinit();
        var formatter: ghostty_vt.formatter.ScreenFormatter = .init(
            terminal.screens.active,
            .{
                .emit = switch (format) {
                    .html => .html,
                    .plain => .plain,
                    .vt => .vt,
                },
                .palette = &terminal.colors.palette.current,
                .trim = true,
                .unwrap = true,
            },
        );
        formatter.content = .{ .selection = selection };
        formatter.format(&writer.writer) catch return error.ExportTooLarge;
        const formatted = writer.toOwnedSliceSentinel(0) catch return error.ExportTooLarge;
        if (formatted.len > max_export_bytes) return error.ExportTooLarge;
        const bytes_offset = @intFromPtr(formatted.ptr) - @intFromPtr(storage.ptr);
        self.active = .{
            .attachment_id = attachment_id,
            .bytes_length = formatted.len,
            .bytes_offset = bytes_offset,
            .export_sequence = export_sequence,
            .format = format,
            .storage = storage,
        };
        // A newer export never clears the completion watermark of the last
        // completed export. Its final acknowledgement remains idempotent.
        self.last_sequence = export_sequence;
        return .{
            .attachment_id = attachment_id,
            .byte_length = formatted.len,
            .export_sequence = export_sequence,
            .format = format,
        };
    }

    pub fn startTransaction(
        self: *State,
        allocator: std.mem.Allocator,
        terminal: *ghostty_vt.Terminal,
        selection: ghostty_vt.Selection,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: Format,
    ) (Error || std.mem.Allocator.Error)!StartTransaction {
        const previous_completed_export_sequence = self.last_completed_sequence;
        const previous_last_export_sequence = self.last_sequence;
        return .{
            .previous_completed_export_sequence = previous_completed_export_sequence,
            .previous_last_export_sequence = previous_last_export_sequence,
            .started = try self.start(
                allocator,
                terminal,
                selection,
                attachment_id,
                export_sequence,
                format,
            ),
        };
    }

    pub fn takeCancellation(self: *State) ?Cancellation {
        if (self.pending_cancellation_count == 0) return null;
        const cancellation = self.pending_cancellations[0];
        self.removeCancellationHead();
        return cancellation;
    }

    /// Prepares the exact FIFO head without consuming it. The owner commits
    /// only after the matching event joins a complete outbound batch.
    pub fn prepareCancellation(self: *const State) ?PreparedCancellation {
        if (self.pending_cancellation_count == 0) return null;
        return .{ .cancellation = self.pending_cancellations[0] };
    }

    pub fn commitPreparedCancellation(
        self: *State,
        prepared: PreparedCancellation,
    ) void {
        std.debug.assert(self.pending_cancellation_count > 0);
        std.debug.assert(sameCancellation(self.pending_cancellations[0], prepared.cancellation));
        self.removeCancellationHead();
    }

    pub fn snapshotInvalidations(self: *const State) InvalidationSnapshot {
        var snapshot = std.mem.zeroes(InvalidationSnapshot);
        snapshot.count = self.invalidated_count;
        @memcpy(snapshot.entries[0..snapshot.count], self.invalidated[0..snapshot.count]);
        return snapshot;
    }

    pub fn restoreInvalidations(self: *State, snapshot: InvalidationSnapshot) void {
        std.debug.assert(snapshot.count <= max_pending_cancellations);
        @memcpy(self.invalidated[0..snapshot.count], snapshot.entries[0..snapshot.count]);
        self.invalidated_count = snapshot.count;
    }

    pub fn restoreCancellations(self: *State, cancellations: []const Cancellation) Error!void {
        if (self.pending_cancellation_count != 0 or
            cancellations.len > max_pending_cancellations)
        {
            return error.InvalidRequest;
        }
        @memcpy(self.pending_cancellations[0..cancellations.len], cancellations);
        self.pending_cancellation_count = cancellations.len;
    }

    pub fn rollbackStart(
        self: *State,
        allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
        export_sequence: u64,
        previous_completed_export_sequence: ?u64,
        previous_last_export_sequence: u64,
        cancellations: []const Cancellation,
        invalidation_snapshot: InvalidationSnapshot,
    ) void {
        std.debug.assert(cancellations.len <= max_pending_cancellations);
        if (previous_last_export_sequence >= export_sequence or
            self.last_sequence != export_sequence)
        {
            std.debug.assert(false);
            return;
        }
        if (self.active) |active| {
            if (active.export_sequence != export_sequence or
                self.pending_cancellation_count != 0)
            {
                std.debug.assert(false);
                return;
            }
            self.releaseActive(allocator);
        } else if (self.pending_cancellation_count == 1 and
            sameExport(self.pending_cancellations[0], attachment_id, export_sequence))
        {
            // A reader may invalidate the unannounced replacement while its
            // response is built. That cancellation is not part of the prior FIFO.
            self.pending_cancellation_count = 0;
        } else {
            std.debug.assert(false);
            return;
        }
        std.debug.assert(self.pending_cancellation_count == 0);
        @memcpy(self.pending_cancellations[0..cancellations.len], cancellations);
        self.pending_cancellation_count = cancellations.len;
        self.last_completed_sequence = previous_completed_export_sequence;
        self.last_sequence = previous_last_export_sequence;
        self.restoreInvalidations(invalidation_snapshot);
    }

    pub fn invalidate(self: *State, allocator: std.mem.Allocator) void {
        const active = self.active orelse return;
        const cancellation: Cancellation = .{
            .attachment_id = active.attachment_id,
            .byte_length = active.bytes_length,
            .export_sequence = active.export_sequence,
        };
        std.debug.assert(self.pending_cancellation_count < max_pending_cancellations);
        self.rememberInvalidation(cancellation);
        self.pending_cancellations[self.pending_cancellation_count] = cancellation;
        self.pending_cancellation_count += 1;
        self.releaseActive(allocator);
    }

    pub fn encodeNextChunk(
        self: *State,
        output_allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
    ) std.mem.Allocator.Error!?EncodedChunk {
        const active = if (self.active) |*value| value else return null;
        const empty_first_chunk = active.bytes_length == 0 and active.chunk_sequence == 0;
        if (active.chunk_in_flight or
            (active.next_offset >= active.bytes_length and !empty_first_chunk))
        {
            return null;
        }
        const chunk_length = @min(max_chunk_bytes, active.bytes_length - active.next_offset);
        const final = active.next_offset + chunk_length == active.bytes_length;
        const bytes = try output_allocator.alloc(u8, chunk_header_bytes + chunk_length);
        errdefer output_allocator.free(bytes);
        @memcpy(bytes[0..16], &attachment_id.bytes);
        std.mem.writeInt(u64, bytes[16..24], active.export_sequence, .little);
        std.mem.writeInt(u32, bytes[24..28], active.chunk_sequence, .little);
        bytes[28] = if (final) 1 else 0;
        @memset(bytes[29..32], 0);
        const source_start = active.bytes_offset + active.next_offset;
        @memcpy(
            bytes[chunk_header_bytes..],
            active.storage[source_start..][0..chunk_length],
        );
        active.chunk_in_flight = true;
        active.chunk_length = chunk_length;
        return .{
            .bytes = bytes,
            .token = .{
                .attachment_id = attachment_id,
                .chunk_sequence = active.chunk_sequence,
                .export_sequence = active.export_sequence,
            },
        };
    }

    pub fn rollbackChunk(self: *State, expected: ChunkToken) Error!void {
        // Runtime output may invalidate an encoded chunk before handoff. The
        // cancellation state then already supersedes it.
        const active = if (self.active) |*value| value else return;
        if (!active.chunk_in_flight or
            active.export_sequence != expected.export_sequence or
            active.chunk_sequence != expected.chunk_sequence)
        {
            return error.InvalidAcknowledgement;
        }
        active.chunk_in_flight = false;
        active.chunk_length = 0;
    }

    pub fn chunkApplied(
        self: *State,
        allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
        export_sequence: u64,
        chunk_sequence: u32,
    ) Error!?Finished {
        if (export_sequence == 0) return error.InvalidAcknowledgement;
        for (self.invalidated[0..self.invalidated_count]) |cancellation| {
            if (sameExport(cancellation, attachment_id, export_sequence)) return null;
        }
        // Replaying the last completed export never advances or clears a newer
        // export's offset, chunk identity, or in-flight state.
        if (self.last_completed_sequence) |completed| {
            if (completed == export_sequence) return null;
        }
        const active = if (self.active) |*value| value else return error.InvalidAcknowledgement;
        if (export_sequence != active.export_sequence) return error.InvalidAcknowledgement;
        if (chunk_sequence < active.chunk_sequence) return null;
        if (!active.chunk_in_flight or chunk_sequence != active.chunk_sequence) {
            return error.InvalidAcknowledgement;
        }
        const next_offset = active.next_offset + active.chunk_length;
        const finished_export = next_offset >= active.bytes_length;
        const next_chunk_sequence = if (finished_export)
            active.chunk_sequence
        else
            std.math.add(u32, active.chunk_sequence, 1) catch
                return error.InvalidAcknowledgement;
        active.next_offset = next_offset;
        active.chunk_length = 0;
        active.chunk_in_flight = false;
        active.chunk_sequence = next_chunk_sequence;
        if (!finished_export) return null;
        const finished: Finished = .{
            .attachment_id = attachment_id,
            .byte_length = active.bytes_length,
            .export_sequence = export_sequence,
        };
        self.last_completed_sequence = export_sequence;
        self.releaseActive(allocator);
        return finished;
    }

    pub fn cancel(
        self: *State,
        allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
        export_sequence: u64,
    ) Error!Finished {
        const active = self.active orelse return error.InvalidAcknowledgement;
        if (active.export_sequence != export_sequence) return error.InvalidAcknowledgement;
        const finished: Finished = .{
            .attachment_id = attachment_id,
            .byte_length = active.bytes_length,
            .export_sequence = export_sequence,
        };
        self.releaseActive(allocator);
        return finished;
    }

    fn removeCancellationHead(self: *State) void {
        if (self.pending_cancellation_count > 1) {
            std.mem.copyForwards(
                Cancellation,
                self.pending_cancellations[0 .. self.pending_cancellation_count - 1],
                self.pending_cancellations[1..self.pending_cancellation_count],
            );
        }
        self.pending_cancellation_count -= 1;
    }

    fn rememberInvalidation(self: *State, cancellation: Cancellation) void {
        if (self.invalidated_count < max_pending_cancellations) {
            self.invalidated[self.invalidated_count] = cancellation;
            self.invalidated_count += 1;
            return;
        }
        for (1..max_pending_cancellations) |index| {
            self.invalidated[index - 1] = self.invalidated[index];
        }
        self.invalidated[max_pending_cancellations - 1] = cancellation;
    }

    fn releaseActive(self: *State, allocator: std.mem.Allocator) void {
        if (self.active) |active| allocator.free(active.storage);
        self.active = null;
    }
};

fn sameExport(
    cancellation: Cancellation,
    attachment_id: AttachmentId,
    export_sequence: u64,
) bool {
    return cancellation.export_sequence == export_sequence and
        cancellation.attachment_id.eql(attachment_id);
}

fn sameCancellation(left: Cancellation, right: Cancellation) bool {
    return left.byte_length == right.byte_length and
        left.export_sequence == right.export_sequence and
        left.attachment_id.eql(right.attachment_id);
}
