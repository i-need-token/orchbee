//! Terminal settings that only the desktop app applies.
use super::{
    Fields,
    registry::{SettingDefault, find_setting},
};

/// Which macOS Option keys the desktop terminal sends as Alt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MacosOptionAsAlt {
    #[default]
    Off,
    Left,
    Right,
    Both,
}

impl MacosOptionAsAlt {
    /// The value as `config.toml` spells it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Left => "left",
            Self::Right => "right",
            Self::Both => "both",
        }
    }
}

/// Desktop terminal text and input settings. The desktop app does not read them from here yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopTerminalSettings {
    /// A CSS font-family list; `None` keeps the built-in terminal font stack.
    pub font_family: Option<String>,
    pub font_size: u8,
    pub font_weight: u16,
    pub font_weight_bold: u16,
    pub font_ligatures: bool,
    pub padding: u8,
    /// Background opacity in percent.
    pub background_opacity: u8,
    pub macos_option_as_alt: MacosOptionAsAlt,
}

/// The registered default of an integer key; the registry is the one source of these values.
fn registered_integer<T: TryFrom<i64> + Default>(path: &str) -> T {
    let segments: Vec<&str> = path.split('.').collect();
    match find_setting(&segments).map(|setting| setting.default) {
        Some(SettingDefault::Integer(value)) => value.try_into().unwrap_or_default(),
        _ => T::default(),
    }
}

/// Store an admitted integer; the registry's range already fits `target`.
fn integer_into<T: TryFrom<i64>>(fields: &mut Fields<'_>, path: &str, target: &mut T) {
    if let Some(value) = fields.integer(path).and_then(|value| value.try_into().ok()) {
        *target = value;
    }
}

fn registered_boolean(path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    matches!(
        find_setting(&segments).map(|setting| setting.default),
        Some(SettingDefault::Boolean(true))
    )
}

impl Default for DesktopTerminalSettings {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: registered_integer("terminal.font_size"),
            font_weight: registered_integer("terminal.font_weight"),
            font_weight_bold: registered_integer("terminal.font_weight_bold"),
            font_ligatures: registered_boolean("terminal.font_ligatures"),
            padding: registered_integer("terminal.padding"),
            background_opacity: registered_integer("terminal.background_opacity"),
            macos_option_as_alt: MacosOptionAsAlt::Off,
        }
    }
}

impl DesktopTerminalSettings {
    pub(super) fn parse(fields: &mut Fields<'_>) -> Self {
        let mut settings = Self::default();
        if let Some(value) = fields.text("terminal.font_family") {
            if value.trim().is_empty() {
                fields.push("terminal.font_family", "invalid-value");
            } else {
                settings.font_family = Some(value.trim().to_owned());
            }
        }
        integer_into(fields, "terminal.font_size", &mut settings.font_size);
        integer_into(fields, "terminal.padding", &mut settings.padding);
        integer_into(
            fields,
            "terminal.background_opacity",
            &mut settings.background_opacity,
        );
        integer_into(fields, "terminal.font_weight", &mut settings.font_weight);
        integer_into(
            fields,
            "terminal.font_weight_bold",
            &mut settings.font_weight_bold,
        );
        if let Some(value) = fields.boolean("terminal.font_ligatures") {
            settings.font_ligatures = value;
        }
        settings.macos_option_as_alt = match fields.text("terminal.macos_option_as_alt").as_deref()
        {
            Some("left") => MacosOptionAsAlt::Left,
            Some("right") => MacosOptionAsAlt::Right,
            Some("both") => MacosOptionAsAlt::Both,
            _ => MacosOptionAsAlt::Off,
        };
        settings
    }
}
