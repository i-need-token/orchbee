const std = @import("std");

const protocol = @import("terminal-protocol");

pub const client_capacity_bytes = protocol.Limits.pending_input_bytes;
pub const client_capacity_entries = 4096;
pub const terminal_reply_capacity_bytes = protocol.Limits.pty_read_batch_bytes;
pub const terminal_reply_capacity_entries = 4096;
pub const writer_slice_bytes = 64 * 1024;
pub const terminal_reply_burst_slices = 64;

pub const Source = enum {
    client,
    terminal,
};

pub const Charge = struct {
    bytes: usize = 0,
    entries: usize = 0,
};

pub const Reservation = struct {
    identifier: u64,
    reserved_bytes: usize,
};

pub const Delivery = union(enum) {
    ordered,
    mouse_hover: u64,
};

pub const WriterSlice = struct {
    identifier: u64,
    length: usize,
    offset: usize,
    source: Source,
};

pub const AccountOutcome = enum {
    advanced,
    completed,
    ownership_lost,
};

pub const StartOutcome = enum {
    started,
    retired,
    ownership_lost,
};

const Entry = struct {
    bytes: ?[]u8 = null,
    delivery: Delivery = .ordered,
    identifier: u64,
    offset: usize = 0,
    reserved_bytes: usize,
    source: Source,
    write_started: bool = false,
};

/// Owns the two bounded PTY-write queues and their reservation accounting.
/// TerminalRuntime serializes every operation with its input mutex because
/// queue publication and attachment held-input ownership share one commit.
pub const Queue = struct {
    allocator: std.mem.Allocator,
    client_charge: Charge = .{},
    entries: std.ArrayList(Entry) = .empty,
    next_identifier: u64 = 1,
    terminal_reply_burst_remaining: usize = terminal_reply_burst_slices,
    terminal_reply_charge: Charge = .{},

    pub fn init(allocator: std.mem.Allocator) Queue {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Queue) void {
        self.clear();
        self.entries.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn hasCapacity(
        self: *const Queue,
        byte_count: usize,
        source: Source,
        repair_charge: Charge,
    ) bool {
        const capacity_bytes = capacityBytes(source);
        const capacity_entries = capacityEntries(source);
        if (byte_count > capacity_bytes) return false;
        const source_charge = self.charge(source);
        return byte_count <= capacity_bytes -| source_charge.bytes -| repair_charge.bytes and
            source_charge.entries < capacity_entries -| repair_charge.entries;
    }

    pub fn reserve(
        self: *Queue,
        byte_count: usize,
        source: Source,
        repair_charge: Charge,
    ) !Reservation {
        if (!self.hasCapacity(byte_count, source, repair_charge)) {
            return error.InputBackpressure;
        }
        return self.reserveAssumeCapacity(byte_count, source, .ordered);
    }

    pub fn reserveMouseHover(
        self: *Queue,
        byte_count: usize,
        mouse_input_epoch: u64,
        repair_charge: Charge,
    ) !Reservation {
        if (!self.hasCapacity(byte_count, .client, repair_charge)) {
            return error.InputBackpressure;
        }
        return self.reserveAssumeCapacity(
            byte_count,
            .client,
            .{ .mouse_hover = mouse_input_epoch },
        );
    }

    /// Converts capacity already charged to an attachment cleanup ledger into
    /// one client FIFO reservation. The caller validates the exact ledger and
    /// repair byte count before invoking this operation.
    pub fn reservePrechargedClient(self: *Queue, byte_count: usize) !Reservation {
        std.debug.assert(self.client_charge.bytes <= client_capacity_bytes -| byte_count);
        std.debug.assert(self.client_charge.entries < client_capacity_entries);
        return self.reserveAssumeCapacity(byte_count, .client, .ordered);
    }

    pub fn reservationSource(self: *const Queue, identifier: u64) ?Source {
        const index = self.entryIndex(identifier) orelse return null;
        return self.entries.items[index].source;
    }

    /// Adopts the caller-owned bytes into an exact unpublished reservation.
    /// Returns false when the reservation is absent, already published, forged,
    /// or too small; ownership then stays with the caller.
    pub fn publish(self: *Queue, reservation: Reservation, bytes: []u8) bool {
        const index = self.entryIndex(reservation.identifier) orelse return false;
        const entry = &self.entries.items[index];
        if (entry.bytes != null or
            entry.reserved_bytes != reservation.reserved_bytes or
            bytes.len > entry.reserved_bytes)
        {
            return false;
        }

        self.mutableCharge(entry.source).bytes -= reservation.reserved_bytes - bytes.len;
        if (bytes.len == 0) {
            self.mutableCharge(entry.source).entries -= 1;
            _ = self.entries.orderedRemove(index);
            self.allocator.free(bytes);
            return true;
        }
        entry.reserved_bytes = bytes.len;
        entry.bytes = bytes;
        return true;
    }

    pub fn cancel(self: *Queue, identifier: u64) bool {
        const index = self.entryIndex(identifier) orelse return false;
        const entry = self.entries.items[index];
        self.releaseCharge(entry);
        if (entry.bytes) |bytes| self.allocator.free(bytes);
        _ = self.entries.orderedRemove(index);
        return true;
    }

    pub fn clear(self: *Queue) void {
        for (self.entries.items) |entry| {
            if (entry.bytes) |bytes| self.allocator.free(bytes);
        }
        self.entries.clearRetainingCapacity();
        self.client_charge = .{};
        self.terminal_reply_charge = .{};
    }

    pub fn pendingBytes(self: *const Queue) usize {
        return self.client_charge.bytes + self.terminal_reply_charge.bytes;
    }

    pub fn entryCount(self: *const Queue) usize {
        return self.entries.items.len;
    }

    pub fn publishedCount(self: *const Queue, source: Source) usize {
        var count: usize = 0;
        for (self.entries.items) |entry| {
            if (entry.source == source and entry.bytes != null) count += 1;
        }
        return count;
    }

    pub fn charge(self: *const Queue, source: Source) Charge {
        return switch (source) {
            .client => self.client_charge,
            .terminal => self.terminal_reply_charge,
        };
    }

    pub fn hasWritableEntry(self: *const Queue) bool {
        return selectWritableEntry(
            self.entries.items,
            self.terminal_reply_burst_remaining,
        ) != null;
    }

    /// Copies the next bounded write slice while the owner holds its input
    /// mutex. The returned identity is revalidated immediately before the
    /// platform write begins.
    pub fn copyNextWriterSlice(self: *const Queue, buffer: []u8) WriterSlice {
        const index = selectWritableEntry(
            self.entries.items,
            self.terminal_reply_burst_remaining,
        ) orelse unreachable;
        const entry = self.entries.items[index];
        const bytes = entry.bytes.?;
        std.debug.assert(bytes.len != 0);
        const available = bytes[entry.offset..];
        const length = @min(buffer.len, available.len);
        std.debug.assert(length != 0);
        @memcpy(buffer[0..length], available[0..length]);
        return .{
            .identifier = entry.identifier,
            .length = length,
            .offset = entry.offset,
            .source = entry.source,
        };
    }

    pub fn ownsWriterSlice(self: *const Queue, slice: WriterSlice) bool {
        const index = self.entryIndex(slice.identifier) orelse return false;
        const entry = self.entries.items[index];
        const bytes = entry.bytes orelse return false;
        return entry.identifier == slice.identifier and
            entry.offset == slice.offset and
            entry.source == slice.source and
            entry.offset <= bytes.len and
            slice.length != 0 and
            slice.length <= bytes.len - entry.offset;
    }

    /// Revalidates the copied entry at the exact platform-write start point.
    /// Only a wholly unstarted stale hover may be removed; a started entry is
    /// permanently ordered through all of its remaining partial slices.
    pub fn startWriterSlice(
        self: *Queue,
        slice: WriterSlice,
        mouse_input_epoch: u64,
    ) StartOutcome {
        const index = self.entryIndex(slice.identifier) orelse return .ownership_lost;
        const entry = &self.entries.items[index];
        const bytes = entry.bytes orelse return .ownership_lost;
        if (entry.offset != slice.offset or entry.source != slice.source or
            entry.offset > bytes.len or slice.length == 0 or
            slice.length > bytes.len - entry.offset)
        {
            return .ownership_lost;
        }
        if (!entry.write_started) switch (entry.delivery) {
            .ordered => {},
            .mouse_hover => |prepared_epoch| if (prepared_epoch != mouse_input_epoch) {
                self.releaseCharge(entry.*);
                self.allocator.free(bytes);
                _ = self.entries.orderedRemove(index);
                return .retired;
            },
        };
        entry.write_started = true;
        return .started;
    }

    pub fn accountWriterSlice(
        self: *Queue,
        slice: WriterSlice,
        written: usize,
    ) AccountOutcome {
        const index = self.entryIndex(slice.identifier) orelse return .ownership_lost;
        const entry = &self.entries.items[index];
        if (entry.offset != slice.offset or entry.source != slice.source or
            entry.bytes == null or written == 0 or written > slice.length)
        {
            return .ownership_lost;
        }

        entry.offset += written;
        const completed = entry.offset == entry.bytes.?.len;
        if (completed) {
            self.releaseCharge(entry.*);
            self.allocator.free(entry.bytes.?);
            _ = self.entries.orderedRemove(index);
        }

        // A terminal slice consumes the reply burst; a client slice restarts
        // it. This bounds the reply work a waiting client entry may sit behind.
        if (slice.source == .terminal) {
            if (self.terminal_reply_burst_remaining > 0) {
                self.terminal_reply_burst_remaining -= 1;
            }
        } else {
            self.terminal_reply_burst_remaining = terminal_reply_burst_slices;
        }
        return if (completed) .completed else .advanced;
    }

    fn reserveAssumeCapacity(
        self: *Queue,
        byte_count: usize,
        source: Source,
        delivery: Delivery,
    ) !Reservation {
        const identifier = self.next_identifier;
        if (identifier == std.math.maxInt(u64)) return error.InputSequenceExhausted;
        try self.entries.ensureUnusedCapacity(self.allocator, 1);
        self.entries.appendAssumeCapacity(.{
            .delivery = delivery,
            .identifier = identifier,
            .reserved_bytes = byte_count,
            .source = source,
        });
        self.next_identifier = identifier + 1;
        const source_charge = self.mutableCharge(source);
        source_charge.bytes += byte_count;
        source_charge.entries += 1;
        return .{ .identifier = identifier, .reserved_bytes = byte_count };
    }

    fn mutableCharge(self: *Queue, source: Source) *Charge {
        return switch (source) {
            .client => &self.client_charge,
            .terminal => &self.terminal_reply_charge,
        };
    }

    fn releaseCharge(self: *Queue, entry: Entry) void {
        const source_charge = self.mutableCharge(entry.source);
        std.debug.assert(source_charge.bytes >= entry.reserved_bytes);
        std.debug.assert(source_charge.entries > 0);
        source_charge.bytes -= entry.reserved_bytes;
        source_charge.entries -= 1;
    }

    fn entryIndex(self: *const Queue, identifier: u64) ?usize {
        for (self.entries.items, 0..) |entry, index| {
            if (entry.identifier == identifier) return index;
        }
        return null;
    }
};

fn capacityBytes(source: Source) usize {
    return switch (source) {
        .client => client_capacity_bytes,
        .terminal => terminal_reply_capacity_bytes,
    };
}

fn capacityEntries(source: Source) usize {
    return switch (source) {
        .client => client_capacity_entries,
        .terminal => terminal_reply_capacity_entries,
    };
}

/// Terminal-generated replies retain priority while the reply burst has
/// budget. Once exhausted, the writer yields to the first waiting client entry.
/// Relative order remains FIFO inside each source. An unpublished terminal
/// reservation blocks all later writes until it is published or cancelled.
fn selectWritableEntry(
    entries: []const Entry,
    terminal_reply_burst_remaining: usize,
) ?usize {
    for (entries, 0..) |entry, index| {
        if (entry.source != .terminal) continue;
        if (entry.bytes == null) return null;
        if (terminal_reply_burst_remaining == 0 and
            entries[0].bytes != null and entries[0].source == .client) return 0;
        return index;
    }
    if (entries.len == 0 or entries[0].bytes == null) return null;
    return 0;
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const EntryType = Entry;
    pub const selectWritableEntryFn = selectWritableEntry;
} else struct {};
