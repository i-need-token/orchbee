const std = @import("std");

const control_wire = @import("wire.zig");
const core_module = @import("terminal-host-core").state;
const engine = @import("terminal-host-engine");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_history = engine.history;
const terminal_launch = engine.launch;
const terminal_profile = engine.profile;

pub const DecodeError = control_wire.DecodeError || error{
    CommandInspectionOutOfMemory,
    InvalidField,
    InvalidIdentifier,
};

pub fn Decoded(comptime Wire: type, comptime Command: type) type {
    return struct {
        parsed: control_wire.Parsed(Wire),
        value: Command,

        pub fn deinit(self: *@This()) void {
            self.parsed.deinit();
            self.* = undefined;
        }
    };
}

pub const ProtocolVersion = struct {
    major: u16,
    minor: u16,
    patch: u16,
};

pub const Hello = struct {
    client_id: identifiers.ClientId,
    frame_bytes: usize,
    input_bytes: usize,
    protocol_version: ProtocolVersion,
    row_window_bytes: usize,
};

pub const InventoryCursor = struct {
    generation: u64,
    offset: usize,
};

pub const ListTerminals = struct {
    cursor: ?InventoryCursor,
    maximum_terminals: usize,
};

pub const CreateTerminal = struct {
    creation_id: identifiers.CreationId,
    launch: terminal_launch.Validated,
};

pub const UpdateTerminalConfiguration = struct {
    configuration: terminal_configuration.Configuration,
    terminal_id: identifiers.TerminalId,
};

pub const AttachTerminal = struct {
    attachment_id: identifiers.AttachmentId,
    capabilities: core_module.Capabilities,
    request_resize_lease: bool,
    terminal_id: identifiers.TerminalId,
};

pub const ResizeTerminal = struct {
    attachment_id: identifiers.AttachmentId,
    geometry: terminal_geometry.Geometry,
};

pub const FrameApplied = struct {
    attachment_id: identifiers.AttachmentId,
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub const SnapshotReason = enum {
    base_mismatch,
    decoder_rejected,
    dimensions_mismatch,
    generation_mismatch,
    identity_mismatch,
    sequence_gap,
    store_reset,
};

pub const NeedSnapshot = struct {
    attachment_id: identifiers.AttachmentId,
    reason: SnapshotReason,
    sequence: u64,
};

pub const ProjectionDemand = struct {
    attachment_id: identifiers.AttachmentId,
    demand: core_module.ProjectionDemand,
};

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
};

pub const SelectionGesture = struct {
    action: terminal_history.SelectionGestureAction,
    attachment_id: identifiers.AttachmentId,
    click_count: u2,
    generation: u64,
    point: terminal_history.SelectionPoint,
    rectangle: bool,
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

pub fn parseCanonicalU64(value: []const u8) error{InvalidField}!u64 {
    if (value.len == 0 or (value.len > 1 and value[0] == '0')) return error.InvalidField;
    for (value) |byte| if (byte < '0' or byte > '9') return error.InvalidField;
    return std.fmt.parseInt(u64, value, 10) catch error.InvalidField;
}

fn parsePositiveCanonicalU64(value: []const u8) error{InvalidField}!u64 {
    const parsed = try parseCanonicalU64(value);
    if (parsed == 0) return error.InvalidField;
    return parsed;
}

pub fn decodeEmpty(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.Empty, void) {
    const parsed = try control_wire.decode(control_wire.Empty, allocator, payload);
    return .{ .parsed = parsed, .value = {} };
}

pub fn decodeHello(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.Hello, Hello) {
    const parsed = try control_wire.decode(control_wire.Hello, allocator, payload);
    errdefer parsed.deinit();
    if (!validHello(parsed.value)) return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .client_id = try identifiers.parseLowerHex(identifiers.ClientId, parsed.value.clientId),
            .frame_bytes = parsed.value.frameBytes,
            .input_bytes = parsed.value.inputBytes,
            .protocol_version = .{
                .major = parsed.value.protocol.major,
                .minor = parsed.value.protocol.minor,
                .patch = parsed.value.protocol.patch,
            },
            .row_window_bytes = parsed.value.rowWindowBytes,
        },
    };
}

pub fn decodeListTerminals(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.ListTerminals, ListTerminals) {
    const parsed = try control_wire.decode(control_wire.ListTerminals, allocator, payload);
    errdefer parsed.deinit();
    if (parsed.value.maximumTerminals == 0 or
        parsed.value.maximumTerminals > protocol.Limits.inventory_page_terminals)
    {
        return error.InvalidField;
    }
    const cursor: ?InventoryCursor = if (parsed.value.cursor) |wire_cursor| .{
        .generation = try parsePositiveCanonicalU64(wire_cursor.inventoryGeneration),
        .offset = std.math.cast(
            usize,
            try parseCanonicalU64(wire_cursor.offset),
        ) orelse return error.InvalidField,
    } else null;
    return .{
        .parsed = parsed,
        .value = .{
            .cursor = cursor,
            .maximum_terminals = parsed.value.maximumTerminals,
        },
    };
}

pub fn decodeCreateTerminal(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.Create, CreateTerminal) {
    const parsed = try control_wire.decode(control_wire.Create, allocator, payload);
    errdefer parsed.deinit();
    const creation_id = try identifiers.parseLowerHex(
        identifiers.CreationId,
        parsed.value.creationId,
    );
    const profile = profileFromWire(parsed.value.launch.terminalProfile) orelse
        return error.InvalidField;
    // The translated slice borrows wire strings, so make it part of the same
    // arena and lifetime as the parsed command.
    const environment = parsed.arena.allocator().alloc(
        terminal_launch.EnvironmentEntry,
        parsed.value.launch.environment.len,
    ) catch return error.CommandInspectionOutOfMemory;
    for (environment, parsed.value.launch.environment) |*target, source| {
        target.* = .{ .name = source.name, .value = source.value };
    }
    const wire_launch = parsed.value.launch;
    const launch: terminal_launch.Launch = .{
        .arguments = wire_launch.arguments,
        .configuration = if (wire_launch.configuration) |configuration| .{
            .scrollback_bytes = configuration.scrollbackBytes,
        } else null,
        .current_directory = wire_launch.currentDirectory,
        .environment = environment,
        .executable = wire_launch.executable,
        .geometry = .{
            .cell_pixel_height = wire_launch.cellPixelHeight,
            .cell_pixel_width = wire_launch.cellPixelWidth,
            .columns = wire_launch.columns,
            .rows = wire_launch.rows,
        },
        .profile = profile,
    };
    return .{
        .parsed = parsed,
        .value = .{
            .creation_id = creation_id,
            .launch = terminal_launch.inspect(allocator, launch) catch |err| switch (err) {
                error.InvalidLaunch => return error.InvalidField,
                error.OutOfMemory => return error.CommandInspectionOutOfMemory,
            },
        },
    };
}

fn profileFromWire(profile: control_wire.TerminalProfile) ?terminal_profile.Profile {
    const color_scheme: terminal_profile.ColorScheme = if (std.mem.eql(u8, profile.colorScheme, "dark"))
        .dark
    else if (std.mem.eql(u8, profile.colorScheme, "light"))
        .light
    else
        return null;
    return .{
        .color_scheme = color_scheme,
        .default_background_rgb = profile.defaultBackgroundRgb,
        .default_cursor_rgb = profile.defaultCursorRgb,
        .default_foreground_rgb = profile.defaultForegroundRgb,
        .device_primary_conformance_level = profile.deviceAttributes.primaryConformanceLevel,
        .device_primary_features = profile.deviceAttributes.primaryFeatures,
        .device_secondary_type = profile.deviceAttributes.secondaryDeviceType,
        .device_secondary_firmware_version = profile.deviceAttributes.secondaryFirmwareVersion,
        .device_secondary_rom_cartridge = profile.deviceAttributes.secondaryRomCartridge,
        .device_tertiary_unit_id = profile.deviceAttributes.tertiaryUnitId,
        .enquiry_response = profile.enquiryResponse,
        .palette = profile.palette,
        .version_response = profile.versionResponse,
    };
}

pub fn decodeSetTerminalDefaults(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SetTerminalDefaults, terminal_configuration.Configuration) {
    const parsed = try control_wire.decode(control_wire.SetTerminalDefaults, allocator, payload);
    errdefer parsed.deinit();
    const configuration: terminal_configuration.Configuration = .{
        .scrollback_bytes = parsed.value.configuration.scrollbackBytes,
    };
    if (!terminal_configuration.valid(configuration)) return error.InvalidField;
    return .{ .parsed = parsed, .value = configuration };
}

pub fn decodeUpdateTerminalConfiguration(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.UpdateTerminalConfiguration, UpdateTerminalConfiguration) {
    const parsed = try control_wire.decode(
        control_wire.UpdateTerminalConfiguration,
        allocator,
        payload,
    );
    errdefer parsed.deinit();
    const configuration: terminal_configuration.Configuration = .{
        .scrollback_bytes = parsed.value.configuration.scrollbackBytes,
    };
    if (!terminal_configuration.valid(configuration)) return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .configuration = configuration,
            .terminal_id = try identifiers.parseLowerHex(
                identifiers.TerminalId,
                parsed.value.terminalId,
            ),
        },
    };
}

pub fn decodeTerminalTarget(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.TerminalTarget, identifiers.TerminalId) {
    const parsed = try control_wire.decode(control_wire.TerminalTarget, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = try identifiers.parseLowerHex(identifiers.TerminalId, parsed.value.terminalId),
    };
}

pub fn decodeAttachTerminal(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.Attach, AttachTerminal) {
    const parsed = try control_wire.decode(control_wire.Attach, allocator, payload);
    errdefer parsed.deinit();
    const capabilities: core_module.Capabilities = .{
        .input = parsed.value.capabilities.input,
        .read = parsed.value.capabilities.read,
        .resize = parsed.value.capabilities.resize,
    };
    if (!capabilities.input and !capabilities.read and !capabilities.resize) {
        return error.InvalidField;
    }
    if (parsed.value.requestResizeLease and !capabilities.resize) {
        return error.InvalidField;
    }
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .capabilities = capabilities,
            .request_resize_lease = parsed.value.requestResizeLease,
            .terminal_id = try identifiers.parseLowerHex(
                identifiers.TerminalId,
                parsed.value.terminalId,
            ),
        },
    };
}

pub fn decodeDetachTerminal(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.Detach, identifiers.AttachmentId) {
    const parsed = try control_wire.decode(control_wire.Detach, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = try identifiers.parseLowerHex(
            identifiers.AttachmentId,
            parsed.value.attachmentId,
        ),
    };
}

pub fn decodeResizeTerminal(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.Resize, ResizeTerminal) {
    const parsed = try control_wire.decode(control_wire.Resize, allocator, payload);
    errdefer parsed.deinit();
    const geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = parsed.value.cellPixelHeight,
        .cell_pixel_width = parsed.value.cellPixelWidth,
        .columns = parsed.value.columns,
        .rows = parsed.value.rows,
    };
    if (!terminal_geometry.isValid(geometry)) return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .geometry = geometry,
        },
    };
}

pub fn decodeFrameApplied(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.FrameApplied, FrameApplied) {
    const parsed = try control_wire.decode(control_wire.FrameApplied, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .generation = try parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try parseCanonicalU64(parsed.value.revision),
            .sequence = try parsePositiveCanonicalU64(parsed.value.sequence),
        },
    };
}

pub fn decodeNeedSnapshot(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.NeedSnapshot, NeedSnapshot) {
    const parsed = try control_wire.decode(control_wire.NeedSnapshot, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .reason = try parseSnapshotReason(parsed.value.reason),
            .sequence = try parsePositiveCanonicalU64(parsed.value.sequence),
        },
    };
}

pub fn decodeProjectionDemand(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SetProjectionDemand, ProjectionDemand) {
    const parsed = try control_wire.decode(control_wire.SetProjectionDemand, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .demand = try parseProjectionDemand(parsed.value.demand),
        },
    };
}

pub fn decodeReadRows(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.ReadRows, ReadRows) {
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
            .generation = try parsePositiveCanonicalU64(parsed.value.generation),
            .maximum_bytes = parsed.value.maximumBytes,
            .origin = try parseRowOrigin(parsed.value.origin),
            .purpose = try parseRowPurpose(parsed.value.purpose),
            .request_sequence = try parsePositiveCanonicalU64(parsed.value.requestSequence),
            .revision = try parseCanonicalU64(parsed.value.revision),
            .row_count = parsed.value.rowCount,
        },
    };
}

pub fn decodeRowsApplied(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.RowsApplied, RowsApplied) {
    const parsed = try control_wire.decode(control_wire.RowsApplied, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .generation = try parsePositiveCanonicalU64(parsed.value.generation),
            .request_sequence = try parsePositiveCanonicalU64(parsed.value.requestSequence),
            .revision = try parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeHistoryDemand(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SetHistoryDemand, HistoryDemand) {
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
) DecodeError!Decoded(control_wire.HistoryObserved, HistoryObserved) {
    const parsed = try control_wire.decode(control_wire.HistoryObserved, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .generation = try parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeSelectionGesture(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SelectionGesture, SelectionGesture) {
    const parsed = try control_wire.decode(control_wire.SelectionGesture, allocator, payload);
    errdefer parsed.deinit();
    if (parsed.value.clickCount == 0) return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .action = try parseSelectionGestureAction(parsed.value.action),
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .click_count = parsed.value.clickCount,
            .generation = try parsePositiveCanonicalU64(parsed.value.generation),
            .point = try parseSelectionPoint(parsed.value.point),
            .rectangle = parsed.value.rectangle,
            .revision = try parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeSelectionCommand(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SelectionCommand, SelectionCommand) {
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
            .generation = try parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try parseCanonicalU64(parsed.value.revision),
        },
    };
}

pub fn decodeSelectionExport(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SelectionExport, SelectionExport) {
    const parsed = try control_wire.decode(control_wire.SelectionExport, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .export_sequence = try parsePositiveCanonicalU64(parsed.value.exportSequence),
            .format = try parseSelectionExportFormat(parsed.value.format),
        },
    };
}

pub fn decodeSelectionExportChunkApplied(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SelectionExportChunkApplied, SelectionExportChunkApplied) {
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
            .export_sequence = try parsePositiveCanonicalU64(parsed.value.exportSequence),
        },
    };
}

pub fn decodeSelectionExportCancel(
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!Decoded(control_wire.SelectionExportCancel, SelectionExportCancel) {
    const parsed = try control_wire.decode(control_wire.SelectionExportCancel, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .export_sequence = try parsePositiveCanonicalU64(parsed.value.exportSequence),
            .reason = try parseExportCancelReason(parsed.value.reason),
        },
    };
}

pub fn validHello(hello: control_wire.Hello) bool {
    if (hello.frameBytes == 0 or hello.frameBytes > protocol.Limits.frame_bytes or
        hello.inputBytes == 0 or hello.inputBytes > protocol.Limits.input_bytes or
        hello.rowWindowBytes == 0 or hello.rowWindowBytes > protocol.Limits.row_window_bytes or
        hello.supportedOptionalFeatures.len > protocol.Limits.optional_features)
    {
        return false;
    }
    for (hello.supportedOptionalFeatures) |feature| {
        if (feature.len == 0 or feature.len > protocol.Limits.optional_feature_name_bytes) {
            return false;
        }
    }
    return true;
}

pub fn parseRowOrigin(value: control_wire.RowOrigin) error{InvalidField}!terminal_history.Origin {
    if (std.mem.eql(u8, value.kind, "absolute")) {
        if (value.delta != .missing or value.offset != .missing or value.rowsBefore != .missing) {
            return error.InvalidField;
        }
        return .{ .absolute = try parseCanonicalU64(switch (value.row) {
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

fn parseSnapshotReason(value: []const u8) error{InvalidField}!SnapshotReason {
    inline for (.{
        .{ "base-mismatch", SnapshotReason.base_mismatch },
        .{ "decoder-rejected", SnapshotReason.decoder_rejected },
        .{ "dimensions-mismatch", SnapshotReason.dimensions_mismatch },
        .{ "generation-mismatch", SnapshotReason.generation_mismatch },
        .{ "identity-mismatch", SnapshotReason.identity_mismatch },
        .{ "sequence-gap", SnapshotReason.sequence_gap },
        .{ "store-reset", SnapshotReason.store_reset },
    }) |candidate| {
        if (std.mem.eql(u8, value, candidate[0])) return candidate[1];
    }
    return error.InvalidField;
}

fn parseProjectionDemand(value: []const u8) error{InvalidField}!core_module.ProjectionDemand {
    if (std.mem.eql(u8, value, "active")) return .active;
    if (std.mem.eql(u8, value, "paused")) return .paused;
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

fn parseSelectionGestureAction(
    value: []const u8,
) error{InvalidField}!terminal_history.SelectionGestureAction {
    if (std.mem.eql(u8, value, "end")) return .end;
    if (std.mem.eql(u8, value, "start")) return .start;
    if (std.mem.eql(u8, value, "update")) return .update;
    return error.InvalidField;
}

fn parseSelectionPoint(
    value: control_wire.SelectionPoint,
) error{InvalidField}!terminal_history.SelectionPoint {
    if (value.column >= protocol.Limits.frame_columns) return error.InvalidField;
    return .{
        .column = value.column,
        .row = try parseCanonicalU64(value.row),
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
