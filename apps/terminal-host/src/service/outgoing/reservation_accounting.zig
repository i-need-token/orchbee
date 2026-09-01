const std = @import("std");

const protocol = @import("terminal-protocol");

pub const max_records = 256;
pub const max_bytes = protocol.Limits.transport_record_bytes;

pub const AppendToken = struct {
    // Append identity is never reused across rollback or cancelled reservation,
    // so an old checkpoint cannot match a later record with the same counts.
    append_id: u64,
    byte_count_before: usize,
    previous_append_id: u64,
    record_count_before: usize,
};

pub fn Plan(comptime DeliveryMetadata: type) type {
    return struct {
        append_id: u64,
        delivery: DeliveryMetadata,
        maximum_record_length: usize,
        next_append_id: u64,
    };
}

pub fn plan(
    outgoing: anytype,
    maximum_payload_length: usize,
    delivery: anytype,
) !?Plan(@TypeOf(delivery)) {
    if (maximum_payload_length > max_bytes - protocol.transport_envelope_header_bytes) {
        return error.OutgoingRecordTooLarge;
    }
    if (outgoing.record_storage.items.len + outgoing.reserved_records >= max_records) {
        return null;
    }
    // Pending records account for their promised maximum while committed
    // records account for the allocation they actually retain.
    const occupied_bytes = outgoing.allocation_byte_count + outgoing.reserved_bytes;
    const remaining_bytes = max_bytes -| occupied_bytes;
    if (remaining_bytes < protocol.transport_envelope_header_bytes or
        maximum_payload_length > remaining_bytes - protocol.transport_envelope_header_bytes)
    {
        return null;
    }

    const maximum_record_length = try protocol.transportEnvelopeAllocationLength(
        maximum_payload_length,
        delivery.correlation_id,
        if (delivery.host_epoch) |epoch| epoch.bytes else null,
    );
    const append_id = outgoing.next_append_id;
    const next_append_id = std.math.add(u64, append_id, 1) catch
        return error.OutgoingAppendIdentityExhausted;
    const required_capacity = outgoing.record_storage.items.len +
        outgoing.reserved_records + 1;
    try outgoing.record_storage.ensureTotalCapacity(outgoing.allocator, required_capacity);
    try outgoing.record_views.ensureTotalCapacity(outgoing.allocator, required_capacity);
    return .{
        .append_id = append_id,
        .delivery = delivery,
        .maximum_record_length = maximum_record_length,
        .next_append_id = next_append_id,
    };
}

pub fn activate(outgoing: anytype, reservation: anytype) void {
    outgoing.next_append_id = reservation.next_append_id;
    outgoing.reserved_bytes += reservation.maximum_record_length;
    outgoing.reserved_records += 1;
}

pub fn commit(
    outgoing: anytype,
    append_id: u64,
    reserved_record_length: usize,
    allocation: []u8,
    record: []const u8,
) AppendToken {
    std.debug.assert(outgoing.reserved_records > 0);
    std.debug.assert(outgoing.reserved_bytes >= reserved_record_length);
    std.debug.assert(allocation.len <= reserved_record_length);
    std.debug.assert(record.ptr == allocation.ptr and record.len <= allocation.len);
    const token: AppendToken = .{
        .append_id = append_id,
        .byte_count_before = outgoing.byte_count,
        .previous_append_id = outgoing.last_append_id,
        .record_count_before = outgoing.record_storage.items.len,
    };
    outgoing.reserved_records -= 1;
    outgoing.reserved_bytes -= reserved_record_length;
    outgoing.record_storage.appendAssumeCapacity(.{
        .allocation = allocation,
        .bytes = record,
    });
    outgoing.record_views.appendAssumeCapacity(record);
    outgoing.allocation_byte_count += allocation.len;
    outgoing.byte_count += record.len;
    outgoing.last_append_id = append_id;
    return token;
}

pub fn cancelOwned(
    outgoing: anytype,
    reserved_record_length: usize,
    allocation: []u8,
) void {
    std.debug.assert(outgoing.reserved_records > 0);
    std.debug.assert(outgoing.reserved_bytes >= reserved_record_length);
    std.debug.assert(allocation.len <= reserved_record_length);
    outgoing.reserved_records -= 1;
    outgoing.reserved_bytes -= reserved_record_length;
    outgoing.allocator.free(allocation);
}

pub fn cancelCapacity(outgoing: anytype, reserved_record_length: usize) void {
    std.debug.assert(outgoing.reserved_records > 0);
    std.debug.assert(outgoing.reserved_bytes >= reserved_record_length);
    outgoing.reserved_records -= 1;
    outgoing.reserved_bytes -= reserved_record_length;
}
