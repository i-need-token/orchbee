//! The files a Kitty graphics medium names: which paths the Host may touch at all, the checks made
//! before Ghostty's loader opens a file, and the removal that loader would have made of a medium
//! the Host refuses or drops before it runs.
//!
//! Ghostty deletes a temporary file once it has opened it and found its real path inside a
//! temporary directory with `tty-graphics-protocol` in its name, and unlinks a shared-memory object
//! once it has opened it, even when the read then fails. A medium the Host refuses or drops before
//! that loader runs is removed under the same conditions, so it leaves nothing behind either. The
//! Host never opens a medium that is not a regular file, or a path it does not admit, so it removes
//! neither.
const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const platform = @import("terminal-host-platform");
const fatal_record = platform.fatal_record;
const thread_sync = platform.thread_sync;
const windows_path = platform.windows_path;
const protocol = @import("terminal-protocol");

const graphics = ghostty_vt.kitty.graphics;
const Command = graphics.Command;

/// The largest file or temporary file read, the terminal's whole image budget.
pub const file_bytes_limit: u64 = protocol.Limits.terminal_graphics_resource_bytes;
/// Removal threads running at once across the Host; a removal beyond them is skipped and logged.
const removal_thread_limit = 4;
var removal_threads: std.atomic.Value(u32) = .init(0);

/// Whether the Host may touch the path a medium names. Ghostty's own path checks cover only POSIX
/// paths, so on Windows only a file on a local drive is admitted: opening a UNC path would contact
/// another machine with the user's credentials, and a device or pipe path could block the read.
pub fn pathAdmitted(path: []const u8) bool {
    return target_builtin.os.tag != .windows or windows_path.isLocalDrivePath(path);
}

pub const Refusal = enum {
    @"not a regular file",
    @"larger than 32 MiB",
    @"no bytes to read",
    @"rejected by the loader",
};

pub const Precheck = union(enum) {
    /// The byte count the loader reads.
    read: u32,
    refused: struct { refusal: Refusal, cause: ?anyerror = null },
};

/// Measures a file or temporary-file medium before Ghostty's loader opens it, so a FIFO or device is
/// refused unopened and only the checked size is read, whatever the file becomes meanwhile. A
/// refused regular temporary file is removed as the loader would have removed it.
pub fn precheck(io: std.Io, command: *const Command, temporary_directory: ?[]const u8) Precheck {
    const transmission = command.transmission().?;
    const stat = std.Io.Dir.cwd().statFile(io, command.data, .{}) catch |err|
        return .{ .refused = .{ .refusal = .@"rejected by the loader", .cause = err } };
    if (stat.kind != .file) return .{ .refused = .{ .refusal = .@"not a regular file" } };
    const available = stat.size -| transmission.offset;
    const wanted: u64 = if (transmission.size > 0) transmission.size else available;
    // Without a size Ghostty reads to the end of the file, so an empty measure bounds nothing.
    const refusal: Refusal = if (wanted == 0)
        .@"no bytes to read"
    else if (wanted > file_bytes_limit)
        .@"larger than 32 MiB"
    else
        return .{ .read = @intCast(wanted) };
    if (transmission.medium == .temporary_file) removeTemporaryFile(io, command.data, temporary_directory.?);
    return .{ .refused = .{ .refusal = refusal } };
}

/// Removes what Ghostty's loader would have removed of a medium dropped before it ran. Unlinking a
/// shared-memory object never blocks, so that happens at once and without opening it; a temporary
/// file's checks touch its file system, so they run on a thread of their own, since the caller may
/// hold the terminal lock.
pub fn removeUnread(command: *const Command, limits: graphics.LoadingImage.Limits) void {
    const transmission = command.transmission() orelse return;
    // The loader refuses these before it opens anything.
    if (transmission.format_unknown or !pathAdmitted(command.data)) return;
    if (transmission.format == .png and ghostty_vt.sys.decode_png == null) return;
    if (target_builtin.os.tag != .windows and std.mem.indexOfScalar(u8, command.data, 0) != null) return;
    switch (transmission.medium) {
        .direct, .file => {},
        .shared_memory => if (limits.shared_memory) unlinkSharedMemory(command.data),
        .temporary_file => switch (limits.temporary_file) {
            .enabled => |enabled| removeLater(command.data, enabled.directory),
            .disabled => {},
        },
    }
}

fn unlinkSharedMemory(name: []const u8) void {
    if (comptime target_builtin.abi.isAndroid() or target_builtin.os.tag == .windows or !target_builtin.link_libc)
        return;
    var buffer: [std.fs.max_path_bytes]u8 = undefined;
    const terminated = std.fmt.bufPrintZ(&buffer, "{s}", .{name}) catch return;
    _ = std.c.shm_unlink(terminated);
}

/// The loader's removal: it opens the file, takes the real path from the open handle, and deletes
/// the file when that path passes Ghostty's file blocklist, lies inside a temporary directory, and
/// contains `tty-graphics-protocol`.
fn removeTemporaryFile(io: std.Io, path: []const u8, temporary_directory: []const u8) void {
    const cwd = std.Io.Dir.cwd();
    const stat = cwd.statFile(io, path, .{}) catch return;
    if (stat.kind != .file) return;
    var real_buffer: [std.fs.max_path_bytes]u8 = undefined;
    const real = real: {
        const file = cwd.openFile(io, path, .{}) catch return;
        defer file.close(io);
        break :real real_buffer[0 .. file.realPath(io, &real_buffer) catch return];
    };
    if (!removable(io, temporary_directory, real)) return;
    cwd.deleteFile(io, real) catch |err|
        std.log.warn("failed to delete a Kitty graphics temporary file: {s}", .{@errorName(err)});
}

fn removable(io: std.Io, temporary_directory: []const u8, real: []const u8) bool {
    if (std.mem.startsWith(u8, real, "/proc/") or std.mem.startsWith(u8, real, "/sys/") or
        (std.mem.startsWith(u8, real, "/dev/") and !std.mem.startsWith(u8, real, "/dev/shm/")))
        return false;
    if (std.mem.indexOf(u8, real, "tty-graphics-protocol") == null) return false;
    if (inDirectory("/tmp", real) or inDirectory("/dev/shm", real) or inDirectory(temporary_directory, real))
        return true;
    // The temporary directory may itself be a link, as on macOS.
    var buffer: [std.fs.max_path_bytes]u8 = undefined;
    const real_directory = buffer[0 .. std.Io.Dir.cwd().realPathFile(io, temporary_directory, &buffer) catch
        return false];
    return inDirectory(real_directory, real);
}

fn inDirectory(directory: []const u8, path: []const u8) bool {
    if (directory.len == 0 or !std.mem.startsWith(u8, path, directory)) return false;
    if (path.len == directory.len or std.fs.path.isSep(directory[directory.len - 1])) return true;
    return std.fs.path.isSep(path[directory.len]);
}

/// A temporary file a dropped command named, removed on its own thread. A removal can outlive the
/// runtime that dropped the medium, so it allocates from the process's page allocator.
const Removal = struct {
    path: []u8,
    temporary_directory: []u8,

    const allocator = std.heap.page_allocator;

    fn create(path: []const u8, temporary_directory: []const u8) !*Removal {
        const removal = try allocator.create(Removal);
        errdefer allocator.destroy(removal);
        const owned_path = try allocator.dupe(u8, path);
        errdefer allocator.free(owned_path);
        removal.* = .{
            .path = owned_path,
            .temporary_directory = try allocator.dupe(u8, temporary_directory),
        };
        return removal;
    }

    fn destroy(self: *Removal) void {
        allocator.free(self.path);
        allocator.free(self.temporary_directory);
        allocator.destroy(self);
    }
};

fn removeLater(path: []const u8, temporary_directory: []const u8) void {
    if (removal_threads.fetchAdd(1, .acq_rel) >= removal_thread_limit) return skipRemoval("removals busy");
    const removal = Removal.create(path, temporary_directory) catch |err|
        return skipRemoval(@errorName(err));
    const thread = std.Thread.spawn(.{}, runRemoval, .{removal}) catch |err| {
        removal.destroy();
        return skipRemoval(@errorName(err));
    };
    thread.detach();
}

fn skipRemoval(reason: []const u8) void {
    _ = removal_threads.fetchSub(1, .release);
    std.log.warn("left a Kitty graphics temporary file in place: {s}", .{reason});
}

fn runRemoval(removal: *Removal) void {
    fatal_record.markThread(.@"terminal-media");
    defer _ = removal_threads.fetchSub(1, .release);
    defer removal.destroy();
    removeTemporaryFile(thread_sync.io(), removal.path, removal.temporary_directory);
}
