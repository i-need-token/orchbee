const terminal_configuration = @import("terminal-host-model").configuration;
const terminal_profile = @import("profile.zig");

pub const Options = struct {
    // A paired-zero cell-pixel dimension means pixel metrics are unknown. It is
    // the default because no measurement exists until a caller supplies one;
    // it must never be rewritten to a synthetic positive value.
    cell_pixel_height: u16 = 0,
    cell_pixel_width: u16 = 0,
    profile: ?terminal_profile.Profile = null,
    scrollback_bytes: usize = terminal_configuration.default_scrollback_bytes,
};
