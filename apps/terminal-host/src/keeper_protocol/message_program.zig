//! The background program types of `keeper`, under `programs`: a program's handles on Windows, the
//! facts every program holding records, and the hold and inspection that carry them.

const message = @import("message.zig");

const Process = message.Process;
const RandomId = message.RandomId;

/// A background program's handles in the process that holds them: its Job and process, and the
/// read end of each stream the Host captures.
pub const ProgramHandles = struct {
    job: u32,
    process: u32,
    stdout: ?u32 = null,
    stderr: ?u32 = null,
};

/// At most this many bytes in a program's label.
pub const maximum_label_bytes = 2048;

/// What a program holding records: the program's process, the id its Worker gave it, and the
/// Worker's label, 1 to 2,048 characters of the base64 alphabet that the Keeper returns unread.
pub const ProgramFacts = struct {
    process: Process,
    program_id: [16]u8,
    label: []const u8,

    pub fn valid(self: ProgramFacts) bool {
        return !message.isZero(&self.program_id) and validLabel(self.label);
    }
};

pub fn validLabel(label: []const u8) bool {
    if (label.len == 0 or label.len > maximum_label_bytes) return false;
    for (label) |byte| switch (byte) {
        'A'...'Z', 'a'...'z', '0'...'9', '+', '/', '=' => {},
        else => return false,
    };
    return true;
}

pub const HoldProgram = struct {
    holding: RandomId,
    handles: ProgramHandles,
    facts: ProgramFacts,
};

pub const ProgramInspection = struct {
    handles: ProgramHandles,
    facts: ProgramFacts,
};
