const std = @import("std");

const common = @import("encode_common.zig");
const theme_encode = @import("encode_theme.zig");
const lifecycle_encode = @import("encode_lifecycle.zig");
const protocol = @import("terminal-protocol");

const identifierText = common.identifierText;
// Cross-language contract: search for `typed-failure-wire-limit` before
// changing either bound, then update every occurrence together.
const maximum_failure_code_bytes = 128;
const maximum_failure_message_bytes = 1024;

pub fn protocolRejected(
    allocator: std.mem.Allocator,
    client_major: u16,
    client_minor: u16,
    client_patch: u16,
    host_major: u16,
    host_minor: u16,
    host_patch: u16,
) ![]u8 {
    return std.fmt.allocPrint(
        allocator,
        "{{\"client\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"host\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"reason\":\"version-mismatch\"}}",
        .{ client_major, client_minor, client_patch, host_major, host_minor, host_patch },
    );
}

pub fn helloAccepted(
    allocator: std.mem.Allocator,
    limits: anytype,
    host_epoch: anytype,
    host_version: []const u8,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeHelloAccepted(&writer.writer, limits, host_epoch, host_version);
    return writer.toOwnedSlice();
}

pub fn writeHelloAccepted(
    writer: *std.Io.Writer,
    limits: anytype,
    host_epoch: anytype,
    host_version: []const u8,
) !void {
    var epoch_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"frameBytes\":{d},\"hostEpoch\":\"{s}\",\"hostVersion\":",
        .{
            limits.frame_bytes,
            identifierText(host_epoch, &epoch_buffer),
        },
    );
    try std.json.Stringify.value(host_version, .{}, writer);
    try writer.print(
        ",\"inputBytes\":{d},\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":{d},\"supportedOptionalFeatures\":[]}}",
        .{
            limits.input_bytes,
            protocol.current_protocol_major,
            protocol.current_protocol_minor,
            protocol.current_protocol_patch,
            limits.row_window_bytes,
        },
    );
}

pub fn typedFailure(
    allocator: std.mem.Allocator,
    code: []const u8,
    message: []const u8,
    retryable: bool,
) ![]u8 {
    return typedFailureWithReason(allocator, code, message, retryable, null);
}

pub fn typedFailureWithReason(
    allocator: std.mem.Allocator,
    code: []const u8,
    message: []const u8,
    retryable: bool,
    reason: ?[]const u8,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTypedFailureWithReason(&writer.writer, code, message, retryable, reason);
    return writer.toOwnedSlice();
}

pub fn writeTypedFailure(
    writer: *std.Io.Writer,
    code: []const u8,
    message: []const u8,
    retryable: bool,
) !void {
    return writeTypedFailureWithReason(writer, code, message, retryable, null);
}

/// A present `reason` refines the code with one closed kebab-case name and is
/// written after `retryable`; today only `prompt-unconfirmed` carries one.
pub fn writeTypedFailureWithReason(
    writer: *std.Io.Writer,
    code: []const u8,
    message: []const u8,
    retryable: bool,
    reason: ?[]const u8,
) !void {
    if (code.len == 0 or code.len > maximum_failure_code_bytes or
        message.len == 0 or message.len > maximum_failure_message_bytes or
        !std.unicode.utf8ValidateSlice(code) or
        !std.unicode.utf8ValidateSlice(message))
    {
        return error.InvalidControlValue;
    }
    if (reason) |value| {
        if (value.len == 0 or value.len > maximum_failure_code_bytes or
            !std.unicode.utf8ValidateSlice(value)) return error.InvalidControlValue;
    }
    try writer.writeAll("{\"code\":");
    try std.json.Stringify.value(code, .{}, writer);
    try writer.writeAll(",\"message\":");
    try std.json.Stringify.value(message, .{}, writer);
    try writer.print(",\"retryable\":{}", .{retryable});
    if (reason) |value| {
        try writer.writeAll(",\"reason\":");
        try std.json.Stringify.value(value, .{}, writer);
    }
    try writer.writeAll("}");
}

pub fn terminalIdentity(
    allocator: std.mem.Allocator,
    creation_id: anytype,
    host_epoch: anytype,
    terminal_id: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}}",
        .{
            identifierText(creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
}

pub fn terminalDefaultsAccepted(
    allocator: std.mem.Allocator,
    scrollback_bytes: usize,
    revision: u64,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalDefaultsAccepted(&writer.writer, scrollback_bytes, revision);
    return writer.toOwnedSlice();
}

pub fn writeTerminalDefaultsAccepted(
    writer: *std.Io.Writer,
    scrollback_bytes: usize,
    revision: u64,
) !void {
    try writer.print(
        "{{\"configuration\":{{\"scrollbackBytes\":{d}}},\"revision\":\"{d}\"}}",
        .{ scrollback_bytes, revision },
    );
}

pub fn terminalConfigurationAccepted(allocator: std.mem.Allocator, update: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalConfigurationAccepted(&writer.writer, update);
    return writer.toOwnedSlice();
}

pub fn writeTerminalConfigurationAccepted(writer: *std.Io.Writer, update: anytype) !void {
    var terminal_buffer: [32]u8 = undefined;
    try writer.print(
        "{{\"configuration\":{{\"scrollbackBytes\":{d}}},\"configurationRevision\":\"{d}\",\"generation\":\"{d}\",\"historyRowsEvicted\":\"{d}\",\"revision\":\"{d}\",\"terminalId\":\"{s}\"}}",
        .{
            update.configuration.scrollback_bytes,
            update.configuration_revision,
            update.generation,
            update.history_rows_evicted,
            update.revision,
            identifierText(update.terminal_id, &terminal_buffer),
        },
    );
}

pub fn terminalList(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    inventory_generation: u64,
    terminals: anytype,
    offset: usize,
    maximum_terminals: usize,
) ![]u8 {
    std.debug.assert(offset <= terminals.len);
    std.debug.assert(maximum_terminals > 0);
    const requested_length = @min(maximum_terminals, terminals.len - offset);
    var summaries: std.ArrayList([]u8) = .empty;
    defer {
        for (summaries.items) |summary| allocator.free(summary);
        summaries.deinit(allocator);
    }
    var summaries_bytes: usize = 0;
    for (terminals[offset .. offset + requested_length]) |terminal| {
        const summary = try terminalSummary(allocator, host_epoch, terminal);
        errdefer allocator.free(summary);
        const candidate_end = offset + summaries.items.len + 1;
        const framing_bytes = terminalListFramingLength(
            inventory_generation,
            candidate_end,
            terminals.len,
        );
        const separator_bytes = summaries.items.len;
        const available_summary_bytes = protocol.Limits.control_json_bytes -| framing_bytes -| separator_bytes;
        if (summaries_bytes > available_summary_bytes or
            summary.len > available_summary_bytes - summaries_bytes)
        {
            if (summaries.items.len == 0) return error.InventoryTerminalTooLarge;
            allocator.free(summary);
            break;
        }
        try summaries.append(allocator, summary);
        summaries_bytes += summary.len;
    }
    const end = offset + summaries.items.len;
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalListPrefix(
        &writer.writer,
        inventory_generation,
        end,
        terminals.len,
    );
    for (summaries.items, 0..) |summary, index| {
        if (index != 0) try writer.writer.writeByte(',');
        try writer.writer.writeAll(summary);
    }
    try writer.writer.writeAll("]}");
    const payload = try writer.toOwnedSlice();
    errdefer allocator.free(payload);
    std.debug.assert(payload.len <= protocol.Limits.control_json_bytes);
    return payload;
}

fn terminalListFramingLength(
    inventory_generation: u64,
    end: usize,
    terminal_count: usize,
) usize {
    var storage: [256]u8 = undefined;
    var writer: std.Io.Writer = .fixed(&storage);
    writeTerminalListPrefix(
        &writer,
        inventory_generation,
        end,
        terminal_count,
    ) catch unreachable;
    return writer.buffered().len + "]}".len;
}

fn writeTerminalListPrefix(
    writer: *std.Io.Writer,
    inventory_generation: u64,
    end: usize,
    terminal_count: usize,
) !void {
    try writer.print(
        "{{\"inventoryGeneration\":\"{d}\",\"nextCursor\":",
        .{inventory_generation},
    );
    if (end < terminal_count) {
        try writer.print(
            "{{\"inventoryGeneration\":\"{d}\",\"offset\":\"{d}\"}}",
            .{ inventory_generation, end },
        );
    } else {
        try writer.writeAll("null");
    }
    try writer.writeAll(",\"terminals\":[");
}

pub fn terminalSummary(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    terminal: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const exit_json = try lifecycle_encode.optionalExitResultJson(allocator, terminal.exitResult());
    defer allocator.free(exit_json);
    const failure_json = if (terminal.runtimeFailure()) |failure|
        try lifecycle_encode.runtimeFailureJson(allocator, failure)
    else
        try allocator.dupe(u8, "null");
    defer allocator.free(failure_json);
    const containment_json = if (terminal.failureContainment()) |containment|
        try lifecycle_encode.failureContainmentJson(allocator, containment)
    else
        try allocator.dupe(u8, "null");
    defer allocator.free(containment_json);
    const process_json = if (terminal.failedProcess()) |process|
        try lifecycle_encode.processObservationJson(allocator, process)
    else
        try allocator.dupe(u8, "null");
    defer allocator.free(process_json);
    const configuration = terminal.effectiveConfiguration();
    const columns_json = try terminalDimensionJson(allocator, terminal, "columns");
    defer allocator.free(columns_json);
    const rows_json = try terminalDimensionJson(allocator, terminal, "rows");
    defer allocator.free(rows_json);
    const theme_json = try theme_encode.terminalThemeState(allocator, terminal.themeState());
    defer allocator.free(theme_json);
    return std.fmt.allocPrint(
        allocator,
        "{{\"columns\":{s},\"configuration\":{{\"scrollbackBytes\":{d}}},\"configurationRevision\":\"{d}\",\"exit\":{s},\"failure\":{s},\"failureContainment\":{s},\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"phase\":\"{s}\",\"process\":{s},\"revision\":\"{d}\",\"rows\":{s},\"terminateRequested\":{},\"theme\":{s}}}",
        .{
            columns_json,
            configuration.scrollback_bytes,
            terminal.configurationRevision(),
            exit_json,
            failure_json,
            containment_json,
            identifierText(terminal.creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(terminal.terminal_id, &terminal_buffer),
            lifecycle_encode.terminalPhaseText(terminal.phase()),
            process_json,
            terminal.observedRevision(),
            rows_json,
            terminal.terminateRequested(),
            theme_json,
        },
    );
}

fn terminalDimensionJson(
    allocator: std.mem.Allocator,
    terminal: anytype,
    comptime field: []const u8,
) ![]u8 {
    const Value = switch (@typeInfo(@TypeOf(terminal))) {
        .pointer => |pointer| pointer.child,
        else => @TypeOf(terminal),
    };
    if (comptime @hasDecl(Value, "maybeObservedGeometry")) {
        const geometry = terminal.maybeObservedGeometry() orelse
            return allocator.dupe(u8, "null");
        return std.fmt.allocPrint(allocator, "{d}", .{@field(geometry, field)});
    }
    const geometry = terminal.observedGeometry();
    return std.fmt.allocPrint(allocator, "{d}", .{@field(geometry, field)});
}

pub const terminalExited = lifecycle_encode.terminalExited;
pub const terminalFailed = lifecycle_encode.terminalFailed;
