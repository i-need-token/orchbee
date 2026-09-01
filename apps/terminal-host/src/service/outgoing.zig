const std = @import("std");

const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const reservation_accounting = @import("outgoing/reservation_accounting.zig");

pub const owned_payload_prefix_bytes = protocol.transport_envelope_header_bytes;

// Host-originated envelopes have one of these three identity shapes. Normal
// responses and events cannot omit their post-handshake Host identities;
// only a handshake response may carry absent envelope identities.
pub const Delivery = union(enum) {
    event: identifiers.HostEpoch,
    handshake_response: struct {
        correlation_id: ?identifiers.IdentifierBytes,
        host_epoch: ?identifiers.HostEpoch,
    },
    response: struct {
        correlation_id: identifiers.IdentifierBytes,
        host_epoch: identifiers.HostEpoch,
    },
};

pub const AppendToken = reservation_accounting.AppendToken;

pub const AppendOutcome = union(enum) {
    appended: AppendToken,
    full,
};

pub const ReserveOutcome = union(enum) {
    full,
    reserved: RecordReservation,
};

pub const CapacityReserveOutcome = union(enum) {
    full,
    reserved: RecordCapacityReservation,
};

pub const AdoptionOutcome = union(enum) {
    full,
    prepared: PayloadAdoption,
};

pub const ScheduleOutcome = enum {
    complete,
    batch_full,
};

const DeliveryMetadata = struct {
    correlation_id: ?identifiers.IdentifierBytes,
    event: bool,
    host_epoch: ?identifiers.HostEpoch,
    response: bool,
};

const StoredRecord = struct {
    allocation: []u8,
    bytes: []const u8,
};

const ReservationState = enum {
    pending,
    consumed,
};

/// Owns one record allocation and its batch capacity until the caller either
/// commits an initialized payload or abandons the request. The payload buffer
/// can be written after a domain effect without any further allocation.
pub const RecordReservation = struct {
    allocation: ?[]u8,
    append_id: u64,
    delivery: DeliveryMetadata,
    maximum_record_length: usize,
    outgoing: *Outgoing,
    state: ReservationState = .pending,

    pub fn payloadBuffer(self: *RecordReservation) []u8 {
        std.debug.assert(self.state == .pending);
        const allocation = self.allocation.?;
        return allocation[protocol.transport_envelope_header_bytes..];
    }

    pub fn commitPayload(
        self: *RecordReservation,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        payload: []const u8,
    ) AppendToken {
        const destination = self.payloadBuffer();
        std.debug.assert(payload.len <= destination.len);
        @memcpy(destination[0..payload.len], payload);
        return self.commitWritten(kind, encoding, payload.len);
    }

    pub fn commitWritten(
        self: *RecordReservation,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        payload_length: usize,
    ) AppendToken {
        std.debug.assert(self.state == .pending);
        const allocation = self.allocation.?;
        std.debug.assert(payload_length <= allocation.len - protocol.transport_envelope_header_bytes);
        const record = protocol.finalizeTransportEnvelope(
            allocation,
            kind,
            encoding,
            payload_length,
            self.delivery.correlation_id,
            if (self.delivery.host_epoch) |epoch| epoch.bytes else null,
            self.delivery.response,
            self.delivery.event,
        ) catch unreachable;
        const token = reservation_accounting.commit(
            self.outgoing,
            self.append_id,
            self.maximum_record_length,
            allocation,
            record,
        );
        self.allocation = null;
        self.state = .consumed;
        return token;
    }

    pub fn deinit(self: *RecordReservation) void {
        if (self.state == .pending) {
            reservation_accounting.cancelOwned(
                self.outgoing,
                self.maximum_record_length,
                self.allocation.?,
            );
        }
        self.* = undefined;
    }
};

/// Promises one bounded record slot before a domain effect without allocating
/// the maximum payload buffer. The caller can later transfer an already-encoded
/// allocation at an allocation-free commit point.
pub const RecordCapacityReservation = struct {
    append_id: u64,
    delivery: DeliveryMetadata,
    maximum_record_length: usize,
    outgoing: *Outgoing,
    state: ReservationState = .pending,

    pub fn commitOwnedPayload(
        self: *RecordCapacityReservation,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        allocation: []u8,
        payload: []const u8,
    ) AppendToken {
        std.debug.assert(self.state == .pending);
        std.debug.assert(allocation.len <= self.maximum_record_length);
        std.debug.assert(allocation.len >= owned_payload_prefix_bytes);
        std.debug.assert(payload.ptr == allocation.ptr + owned_payload_prefix_bytes);
        std.debug.assert(payload.len <= allocation.len - owned_payload_prefix_bytes);
        const record = protocol.finalizeTransportEnvelope(
            allocation,
            kind,
            encoding,
            payload.len,
            self.delivery.correlation_id,
            if (self.delivery.host_epoch) |epoch| epoch.bytes else null,
            self.delivery.response,
            self.delivery.event,
        ) catch unreachable;
        const token = reservation_accounting.commit(
            self.outgoing,
            self.append_id,
            self.maximum_record_length,
            allocation,
            record,
        );
        self.state = .consumed;
        return token;
    }

    pub fn deinit(self: *RecordCapacityReservation) void {
        if (self.state == .pending) {
            reservation_accounting.cancelCapacity(
                self.outgoing,
                self.maximum_record_length,
            );
        }
        self.* = undefined;
    }
};

/// Owns one already-encoded payload and its promised batch capacity until the
/// caller reaches an allocation-free commit point. Cancelling a pending value
/// releases both the capacity and the adopted allocation without publishing a
/// record.
pub const PayloadAdoption = struct {
    allocation: ?[]u8,
    append_id: u64,
    delivery: DeliveryMetadata,
    encoding: protocol.PayloadEncoding,
    kind: u16,
    outgoing: *Outgoing,
    payload_length: usize,
    state: ReservationState = .pending,

    pub fn commit(self: *PayloadAdoption) AppendToken {
        std.debug.assert(self.state == .pending);
        const allocation = self.allocation.?;
        const payload = allocation[owned_payload_prefix_bytes..];
        std.debug.assert(payload.len == self.payload_length);
        const record = protocol.finalizeTransportEnvelope(
            allocation,
            self.kind,
            self.encoding,
            payload.len,
            self.delivery.correlation_id,
            if (self.delivery.host_epoch) |epoch| epoch.bytes else null,
            self.delivery.response,
            self.delivery.event,
        ) catch unreachable;
        const token = reservation_accounting.commit(
            self.outgoing,
            self.append_id,
            allocation.len,
            allocation,
            record,
        );
        self.allocation = null;
        self.state = .consumed;
        return token;
    }

    pub fn deinit(self: *PayloadAdoption) void {
        if (self.state == .pending) {
            const allocation = self.allocation.?;
            reservation_accounting.cancelOwned(self.outgoing, allocation.len, allocation);
        }
        self.* = undefined;
    }
};

// Owns one bounded dispatch batch and every encoded transport record in it.
// Deinitialization releases the complete batch; rollback releases exactly the
// most recently appended record and restores byte accounting.
pub const Outgoing = struct {
    allocation_byte_count: usize = 0,
    allocator: std.mem.Allocator,
    byte_count: usize = 0,
    last_append_id: u64 = 0,
    next_append_id: u64 = 1,
    record_storage: std.ArrayList(StoredRecord) = .empty,
    record_views: std.ArrayList([]const u8) = .empty,
    reserved_bytes: usize = 0,
    reserved_records: usize = 0,

    pub fn init(allocator: std.mem.Allocator) Outgoing {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Outgoing) void {
        std.debug.assert(self.reserved_bytes == 0);
        std.debug.assert(self.reserved_records == 0);
        for (self.record_storage.items) |record| self.allocator.free(record.allocation);
        self.record_storage.deinit(self.allocator);
        self.record_views.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn records(self: *const Outgoing) []const []const u8 {
        return self.record_views.items;
    }

    pub fn ownedPayloadAllocator(self: *const Outgoing) std.mem.Allocator {
        return self.allocator;
    }

    pub fn reserve(
        self: *Outgoing,
        maximum_payload_length: usize,
        delivery: Delivery,
    ) !ReserveOutcome {
        const plan = try reservation_accounting.plan(
            self,
            maximum_payload_length,
            deliveryMetadata(delivery),
        ) orelse return .full;
        const allocation = try self.allocator.alloc(u8, plan.maximum_record_length);
        reservation_accounting.activate(self, plan);
        return .{ .reserved = .{
            .allocation = allocation,
            .append_id = plan.append_id,
            .delivery = plan.delivery,
            .maximum_record_length = plan.maximum_record_length,
            .outgoing = self,
        } };
    }

    /// Reserves bounded batch and record-list capacity without allocating the
    /// caller's maximum payload. Commit can later adopt a smaller exact output
    /// allocation without any further list growth.
    pub fn reserveCapacity(
        self: *Outgoing,
        maximum_payload_length: usize,
        delivery: Delivery,
    ) !CapacityReserveOutcome {
        const plan = try reservation_accounting.plan(
            self,
            maximum_payload_length,
            deliveryMetadata(delivery),
        ) orelse return .full;
        reservation_accounting.activate(self, plan);
        return .{ .reserved = .{
            .append_id = plan.append_id,
            .delivery = plan.delivery,
            .maximum_record_length = plan.maximum_record_length,
            .outgoing = self,
        } };
    }

    pub fn append(
        self: *Outgoing,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        payload: []const u8,
        delivery: Delivery,
    ) !AppendOutcome {
        const metadata = deliveryMetadata(delivery);
        _ = try protocol.transportEnvelopeLength(
            kind,
            encoding,
            payload.len,
            metadata.correlation_id,
            if (metadata.host_epoch) |epoch| epoch.bytes else null,
            metadata.response,
            metadata.event,
        );
        var reservation = switch (try self.reserve(payload.len, delivery)) {
            .full => return .full,
            .reserved => |reserved| reserved,
        };
        defer reservation.deinit();
        return .{ .appended = reservation.commitPayload(kind, encoding, payload) };
    }

    /// Prepares one exact allocation whose payload begins after the transport
    /// header prefix. `.prepared` transfers ownership and promises enough batch
    /// capacity for `PayloadAdoption.commit` to remain allocation-free. On
    /// `.full` or error the caller retains ownership.
    pub fn prepareAdoptPayload(
        self: *Outgoing,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        allocation: []u8,
        payload: []const u8,
        delivery: Delivery,
    ) !AdoptionOutcome {
        if (allocation.len != owned_payload_prefix_bytes + payload.len or
            payload.ptr != allocation.ptr + owned_payload_prefix_bytes)
        {
            return error.InvalidOwnedPayload;
        }
        const metadata = deliveryMetadata(delivery);
        _ = try protocol.transportEnvelopeLength(
            kind,
            encoding,
            payload.len,
            metadata.correlation_id,
            if (metadata.host_epoch) |epoch| epoch.bytes else null,
            metadata.response,
            metadata.event,
        );
        const plan = try reservation_accounting.plan(
            self,
            payload.len,
            deliveryMetadata(delivery),
        ) orelse return .full;
        std.debug.assert(plan.maximum_record_length == allocation.len);
        reservation_accounting.activate(self, plan);
        return .{ .prepared = .{
            .allocation = allocation,
            .append_id = plan.append_id,
            .delivery = plan.delivery,
            .encoding = encoding,
            .kind = kind,
            .outgoing = self,
            .payload_length = payload.len,
        } };
    }

    /// Immediately adopts a payload through the provisional reservation path.
    /// Ownership transfers only for `.appended`; on `.full` or error the caller
    /// still owns and must release `allocation`.
    pub fn adoptPayload(
        self: *Outgoing,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        allocation: []u8,
        payload: []const u8,
        delivery: Delivery,
    ) !AppendOutcome {
        var adoption = switch (try self.prepareAdoptPayload(
            kind,
            encoding,
            allocation,
            payload,
            delivery,
        )) {
            .full => return .full,
            .prepared => |prepared| prepared,
        };
        defer adoption.deinit();
        return .{ .appended = adoption.commit() };
    }

    pub fn rollback(self: *Outgoing, token: AppendToken) error{InvalidAppendToken}!void {
        if (self.last_append_id != token.append_id or
            self.record_storage.items.len == 0 or
            self.record_storage.items.len - 1 != token.record_count_before)
        {
            return error.InvalidAppendToken;
        }
        const record = self.record_storage.items[self.record_storage.items.len - 1];
        if (self.byte_count < record.bytes.len or
            self.byte_count - record.bytes.len != token.byte_count_before)
        {
            return error.InvalidAppendToken;
        }
        _ = self.record_storage.pop().?;
        const view = self.record_views.pop().?;
        std.debug.assert(view.ptr == record.bytes.ptr and view.len == record.bytes.len);
        std.debug.assert(self.allocation_byte_count >= record.allocation.len);
        self.allocation_byte_count -= record.allocation.len;
        self.byte_count = token.byte_count_before;
        self.last_append_id = token.previous_append_id;
        self.allocator.free(record.allocation);
    }
};

fn deliveryMetadata(delivery: Delivery) DeliveryMetadata {
    return switch (delivery) {
        .event => |host_epoch| .{
            .correlation_id = null,
            .event = true,
            .host_epoch = host_epoch,
            .response = false,
        },
        .handshake_response => |response| .{
            .correlation_id = response.correlation_id,
            .event = false,
            .host_epoch = response.host_epoch,
            .response = true,
        },
        .response => |response| .{
            .correlation_id = response.correlation_id,
            .event = false,
            .host_epoch = response.host_epoch,
            .response = true,
        },
    };
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const byte_limit = reservation_accounting.max_bytes;
    pub const record_limit = reservation_accounting.max_records;

    pub fn byteCount(outgoing: *const Outgoing) usize {
        return outgoing.byte_count;
    }

    pub fn reservedByteCount(outgoing: *const Outgoing) usize {
        return outgoing.reserved_bytes;
    }

    pub fn reservedRecordCount(outgoing: *const Outgoing) usize {
        return outgoing.reserved_records;
    }

    pub fn forceNextCapacityGrowth(outgoing: *Outgoing) !void {
        try outgoing.record_storage.shrinkAndFreePrecise(
            outgoing.allocator,
            outgoing.record_storage.items.len,
        );
        try outgoing.record_views.shrinkAndFreePrecise(
            outgoing.allocator,
            outgoing.record_views.items.len,
        );
    }
} else struct {};
