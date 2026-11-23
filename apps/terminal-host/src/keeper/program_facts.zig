//! A program holding's facts as the Keeper's table stores them, on Windows and macOS alike.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");

const message = keeper_protocol.message;
const Process = message.Process;

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
