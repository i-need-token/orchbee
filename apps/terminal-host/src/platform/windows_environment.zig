//! Builds each Windows terminal child's environment from current registry state.

const std = @import("std");

const builtin = @import("builtin");
const expansion = @import("windows_environment_expansion.zig");
const Limits = @import("windows_environment_limits.zig");
const process_types = @import("process_types.zig");
const registry = @import("windows_environment_registry.zig");

const EnvironmentDelta = process_types.EnvironmentDelta;
const EnvironmentMap = std.process.Environ.Map;

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
    var expansion_work: expansion.WorkBudget = .{};
    if (try expansion.projectPath(
        allocator,
        &raw_registry,
        &registry_kinds,
        &inherited_context,
        machine_path,
        user_path,
        &expansion_work,
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
        const expanded = try expansion.projectValue(
            allocator,
            &raw_registry,
            &registry_kinds,
            &inherited_context,
            name,
            raw_value,
            &expansion_work,
        );
        defer allocator.free(expanded);
        try putEnvironmentBounded(&result, name, expanded);
    }
    // Session-private values belong to the live Orchbee process, not to the
    // configurable registry snapshot. Reapply them after registry projection.
    try copyInheritedWhere(&result, inherited, expansion.isPrivateInheritedName);
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
    return isWindowsRuntimeInheritedName(name) or expansion.isPrivateInheritedName(name);
}

fn isWindowsRuntimeInheritedName(name: []const u8) bool {
    if (name.len == 3 and name[0] == '=' and
        std.ascii.isAlphabetic(name[1]) and name[2] == ':') return true;
    for (windows_runtime_inherited_names) |candidate| {
        if (expansion.namesEqual(name, candidate)) return true;
    }
    return false;
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
        if (machine and expansion.namesEqual(value.name, "USERNAME")) continue;
        if (expansion.namesEqual(value.name, "PATH")) {
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

pub const testing = if (builtin.is_test) struct {
    pub const buildEnvironment = build;
    pub const Scope = RegistryScope;
    pub const Snapshot = RegistrySnapshot;
    pub const Value = RegistryValue;
    pub const ValueKind = RegistryValueKind;
    pub const limits = Limits;
} else struct {};
