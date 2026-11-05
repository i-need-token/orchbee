pub const TerminalProgressState = enum(u8) {
    remove = 0,
    set = 1,
    @"error" = 2,
    indeterminate = 3,
    pause = 4,
};

pub const TerminalProgress = struct {
    state: TerminalProgressState = .remove,
    value: ?u8 = null,
};

pub const TerminalCommandCompletion = struct {
    sequence: u64,
    duration_milliseconds: u64,
    completed_at_milliseconds: u64,
    exit_code: u8,
};

/// Optional shell observations contain identities and positions, never command text.
pub const TerminalCommandObservation = struct {
    output_epoch: u64,
    ready: bool = false,
    command: ?Command = null,

    pub const Command = struct {
        id: [16]u8,
        completed: bool = false,
        exit_code: i32 = 0,
        start_row: u64,
        start_column: u16,
        end_row: u64 = 0,
        end_column: u16 = 0,
        generation: u64,
        output_epoch: u64,
        /// Real-time Unix nanoseconds when the Host read the start marker; zero when unknown.
        started_at_nanoseconds: u64 = 0,
    };
};
