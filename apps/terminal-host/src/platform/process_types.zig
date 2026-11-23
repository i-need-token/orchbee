const builtin = @import("builtin");

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
    /// macOS: the attached Keeper that starts the leader and parents it; null forks it here.
    keeper: if (builtin.os.tag == .macos) ?@import("process_keeper_darwin.zig").Spawn else void =
        if (builtin.os.tag == .macos) null else {},
};

/// How the Host started the root process. Clients choose a command format from
/// it instead of matching the root's command line.
pub const LaunchKind = enum {
    /// PowerShell with the Host's integration bootstrap.
    managed_powershell,
    /// Interactive cmd with the Host's prompt marks.
    cmd,
    /// zsh with the Host's managed startup.
    managed_zsh,
    /// bash with the Host's managed startup.
    managed_bash,
    other,

    /// The kind's name on the Keeper's wire, such as `managed-zsh`.
    pub fn wireName(self: LaunchKind) []const u8 {
        return switch (self) {
            inline else => |tag| comptime blk: {
                const text = @tagName(tag);
                var spelled: [text.len]u8 = undefined;
                for (text, &spelled) |byte, *out| out.* = if (byte == '_') '-' else byte;
                const final = spelled;
                break :blk &final;
            },
        };
    }
};

pub const ExitResult = union(enum) {
    exit_code: u32,
    signal: u32,
    unknown: u32,
};

/// Whether a PTY read honors a standing pause: the reader reads through one only while its
/// parser is inside a sequence.
pub const ReadMode = enum {
    /// Reports `paused` without reading while a pause stands.
    stop_at_pause,
    /// Reads while a pause stands; a pause requested during the read still cancels it.
    through_pause,
};

/// What one PTY read returned.
pub const Read = union(enum) {
    /// The byte count, never zero.
    bytes: usize,
    /// End of output, or the read was aborted.
    end,
    /// A pause cancelled the read or found it not started; nothing was read.
    paused,
};
