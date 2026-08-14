const std = @import("std");

const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const core_module = @import("terminal-host-core").state;
const engine = @import("terminal-host-engine");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_launch = engine.launch;
const terminal_profile = engine.profile;
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
    // The canonical launch inspector applies the target-OS environment-name
    // identity rule (case-sensitive on POSIX, case-insensitive on Windows) and
    // the CreationId digest normalization before any Host dispatch. A rejected
    // launch, including target-duplicate environment names, is reported as the
    // typed `invalid-field` failure below.
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
