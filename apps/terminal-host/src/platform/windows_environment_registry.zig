//! Reads bounded Windows environment snapshots from the Machine and User registry scopes.

const std = @import("std");

const EnvironmentMap = std.process.Environ.Map;
const windows = std.os.windows;

const maximum_registry_values: windows.DWORD = 4096;
const maximum_registry_name_code_units: windows.DWORD = 32 * 1024;
const maximum_registry_value_bytes: windows.DWORD = 1024 * 1024;

const error_success: windows.LSTATUS = 0;
const error_file_not_found: windows.LSTATUS = 2;
const error_path_not_found: windows.LSTATUS = 3;
const error_no_more_items: windows.LSTATUS = 259;
const key_query_value: windows.REGSAM = 0x0001;
const reg_sz: windows.DWORD = 1;
const reg_expand_sz: windows.DWORD = 2;

const machine_environment_key = std.unicode.utf8ToUtf16LeStringLiteral(
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
);
const user_environment_key = std.unicode.utf8ToUtf16LeStringLiteral("Environment");

pub const ValueKind = enum {
    literal,
    expandable,
};

pub const Value = struct {
    name: []const u8,
    value: []const u8,
    kind: ValueKind,
};

pub const Scope = union(enum) {
    missing,
    unavailable,
    values: []const Value,
};

pub const Snapshot = struct {
    machine: Scope,
    user: Scope,
};

const OwnedScope = union(enum) {
    missing,
    unavailable,
    values: []Value,

    fn deinit(self: *OwnedScope, allocator: std.mem.Allocator) void {
        switch (self.*) {
            .values => |values| {
                for (values) |value| {
                    allocator.free(value.name);
                    allocator.free(value.value);
                }
                allocator.free(values);
            },
            .missing, .unavailable => {},
        }
        self.* = .missing;
    }

    fn borrow(self: *const OwnedScope) Scope {
        return switch (self.*) {
            .missing => .missing,
            .unavailable => .unavailable,
            .values => |values| .{ .values = values },
        };
    }
};

pub const OwnedSnapshot = struct {
    machine: OwnedScope,
    user: OwnedScope,

    pub fn deinit(self: *OwnedSnapshot, allocator: std.mem.Allocator) void {
        self.user.deinit(allocator);
        self.machine.deinit(allocator);
        self.* = undefined;
    }

    pub fn borrow(self: *const OwnedSnapshot) Snapshot {
        return .{
            .machine = self.machine.borrow(),
            .user = self.user.borrow(),
        };
    }
};

pub fn readSnapshot(allocator: std.mem.Allocator) std.mem.Allocator.Error!OwnedSnapshot {
    var machine = try readScope(
        allocator,
        windows.HKEY_LOCAL_MACHINE,
        machine_environment_key,
    );
    errdefer machine.deinit(allocator);
    return .{
        .machine = machine,
        .user = try readScope(
            allocator,
            windows.HKEY_CURRENT_USER,
            user_environment_key,
        ),
    };
}

fn readScope(
    allocator: std.mem.Allocator,
    root: windows.HKEY,
    path: [*:0]const u16,
) std.mem.Allocator.Error!OwnedScope {
    var key: windows.HKEY = undefined;
    const opened = RegOpenKeyExW(root, path, 0, key_query_value, &key);
    if (opened == error_file_not_found or opened == error_path_not_found) return .missing;
    if (opened != error_success) return .unavailable;
    defer _ = RegCloseKey(key);

    var value_count: windows.DWORD = 0;
    var maximum_name_length: windows.DWORD = 0;
    var maximum_data_length: windows.DWORD = 0;
    if (RegQueryInfoKeyW(
        key,
        null,
        null,
        null,
        null,
        null,
        null,
        &value_count,
        &maximum_name_length,
        &maximum_data_length,
        null,
        null,
    ) != error_success or
        value_count > maximum_registry_values or
        maximum_name_length > maximum_registry_name_code_units or
        maximum_data_length > maximum_registry_value_bytes)
    {
        return .unavailable;
    }

    const name_capacity: usize = @as(usize, maximum_name_length) + 1;
    const data_capacity: usize = @max(@as(usize, maximum_data_length), 2);
    const name_buffer = try allocator.alloc(u16, name_capacity);
    defer allocator.free(name_buffer);
    const data_buffer = try allocator.alloc(u8, data_capacity);
    defer allocator.free(data_buffer);
    var values: std.ArrayList(Value) = .empty;
    defer {
        for (values.items) |value| {
            allocator.free(value.name);
            allocator.free(value.value);
        }
        values.deinit(allocator);
    }

    var index: windows.DWORD = 0;
    while (true) : (index += 1) {
        var name_length: windows.DWORD = @intCast(name_buffer.len);
        var data_length: windows.DWORD = @intCast(data_buffer.len);
        var value_type: windows.DWORD = 0;
        const status = RegEnumValueW(
            key,
            index,
            name_buffer.ptr,
            &name_length,
            null,
            &value_type,
            data_buffer.ptr,
            &data_length,
        );
        if (status == error_no_more_items) break;
        if (status != error_success or index >= maximum_registry_values) return .unavailable;
        if (value_type != reg_sz and value_type != reg_expand_sz) continue;
        if (name_length == 0 or
            name_length > name_buffer.len or
            data_length > data_buffer.len)
        {
            return .unavailable;
        }
        const name = try std.unicode.wtf16LeToWtf8Alloc(
            allocator,
            name_buffer[0..name_length],
        );
        errdefer allocator.free(name);
        if (!EnvironmentMap.validateKeyForPut(name)) {
            allocator.free(name);
            return .unavailable;
        }
        const value = decodeString(allocator, data_buffer[0..data_length]) catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            error.InvalidRegistryString => {
                allocator.free(name);
                return .unavailable;
            },
        };
        errdefer allocator.free(value);
        try values.append(allocator, .{
            .name = name,
            .value = value,
            .kind = if (value_type == reg_expand_sz) .expandable else .literal,
        });
    }
    const owned = try values.toOwnedSlice(allocator);
    values = .empty;
    return .{ .values = owned };
}

fn decodeString(allocator: std.mem.Allocator, bytes: []const u8) ![]u8 {
    if (bytes.len % 2 != 0) return error.InvalidRegistryString;
    const units = try allocator.alloc(u16, bytes.len / 2);
    defer allocator.free(units);
    for (units, 0..) |*unit, index| {
        unit.* = std.mem.readInt(u16, bytes[index * 2 ..][0..2], .little);
    }
    var length = units.len;
    while (length != 0 and units[length - 1] == 0) length -= 1;
    if (std.mem.indexOfScalar(u16, units[0..length], 0) != null) {
        return error.InvalidRegistryString;
    }
    return std.unicode.wtf16LeToWtf8Alloc(allocator, units[0..length]);
}

extern "advapi32" fn RegOpenKeyExW(
    root: windows.HKEY,
    sub_key: [*:0]const u16,
    options: windows.DWORD,
    access: windows.REGSAM,
    result: *windows.HKEY,
) callconv(.winapi) windows.LSTATUS;

extern "advapi32" fn RegCloseKey(key: windows.HKEY) callconv(.winapi) windows.LSTATUS;

extern "advapi32" fn RegQueryInfoKeyW(
    key: windows.HKEY,
    class: ?[*]u16,
    class_length: ?*windows.DWORD,
    reserved: ?*windows.DWORD,
    sub_key_count: ?*windows.DWORD,
    maximum_sub_key_length: ?*windows.DWORD,
    maximum_class_length: ?*windows.DWORD,
    value_count: ?*windows.DWORD,
    maximum_value_name_length: ?*windows.DWORD,
    maximum_value_data_length: ?*windows.DWORD,
    security_descriptor_length: ?*windows.DWORD,
    last_write_time: ?*windows.FILETIME,
) callconv(.winapi) windows.LSTATUS;

extern "advapi32" fn RegEnumValueW(
    key: windows.HKEY,
    index: windows.DWORD,
    value_name: [*]u16,
    value_name_length: *windows.DWORD,
    reserved: ?*windows.DWORD,
    value_type: *windows.DWORD,
    data: [*]u8,
    data_length: *windows.DWORD,
) callconv(.winapi) windows.LSTATUS;
