pub const EnvironmentDelta = struct {
    name: []const u8,
    value: ?[]const u8,
};

pub const Launch = struct {
    arguments: []const []const u8,
    cell_pixel_height: u16 = 0,
    cell_pixel_width: u16 = 0,
    columns: u16,
    current_directory: ?[]const u8,
    environment: []const EnvironmentDelta,
    executable: []const u8,
    rows: u16,
    // POSIX-only borrowed descriptor used as the child's stdin. The benchmark
    // opens and validates its fixture before launch, then the child inherits it.
    stdin_fd: ?isize = null,
};

pub const ExitResult = union(enum) {
    exit_code: u32,
    signal: u32,
    unknown: u32,
};
