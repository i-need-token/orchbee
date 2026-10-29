const std = @import("std");

pub const maximum_processes = 256;
pub const maximum_text_code_units = 4096;
pub const Error = error{ OutOfMemory, ProcessSampleUnavailable, ProcessSampleUnsupported, ProcessSampleLimit };

/// Whether a member shares the live Terminal root's console host. Another console and a
/// GUI-subsystem program (which reports its creator, never a console host) are `detached`;
/// a console program started without any console is `none`.
pub const Console = enum {
    attached,
    detached,
    none,
    unknown,
};

/// Owned diagnostic facts. No field grants foreground or input authority. A row
/// without an image is opaque: the member denied limited query rights, so only
/// its identity and parent come from the system process list.
pub const Process = struct {
    pid: u32,
    parent_pid: ?u32,
    creation_time: u64,
    console: Console,
    image: ?[]const u8,
    command_line: ?[]const u8,
    current_directory: ?[]const u8,

    pub fn deinit(self: Process, allocator: std.mem.Allocator) void {
        if (self.image) |value| allocator.free(value);
        if (self.command_line) |value| allocator.free(value);
        if (self.current_directory) |value| allocator.free(value);
    }
};

pub const Sample = struct {
    allocator: std.mem.Allocator,
    root_pid: u32,
    processes: []Process,

    pub fn deinit(self: *Sample) void {
        for (self.processes) |process| process.deinit(self.allocator);
        self.allocator.free(self.processes);
        self.* = undefined;
    }
};
