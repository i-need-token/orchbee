//! Bounded registry expansion and PATH projection for Windows child environments.

const std = @import("std");

const builtin = @import("builtin");
const Limits = @import("windows_environment_limits.zig");
const registry = @import("windows_environment_registry.zig");

const EnvironmentMap = std.process.Environ.Map;
const RegistryValue = registry.Value;
const windows = std.os.windows;

const maximum_expansion_depth: usize = 128;

pub const WorkBudget = struct {
    remaining: usize = Limits.expansion_work_units,

    fn consume(self: *WorkBudget) !void {
        if (self.remaining == 0) return error.RegistryExpansionWorkLimitExceeded;
        self.remaining -= 1;
    }
};

pub const ProjectedPath = struct {
    name: []const u8,
    value: []u8,
};

pub fn projectValue(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    name: []const u8,
    value: []const u8,
    work: *WorkBudget,
) ![]u8 {
    return projectValueWithMachinePath(
        allocator,
        raw,
        kinds,
        inherited,
        name,
        value,
        null,
        work,
    );
}

pub fn projectPath(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    machine: ?RegistryValue,
    user: ?RegistryValue,
    work: *WorkBudget,
) !?ProjectedPath {
    if (machine == null and user == null) return null;
    // Preserve each scope's registry type: an expandable User value must not
    // cause a literal Machine value (or the reverse) to expand accidentally.
    const machine_value = if (machine) |entry|
        try projectPathEntry(allocator, raw, kinds, inherited, entry, null, work)
    else
        null;
    defer if (machine_value) |value| allocator.free(value);
    const user_value = if (user) |entry|
        try projectPathEntry(allocator, raw, kinds, inherited, entry, machine_value, work)
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
    return .{
        .name = if (user) |entry| entry.name else machine.?.name,
        .value = try deduplicatePath(allocator, combined.items),
    };
}

pub fn isPrivateInheritedName(name: []const u8) bool {
    const prefix = "ORCHBEE_";
    return name.len >= prefix.len and std.ascii.eqlIgnoreCase(name[0..prefix.len], prefix);
}

pub fn namesEqual(left: []const u8, right: []const u8) bool {
    if (builtin.os.tag == .windows) return windows.eqlIgnoreCaseWtf8(left, right);
    return std.ascii.eqlIgnoreCase(left, right);
}

fn projectPathEntry(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    entry: RegistryValue,
    machine_path: ?[]const u8,
    work: *WorkBudget,
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
        work,
    );
}

fn projectValueWithMachinePath(
    allocator: std.mem.Allocator,
    raw: *const EnvironmentMap,
    kinds: *const EnvironmentMap,
    inherited: *const EnvironmentMap,
    name: []const u8,
    value: []const u8,
    machine_path: ?[]const u8,
    work: *WorkBudget,
) ![]u8 {
    if (!isExpandable(kinds, name)) return allocator.dupe(u8, value);
    var active: std.ArrayList([]const u8) = .empty;
    defer active.deinit(allocator);
    return expandRegistryValue(
        allocator,
        raw,
        kinds,
        inherited,
        name,
        value,
        &active,
        machine_path,
        work,
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
    work: *WorkBudget,
) ![]u8 {
    try work.consume();
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
        try work.consume();
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
                    work,
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
    var segment_count: usize = 0;
    var segments = std.mem.splitScalar(u8, path, ';');
    while (segments.next()) |segment| {
        if (segment_count == Limits.path_segments) return error.PathSegmentLimitExceeded;
        segment_count += 1;
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
