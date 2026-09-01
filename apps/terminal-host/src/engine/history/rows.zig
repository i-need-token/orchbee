const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const row_encoding = @import("../row_encoding.zig");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const max_row_window_rows = contracts.max_row_window_rows;
const max_pending_export_cancellations = contracts.max_pending_export_cancellations;
const AttachmentId = contracts.AttachmentId;
const HostEpoch = contracts.HostEpoch;
const Detail = contracts.Detail;
const Purpose = contracts.Purpose;
const Origin = contracts.Origin;
const ReadRequest = contracts.ReadRequest;
const EncodedRows = contracts.EncodedRows;
const OwnedEncodedRows = contracts.OwnedEncodedRows;
const HistoryDemand = contracts.HistoryDemand;
const AnchorStatus = contracts.AnchorStatus;
const HistoryEvent = contracts.HistoryEvent;
const SelectionPoint = contracts.SelectionPoint;
const SelectionGestureGeometry = contracts.SelectionGestureGeometry;
const SelectionGesturePosition = contracts.SelectionGesturePosition;
const SelectionGestureEvent = contracts.SelectionGestureEvent;
const SelectionGesture = contracts.SelectionGesture;
const SelectionCommand = contracts.SelectionCommand;
const SelectionCommandRequest = contracts.SelectionCommandRequest;
const SelectionSnapshot = contracts.SelectionSnapshot;
const ExportFormat = contracts.ExportFormat;
const ExportStarted = contracts.ExportStarted;
const ExportStartTransaction = contracts.ExportStartTransaction;
const ExportFinished = contracts.ExportFinished;
const ExportCancellation = contracts.ExportCancellation;
const PreparedExportCancellation = contracts.PreparedExportCancellation;
const ExportChunkToken = contracts.ExportChunkToken;
const EncodedExportChunk = contracts.EncodedExportChunk;
const ExportInvalidationSnapshot = contracts.ExportInvalidationSnapshot;
const HistoryError = contracts.HistoryError;
const AttachmentState = storage.AttachmentState;
const RowEncodingTarget = union(enum) {
    allocated: struct {
        allocator: std.mem.Allocator,
        output_prefix_bytes: usize,
    },
    fixed: struct {
        destination: []u8,
        scratch_allocator: std.mem.Allocator,
    },
};
const RowEncodingOutput = struct {
    allocation: ?[]u8,
    bytes: []u8,
};
const ReadRowsOutput = struct {
    allocation: ?[]u8,
    encoded: EncodedRows,
};
const max_row_window_bytes = row_encoding.max_bytes;
const row_window_header_bytes = row_encoding.header_bytes;
const max_row_window_entries = row_encoding.max_entries;

pub fn readRows(
    self: anytype,
    output_allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    current_revision: u64,
    current_generation: u64,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    request: ReadRequest,
) (HistoryError || std.mem.Allocator.Error)!EncodedRows {
    const owned = try readRowsOwned(
        self,
        output_allocator,
        0,
        terminal,
        current_revision,
        current_generation,
        cell_pixel_width,
        cell_pixel_height,
        request,
    );
    std.debug.assert(owned.allocation.ptr == owned.encoded.bytes.ptr);
    std.debug.assert(owned.allocation.len == owned.encoded.bytes.len);
    return owned.encoded;
}

pub fn readRowsOwned(
    self: anytype,
    output_allocator: std.mem.Allocator,
    output_prefix_bytes: usize,
    terminal: *ghostty_vt.Terminal,
    current_revision: u64,
    current_generation: u64,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    request: ReadRequest,
) (HistoryError || std.mem.Allocator.Error)!OwnedEncodedRows {
    const output = try readRowsTo(
        self,
        .{ .allocated = .{
            .allocator = output_allocator,
            .output_prefix_bytes = output_prefix_bytes,
        } },
        terminal,
        current_revision,
        current_generation,
        cell_pixel_width,
        cell_pixel_height,
        request,
    );
    return .{
        .allocation = output.allocation.?,
        .encoded = output.encoded,
    };
}

pub fn readRowsInto(
    self: anytype,
    scratch_allocator: std.mem.Allocator,
    destination: []u8,
    terminal: *ghostty_vt.Terminal,
    current_revision: u64,
    current_generation: u64,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    request: ReadRequest,
) (HistoryError || std.mem.Allocator.Error)!EncodedRows {
    const output = try readRowsTo(
        self,
        .{ .fixed = .{
            .destination = destination,
            .scratch_allocator = scratch_allocator,
        } },
        terminal,
        current_revision,
        current_generation,
        cell_pixel_width,
        cell_pixel_height,
        request,
    );
    std.debug.assert(output.allocation == null);
    return output.encoded;
}

fn readRowsTo(
    self: anytype,
    target: RowEncodingTarget,
    terminal: *ghostty_vt.Terminal,
    current_revision: u64,
    current_generation: u64,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    request: ReadRequest,
) (HistoryError || std.mem.Allocator.Error)!ReadRowsOutput {
    switch (request.revision_policy) {
        .exact => if (request.revision != current_revision) return error.StaleRevision,
        .current_view => if (request.purpose != .viewport or switch (request.origin) {
            .committed, .tail => false,
            .absolute, .oldest => true,
        }) return error.InvalidRequest,
    }
    if (request.generation != current_generation) return error.StaleGeneration;
    if (request.request_sequence == 0 or request.row_count == 0 or
        request.row_count > max_row_window_rows or request.maximum_bytes < row_window_header_bytes or
        request.maximum_bytes > max_row_window_bytes)
    {
        return error.InvalidRequest;
    }
    const state = storage.find(self, request.attachment_id) orelse return error.MissingAttachment;
    if (request.request_sequence <= state.last_row_request_sequence) {
        return error.InvalidRequest;
    }
    if (request.purpose == .viewport and state.pending != null) return error.InvalidRequest;

    const screen = terminal.screens.active;
    const total_rows: u64 = @intCast(screen.pages.total_rows);
    if (total_rows == 0 or total_rows > std.math.maxInt(u32)) return error.InvalidRequest;
    const maximum_rows_by_entries =
        @as(usize, max_row_window_entries) / @as(usize, terminal.cols);
    const bounded_request_rows: u16 = @intCast(@min(
        @as(usize, request.row_count),
        maximum_rows_by_entries,
    ));
    const start_row = try resolveStart(
        self,
        screen,
        state,
        request.origin,
        bounded_request_rows,
    );
    const available = total_rows - start_row;
    const row_count: u16 = @intCast(@min(@as(u64, bounded_request_rows), available));
    if (row_count == 0) return error.InvalidRequest;
    const start_pin = screen.pages.pin(.{ .screen = .{
        .x = 0,
        .y = @intCast(start_row),
    } }) orelse return error.InvalidPoint;

    const hard_limit = @min(request.maximum_bytes, max_row_window_bytes);
    const parameters: row_encoding.Parameters = .{
        .cell_pixel_height = cell_pixel_height,
        .cell_pixel_width = cell_pixel_width,
        .hard_limit = hard_limit,
        .request = .{
            .animation_tick = self.animation_tick,
            .attachment_id = request.attachment_id,
            .detail = request.detail,
            .generation = current_generation,
            .host_epoch = request.host_epoch,
            .request_sequence = request.request_sequence,
            .revision = current_revision,
            .theme_revision = request.theme_revision,
        },
        .row_count = row_count,
        .start_pin = start_pin,
        .start_row = start_row,
        .total_rows = total_rows,
    };
    const encoded: RowEncodingOutput = switch (target) {
        .allocated => |output| blk: {
            const owned = try row_encoding.encodeOwned(
                output.allocator,
                output.output_prefix_bytes,
                terminal,
                parameters,
            );
            break :blk .{
                .allocation = owned.allocation,
                .bytes = owned.bytes,
            };
        },
        .fixed => |fixed| .{
            .allocation = null,
            .bytes = try row_encoding.encodeInto(
                fixed.scratch_allocator,
                terminal,
                parameters,
                fixed.destination,
            ),
        },
    };
    errdefer if (encoded.allocation) |allocation| switch (target) {
        .allocated => |output| output.allocator.free(allocation),
        .fixed => unreachable,
    };

    if (request.purpose == .viewport) {
        const tracked = try screen.pages.trackPin(start_pin);
        state.pending = .{
            .generation = current_generation,
            .pin = tracked,
            .request_sequence = request.request_sequence,
            .revision = current_revision,
            .screen_generation = terminal.screens.generation(terminal.screens.active_key),
            .screen_key = terminal.screens.active_key,
        };
    }
    state.last_row_request_sequence = request.request_sequence;
    return .{
        .allocation = encoded.allocation,
        .encoded = .{
            .bytes = encoded.bytes,
            .generation = current_generation,
            .revision = current_revision,
            .row_count = row_count,
            .sequence = request.request_sequence,
            .start_row = start_row,
            .total_rows = total_rows,
        },
    };
}

pub fn rowsApplied(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    attachment_id: AttachmentId,
    request_sequence: u64,
    revision: u64,
    generation: u64,
) HistoryError!void {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    if (request_sequence == 0) return error.InvalidAcknowledgement;
    // A replay of the already-applied rows is an idempotent no-op even
    // while a newer row window is pending; it never clears or advances the
    // newer flight. The immutable acknowledgement tuple is preserved
    // separately from the mutable committed anchor, whose revision and
    // generation may be refreshed by later mutations.
    if (state.committed_ack) |ack| {
        if (ack.sequence == request_sequence) {
            if (ack.revision == revision and ack.generation == generation) return;
            return error.InvalidAcknowledgement;
        }
    }
    const pending = state.pending orelse {
        // Demand, screen, and generation transitions can abandon a row flight
        // after its response has crossed the process boundary. A later
        // acknowledgement below the issued high-water mark has no anchor left
        // to promote, so it is an idempotent no-op. A future sequence was never
        // admitted by this attachment and remains invalid.
        if (request_sequence <= state.last_row_request_sequence) return;
        return error.InvalidAcknowledgement;
    };
    // An older acknowledgement cannot clear or promote the newer pending
    // anchor. The exact pending sequence still validates its immutable tuple.
    if (request_sequence < pending.request_sequence) return;
    if (pending.request_sequence != request_sequence or pending.revision != revision or
        pending.generation != generation)
    {
        return error.InvalidAcknowledgement;
    }
    if (!storage.anchorValid(terminal, pending)) {
        storage.releaseAnchor(self, terminal, &state.pending);
        return error.AnchorEvicted;
    }
    storage.releaseAnchor(self, terminal, &state.committed);
    state.committed = pending;
    state.committed_ack = .{
        .generation = generation,
        .revision = revision,
        .sequence = request_sequence,
    };
    state.pending = null;
}

pub fn resolveStart(
    self: anytype,
    screen: *ghostty_vt.Screen,
    state: *AttachmentState,
    origin: Origin,
    row_count: u16,
) HistoryError!u64 {
    _ = self;
    const total: u64 = @intCast(screen.pages.total_rows);
    return switch (origin) {
        .absolute => |row| if (row < total) row else error.InvalidPoint,
        .oldest => |offset| if (offset < total) offset else error.InvalidPoint,
        .tail => |rows_before| blk: {
            const end = total - @min(total, @as(u64, rows_before));
            break :blk end - @min(end, @as(u64, row_count));
        },
        .committed => |delta| blk: {
            const anchor = state.committed orelse return error.InvalidRequest;
            if (!storage.anchorValidScreen(screen, anchor)) return error.AnchorEvicted;
            const point = screen.pages.pointFromPin(.screen, anchor.pin.*) orelse
                return error.AnchorEvicted;
            const base: i64 = point.screen.y;
            const candidate = std.math.add(i64, base, delta) catch
                if (delta < 0) @as(i64, 0) else std.math.maxInt(i64);
            const maximum_start = total - @min(total, @as(u64, row_count));
            break :blk @intCast(@min(@max(candidate, 0), @as(i64, @intCast(maximum_start))));
        },
    };
}

pub const row_window_byte_limit = max_row_window_bytes;
