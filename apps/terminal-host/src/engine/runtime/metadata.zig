const std = @import("std");

const contracts = @import("contracts.zig");
const failure_ops = @import("failure.zig");
const state = @import("state.zig");

pub fn markChangedLocked(self: *state.State) void {
    std.debug.assert(self.reader_commit_in_progress);
    self.metadata_change_pending = true;
}

pub fn commitRevisionLocked(self: *state.State, revision: u64) void {
    if (!self.metadata_change_pending) return;
    std.debug.assert(revision > 0);
    self.metadata_change_pending = false;
    self.metadata_revision = revision;
}

pub fn commitResetLocked(self: *state.State, revision: u64, had_metadata: bool) void {
    if (!had_metadata) return;
    std.debug.assert(revision > 0);
    self.metadata_change_pending = false;
    self.metadata_revision = revision;
}

pub fn prepareEvent(
    self: *state.State,
    allocator: std.mem.Allocator,
    connection_id: u64,
) !?contracts.MetadataEvent {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    if (self.metadata_revision == 0 or
        (self.metadata_published_connection_id == connection_id and
            self.metadata_published_revision >= self.metadata_revision))
    {
        return null;
    }

    const title = try duplicateOptional(allocator, self.terminal.getTitle());
    errdefer if (title) |value| allocator.free(value);
    const current_directory = try duplicateOptional(allocator, self.terminal.getPwd());
    errdefer if (current_directory) |value| allocator.free(value);
    return .{
        .allocator = allocator,
        .connection_id = connection_id,
        .current_directory = current_directory,
        .revision = self.metadata_revision,
        .title = title,
    };
}

pub fn commitEvent(self: *state.State, event: contracts.MetadataEvent) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    std.debug.assert(event.revision > 0);
    std.debug.assert(event.revision <= self.metadata_revision);
    if (self.metadata_published_connection_id != event.connection_id) {
        self.metadata_published_connection_id = event.connection_id;
        self.metadata_published_revision = event.revision;
        return;
    }
    if (event.revision > self.metadata_published_revision) {
        self.metadata_published_revision = event.revision;
    }
}

fn duplicateOptional(
    allocator: std.mem.Allocator,
    value: ?[]const u8,
) !?[]u8 {
    return if (value) |present| try allocator.dupe(u8, present) else null;
}
