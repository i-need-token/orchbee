const std = @import("std");
const protocol = @import("terminal-protocol");

// Cross-language contract: search for `terminal-scrollback-configuration-contract` before
// changing either value, then update every occurrence in the same change.
pub const default_scrollback_bytes: usize = 10_000_000;
const max_scrollback_bytes = protocol.Limits.scrollback_bytes;

pub const Configuration = struct {
    scrollback_bytes: usize = default_scrollback_bytes,
};

pub fn valid(configuration: Configuration) bool {
    return configuration.scrollback_bytes <= max_scrollback_bytes;
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const scrollback_limit = max_scrollback_bytes;
} else struct {};
