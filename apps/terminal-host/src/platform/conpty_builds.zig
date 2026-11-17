//! The Host's table of known conpty builds, keyed by OpenConsole's SHA-256. An entry gives the
//! build's signal format, which a claim compares with that of the Host's own pin. Only the Host's own
//! build gives the private HPCON layout, because a Host reads only pseudoconsoles its own conpty
//! created or packed. A conpty pin whose OpenConsole is missing here, or lacks its layout, fails the
//! build, so every pin bump repeats prototype 1 and records what it found.

const std = @import("std");
const pins = @import("windows-conpty-options");

/// Byte offsets of the three handles in conpty's private `struct _PseudoConsole`.
pub const Layout = struct {
    signal: usize,
    reference: usize,
    console: usize,
};

pub const Build = struct {
    /// OpenConsole's SHA-256.
    console_sha256: [32]u8,
    /// The signal-pipe format this OpenConsole reads; a Host signals a moved console through its
    /// own conpty, so the formats must match.
    signal_format: u16,
    layout: ?Layout = null,
};

pub const known = [_]Build{
    // Microsoft.Windows.Console.ConPTY 1.25.260930003, from terminal v1.25.2733.0. Its
    // src/winconpty/winconpty.h declares `typedef struct _PseudoConsole { HANDLE hSignal; HANDLE
    // hPtyReference; HANDLE hConPtyProcess; } PseudoConsole;`, and `ConptyPackPseudoConsole` in
    // winconpty.cpp only validates the three handles and stores them in a new block: it sends
    // nothing to OpenConsole and duplicates nothing.
    .{
        .console_sha256 = digest("3d66b23d0a71bb8eed2b77edc8b9df9bf54ce6c8fb74c863a30e760997f80586"),
        .signal_format = 1,
        .layout = .{ .signal = 0, .reference = 8, .console = 16 },
    },
};

/// The entry of the Host's own pinned OpenConsole.
pub const own: Build = ownBuild();
pub const own_layout: Layout = own.layout.?;

/// The entry for an OpenConsole build, or null for an unknown one.
pub fn find(console_sha256: [32]u8) ?Build {
    for (known) |entry| {
        if (std.mem.eql(u8, &entry.console_sha256, &console_sha256)) return entry;
    }
    return null;
}

fn ownBuild() Build {
    @setEvalBranchQuota(10_000);
    const pin = digest(pins.console_sha256);
    for (known) |entry| {
        if (!std.mem.eql(u8, &entry.console_sha256, &pin)) continue;
        if (entry.layout == null) @compileError("the pinned conpty build has no HPCON layout in the table");
        return entry;
    }
    @compileError("the pinned OpenConsole is not in the table of known conpty builds");
}

fn digest(comptime text: []const u8) [32]u8 {
    @setEvalBranchQuota(10_000);
    var bytes: [32]u8 = undefined;
    _ = std.fmt.hexToBytes(&bytes, text) catch @compileError("invalid OpenConsole SHA-256");
    return bytes;
}
