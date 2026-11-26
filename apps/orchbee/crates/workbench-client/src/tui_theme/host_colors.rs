//! Pure host-color decision. The Shell owns the one-time environment observation.
use super::ColorLevel;

/// Terminal.app took 24-bit color in macOS 26, whose first release reports a version above this;
/// macOS 15's reports 455.
const APPLE_TERMINAL_TRUECOLOR_VERSION: u32 = 460;

/// The environment facts the color level follows, read once at startup.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostColorEnvironment<'a> {
    pub no_color: Option<&'a str>,
    pub force_color: Option<&'a str>,
    pub color_term: Option<&'a str>,
    pub term: Option<&'a str>,
    pub term_program: Option<&'a str>,
    pub term_program_version: Option<&'a str>,
    /// `STY`, which GNU screen sets in its windows.
    pub screen_session: bool,
    /// `TMUX`, which tmux sets in its panes.
    pub tmux_session: bool,
}

/// Explicit color requests decide first. Otherwise the level is truecolor, which nearly every
/// terminal in use draws, and only a host known to draw fewer colors lowers it: `TERM=dumb` has
/// none, the Linux console and VT-family descriptors have 16, and Terminal.app before macOS 26 and
/// GNU screen have 256. `TERM=xterm-256color` says nothing either way: most truecolor terminals
/// set it, and SSH carries it without `COLORTERM`.
pub fn resolve_color_level(host: HostColorEnvironment<'_>) -> ColorLevel {
    if host.no_color.is_some_and(|value| !value.is_empty()) {
        return ColorLevel::Monochrome;
    }
    if let Some(value) = host.force_color {
        return match value {
            "0" => ColorLevel::Monochrome,
            "1" => ColorLevel::Ansi16,
            "2" => ColorLevel::Ansi256,
            "3" => ColorLevel::Truecolor,
            _ => ColorLevel::Ansi16,
        };
    }
    if host.color_term.is_some_and(|value| {
        value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit")
    }) {
        return ColorLevel::Truecolor;
    }
    let term = host
        .term
        .filter(|value| value.len() <= 128)
        .map(str::to_ascii_lowercase);
    let term = term.as_deref().unwrap_or_default();
    if term == "dumb" {
        return ColorLevel::Monochrome;
    }
    if term == "linux" || term == "ansi" || term.starts_with("vt") {
        return ColorLevel::Ansi16;
    }
    if host.term_program == Some("Apple_Terminal")
        && !host
            .term_program_version
            .and_then(|version| version.split('.').next()?.parse::<u32>().ok())
            .is_some_and(|major| major >= APPLE_TERMINAL_TRUECOLOR_VERSION)
    {
        return ColorLevel::Ansi256;
    }
    // tmux also sets `TERM=screen*` and passes truecolor through or reduces it itself.
    if host.screen_session || term.starts_with("screen") && !host.tmux_session {
        return ColorLevel::Ansi256;
    }
    ColorLevel::Truecolor
}
