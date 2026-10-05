//! Pure host-color decision. The Shell owns the one-time environment observation.
use super::ColorLevel;

pub(crate) fn resolve_color_level(
    no_color: Option<&str>,
    force_color: Option<&str>,
    color_term: Option<&str>,
    term: Option<&str>,
) -> ColorLevel {
    if no_color.is_some_and(|value| !value.is_empty()) {
        return ColorLevel::Monochrome;
    }
    if let Some(value) = force_color {
        return match value {
            "0" => ColorLevel::Monochrome,
            "1" => ColorLevel::Ansi16,
            "2" => ColorLevel::Ansi256,
            "3" => ColorLevel::Truecolor,
            _ => ColorLevel::Ansi16,
        };
    }
    if color_term.is_some_and(|value| {
        value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit")
    }) {
        return ColorLevel::Truecolor;
    }
    if let Some(value) = term.filter(|value| value.len() <= 128) {
        if value.eq_ignore_ascii_case("dumb") {
            return ColorLevel::Monochrome;
        }
        if value.to_ascii_lowercase().contains("256color") {
            return ColorLevel::Ansi256;
        }
    }
    ColorLevel::Ansi16
}
