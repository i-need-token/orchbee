const std = @import("std");
const target_builtin = @import("builtin");

const protocol = @import("terminal-protocol");
const terminal_configuration = @import("configuration.zig");
const terminal_geometry = @import("geometry.zig");
const terminal_profile = @import("profile.zig");

pub const Digest = [32]u8;

pub const EnvironmentEntry = struct {
    name: []const u8,
    value: ?[]const u8,
};

pub const EnvironmentOverlayEntry = struct {
    name: []const u8,
    value: []const u8,
};

pub const ValidatedEnvironmentOverlay = struct {
    digest: Digest,
    entries: []const EnvironmentOverlayEntry,
};

// Launch is the domain input. Boundary adapters translate their
// transport-specific shapes into this vocabulary before validation.
pub const Launch = struct {
    arguments: []const []const u8,
    configuration: ?terminal_configuration.Configuration,
    current_directory: ?[]const u8,
    environment: []const EnvironmentEntry,
    executable: []const u8,
    /// Null reserves identity and launch ownership for a later geometry-pinned start.
    geometry: ?terminal_geometry.Geometry,
    profile: terminal_profile.Profile,
};

// Strings and slices remain owned by the caller. Keep their owner alive until
// every runtime preparation derived from this value has been consumed.
pub const Validated = struct {
    environment_overlay: ValidatedEnvironmentOverlay,
    launch_digest: Digest,
    launch: Launch,
};

pub fn inspect(
    allocator: std.mem.Allocator,
    launch: Launch,
) (error{InvalidLaunch} || std.mem.Allocator.Error)!Validated {
    return inspectWithOverlay(allocator, launch, &.{}) catch |err| switch (err) {
        error.InvalidEnvironmentOverlay => unreachable,
        else => |forwarded| return forwarded,
    };
}

pub fn inspectWithOverlay(
    allocator: std.mem.Allocator,
    launch: Launch,
    environment_overlay: []const EnvironmentOverlayEntry,
) (error{ InvalidLaunch, InvalidEnvironmentOverlay } || std.mem.Allocator.Error)!Validated {
    if (!isValid(launch)) return error.InvalidLaunch;
    return .{
        .environment_overlay = try inspectEnvironmentOverlay(
            allocator,
            launch.environment,
            environment_overlay,
        ),
        .launch_digest = try launchDigest(allocator, launch),
        .launch = launch,
    };
}

pub fn inspectEnvironmentOverlay(
    allocator: std.mem.Allocator,
    launch_environment: []const EnvironmentEntry,
    environment_overlay: []const EnvironmentOverlayEntry,
) (error{InvalidEnvironmentOverlay} || std.mem.Allocator.Error)!ValidatedEnvironmentOverlay {
    if (environment_overlay.len > protocol.Limits.environment_overlay_entries) {
        return error.InvalidEnvironmentOverlay;
    }
    for (environment_overlay, 0..) |entry, index| {
        if (entry.name.len == 0 or entry.name.len > protocol.Limits.environment_name_bytes or
            containsNul(entry.name) or
            std.mem.indexOfScalar(u8, entry.name, '=') != null or
            entry.value.len > protocol.Limits.environment_overlay_value_bytes or
            containsNul(entry.value))
        {
            return error.InvalidEnvironmentOverlay;
        }
        for (launch_environment) |durable| {
            if (environmentNamesEqual(durable.name, entry.name)) {
                return error.InvalidEnvironmentOverlay;
            }
        }
        for (environment_overlay[0..index]) |previous| {
            if (environmentNamesEqual(previous.name, entry.name)) {
                return error.InvalidEnvironmentOverlay;
            }
        }
    }
    return .{
        .digest = try environmentOverlayDigest(allocator, environment_overlay),
        .entries = environment_overlay,
    };
}

fn isValid(launch: Launch) bool {
    if (launch.executable.len == 0 or containsNul(launch.executable) or
        launch.arguments.len > protocol.Limits.launch_arguments or
        launch.environment.len > protocol.Limits.launch_environment_entries)
    {
        return false;
    }
    if (launch.geometry) |geometry| {
        if (!terminal_geometry.isValid(geometry)) return false;
    }
    if (!terminal_profile.valid(launch.profile)) return false;
    if (launch.configuration) |configuration| {
        if (!terminal_configuration.valid(configuration)) return false;
    }
    if (launch.current_directory) |directory| if (containsNul(directory)) return false;
    for (launch.arguments) |argument| if (containsNul(argument)) return false;
    for (launch.environment, 0..) |entry, index| {
        if (entry.name.len == 0 or entry.name.len > protocol.Limits.environment_name_bytes or
            containsNul(entry.name) or
            std.mem.indexOfScalar(u8, entry.name, '=') != null or
            (entry.value != null and containsNul(entry.value.?)))
        {
            return false;
        }
        for (launch.environment[0..index]) |previous| {
            if (environmentNamesEqual(previous.name, entry.name)) return false;
        }
    }
    return true;
}

fn environmentNamesEqual(left: []const u8, right: []const u8) bool {
    if (target_builtin.os.tag == .windows) {
        var left_iterator = std.unicode.Wtf8View.initUnchecked(left).iterator();
        var right_iterator = std.unicode.Wtf8View.initUnchecked(right).iterator();
        while (true) {
            const left_codepoint = left_iterator.nextCodepoint() orelse break;
            const right_codepoint = right_iterator.nextCodepoint() orelse return false;
            if (upcaseEnvironmentCodepoint(left_codepoint) !=
                upcaseEnvironmentCodepoint(right_codepoint)) return false;
        }
        return right_iterator.nextCodepoint() == null;
    }
    return std.mem.eql(u8, left, right);
}

/// Environment names are target-operating-system identifiers: POSIX names are
/// case-sensitive, while Windows names are case-insensitive. This single
/// codepoint fold is the canonical identity rule for both duplicate rejection
/// and the CreationId launch digest, so the fingerprint is stable and
/// platform-consistent.
fn upcaseEnvironmentCodepoint(codepoint: u21) u21 {
    if (codepoint <= std.math.maxInt(u16))
        return std.os.windows.ntdll.RtlUpcaseUnicodeChar(@as(u16, @intCast(codepoint)));
    return codepoint;
}

fn containsNul(value: []const u8) bool {
    return std.mem.indexOfScalar(u8, value, 0) != null;
}

fn launchDigest(allocator: std.mem.Allocator, launch: Launch) std.mem.Allocator.Error!Digest {
    var hash = std.crypto.hash.sha2.Sha256.init(.{});
    hash.update("orchbee-terminal-launch-v4");
    hashField(&hash, launch.executable);
    hashCount(&hash, launch.arguments.len);
    for (launch.arguments) |argument| hashField(&hash, argument);
    if (launch.current_directory) |directory| {
        hash.update(&.{1});
        hashField(&hash, directory);
    } else hash.update(&.{0});
    if (launch.geometry) |geometry| {
        hash.update(&.{1});
        var dimensions: [4]u8 = undefined;
        std.mem.writeInt(u16, dimensions[0..2], geometry.columns, .little);
        std.mem.writeInt(u16, dimensions[2..4], geometry.rows, .little);
        hash.update(&dimensions);
        hashInt(&hash, u16, geometry.cell_pixel_width);
        hashInt(&hash, u16, geometry.cell_pixel_height);
    } else hash.update(&.{0});
    if (launch.configuration) |configuration| {
        hash.update(&.{1});
        hashInt(&hash, u64, @intCast(configuration.scrollback_bytes));
    } else hash.update(&.{0});

    hashCount(&hash, launch.environment.len);
    const digest_environment = try allocator.alloc(DigestEnvironmentEntry, launch.environment.len);
    @memset(digest_environment, .{ .name = &.{}, .owned = false, .value = null });
    defer {
        for (digest_environment) |entry| if (entry.owned) allocator.free(entry.name);
        allocator.free(digest_environment);
    }
    for (digest_environment, launch.environment) |*digest_entry, entry| {
        const canonical_name = try canonicalEnvironmentName(allocator, entry.name);
        digest_entry.* = .{
            .name = canonical_name.name,
            .owned = canonical_name.owned,
            .value = entry.value,
        };
    }
    std.mem.sort(DigestEnvironmentEntry, digest_environment, {}, struct {
        fn lessThan(_: void, left: DigestEnvironmentEntry, right: DigestEnvironmentEntry) bool {
            return std.mem.lessThan(u8, left.name, right.name);
        }
    }.lessThan);
    for (digest_environment) |entry| {
        hashField(&hash, entry.name);
        if (entry.value) |value| {
            hash.update(&.{1});
            hashField(&hash, value);
        } else hash.update(&.{0});
    }
    const profile = launch.profile;
    hashField(&hash, @tagName(profile.color_scheme));
    hashInt(&hash, u32, profile.default_background_rgb);
    hash.update(&.{@intFromBool(profile.default_cursor_blinking)});
    if (profile.default_cursor_rgb) |cursor| {
        hash.update(&.{1});
        hashInt(&hash, u32, cursor);
    } else hash.update(&.{0});
    hashField(&hash, @tagName(profile.default_cursor_shape));
    hashInt(&hash, u32, profile.default_foreground_rgb);
    hashInt(&hash, u16, profile.device_primary_conformance_level);
    hashCount(&hash, profile.device_primary_features.len);
    for (profile.device_primary_features) |feature| hashInt(&hash, u16, feature);
    hashInt(&hash, u16, profile.device_secondary_type);
    hashInt(&hash, u16, profile.device_secondary_firmware_version);
    hashInt(&hash, u16, profile.device_secondary_rom_cartridge);
    hashInt(&hash, u32, profile.device_tertiary_unit_id);
    hashField(&hash, profile.enquiry_response);
    hashCount(&hash, profile.palette.len);
    for (profile.palette) |color| hashInt(&hash, u32, color);
    hashField(&hash, profile.version_response);
    var digest: Digest = undefined;
    hash.final(&digest);
    return digest;
}

fn environmentOverlayDigest(
    allocator: std.mem.Allocator,
    environment_overlay: []const EnvironmentOverlayEntry,
) std.mem.Allocator.Error!Digest {
    var hash = std.crypto.hash.sha2.Sha256.init(.{});
    hash.update("orchbee-terminal-environment-overlay-v1");
    hashCount(&hash, environment_overlay.len);
    const digest_environment = try allocator.alloc(
        DigestEnvironmentEntry,
        environment_overlay.len,
    );
    @memset(digest_environment, .{ .name = &.{}, .owned = false, .value = null });
    defer {
        for (digest_environment) |entry| if (entry.owned) allocator.free(entry.name);
        allocator.free(digest_environment);
    }
    for (digest_environment, environment_overlay) |*digest_entry, entry| {
        const canonical_name = try canonicalEnvironmentName(allocator, entry.name);
        digest_entry.* = .{
            .name = canonical_name.name,
            .owned = canonical_name.owned,
            .value = entry.value,
        };
    }
    std.mem.sort(DigestEnvironmentEntry, digest_environment, {}, struct {
        fn lessThan(_: void, left: DigestEnvironmentEntry, right: DigestEnvironmentEntry) bool {
            return std.mem.lessThan(u8, left.name, right.name);
        }
    }.lessThan);
    for (digest_environment) |entry| {
        hashField(&hash, entry.name);
        hashField(&hash, entry.value.?);
    }
    var digest: Digest = undefined;
    hash.final(&digest);
    return digest;
}

const DigestEnvironmentEntry = struct {
    name: []const u8,
    owned: bool,
    value: ?[]const u8,
};

const CanonicalEnvironmentName = struct {
    name: []const u8,
    owned: bool,
};

fn canonicalEnvironmentName(
    allocator: std.mem.Allocator,
    name: []const u8,
) std.mem.Allocator.Error!CanonicalEnvironmentName {
    if (target_builtin.os.tag != .windows) return .{ .name = name, .owned = false };
    var output: std.ArrayList(u8) = .empty;
    errdefer output.deinit(allocator);
    var iterator = std.unicode.Wtf8View.initUnchecked(name).iterator();
    while (iterator.nextCodepoint()) |codepoint| {
        const normalized = upcaseEnvironmentCodepoint(codepoint);
        var encoded: [4]u8 = undefined;
        const length = std.unicode.wtf8Encode(normalized, &encoded) catch unreachable;
        try output.appendSlice(allocator, encoded[0..length]);
    }
    return .{ .name = try output.toOwnedSlice(allocator), .owned = true };
}

fn hashInt(
    hash: *std.crypto.hash.sha2.Sha256,
    comptime Integer: type,
    value: Integer,
) void {
    var bytes: [@sizeOf(Integer)]u8 = undefined;
    std.mem.writeInt(Integer, &bytes, value, .little);
    hash.update(&bytes);
}

fn hashCount(hash: *std.crypto.hash.sha2.Sha256, value: usize) void {
    var bytes: [8]u8 = undefined;
    std.mem.writeInt(u64, &bytes, value, .little);
    hash.update(&bytes);
}

fn hashField(hash: *std.crypto.hash.sha2.Sha256, value: []const u8) void {
    var length: [8]u8 = undefined;
    std.mem.writeInt(u64, &length, value.len, .little);
    hash.update(&length);
    hash.update(value);
}
