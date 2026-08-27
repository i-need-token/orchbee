const std = @import("std");

const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const terminal_history = model.interaction;
pub const ReadRows = struct {
    attachment_id: identifiers.AttachmentId,
    detail: terminal_history.Detail,
    generation: u64,
    maximum_bytes: usize,
    origin: terminal_history.Origin,
    purpose: terminal_history.Purpose,
    request_sequence: u64,
    revision: u64,
    revision_policy: terminal_history.RevisionPolicy,
    row_count: u16,
};

pub const RowsApplied = struct {
    attachment_id: identifiers.AttachmentId,
    generation: u64,
    request_sequence: u64,
    revision: u64,
};

pub const HistoryDemand = struct {
    attachment_id: identifiers.AttachmentId,
    demand: terminal_history.HistoryDemand,
};

pub const HistoryObserved = struct {
    attachment_id: identifiers.AttachmentId,
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub fn decodeReadRows(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.ReadRows, ReadRows) {
    const parsed = try control_wire.decode(control_wire.ReadRows, allocator, payload);
    errdefer parsed.deinit();
    if (parsed.value.maximumBytes == 0 or
        parsed.value.maximumBytes > protocol.Limits.row_window_bytes or
        parsed.value.rowCount == 0 or
        parsed.value.rowCount > protocol.Limits.row_window_rows)
    {
        return error.InvalidField;
    }
    const origin = try parseRowOrigin(parsed.value.origin);
    const purpose = try parseRowPurpose(parsed.value.purpose);
    const revision_policy = try parseRowRevisionPolicy(parsed.value.revisionPolicy);
    if (revision_policy == .current_view and
        (purpose != .viewport or switch (origin) {
            .committed, .tail => false,
            .absolute, .oldest => true,
        }))
    {
        return error.InvalidField;
    }
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .detail = try parseRowDetail(parsed.value.detail),
            .generation = try decode_common.parsePositiveCanonicalU64(parsed.value.generation),
            .maximum_bytes = parsed.value.maximumBytes,
            .origin = origin,
            .purpose = purpose,
            .request_sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.requestSequence),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
            .revision_policy = revision_policy,
            .row_count = parsed.value.rowCount,
        },
    };
}

pub fn decodeRowsApplied(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.RowsApplied, RowsApplied) {
    const parsed = try control_wire.decode(control_wire.RowsApplied, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .generation = try decode_common.parsePositiveCanonicalU64(parsed.value.generation),
            .request_sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.requestSequence),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeHistoryDemand(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SetHistoryDemand, HistoryDemand) {
    const parsed = try control_wire.decode(control_wire.SetHistoryDemand, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .demand = try parseHistoryDemand(parsed.value.demand),
        },
    };
}

pub fn decodeHistoryObserved(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.HistoryObserved, HistoryObserved) {
    const parsed = try control_wire.decode(control_wire.HistoryObserved, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .generation = try decode_common.parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
            .sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.sequence),
        },
    };
}

pub fn parseRowOrigin(value: control_wire.RowOrigin) error{InvalidField}!terminal_history.Origin {
    if (std.mem.eql(u8, value.kind, "absolute")) {
        if (value.delta != .missing or value.offset != .missing or value.rowsBefore != .missing) {
            return error.InvalidField;
        }
        return .{ .absolute = try decode_common.parseCanonicalU64(switch (value.row) {
            .value => |row| row,
            .missing, .null_value => return error.InvalidField,
        }) };
    }
    if (std.mem.eql(u8, value.kind, "oldest")) {
        if (value.delta != .missing or value.row != .missing or value.rowsBefore != .missing) {
            return error.InvalidField;
        }
        return .{ .oldest = switch (value.offset) {
            .value => |offset| offset,
            .missing, .null_value => return error.InvalidField,
        } };
    }
    if (std.mem.eql(u8, value.kind, "committed")) {
        if (value.offset != .missing or value.row != .missing or value.rowsBefore != .missing) {
            return error.InvalidField;
        }
        return .{ .committed = switch (value.delta) {
            .value => |delta| if (delta >= -terminal_history.max_row_window_rows and
                delta <= terminal_history.max_row_window_rows)
                delta
            else
                return error.InvalidField,
            .missing, .null_value => return error.InvalidField,
        } };
    }
    if (std.mem.eql(u8, value.kind, "tail")) {
        if (value.delta != .missing or value.offset != .missing or value.row != .missing) {
            return error.InvalidField;
        }
        return .{ .tail = switch (value.rowsBefore) {
            .value => |rows_before| rows_before,
            .missing, .null_value => return error.InvalidField,
        } };
    }
    return error.InvalidField;
}
fn parseRowDetail(value: []const u8) error{InvalidField}!terminal_history.Detail {
    if (std.mem.eql(u8, value, "render")) return .render;
    if (std.mem.eql(u8, value, "text")) return .text;
    return error.InvalidField;
}

fn parseRowPurpose(value: []const u8) error{InvalidField}!terminal_history.Purpose {
    if (std.mem.eql(u8, value, "buffer")) return .buffer;
    if (std.mem.eql(u8, value, "search")) return .search;
    if (std.mem.eql(u8, value, "viewport")) return .viewport;
    return error.InvalidField;
}

fn parseRowRevisionPolicy(
    value: []const u8,
) error{InvalidField}!terminal_history.RevisionPolicy {
    if (std.mem.eql(u8, value, "current-view")) return .current_view;
    if (std.mem.eql(u8, value, "exact")) return .exact;
    return error.InvalidField;
}

fn parseHistoryDemand(value: []const u8) error{InvalidField}!terminal_history.HistoryDemand {
    if (std.mem.eql(u8, value, "following-bottom")) return .following_bottom;
    if (std.mem.eql(u8, value, "viewing-history")) return .viewing_history;
    return error.InvalidField;
}
