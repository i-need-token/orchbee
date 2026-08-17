pub const TerminalCloseActivity = enum(u8) {
    unknown = 0,
    idle = 1,
    active = 2,
};

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
