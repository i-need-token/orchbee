const std = @import("std");

pub const Parsed = std.json.Parsed;

pub const DecodeError = error{
    InvalidField,
    InvalidJson,
    InvalidUtf8,
    OutOfMemory,
};

const max_safe_integer: f64 = 9_007_199_254_740_991.0;

pub fn decode(
    comptime Wire: type,
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!std.json.Parsed(Wire) {
    if (!std.unicode.utf8ValidateSlice(payload)) return error.InvalidUtf8;

    var dynamic = std.json.parseFromSlice(
        std.json.Value,
        allocator,
        payload,
        .{ .parse_numbers = false },
    ) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.InvalidJson,
    };
    errdefer dynamic.deinit();

    refineValue(Wire, &dynamic.value) catch return error.InvalidField;
    const value = std.json.parseFromValueLeaky(
        Wire,
        dynamic.arena.allocator(),
        dynamic.value,
        .{},
    ) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.InvalidField,
    };
    return .{ .arena = dynamic.arena, .value = value };
}

fn refineValue(comptime T: type, value: *std.json.Value) error{InvalidField}!void {
    if (T == std.json.Value) return;

    switch (@typeInfo(T)) {
        .bool => if (value.* != .bool) return error.InvalidField,
        .int => try refineInteger(T, value),
        .optional => |optional| switch (value.*) {
            .null => {},
            else => try refineValue(optional.child, value),
        },
        .@"struct" => |struct_info| {
            if (struct_info.is_tuple) {
                if (value.* != .array or value.array.items.len != struct_info.fields.len) {
                    return error.InvalidField;
                }
                inline for (struct_info.fields, 0..) |field, index| {
                    try refineValue(field.type, &value.array.items[index]);
                }
                return;
            }
            if (value.* != .object) return error.InvalidField;

            var fields_seen = [_]bool{false} ** struct_info.fields.len;
            var iterator = value.object.iterator();
            while (iterator.next()) |entry| {
                inline for (struct_info.fields, 0..) |field, index| {
                    if (std.mem.eql(u8, field.name, entry.key_ptr.*)) {
                        try refineValue(field.type, entry.value_ptr);
                        fields_seen[index] = true;
                        break;
                    }
                } else return error.InvalidField;
            }
            inline for (struct_info.fields, fields_seen) |field, seen| {
                if (!seen and field.default_value_ptr == null) return error.InvalidField;
            }
        },
        .@"union" => {
            if (!@hasDecl(T, "wire_present_child")) return error.InvalidField;
            switch (value.*) {
                .null => {},
                else => try refineValue(T.wire_present_child, value),
            }
        },
        .array => |array| {
            if (value.* != .array or value.array.items.len != array.len) {
                return error.InvalidField;
            }
            for (value.array.items) |*item| try refineValue(array.child, item);
        },
        .pointer => |pointer| switch (pointer.size) {
            .one => try refineValue(pointer.child, value),
            .slice => {
                if (pointer.child == u8) {
                    if (value.* != .string) return error.InvalidField;
                    return;
                }
                if (value.* != .array) return error.InvalidField;
                for (value.array.items) |*item| try refineValue(pointer.child, item);
            },
            else => return error.InvalidField,
        },
        else => return error.InvalidField,
    }
}

fn refineInteger(comptime T: type, value: *std.json.Value) error{InvalidField}!void {
    const lexeme = switch (value.*) {
        .number_string => |number| number,
        else => return error.InvalidField,
    };
    // The pinned Zig conversion is the admitted refinement for ECMAScript
    // decimal-to-binary64 conversion; integer constraints apply afterward.
    const number = std.fmt.parseFloat(f64, lexeme) catch return error.InvalidField;
    if (!std.math.isFinite(number) or @trunc(number) != number or
        number < -max_safe_integer or number > max_safe_integer)
    {
        return error.InvalidField;
    }
    const minimum: f64 = @floatFromInt(std.math.minInt(T));
    const maximum: f64 = @floatFromInt(std.math.maxInt(T));
    if (number < minimum or number > maximum) return error.InvalidField;

    const normalized: i64 = @intFromFloat(number);
    value.* = .{ .integer = normalized };
}

// These types own the control JSON shape only. host_control_decode owns the
// lossless conversion into typed commands, so field names match the wire.
pub fn Present(comptime T: type) type {
    return union(enum) {
        missing,
        null_value,
        value: T,

        pub const wire_present_child = T;

        pub fn jsonParse(
            allocator: std.mem.Allocator,
            source: anytype,
            options: std.json.ParseOptions,
        ) !@This() {
            if (try source.peekNextTokenType() == .null) {
                _ = try source.next();
                return .null_value;
            }
            return .{ .value = try std.json.innerParse(T, allocator, source, options) };
        }

        pub fn jsonParseFromValue(
            allocator: std.mem.Allocator,
            source: std.json.Value,
            options: std.json.ParseOptions,
        ) !@This() {
            if (source == .null) return .null_value;
            return .{
                .value = try std.json.innerParseFromValue(T, allocator, source, options),
            };
        }
    };
}

pub const ProtocolVersion = struct {
    major: u16,
    minor: u16,
    patch: u16,
};

pub const Hello = struct {
    clientId: []const u8,
    frameBytes: usize,
    inputBytes: usize,
    protocol: ProtocolVersion,
    rowWindowBytes: usize,
    supportedOptionalFeatures: []const []const u8,
};

pub const Environment = struct {
    name: []const u8,
    value: ?[]const u8,
};

pub const EnvironmentOverlay = struct {
    name: []const u8,
    value: []const u8,
};

pub const Launch = struct {
    arguments: []const []const u8,
    cellPixelHeight: ?u16,
    cellPixelWidth: ?u16,
    columns: ?u16,
    configuration: ?TerminalConfiguration,
    currentDirectory: ?[]const u8,
    environment: []const Environment,
    executable: []const u8,
    rows: ?u16,
    terminalProfile: TerminalProfile,
};

pub const TerminalConfiguration = struct {
    scrollbackBytes: usize,
};

pub const DeviceAttributes = struct {
    primaryConformanceLevel: u16,
    primaryFeatures: []const u16,
    secondaryDeviceType: u16,
    secondaryFirmwareVersion: u16,
    secondaryRomCartridge: u16,
    tertiaryUnitId: u32,
};

pub const TerminalProfile = struct {
    colorScheme: []const u8,
    defaultBackgroundRgb: u32,
    defaultCursorBlinking: bool = false,
    defaultCursorRgb: ?u32,
    defaultCursorShape: []const u8 = "block",
    defaultForegroundRgb: u32,
    deviceAttributes: DeviceAttributes,
    enquiryResponse: []const u8,
    palette: []const u32,
    versionResponse: []const u8,
};

pub const Create = struct {
    creationId: []const u8,
    environmentOverlay: []const EnvironmentOverlay,
    launch: Launch,
};

pub const Geometry = struct {
    cellPixelHeight: u16,
    cellPixelWidth: u16,
    columns: u16,
    rows: u16,
};

pub const Start = struct {
    geometry: Geometry,
    terminalId: []const u8,
};

pub const InventoryCursor = struct {
    inventoryGeneration: []const u8,
    offset: []const u8,
};

pub const ListTerminals = struct {
    cursor: ?InventoryCursor,
    maximumTerminals: usize,
};

pub const TerminalTarget = struct { terminalId: []const u8 };

pub const Capabilities = struct {
    input: bool,
    read: bool,
    resize: bool,
};

pub const Attach = struct {
    attachmentId: []const u8,
    capabilities: Capabilities,
    requestResizeLease: bool,
    terminalId: []const u8,
};

pub const Detach = struct { attachmentId: []const u8 };

pub const UpdateAttachmentInputCapability = struct {
    attachmentId: []const u8,
    capabilityRevision: []const u8,
    input: bool,
};

pub const ClaimResizeLease = struct { attachmentId: []const u8 };

pub const TerminalControl = struct { attachmentId: []const u8 };
pub const ClearTerminal = struct {
    attachmentId: []const u8,
    preservePrompt: bool = false,
};

pub const Resize = struct {
    attachmentId: []const u8,
    cellPixelHeight: u16,
    cellPixelWidth: u16,
    columns: u16,
    rows: u16,
};

pub const SetTerminalDefaults = struct {
    configuration: TerminalConfiguration,
};

pub const UpdateTerminalConfiguration = struct {
    configuration: TerminalConfiguration,
    terminalId: []const u8,
};

pub const TerminalTheme = struct {
    colorScheme: []const u8,
    defaultBackgroundRgb: u32,
    defaultCursorRgb: ?u32,
    defaultForegroundRgb: u32,
    palette: []const u32,
};

pub const ApplyTerminalTheme = struct {
    expectedThemeRevision: []const u8,
    operationId: []const u8,
    terminal: TerminalTheme,
    terminalId: []const u8,
};

pub const FrameApplied = struct {
    attachmentId: []const u8,
    generation: []const u8,
    revision: []const u8,
    sequence: []const u8,
};

pub const NeedSnapshot = struct {
    attachmentId: []const u8,
    reason: []const u8,
    sequence: []const u8,
};

pub const SetProjectionDemand = struct {
    attachmentId: []const u8,
    demand: []const u8,
};

pub const RowOrigin = struct {
    delta: Present(i32) = .missing,
    kind: []const u8,
    offset: Present(u32) = .missing,
    row: Present([]const u8) = .missing,
    rowsBefore: Present(u32) = .missing,
};

pub const ReadRows = struct {
    attachmentId: []const u8,
    detail: []const u8,
    generation: []const u8,
    maximumBytes: usize,
    origin: RowOrigin,
    purpose: []const u8,
    requestSequence: []const u8,
    revision: []const u8,
    revisionPolicy: []const u8,
    rowCount: u16,
};

pub const RowsApplied = struct {
    attachmentId: []const u8,
    generation: []const u8,
    requestSequence: []const u8,
    revision: []const u8,
};

pub const SetHistoryDemand = struct {
    attachmentId: []const u8,
    demand: []const u8,
};

pub const HistoryObserved = struct {
    attachmentId: []const u8,
    generation: []const u8,
    revision: []const u8,
    sequence: []const u8,
};

pub const SelectionPoint = struct {
    column: u16,
    row: []const u8,
};

pub const SelectionViewportPoint = struct {
    column: u16,
    row: u16,
};

pub const SelectionGestureGeometry = struct {
    cellWidth: u16,
    columns: u16,
    paddingLeft: u16,
    screenHeight: u16,
};

pub const SelectionGesturePosition = struct {
    x: i32,
    y: i32,
};

pub const SelectionGesture = struct {
    action: []const u8,
    attachmentId: []const u8,
    clickCount: Present(u2) = .missing,
    generation: []const u8,
    geometry: Present(SelectionGestureGeometry) = .missing,
    point: Present(SelectionViewportPoint) = .missing,
    position: Present(SelectionGesturePosition) = .missing,
    rectangle: Present(bool) = .missing,
    selectOutput: Present(bool) = .missing,
};

pub const SelectionCommand = struct {
    attachmentId: []const u8,
    command: []const u8,
    end: Present(SelectionPoint) = .missing,
    generation: []const u8,
    point: Present(SelectionPoint) = .missing,
    rectangle: Present(bool) = .missing,
    revision: []const u8,
    start: Present(SelectionPoint) = .missing,
};

pub const SelectionExport = struct {
    attachmentId: []const u8,
    exportSequence: []const u8,
    format: []const u8,
};

pub const SelectionExportChunkApplied = struct {
    attachmentId: []const u8,
    chunkSequence: u32,
    exportSequence: []const u8,
};

pub const SelectionExportCancel = struct {
    attachmentId: []const u8,
    exportSequence: []const u8,
    reason: []const u8,
};

pub const Empty = struct {};
