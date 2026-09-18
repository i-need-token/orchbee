const std = @import("std");

const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const model = @import("terminal-host-model");
const protocol = @import("terminal-protocol");
const attachment_model = model.attachment;
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_launch = model.launch;
const terminal_profile = model.profile;
pub const CreateTerminal = struct {
    creation_id: identifiers.CreationId,
    launch: terminal_launch.Validated,
};

pub const StartTerminal = struct {
    geometry: terminal_geometry.Geometry,
    terminal_id: identifiers.TerminalId,
};

pub const UpdateTerminalConfiguration = struct {
    configuration: terminal_configuration.Configuration,
    terminal_id: identifiers.TerminalId,
};

pub const AttachTerminal = struct {
    attachment_id: identifiers.AttachmentId,
    capabilities: attachment_model.Capabilities,
    request_resize_lease: bool,
    terminal_id: identifiers.TerminalId,
};

pub const UpdateAttachmentInputCapability = struct {
    attachment_id: identifiers.AttachmentId,
    capability_revision: u64,
    input: bool,
};

pub const ClaimResizeLease = struct {
    attachment_id: identifiers.AttachmentId,
};

pub const ResizeTerminal = struct {
    attachment_id: identifiers.AttachmentId,
    geometry: terminal_geometry.Geometry,
};
pub fn decodeCreateTerminal(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.Create, CreateTerminal) {
    const parsed = try control_wire.decode(control_wire.Create, allocator, payload);
    errdefer parsed.deinit();
    const creation_id = try identifiers.parseLowerHex(
        identifiers.CreationId,
        parsed.value.creationId,
    );
    const profile = profileFromWire(parsed.value.launch.terminalProfile) orelse
        return error.InvalidField;
    const environment = try translateEnvironment(
        parsed.arena.allocator(),
        parsed.value.launch.environment,
    );
    const environment_overlay = try translateEnvironmentOverlay(
        parsed.arena.allocator(),
        parsed.value.environmentOverlay,
    );
    const wire_launch = parsed.value.launch;
    const launch: terminal_launch.Launch = .{
        .arguments = wire_launch.arguments,
        .configuration = if (wire_launch.configuration) |configuration| .{
            .scrollback_bytes = configuration.scrollbackBytes,
        } else null,
        .current_directory = wire_launch.currentDirectory,
        .environment = environment,
        .executable = wire_launch.executable,
        .geometry = try optionalLaunchGeometry(wire_launch),
        .profile = profile,
    };
    // The canonical launch inspector applies the target-OS environment-name
    // identity rule (case-sensitive on POSIX, case-insensitive on Windows) and
    // the CreationId digest normalization before any Host dispatch. A rejected
    // launch, including target-duplicate environment names, is reported as the
    // typed `invalid-field` failure below.
    return .{
        .parsed = parsed,
        .value = .{
            .creation_id = creation_id,
            .launch = terminal_launch.inspectWithOverlay(
                allocator,
                launch,
                environment_overlay,
            ) catch |err| switch (err) {
                error.InvalidLaunch, error.InvalidEnvironmentOverlay => return error.InvalidField,
                error.OutOfMemory => return error.CommandInspectionOutOfMemory,
            },
        },
    };
}

pub fn decodeStartTerminal(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.Start, StartTerminal) {
    const parsed = try control_wire.decode(control_wire.Start, allocator, payload);
    errdefer parsed.deinit();
    const geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = parsed.value.geometry.cellPixelHeight,
        .cell_pixel_width = parsed.value.geometry.cellPixelWidth,
        .columns = parsed.value.geometry.columns,
        .rows = parsed.value.geometry.rows,
    };
    if (!terminal_geometry.isValid(geometry)) return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .geometry = geometry,
            .terminal_id = try identifiers.parseLowerHex(
                identifiers.TerminalId,
                parsed.value.terminalId,
            ),
        },
    };
}

fn optionalLaunchGeometry(
    launch: control_wire.Launch,
) decode_common.Error!?terminal_geometry.Geometry {
    if (launch.columns == null and launch.rows == null and
        launch.cellPixelWidth == null and launch.cellPixelHeight == null)
    {
        return null;
    }
    const geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = launch.cellPixelHeight orelse return error.InvalidField,
        .cell_pixel_width = launch.cellPixelWidth orelse return error.InvalidField,
        .columns = launch.columns orelse return error.InvalidField,
        .rows = launch.rows orelse return error.InvalidField,
    };
    if (!terminal_geometry.isValid(geometry)) return error.InvalidField;
    return geometry;
}

fn translateEnvironment(
    allocator: std.mem.Allocator,
    source_environment: []const control_wire.Environment,
) decode_common.Error![]terminal_launch.EnvironmentEntry {
    if (source_environment.len > protocol.Limits.launch_environment_entries) {
        return error.InvalidField;
    }
    // The translated slice borrows wire strings, so its caller allocates it in
    // the parsed command's arena and keeps both lifetimes aligned.
    const environment = allocator.alloc(
        terminal_launch.EnvironmentEntry,
        source_environment.len,
    ) catch return error.CommandInspectionOutOfMemory;
    for (environment, source_environment) |*target, source| {
        target.* = .{ .name = source.name, .value = source.value };
    }
    return environment;
}

fn translateEnvironmentOverlay(
    allocator: std.mem.Allocator,
    source_environment: []const control_wire.EnvironmentOverlay,
) decode_common.Error![]terminal_launch.EnvironmentOverlayEntry {
    if (source_environment.len > protocol.Limits.environment_overlay_entries) {
        return error.InvalidField;
    }
    const environment = allocator.alloc(
        terminal_launch.EnvironmentOverlayEntry,
        source_environment.len,
    ) catch return error.CommandInspectionOutOfMemory;
    for (environment, source_environment) |*target, source| {
        target.* = .{ .name = source.name, .value = source.value };
    }
    return environment;
}

pub const testing = struct {
    pub const optionalLaunchGeometryForTest = optionalLaunchGeometry;
    pub const translateEnvironmentForTest = translateEnvironment;
    pub const translateEnvironmentOverlayForTest = translateEnvironmentOverlay;
};

fn profileFromWire(profile: control_wire.TerminalProfile) ?terminal_profile.Profile {
    const color_scheme: terminal_profile.ColorScheme = if (std.mem.eql(u8, profile.colorScheme, "dark"))
        .dark
    else if (std.mem.eql(u8, profile.colorScheme, "light"))
        .light
    else
        return null;
    const cursor_shape: terminal_profile.CursorShape = if (std.mem.eql(u8, profile.defaultCursorShape, "block"))
        .block
    else if (std.mem.eql(u8, profile.defaultCursorShape, "underline"))
        .underline
    else if (std.mem.eql(u8, profile.defaultCursorShape, "bar"))
        .bar
    else
        return null;
    return .{
        .color_scheme = color_scheme,
        .default_background_rgb = profile.defaultBackgroundRgb,
        .default_cursor_blinking = profile.defaultCursorBlinking,
        .default_cursor_rgb = profile.defaultCursorRgb,
        .default_cursor_shape = cursor_shape,
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
) decode_common.Error!decode_common.Decoded(control_wire.SetTerminalDefaults, terminal_configuration.Configuration) {
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
) decode_common.Error!decode_common.Decoded(control_wire.UpdateTerminalConfiguration, UpdateTerminalConfiguration) {
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
) decode_common.Error!decode_common.Decoded(control_wire.TerminalTarget, identifiers.TerminalId) {
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
) decode_common.Error!decode_common.Decoded(control_wire.Attach, AttachTerminal) {
    const parsed = try control_wire.decode(control_wire.Attach, allocator, payload);
    errdefer parsed.deinit();
    const capabilities: attachment_model.Capabilities = .{
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
) decode_common.Error!decode_common.Decoded(control_wire.Detach, identifiers.AttachmentId) {
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

pub fn decodeUpdateAttachmentInputCapability(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(
    control_wire.UpdateAttachmentInputCapability,
    UpdateAttachmentInputCapability,
) {
    const parsed = try control_wire.decode(
        control_wire.UpdateAttachmentInputCapability,
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
            .capability_revision = try decode_common.parsePositiveCanonicalU64(
                parsed.value.capabilityRevision,
            ),
            .input = parsed.value.input,
        },
    };
}

pub fn decodeClaimResizeLease(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.ClaimResizeLease, ClaimResizeLease) {
    const parsed = try control_wire.decode(control_wire.ClaimResizeLease, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
        },
    };
}

pub fn decodeTerminalControl(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.TerminalControl, identifiers.AttachmentId) {
    const parsed = try control_wire.decode(control_wire.TerminalControl, allocator, payload);
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
) decode_common.Error!decode_common.Decoded(control_wire.Resize, ResizeTerminal) {
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
