const std = @import("std");
const builtin = @import("builtin");
const thread_sync = @import("thread_sync.zig");

pub const Phase = enum { reserved, writing, ready, released };

// Cross-runtime contract: search for `editor-document-name` before changing a
// document name; the Worker removes day-old documents by it. A document is
// `scrollback-<UTC time>.txt`, the time spelled `YYYY-MM-DDTHH-MM-SSZ`, with
// `-2`, `-3`, and so on before `.txt` when that name is taken.
const name_prefix = "scrollback-";
const name_suffix = ".txt";
const time_length = "YYYY-MM-DDTHH-MM-SSZ".len;
/// The last number a taken name advances to before materialization fails.
const maximum_number: u16 = 999;
const maximum_name_length = name_prefix.len + time_length + "-999".len + name_suffix.len;
/// 9999-12-31T23:59:59Z, the last second whose year spells four digits.
const maximum_seconds: u64 = 253_402_300_799;

/// A sole owner of one generated Windows editor document. Reserving metadata
/// has no file effect; the retained deferred terminal must own this value before
/// materialize. A failed write or release keeps the complete owner for disposal.
pub const Document = struct {
    reservation: ?Reservation,

    const Reservation = struct {
        allocator: std.mem.Allocator,
        directory: std.Io.Dir,
        /// The canonical folder and its separator, the start of `path`.
        directory_length: usize,
        time: [time_length]u8,
        number: u16,
        path: []u8,
        phase: Phase,

        fn name(self: *const Reservation) []const u8 {
            return self.path[self.directory_length..];
        }

        /// Moves an unwritten reservation to the next free number. The current
        /// path stays valid when allocation fails.
        fn advance(self: *Reservation) !void {
            std.debug.assert(self.phase == .reserved);
            if (self.number == maximum_number) return error.PathAlreadyExists;
            var buffer: [maximum_name_length]u8 = undefined;
            const next = formatName(&buffer, self.time, self.number + 1);
            const path = try std.mem.concat(self.allocator, u8, &.{ self.path[0..self.directory_length], next });
            self.allocator.free(self.path);
            self.path = path;
            self.number += 1;
        }
    };

    /// Reserves a document in `directory`, the folder the Worker passes to the
    /// Host and creates before an editor launch. A Host started without one
    /// writes no editor document.
    pub fn reserve(allocator: std.mem.Allocator, directory: ?[]const u8) !Document {
        if (builtin.os.tag != .windows) return error.EditorDocumentPlatformUnsupported;
        const path = directory orelse return error.EditorDocumentDirectoryUnavailable;
        return reserveAt(allocator, path);
    }

    pub fn phase(self: *const Document) Phase {
        return if (self.reservation) |value| value.phase else .released;
    }

    /// The literal argv path. A taken name moves an unwritten document to the
    /// next number during materialize, so read it again afterwards; it then
    /// remains valid until release or handoff.
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
        const file = while (true) {
            if (value.directory.createFile(io, value.name(), .{
                .exclusive = true,
                .truncate = false,
            })) |created| break created else |err| {
                // A taken name never grants ownership of the existing file.
                if (err != error.PathAlreadyExists) return err;
                try value.advance();
            }
        };
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
            value.directory.deleteFile(io, value.name()) catch |err| switch (err) {
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
    const seconds = std.Io.Timestamp.now(thread_sync.io(), .real).toSeconds();
    return reserveAtTime(allocator, directory_path, std.math.cast(u64, seconds) orelse
        return error.EditorDocumentClockUnavailable);
}

fn reserveAtTime(
    allocator: std.mem.Allocator,
    directory_path: []const u8,
    seconds: u64,
) !Document {
    if (builtin.os.tag != .windows) return error.EditorDocumentPlatformUnsupported;
    if (!std.fs.path.isAbsolute(directory_path) or
        std.mem.indexOfScalar(u8, directory_path, 0) != null)
        return error.InvalidEditorDocumentDirectory;
    if (seconds > maximum_seconds) return error.EditorDocumentClockUnavailable;
    const io = thread_sync.io();
    const canonical = try std.Io.Dir.cwd().realPathFileAlloc(io, directory_path, allocator);
    defer allocator.free(canonical);
    const directory = try std.Io.Dir.openDirAbsolute(io, canonical, .{ .follow_symlinks = false });
    errdefer directory.close(io);
    const time = formatTime(seconds);
    var buffer: [maximum_name_length]u8 = undefined;
    const name = formatName(&buffer, time, 1);
    const path = try std.fs.path.join(allocator, &.{ canonical, name });
    return .{ .reservation = .{
        .allocator = allocator,
        .directory = directory,
        .directory_length = path.len - name.len,
        .time = time,
        .number = 1,
        .path = path,
        .phase = .reserved,
    } };
}

fn formatTime(seconds: u64) [time_length]u8 {
    std.debug.assert(seconds <= maximum_seconds);
    const epoch: std.time.epoch.EpochSeconds = .{ .secs = seconds };
    const year_day = epoch.getEpochDay().calculateYearDay();
    const month_day = year_day.calculateMonthDay();
    const day_seconds = epoch.getDaySeconds();
    var time: [time_length]u8 = undefined;
    // The bound above keeps the year to four digits, so the text fills `time` exactly.
    const written = std.fmt.bufPrint(&time, "{d:0>4}-{d:0>2}-{d:0>2}T{d:0>2}-{d:0>2}-{d:0>2}Z", .{
        year_day.year,
        month_day.month.numeric(),
        @as(u8, month_day.day_index) + 1,
        day_seconds.getHoursIntoDay(),
        day_seconds.getMinutesIntoHour(),
        day_seconds.getSecondsIntoMinute(),
    }) catch unreachable;
    std.debug.assert(written.len == time_length);
    return time;
}

fn formatName(buffer: *[maximum_name_length]u8, time: [time_length]u8, number: u16) []const u8 {
    std.debug.assert(number >= 1 and number <= maximum_number);
    return (if (number == 1)
        std.fmt.bufPrint(buffer, name_prefix ++ "{s}" ++ name_suffix, .{&time})
    else
        std.fmt.bufPrint(buffer, name_prefix ++ "{s}-{d}" ++ name_suffix, .{ &time, number })) catch unreachable;
}

pub const testing = if (builtin.is_test) struct {
    pub const reserveAt = @import("editor_document.zig").reserveAt;
    pub const reserveAtTime = @import("editor_document.zig").reserveAtTime;
} else struct {};
