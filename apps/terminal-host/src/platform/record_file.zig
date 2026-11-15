//! Native file paths and the C writer that the fatal record and the diagnostic log share. A path
//! is fixed storage filled once, so a crashing thread can use it without allocating.
const std = @import("std");
const builtin = @import("builtin");

extern fn ob_host_record_write(
    path: *const anyopaque,
    bytes: [*]const u8,
    length: u32,
    append: c_int,
    maximum_bytes: u32,
) c_int;
extern fn ob_host_record_capture_executable(base: *usize, end: *usize) void;
extern fn ob_host_record_unix_milliseconds() u64;
extern fn ob_host_record_process_id() u32;
extern fn ob_host_record_stderr_attached() c_int;
extern fn ob_host_record_write_stderr(bytes: [*]const u8, length: u32) void;
extern fn ob_host_record_wait_milliseconds(milliseconds: u32) void;
extern fn ob_host_record_exit(code: u32) noreturn;

const Unit = if (builtin.os.tag == .windows) u16 else u8;
const path_capacity = 1024;
const separator: Unit = if (builtin.os.tag == .windows) '\\' else '/';

/// A NUL-terminated native path to one file in the Host's runtime directory.
pub const Path = struct {
    units: [path_capacity]Unit = undefined,

    /// Joins `directory` and `name`, or returns null when the result does not fit or the
    /// directory is not valid UTF-8.
    pub fn init(directory: []const u8, name: []const u8) ?Path {
        var path: Path = .{};
        var length: usize = 0;
        for ([_][]const u8{ directory, name }, 0..) |part, index| {
            if (index == 1) {
                if (length + 1 >= path_capacity) return null;
                path.units[length] = separator;
                length += 1;
            }
            if (builtin.os.tag == .windows) {
                const needed = std.unicode.calcUtf16LeLen(part) catch return null;
                if (length + needed >= path_capacity) return null;
                length += std.unicode.utf8ToUtf16Le(path.units[length..], part) catch return null;
            } else {
                if (length + part.len >= path_capacity) return null;
                @memcpy(path.units[length..][0..part.len], part);
                length += part.len;
            }
        }
        path.units[length] = 0;
        return path;
    }

    pub const Mode = union(enum) {
        replace,
        /// Appends, but starts the file over when it would grow past this many bytes.
        append_bounded: u32,
    };

    /// Best effort: whether every byte reached the file.
    pub fn write(self: *const Path, bytes: []const u8, mode: Mode) bool {
        const length = std.math.cast(u32, bytes.len) orelse return false;
        return switch (mode) {
            .replace => ob_host_record_write(&self.units, bytes.ptr, length, 0, 0) == 1,
            .append_bounded => |maximum| ob_host_record_write(&self.units, bytes.ptr, length, 1, maximum) == 1,
        };
    }
};

/// The address range this executable's image occupies; empty when it could not be found.
pub const ExecutableRange = struct {
    base: usize = 0,
    end: usize = 0,

    pub fn capture() ExecutableRange {
        var range: ExecutableRange = .{};
        ob_host_record_capture_executable(&range.base, &range.end);
        return range;
    }

    /// The offset of `address` into the image, or null for an address outside it.
    pub fn offset(self: ExecutableRange, address: usize) ?usize {
        if (address < self.base or address >= self.end) return null;
        return address - self.base;
    }
};

pub fn unixMilliseconds() u64 {
    return ob_host_record_unix_milliseconds();
}

pub fn processId() u32 {
    return ob_host_record_process_id();
}

pub fn stderrAttached() bool {
    return ob_host_record_stderr_attached() != 0;
}

pub fn writeStderr(bytes: []const u8) void {
    ob_host_record_write_stderr(bytes.ptr, std.math.cast(u32, bytes.len) orelse return);
}

pub fn waitMilliseconds(milliseconds: u32) void {
    ob_host_record_wait_milliseconds(milliseconds);
}

/// Ends the process without running exit handlers.
pub fn exitNow(code: u8) noreturn {
    ob_host_record_exit(code);
}
