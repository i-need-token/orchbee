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

pub const SelectionGesture = struct {
    attachment_id: identifiers.AttachmentId,
    event: terminal_history.SelectionGestureEvent,
    generation: u64,
    revision: u64,
};

pub const SelectionCommand = struct {
    attachment_id: identifiers.AttachmentId,
    command: terminal_history.SelectionCommand,
    generation: u64,
    revision: u64,
};

pub const SelectionExport = struct {
    attachment_id: identifiers.AttachmentId,
    export_sequence: u64,
    format: terminal_history.ExportFormat,
};

pub const SelectionExportChunkApplied = struct {
    attachment_id: identifiers.AttachmentId,
    chunk_sequence: u32,
    export_sequence: u64,
};

pub const ExportCancelReason = enum {
    clipboard_failed,
    disposed,
    replaced,
    user,
};

pub const SelectionExportCancel = struct {
    attachment_id: identifiers.AttachmentId,
    export_sequence: u64,
    reason: ExportCancelReason,
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
            .origin = try parseRowOrigin(parsed.value.origin),
            .purpose = try parseRowPurpose(parsed.value.purpose),
            .request_sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.requestSequence),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
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

pub fn decodeSelectionGesture(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SelectionGesture, SelectionGesture) {
    const parsed = try control_wire.decode(control_wire.SelectionGesture, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .event = try parseSelectionGestureEvent(parsed.value),
            .generation = try decode_common.parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeSelectionCommand(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SelectionCommand, SelectionCommand) {
    const parsed = try control_wire.decode(control_wire.SelectionCommand, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .command = try parseSelectionCommand(parsed.value),
            .generation = try decode_common.parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeSelectionExport(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SelectionExport, SelectionExport) {
    const parsed = try control_wire.decode(control_wire.SelectionExport, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .export_sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.exportSequence),
            .format = try parseSelectionExportFormat(parsed.value.format),
        },
    };
}

pub fn decodeSelectionExportChunkApplied(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SelectionExportChunkApplied, SelectionExportChunkApplied) {
    const parsed = try control_wire.decode(
        control_wire.SelectionExportChunkApplied,
        allocator,
        payload,
    );
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .chunk_sequence = parsed.value.chunkSequence,
            .export_sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.exportSequence),
        },
    };
}

pub fn decodeSelectionExportCancel(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SelectionExportCancel, SelectionExportCancel) {
    const parsed = try control_wire.decode(control_wire.SelectionExportCancel, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .export_sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.exportSequence),
            .reason = try parseExportCancelReason(parsed.value.reason),
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

fn parseHistoryDemand(value: []const u8) error{InvalidField}!terminal_history.HistoryDemand {
    if (std.mem.eql(u8, value, "following-bottom")) return .following_bottom;
    if (std.mem.eql(u8, value, "viewing-history")) return .viewing_history;
    return error.InvalidField;
}

fn parseSelectionGestureEvent(
    value: control_wire.SelectionGesture,
) error{InvalidField}!terminal_history.SelectionGestureEvent {
    const point = switch (value.point) {
        .value => |candidate| candidate,
        .missing, .null_value => null,
    };
    const position = switch (value.position) {
        .value => |candidate| candidate,
        .missing, .null_value => null,
    };
    if (position) |candidate| {
        if (candidate.x < -0xffff or candidate.x > 0xffff or
            candidate.y < -0xffff or candidate.y > 0xffff)
        {
            return error.InvalidField;
        }
    }
    if (std.mem.eql(u8, value.action, "start") and value.geometry == .missing and
        value.rectangle == .missing)
    {
        const click_count = switch (value.clickCount) {
            .value => |count| count,
            .missing, .null_value => return error.InvalidField,
        };
        if (click_count == 0) return error.InvalidField;
        return .{ .start = .{
            .click_count = click_count,
            .point = try parseSelectionPoint(point orelse return error.InvalidField),
            .position = selectionGesturePosition(position orelse return error.InvalidField),
            .select_output = switch (value.selectOutput) {
                .value => |select_output| select_output,
                .missing, .null_value => return error.InvalidField,
            },
        } };
    }
    if (std.mem.eql(u8, value.action, "update") and value.clickCount == .missing and
        value.selectOutput == .missing)
    {
        const geometry = switch (value.geometry) {
            .value => |candidate| candidate,
            .missing, .null_value => return error.InvalidField,
        };
        if (geometry.columns == 0 or geometry.columns > protocol.Limits.frame_columns or
            geometry.cellWidth == 0 or
            geometry.cellWidth > protocol.Limits.cell_pixel_dimension or
            geometry.screenHeight == 0)
        {
            return error.InvalidField;
        }
        return .{ .update = .{
            .geometry = .{
                .cell_width = geometry.cellWidth,
                .columns = geometry.columns,
                .padding_left = geometry.paddingLeft,
                .screen_height = geometry.screenHeight,
            },
            .point = try parseSelectionPoint(point orelse return error.InvalidField),
            .position = selectionGesturePosition(position orelse return error.InvalidField),
            .rectangle = switch (value.rectangle) {
                .value => |rectangle| rectangle,
                .missing, .null_value => return error.InvalidField,
            },
        } };
    }
    if (std.mem.eql(u8, value.action, "end") and value.clickCount == .missing and
        value.geometry == .missing and value.position == .missing and
        value.rectangle == .missing and value.selectOutput == .missing)
    {
        return .{ .end = try parseSelectionPoint(point orelse return error.InvalidField) };
    }
    if (std.mem.eql(u8, value.action, "cancel") and value.clickCount == .missing and
        value.geometry == .missing and value.point == .missing and value.position == .missing and
        value.rectangle == .missing and value.selectOutput == .missing)
    {
        return .cancel;
    }
    return error.InvalidField;
}

fn selectionGesturePosition(
    value: control_wire.SelectionGesturePosition,
) terminal_history.SelectionGesturePosition {
    return .{ .x = value.x, .y = value.y };
}

fn parseSelectionPoint(
    value: control_wire.SelectionPoint,
) error{InvalidField}!terminal_history.SelectionPoint {
    if (value.column >= protocol.Limits.frame_columns) return error.InvalidField;
    return .{
        .column = value.column,
        .row = try decode_common.parseCanonicalU64(value.row),
    };
}

fn parseSelectionCommand(
    value: control_wire.SelectionCommand,
) error{InvalidField}!terminal_history.SelectionCommand {
    const range_fields_missing = value.start == .missing and value.end == .missing and
        value.rectangle == .missing;
    if (std.mem.eql(u8, value.command, "clear") and value.point == .missing and
        range_fields_missing) return .clear;
    if (std.mem.eql(u8, value.command, "select-all") and value.point == .missing and
        range_fields_missing)
    {
        return .select_all;
    }
    if (std.mem.eql(u8, value.command, "select-output") and range_fields_missing) {
        return .{ .select_output = try parseSelectionPoint(switch (value.point) {
            .value => |point| point,
            .missing, .null_value => return error.InvalidField,
        }) };
    }
    if (std.mem.eql(u8, value.command, "set-range") and value.point == .missing) {
        return .{ .set_range = .{
            .end = try parseSelectionPoint(switch (value.end) {
                .value => |point| point,
                .missing, .null_value => return error.InvalidField,
            }),
            .rectangle = switch (value.rectangle) {
                .value => |rectangle| rectangle,
                .missing, .null_value => return error.InvalidField,
            },
            .start = try parseSelectionPoint(switch (value.start) {
                .value => |point| point,
                .missing, .null_value => return error.InvalidField,
            }),
        } };
    }
    return error.InvalidField;
}

fn parseSelectionExportFormat(value: []const u8) error{InvalidField}!terminal_history.ExportFormat {
    if (std.mem.eql(u8, value, "html")) return .html;
    if (std.mem.eql(u8, value, "plain")) return .plain;
    if (std.mem.eql(u8, value, "vt")) return .vt;
    return error.InvalidField;
}

fn parseExportCancelReason(value: []const u8) error{InvalidField}!ExportCancelReason {
    if (std.mem.eql(u8, value, "clipboard-failed")) return .clipboard_failed;
    if (std.mem.eql(u8, value, "disposed")) return .disposed;
    if (std.mem.eql(u8, value, "replaced")) return .replaced;
    if (std.mem.eql(u8, value, "user")) return .user;
    return error.InvalidField;
}
