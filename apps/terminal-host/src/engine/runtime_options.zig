const terminal_configuration = @import("terminal-host-model").configuration;
const terminal_profile = @import("profile.zig");

pub const Options = struct {
    cell_pixel_height: u16 = 1,
    cell_pixel_width: u16 = 1,
    profile: ?terminal_profile.Profile = null,
    scrollback_bytes: usize = terminal_configuration.default_scrollback_bytes,
};
