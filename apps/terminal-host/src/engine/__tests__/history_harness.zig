pub const std = @import("std");
pub const ghostty_vt = @import("ghostty-vt");
pub const protocol = @import("terminal-protocol");
pub const subject = @import("terminal-host-engine").history;

pub const findState = subject.testing.findStateFn;
pub const Manager = subject.Manager;
pub const max_row_window_bytes = subject.testing.row_window_byte_limit;
pub const max_pending_export_cancellations = subject.max_pending_export_cancellations;
pub const Detail = subject.Detail;
pub const ExportCancellation = subject.ExportCancellation;
pub const SelectionGesture = subject.SelectionGesture;
pub const SelectionSnapshot = subject.SelectionSnapshot;
pub const AttachmentId = subject.AttachmentId;
pub const HostEpoch = subject.HostEpoch;

pub const RecordingAllocator = struct {
    backing: std.mem.Allocator,
    alloc_request_count: usize = 0,
    free_count: usize = 0,
    max_requested: usize = 0,
    row_window_request_count: usize = 0,

    pub fn init(backing: std.mem.Allocator) RecordingAllocator {
        return .{ .backing = backing };
    }

    pub fn allocator(self: *RecordingAllocator) std.mem.Allocator {
        return .{ .ptr = self, .vtable = &vtable };
    }

    pub fn reset(self: *RecordingAllocator) void {
        self.alloc_request_count = 0;
        self.free_count = 0;
        self.max_requested = 0;
        self.row_window_request_count = 0;
    }

    pub fn recordRequest(self: *RecordingAllocator, len: usize) void {
        self.max_requested = @max(self.max_requested, len);
        if (len == protocol.Limits.row_window_bytes) self.row_window_request_count += 1;
    }

    pub fn alloc(
        context: *anyopaque,
        len: usize,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) ?[*]u8 {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.alloc_request_count += 1;
        self.recordRequest(len);
        return self.backing.rawAlloc(len, alignment, return_address);
    }

    pub fn resize(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) bool {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.recordRequest(new_len);
        return self.backing.rawResize(memory, alignment, new_len, return_address);
    }

    pub fn remap(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) ?[*]u8 {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.recordRequest(new_len);
        return self.backing.rawRemap(memory, alignment, new_len, return_address);
    }

    pub fn free(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) void {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.free_count += 1;
        self.backing.rawFree(memory, alignment, return_address);
    }

    const vtable: std.mem.Allocator.VTable = .{
        .alloc = alloc,
        .resize = resize,
        .remap = remap,
        .free = free,
    };
};

pub fn attachmentId(byte: u8) AttachmentId {
    return AttachmentId.fromBytes([_]u8{byte} ** 16);
}

pub fn hostEpoch(byte: u8) HostEpoch {
    return HostEpoch.fromBytes([_]u8{byte} ** 16);
}

pub fn gestureStart(
    attachment: AttachmentId,
    click_count: u2,
    column: u16,
    row: u64,
    select_output: bool,
) SelectionGesture {
    return .{
        .attachment_id = attachment,
        .event = .{ .start = .{
            .click_count = click_count,
            .point = .{ .column = column, .row = row },
            .position = .{ .x = @as(i32, column) * 10 + 1, .y = 1 },
            .select_output = select_output,
        } },
        .generation = 1,
        .revision = 1,
    };
}

pub fn gestureUpdate(
    attachment: AttachmentId,
    column: u16,
    row: u64,
    surface_x: i32,
    rectangle: bool,
) SelectionGesture {
    return .{
        .attachment_id = attachment,
        .event = .{ .update = .{
            .geometry = .{
                .cell_width = 10,
                .columns = 20,
                .padding_left = 0,
                .screen_height = 20,
            },
            .point = .{ .column = column, .row = row },
            .position = .{ .x = surface_x, .y = 1 },
            .rectangle = rectangle,
        } },
        .generation = 1,
        .revision = 1,
    };
}

pub fn gestureEnd(attachment: AttachmentId, column: u16, row: u64) SelectionGesture {
    return .{
        .attachment_id = attachment,
        .event = .{ .end = .{ .column = column, .row = row } },
        .generation = 1,
        .revision = 1,
    };
}

pub fn gestureCancel(attachment: AttachmentId) SelectionGesture {
    return .{
        .attachment_id = attachment,
        .event = .cancel,
        .generation = 1,
        .revision = 1,
    };
}

pub fn readSingleRow(
    terminal: *ghostty_vt.Terminal,
    manager: *Manager,
    attachment: AttachmentId,
    detail: Detail,
    request_sequence: u64,
) ![]u8 {
    try manager.attach(attachment);
    const rows = try manager.readRows(std.testing.allocator, terminal, 1, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = detail,
        .generation = 1,
        .host_epoch = hostEpoch(1),
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = request_sequence,
        .revision = 1,
        .row_count = 1,
    });
    return rows.bytes;
}

// THCR-029 regression: an exact duplicate of the already-applied viewport row
// window acknowledgement must be an idempotent no-op even while a newer row
// window is pending, and it must never clear or advance the newer flight.

// THCR-029 regression: a normal mutation refreshes the committed anchor's
// revision and generation, but the immutable acknowledgement tuple of the
// applied rows must remain replayable as a no-op without touching the
// mutation-refreshed anchor.

// THCR-029 regression: replaying the acknowledgement of the last completed
// export must be an idempotent no-op even while a newer export is in flight,
// and it must not advance the newer export's offset, chunk, or in-flight state.
