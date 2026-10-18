//! Builds each Windows terminal child's environment from current registry state.

const std = @import("std");

const builtin = @import("builtin");
const process_types = @import("process_types.zig");
const registry = @import("windows_environment_registry.zig");

const EnvironmentDelta = process_types.EnvironmentDelta;
const EnvironmentMap = std.process.Environ.Map;
const windows = std.os.windows;

const maximum_expanded_value_bytes: usize = 1024 * 1024;
const maximum_expansion_depth: usize = 128;

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
        inherited,
        machine_path,
        user_path,
    )) |path| {
        defer allocator.free(path.value);
        try raw_registry.put(path.name, path.value);
        try registry_kinds.put(path.name, "literal");
    }

    var result = try inherited.clone(allocator);
    errdefer result.deinit();
    for (raw_registry.keys(), raw_registry.values()) |name, raw_value| {
        var active: std.ArrayList([]const u8) = .empty;
        defer active.deinit(allocator);
        const expanded = if (isExpandable(&registry_kinds, name))
            try expandRegistryValue(
                allocator,
                &raw_registry,
                &registry_kinds,
                inherited,
                name,
                raw_value,
                &active,
            )
        else
            try allocator.dupe(u8, raw_value);
        defer allocator.free(expanded);
        try result.put(name, expanded);
    }
    for (delta) |entry| {
        if (!EnvironmentMap.validateKeyForPut(entry.name) or
            std.mem.indexOfScalar(u8, entry.name, 0) != null or
            (entry.value != null and std.mem.indexOfScalar(u8, entry.value.?, 0) != null))
        {
            return error.InvalidLaunchEnvironment;
        }
        if (entry.value) |value| {
            try result.put(entry.name, value);
        } else {
            _ = result.orderedRemove(entry.name);
        }
    }
    return result;
}

fn scopeValues(scope: RegistryScope) ![]const RegistryValue {
    return switch (scope) {
        .missing => &.{},
        .unavailable => error.RegistrySnapshotUnavailable,
        .values => |values| values,
    };
}

fn collectRegistryScope(
    raw: *EnvironmentMap,
    kinds: *EnvironmentMap,
    values: []const RegistryValue,
    machine: bool,
    path: *?RegistryValue,
) !void {
    for (values) |value| {
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
        try projectPathEntry(allocator, raw, kinds, inherited, entry)
    else
        null;
    defer if (machine_value) |value| allocator.free(value);
    const user_value = if (user) |entry|
        try projectPathEntry(allocator, raw, kinds, inherited, entry)
    else
        null;
    defer if (user_value) |value| allocator.free(value);

    var combined: std.ArrayList(u8) = .empty;
    defer combined.deinit(allocator);
    if (machine_value) |value| try combined.appendSlice(allocator, value);
    if (machine_value != null and user_value != null) try combined.append(allocator, ';');
    if (user_value) |value| try combined.appendSlice(allocator, value);
    const deduplicated = try deduplicatePath(allocator, combined.items);
    return .{
        .name = if (user) |entry| entry.name else machine.?.name,
        .value = deduplicated,
    };
}

fn projectPathEntry(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    entry: RegistryValue,
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
        if (activeContains(active.items, token) or active.items.len >= maximum_expansion_depth) {
            try appendBounded(allocator, &expanded, original);
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
    if (value.len > maximum_expanded_value_bytes -| output.items.len) {
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
} else struct {};
