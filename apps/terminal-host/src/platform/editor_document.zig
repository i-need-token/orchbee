const std = @import("std");
const builtin = @import("builtin");
const temp_directory = @import("temp_directory.zig");
const thread_sync = @import("thread_sync.zig");

pub const Phase = enum { reserved, writing, ready, released };

/// A sole owner of one generated Windows editor document. Reserving metadata
/// has no file effect; the retained deferred terminal must own this value before
/// materialize. A failed write or release keeps the complete owner for disposal.
pub const Document = struct {
    reservation: ?Reservation,

    const Reservation = struct {
        allocator: std.mem.Allocator,
        directory: std.Io.Dir,
        name: [55]u8,
        path: []u8,
        phase: Phase,
    };

    pub fn reserve(allocator: std.mem.Allocator) !Document {
        if (builtin.os.tag != .windows) return error.EditorDocumentPlatformUnsupported;
        const directory = try temp_directory.resolveOwned(allocator);
        defer allocator.free(directory);
        return reserveAt(allocator, directory);
    }

    pub fn phase(self: *const Document) Phase {
        return if (self.reservation) |value| value.phase else .released;
    }

    /// The borrowed literal argv path remains valid until release or handoff.
    pub fn absolutePath(self: *const Document) ?[]const u8 {
        return if (self.reservation) |value| value.path else null;
    }

    pub fn materialize(self: *Document, text: []const u8) !void {
        const value = if (self.reservation) |*reservation| reservation else return error.EditorDocumentReleased;
        switch (value.phase) {
            .ready => return,
            .writing => return error.EditorDocumentWriteIncomplete,
            .released => unreachable,
            .reserved => {},
        }
        if (!std.unicode.utf8ValidateSlice(text)) return error.InvalidEditorDocumentText;
        const io = thread_sync.io();
        const file = try value.directory.createFile(io, &value.name, .{
            .exclusive = true,
            .truncate = false,
        });
        // Only a successful exclusive create grants file ownership. A later
        // write/sync failure cannot revert it to an unowned reservation.
        value.phase = .writing;
        defer file.close(io);
        try file.writeStreamingAll(io, text);
        try file.sync(io);
        value.phase = .ready;
    }

    pub fn release(self: *Document) !void {
        const value = if (self.reservation) |*reservation| reservation else return;
        const io = thread_sync.io();
        if (value.phase != .reserved) {
            value.directory.deleteFile(io, &value.name) catch |err| switch (err) {
                error.FileNotFound => {},
                else => return err,
            };
        }
        value.directory.close(io);
        value.allocator.free(value.path);
        self.reservation = null;
    }

    pub fn take(self: *Document) Document {
        const result = self.*;
        self.reservation = null;
        return result;
    }
};

fn reserveAt(allocator: std.mem.Allocator, directory_path: []const u8) !Document {
    var identity: [16]u8 = undefined;
    thread_sync.io().random(&identity);
    return reserveAtIdentity(allocator, directory_path, identity);
}

fn reserveAtIdentity(
    allocator: std.mem.Allocator,
    directory_path: []const u8,
    identity: [16]u8,
) !Document {
    if (builtin.os.tag != .windows) return error.EditorDocumentPlatformUnsupported;
    if (!std.fs.path.isAbsolute(directory_path) or
        std.mem.indexOfScalar(u8, directory_path, 0) != null)
        return error.InvalidEditorDocumentDirectory;
    const io = thread_sync.io();
    const canonical = try std.Io.Dir.cwd().realPathFileAlloc(io, directory_path, allocator);
    defer allocator.free(canonical);
    const directory = try std.Io.Dir.openDirAbsolute(io, canonical, .{ .follow_symlinks = false });
    errdefer directory.close(io);
    const hex = std.fmt.bytesToHex(identity, .lower);
    const name = "orchbee-scrollback-".* ++ hex ++ ".txt".*;
    const path = try std.fs.path.join(allocator, &.{ canonical, &name });
    return .{ .reservation = .{
        .allocator = allocator,
        .directory = directory,
        .name = name,
        .path = path,
        .phase = .reserved,
    } };
}

pub const testing = if (builtin.is_test) struct {
    pub const reserveAt = @import("editor_document.zig").reserveAt;
    pub const reserveAtIdentity = @import("editor_document.zig").reserveAtIdentity;
} else struct {};
