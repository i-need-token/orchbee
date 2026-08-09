const std = @import("std");

pub const Parsed = std.json.Parsed;

pub const DecodeError = error{
    InvalidControlJson,
    OutOfMemory,
};

pub fn decode(
    comptime Wire: type,
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!std.json.Parsed(Wire) {
    return std.json.parseFromSlice(Wire, allocator, payload, .{}) catch |err| switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        else => error.InvalidControlJson,
    };
}

// These types own the control JSON shape only. host_control_decode owns the
// lossless conversion into typed commands, so field names match the wire.
pub fn Present(comptime T: type) type {
    return union(enum) {
        missing,
        null_value,
        value: T,

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

pub const Launch = struct {
    arguments: []const []const u8,
    cellPixelHeight: u16,
    cellPixelWidth: u16,
    columns: u16,
    configuration: ?TerminalConfiguration,
    currentDirectory: ?[]const u8,
    environment: []const Environment,
    executable: []const u8,
    rows: u16,
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
    defaultCursorRgb: ?u32,
    defaultForegroundRgb: u32,
    deviceAttributes: DeviceAttributes,
    enquiryResponse: []const u8,
    palette: []const u32,
    versionResponse: []const u8,
};

pub const Create = struct {
    creationId: []const u8,
    launch: Launch,
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
};

pub const SelectionPoint = struct {
    column: u16,
    row: []const u8,
};

pub const SelectionGesture = struct {
    action: []const u8,
    attachmentId: []const u8,
    clickCount: u2,
    generation: []const u8,
    point: SelectionPoint,
    rectangle: bool,
    revision: []const u8,
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
