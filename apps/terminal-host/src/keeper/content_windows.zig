//! What a Windows holding holds: the Keeper's duplicates of a terminal's seven handles or of a
//! background program's, with the facts the hold recorded.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const handles = @import("handles_windows.zig");

const message = keeper_protocol.message;
const Process = message.Process;
const launch_capacity = handshake.message.maximum_name_bytes;

pub const StoredFacts = struct {
    terminal: message.Terminal,
    program: Process,
    console: Process,
    console_build: [32]u8,
    signal_format: u16,
    launch: [launch_capacity]u8,
    launch_length: u8,

    pub fn store(facts: message.Facts) ?StoredFacts {
        if (facts.launch.len > launch_capacity) return null;
        var stored: StoredFacts = .{
            .terminal = facts.terminal,
            .program = facts.program,
            .console = facts.console,
            .console_build = facts.console_build,
            .signal_format = facts.signal_format,
            .launch = undefined,
            .launch_length = @intCast(facts.launch.len),
        };
        @memcpy(stored.launch[0..facts.launch.len], facts.launch);
        return stored;
    }

    pub fn view(self: *const StoredFacts, arena: std.mem.Allocator) error{OutOfMemory}!message.Facts {
        return .{
            .terminal = self.terminal,
            .program = self.program,
            .console = self.console,
            .console_build = self.console_build,
            .signal_format = self.signal_format,
            .launch = try arena.dupe(u8, self.launch[0..self.launch_length]),
        };
    }
};

pub const StoredProgramFacts = struct {
    process: Process,
    program_id: [16]u8,
    label: [message.maximum_label_bytes]u8,
    label_length: u16,

    /// Facts the decoder checked, so the label fits.
    pub fn store(facts: message.ProgramFacts) StoredProgramFacts {
        var stored: StoredProgramFacts = .{
            .process = facts.process,
            .program_id = facts.program_id,
            .label = undefined,
            .label_length = @intCast(facts.label.len),
        };
        @memcpy(stored.label[0..facts.label.len], facts.label);
        return stored;
    }

    pub fn view(self: *const StoredProgramFacts, arena: std.mem.Allocator) error{OutOfMemory}!message.ProgramFacts {
        return .{
            .process = self.process,
            .program_id = self.program_id,
            .label = try arena.dupe(u8, self.label[0..self.label_length]),
        };
    }
};

/// A terminal, or a background program a Host runs without one.
pub const Content = union(enum) {
    terminal: struct { duplicates: handles.Duplicates, facts: StoredFacts },
    program: struct { duplicates: handles.ProgramDuplicates, facts: StoredProgramFacts },

    pub fn close(self: *const Content) void {
        switch (self.*) {
            .terminal => |*terminal| handles.closeAll(&terminal.duplicates),
            .program => |*program| program.duplicates.close(),
        }
    }

    pub fn view(self: *const Content, arena: std.mem.Allocator) error{OutOfMemory}!message.HoldingFacts {
        return switch (self.*) {
            .terminal => |*terminal| .{ .terminal = try terminal.facts.view(arena) },
            .program => |*program| .{ .program = try program.facts.view(arena) },
        };
    }

    /// Whether a Worker's `status` lists it: a program holding only to a Worker that negotiated
    /// `programs`, since another reads every entry as a terminal's.
    pub fn listedTo(self: *const Content, worker: anytype) bool {
        return self.* != .program or worker.programs;
    }
};

/// What the table keeps beyond its holdings on Windows: nothing.
pub const TableExtra = struct {
    pub fn reserves(_: *const TableExtra, _: Process) bool {
        return false;
    }
};

/// The descriptors a request carries on macOS; none on Windows.
pub const Descriptors = struct {
    pub fn release(_: *Descriptors) void {}
};
