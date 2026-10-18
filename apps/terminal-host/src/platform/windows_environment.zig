//! Builds each Windows terminal child's environment from current registry state.

const std = @import("std");

const builtin = @import("builtin");
const process_types = @import("process_types.zig");
const registry = @import("windows_environment_registry.zig");

const EnvironmentDelta = process_types.EnvironmentDelta;
const EnvironmentMap = std.process.Environ.Map;
const windows = std.os.windows;

const Limits = registry.Limits;
const maximum_expansion_depth: usize = 128;

// Values synthesized for an interactive logon are not all stored under the
// two environment registry keys. Retain this closed Windows runtime set while
// rebuilding arbitrary configured variables from the fresh snapshot.
const windows_runtime_inherited_names = [_][]const u8{
    "ALLUSERSPROFILE",
    "APPDATA",
    "CLIENTNAME",
    "CommonProgramFiles",
    "CommonProgramFiles(Arm)",
    "CommonProgramFiles(x86)",
    "CommonProgramW6432",
    "COMPUTERNAME",
    "ComSpec",
    "HOMEDRIVE",
    "HOMEPATH",
    "HOMESHARE",
    "LOCALAPPDATA",
    "LOGONSERVER",
    "NUMBER_OF_PROCESSORS",
    "OS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_ARCHITEW6432",
    "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL",
    "PROCESSOR_REVISION",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(Arm)",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "PUBLIC",
    "SESSIONNAME",
    "SystemDrive",
    "SystemRoot",
    "TEMP",
    "TMP",
    "USERDOMAIN",
    "USERDOMAIN_ROAMINGPROFILE",
    "USERDNSDOMAIN",
    "USERNAME",
    "USERPROFILE",
    "windir",
};

pub const RegistryValueKind = registry.ValueKind;
pub const RegistryValue = registry.Value;
pub const RegistryScope = registry.Scope;
pub const RegistrySnapshot = registry.Snapshot;

/// Reads both registry scopes for this launch and applies the launch deltas last.
pub fn current(
    allocator: std.mem.Allocator,
    inherited: *const EnvironmentMap,
    delta: []const EnvironmentDelta,
) !EnvironmentMap {
    var snapshot = try registry.readSnapshot(allocator);
    defer snapshot.deinit(allocator);
    return build(allocator, inherited, snapshot.borrow(), delta);
}

/// Deterministically projects inherited, registry, and explicit launch inputs.
pub fn build(
    allocator: std.mem.Allocator,
    inherited: *const EnvironmentMap,
    snapshot: RegistrySnapshot,
    delta: []const EnvironmentDelta,
) !EnvironmentMap {
    if (builtin.os.tag != .windows) return error.UnsupportedPlatform;
    const machine = try scopeValues(snapshot.machine);
    const user = try scopeValues(snapshot.user);
    try validateRegistrySnapshot(machine, user);

    // Expansion may consult only admitted volatile/runtime values and private
    // Orchbee context. A stale arbitrary process value cannot revive a registry
    // variable that was removed after the Host started.
    var inherited_context = EnvironmentMap.init(allocator);
    defer inherited_context.deinit();
    try copyInheritedWhere(
        &inherited_context,
        inherited,
        isExpansionInheritedName,
    );

    var raw_registry = EnvironmentMap.init(allocator);
    defer raw_registry.deinit();
    var registry_kinds = EnvironmentMap.init(allocator);
    defer registry_kinds.deinit();

    var machine_path: ?RegistryValue = null;
    var user_path: ?RegistryValue = null;
    try collectRegistryScope(
        &raw_registry,
        &registry_kinds,
        machine,
        true,
        &machine_path,
    );
    try collectRegistryScope(
        &raw_registry,
        &registry_kinds,
        user,
        false,
        &user_path,
    );
    if (try projectPath(
        allocator,
        &raw_registry,
        &registry_kinds,
        &inherited_context,
        machine_path,
        user_path,
    )) |path| {
        defer allocator.free(path.value);
        try raw_registry.put(path.name, path.value);
        try registry_kinds.put(path.name, "literal");
    }

    var result = EnvironmentMap.init(allocator);
    errdefer result.deinit();
    try copyInheritedWhere(
        &result,
        inherited,
        isWindowsRuntimeInheritedName,
    );
    for (raw_registry.keys(), raw_registry.values()) |name, raw_value| {
        var active: std.ArrayList([]const u8) = .empty;
        defer active.deinit(allocator);
        const expanded = if (isExpandable(&registry_kinds, name))
            try expandRegistryValue(
                allocator,
                &raw_registry,
                &registry_kinds,
                &inherited_context,
                name,
                raw_value,
                &active,
                null,
            )
        else
            try allocator.dupe(u8, raw_value);
        defer allocator.free(expanded);
        try putEnvironmentBounded(&result, name, expanded);
    }
    // Session-private values belong to the live Orchbee process, not to the
    // configurable registry snapshot. Reapply them after registry projection.
    try copyInheritedWhere(&result, inherited, isPrivateInheritedName);
    for (delta) |entry| {
        if (!EnvironmentMap.validateKeyForPut(entry.name) or
            std.mem.indexOfScalar(u8, entry.name, 0) != null or
            (entry.value != null and std.mem.indexOfScalar(u8, entry.value.?, 0) != null))
        {
            return error.InvalidLaunchEnvironment;
        }
        if (entry.value) |value| {
            try putEnvironmentBounded(&result, entry.name, value);
        } else {
            _ = result.orderedRemove(entry.name);
            try ensureEnvironmentWithinLimits(&result);
        }
    }
    return result;
}

fn scopeValues(scope: RegistryScope) ![]const RegistryValue {
    return switch (scope) {
        .missing => &.{},
        .unavailable => error.RegistrySnapshotUnavailable,
        .too_large => error.RegistrySnapshotTooLarge,
        .values => |values| values,
    };
}

fn validateRegistrySnapshot(
    machine: []const RegistryValue,
    user: []const RegistryValue,
) !void {
    var value_count: usize = 0;
    var byte_count: usize = 0;
    for ([_][]const RegistryValue{ machine, user }) |values| {
        if (values.len > Limits.registry_values -| value_count) {
            return error.RegistrySnapshotTooLarge;
        }
        value_count += values.len;
        for (values) |value| {
            if (value.name.len == 0) continue;
            if (value.name.len > Limits.registry_name_wtf8_bytes or
                value.value.len > Limits.registry_value_bytes)
            {
                return error.RegistrySnapshotTooLarge;
            }
            const additional = value.name.len +| value.value.len +| 2;
            if (additional > Limits.registry_snapshot_bytes -| byte_count) {
                return error.RegistrySnapshotTooLarge;
            }
            byte_count += additional;
        }
    }
}

fn copyInheritedWhere(
    target: *EnvironmentMap,
    inherited: *const EnvironmentMap,
    comptime predicate: fn ([]const u8) bool,
) !void {
    for (inherited.keys(), inherited.values()) |name, value| {
        if (!predicate(name)) continue;
        try putEnvironmentBounded(target, name, value);
    }
}

fn isExpansionInheritedName(name: []const u8) bool {
    return isWindowsRuntimeInheritedName(name) or isPrivateInheritedName(name);
}

fn isWindowsRuntimeInheritedName(name: []const u8) bool {
    if (name.len == 3 and name[0] == '=' and
        std.ascii.isAlphabetic(name[1]) and name[2] == ':') return true;
    for (windows_runtime_inherited_names) |candidate| {
        if (namesEqual(name, candidate)) return true;
    }
    return false;
}

fn isPrivateInheritedName(name: []const u8) bool {
    const prefix = "ORCHBEE_";
    return name.len >= prefix.len and std.ascii.eqlIgnoreCase(name[0..prefix.len], prefix);
}

fn ensureEnvironmentWithinLimits(environment: *const EnvironmentMap) !void {
    if (environment.keys().len > Limits.projected_environment_entries) {
        return error.EnvironmentProjectionTooLarge;
    }
    // A non-empty Windows block has one final NUL after its terminated
    // entries; the empty block is the special two-NUL representation.
    var bytes: usize = if (environment.keys().len == 0) 2 else 1;
    for (environment.keys(), environment.values()) |name, value| {
        const additional = name.len +| value.len +| 2;
        if (additional > Limits.projected_environment_bytes -| bytes) {
            return error.EnvironmentProjectionTooLarge;
        }
        bytes += additional;
    }
}

fn putEnvironmentBounded(
    environment: *EnvironmentMap,
    name: []const u8,
    value: []const u8,
) !void {
    const additional = name.len +| value.len +| 2;
    if (additional > Limits.projected_environment_bytes - 1) {
        return error.EnvironmentProjectionTooLarge;
    }
    try environment.put(name, value);
    try ensureEnvironmentWithinLimits(environment);
}

fn collectRegistryScope(
    raw: *EnvironmentMap,
    kinds: *EnvironmentMap,
    values: []const RegistryValue,
    machine: bool,
    path: *?RegistryValue,
) !void {
    for (values) |value| {
        if (value.name.len == 0) continue;
        if (!EnvironmentMap.validateKeyForPut(value.name) or
            std.mem.indexOfScalar(u8, value.value, 0) != null)
        {
            return error.InvalidRegistryEnvironment;
        }
        // HKLM commonly records USERNAME=SYSTEM. That machine-wide default is
        // not the interactive identity of the child being launched.
        if (machine and namesEqual(value.name, "USERNAME")) continue;
        if (namesEqual(value.name, "PATH")) {
            path.* = value;
            continue;
        }
        try raw.put(value.name, value.value);
        try kinds.put(
            value.name,
            if (value.kind == .expandable) "expandable" else "literal",
        );
    }
}

const ProjectedPath = struct {
    name: []const u8,
    value: []u8,
};

fn projectPath(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    machine: ?RegistryValue,
    user: ?RegistryValue,
) !?ProjectedPath {
    if (machine == null and user == null) return null;
    // Preserve each scope's registry type: an expandable User value must not
    // cause a literal Machine value (or the reverse) to expand accidentally.
    const machine_value = if (machine) |entry|
        try projectPathEntry(allocator, raw, kinds, inherited, entry, null)
    else
        null;
    defer if (machine_value) |value| allocator.free(value);
    const user_value = if (user) |entry|
        try projectPathEntry(allocator, raw, kinds, inherited, entry, machine_value)
    else
        null;
    defer if (user_value) |value| allocator.free(value);

    var combined: std.ArrayList(u8) = .empty;
    defer combined.deinit(allocator);
    if (machine_value) |value| try appendPathBounded(allocator, &combined, value);
    if (machine_value != null and user_value != null) {
        try appendPathBounded(allocator, &combined, ";");
    }
    if (user_value) |value| try appendPathBounded(allocator, &combined, value);
    const deduplicated = try deduplicatePath(allocator, combined.items);
    return .{
        .name = if (user) |entry| entry.name else machine.?.name,
        .value = deduplicated,
    };
}

fn appendPathBounded(
    allocator: std.mem.Allocator,
    output: *std.ArrayList(u8),
    value: []const u8,
) !void {
    if (value.len > Limits.expanded_value_bytes -| output.items.len) {
        return error.EnvironmentProjectionTooLarge;
    }
    try output.appendSlice(allocator, value);
}

fn projectPathEntry(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    entry: RegistryValue,
    machine_path: ?[]const u8,
) ![]u8 {
    if (entry.kind == .literal) return allocator.dupe(u8, entry.value);
    var active: std.ArrayList([]const u8) = .empty;
    defer active.deinit(allocator);
    return expandRegistryValue(
        allocator,
        raw,
        kinds,
        inherited,
        entry.name,
        entry.value,
        &active,
        machine_path,
    );
}

fn isExpandable(kinds: *const EnvironmentMap, name: []const u8) bool {
    return if (kinds.get(name)) |kind| std.mem.eql(u8, kind, "expandable") else false;
}

fn expandRegistryValue(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    name: []const u8,
    value: []const u8,
    active: *std.ArrayList([]const u8),
    machine_path: ?[]const u8,
) ![]u8 {
    const active_length = active.items.len;
    try active.append(allocator, name);
    defer active.items.len = active_length;

    var expanded: std.ArrayList(u8) = .empty;
    errdefer expanded.deinit(allocator);
    var cursor: usize = 0;
    while (std.mem.indexOfScalarPos(u8, value, cursor, '%')) |open| {
        try appendBounded(allocator, &expanded, value[cursor..open]);
        const close = std.mem.indexOfScalarPos(u8, value, open + 1, '%') orelse {
            try appendBounded(allocator, &expanded, value[open..]);
            cursor = value.len;
            break;
        };
        const token = value[open + 1 .. close];
        const original = value[open .. close + 1];
        // User PATH is evaluated after Machine PATH. Its `%PATH%` token names
        // that completed earlier phase rather than itself or the stale Host
        // process PATH.
        if (namesEqual(token, "PATH") and machine_path != null) {
            try appendBounded(allocator, &expanded, machine_path.?);
        } else if (activeContains(active.items, token) or active.items.len >= maximum_expansion_depth) {
            try appendBounded(allocator, &expanded, original);
        } else if (isPrivateInheritedName(token) and inherited.get(token) != null) {
            try appendBounded(allocator, &expanded, inherited.get(token).?);
        } else if (raw.get(token)) |dependency| {
            if (isExpandable(kinds, token)) {
                const replacement = try expandRegistryValue(
                    allocator,
                    raw,
                    kinds,
                    inherited,
                    token,
                    dependency,
                    active,
                    machine_path,
                );
                defer allocator.free(replacement);
                try appendBounded(allocator, &expanded, replacement);
            } else {
                try appendBounded(allocator, &expanded, dependency);
            }
        } else if (inherited.get(token)) |dependency| {
            try appendBounded(allocator, &expanded, dependency);
        } else {
            try appendBounded(allocator, &expanded, original);
        }
        cursor = close + 1;
    }
    try appendBounded(allocator, &expanded, value[cursor..]);
    return expanded.toOwnedSlice(allocator);
}

fn appendBounded(
    allocator: std.mem.Allocator,
    output: *std.ArrayList(u8),
    value: []const u8,
) !void {
    if (value.len > Limits.expanded_value_bytes -| output.items.len) {
        return error.RegistryExpansionTooLarge;
    }
    try output.appendSlice(allocator, value);
}

fn activeContains(active: []const []const u8, name: []const u8) bool {
    for (active) |candidate| {
        if (namesEqual(candidate, name)) return true;
    }
    return false;
}

fn deduplicatePath(allocator: std.mem.Allocator, path: []const u8) ![]u8 {
    var retained: std.ArrayList([]const u8) = .empty;
    defer retained.deinit(allocator);
    var output: std.ArrayList(u8) = .empty;
    errdefer output.deinit(allocator);
    var segments = std.mem.splitScalar(u8, path, ';');
    while (segments.next()) |segment| {
        const duplicate = for (retained.items) |prior| {
            if (namesEqual(prior, segment)) break true;
        } else false;
        if (duplicate) continue;
        if (retained.items.len != 0) try output.append(allocator, ';');
        try output.appendSlice(allocator, segment);
        try retained.append(allocator, segment);
    }
    return output.toOwnedSlice(allocator);
}

fn namesEqual(left: []const u8, right: []const u8) bool {
    if (builtin.os.tag == .windows) return windows.eqlIgnoreCaseWtf8(left, right);
    return std.ascii.eqlIgnoreCase(left, right);
}

pub const testing = if (builtin.is_test) struct {
    pub const buildEnvironment = build;
    pub const Scope = RegistryScope;
    pub const Snapshot = RegistrySnapshot;
    pub const Value = RegistryValue;
    pub const ValueKind = RegistryValueKind;
    pub const limits = Limits;
} else struct {};
