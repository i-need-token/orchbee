//! Validated theme data: identity, declared polarity, provenance, canvas colors, and ANSI 0-15,
//! where every correctable slot keeps the value its gated correction replaced. Parsing rejects a
//! malformed file; whether a theme meets the legibility gates is the gate test's question.

use std::fmt;

use serde::Deserialize;

use crate::Rgb;

#[cfg(test)]
mod tests;

/// The background polarity a theme is designed for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Polarity {
    /// Dark text on a light canvas.
    Light,
    /// Light text on a dark canvas.
    Dark,
}

/// A theme color that a gated lightness correction may have replaced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaletteColor {
    value: Rgb,
    original: Option<Rgb>,
}

impl PaletteColor {
    /// The color the theme ships.
    #[must_use]
    pub const fn value(self) -> Rgb {
        self.value
    }

    /// The value a correction replaced, otherwise `None`: the source's value, or Orchbee's
    /// previous value for a slot the source does not publish. It always differs from
    /// [`Self::value`].
    #[must_use]
    pub const fn original(self) -> Option<Rgb> {
        self.original
    }
}

/// The terminal canvas colors a theme specifies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Canvas {
    /// The default background.
    pub background: Rgb,
    /// The default text color.
    pub foreground: Rgb,
    /// The block cursor fill.
    pub cursor: PaletteColor,
    /// The glyph color under a block cursor.
    pub cursor_text: Rgb,
    /// Absent when the source publishes none; the resolver then derives one.
    pub selection_background: Option<PaletteColor>,
    /// Absent when the source publishes none; selected text then keeps the foreground.
    pub selection_foreground: Option<PaletteColor>,
}

/// Where a theme's values come from, pinned for re-verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    /// The `https` URL of the source repository.
    pub url: String,
    /// The file within the repository that publishes the values.
    pub path: String,
    /// The immutable revision the values were read at.
    pub revision: String,
    /// The license identifier, as the source's license file states it.
    pub license: String,
    /// The copyright notice the license requires to be retained.
    pub copyright: String,
}

/// One complete, structurally valid theme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Theme {
    id: String,
    name: String,
    family: String,
    polarity: Polarity,
    canvas: Canvas,
    ansi: [PaletteColor; 16],
    source: Source,
}

impl Theme {
    /// The stable kebab-case identifier, unique among shipped themes.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The source's own name for the theme. Clients show a localized name keyed by [`Self::id`].
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The kebab-case family that pairs a light and a dark sibling.
    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    /// The declared polarity; the gate test confirms it against the canvas lightness.
    #[must_use]
    pub const fn polarity(&self) -> Polarity {
        self.polarity
    }

    /// The terminal canvas colors.
    #[must_use]
    pub const fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// ANSI colors 0-15 in index order: the normal set, then the bright set.
    #[must_use]
    pub const fn ansi(&self) -> &[PaletteColor; 16] {
        &self.ansi
    }

    /// The provenance the notice entry and re-verification use.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Admits one theme file.
    pub(crate) fn parse(text: &str) -> Result<Self, ThemeFileError> {
        let file: ThemeFile = toml::from_str(text).map_err(ThemeFileError::Syntax)?;
        let identifier = |field: &'static str, value: String| {
            if is_kebab_case(&value) {
                Ok(value)
            } else {
                Err(ThemeFileError::Invalid(field))
            }
        };
        let text = |field: &'static str, value: String| {
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                Err(ThemeFileError::Invalid(field))
            } else {
                Ok(value)
            }
        };
        let source = file.source;
        if !source.url.starts_with("https://") {
            return Err(ThemeFileError::Invalid("source.url"));
        }
        let ansi = file.ansi;
        Ok(Self {
            id: identifier("id", file.id)?,
            name: text("name", file.name)?,
            family: identifier("family", file.family)?,
            polarity: match file.polarity {
                PolarityFile::Light => Polarity::Light,
                PolarityFile::Dark => Polarity::Dark,
            },
            canvas: Canvas {
                background: color("canvas.background", &file.canvas.background)?,
                foreground: color("canvas.foreground", &file.canvas.foreground)?,
                cursor: file.canvas.cursor.admit("canvas.cursor")?,
                cursor_text: color("canvas.cursor_text", &file.canvas.cursor_text)?,
                selection_background: file
                    .canvas
                    .selection_background
                    .map(|value| value.admit("canvas.selection_background"))
                    .transpose()?,
                selection_foreground: file
                    .canvas
                    .selection_foreground
                    .map(|value| value.admit("canvas.selection_foreground"))
                    .transpose()?,
            },
            ansi: [
                ansi.black.admit("ansi.black")?,
                ansi.red.admit("ansi.red")?,
                ansi.green.admit("ansi.green")?,
                ansi.yellow.admit("ansi.yellow")?,
                ansi.blue.admit("ansi.blue")?,
                ansi.magenta.admit("ansi.magenta")?,
                ansi.cyan.admit("ansi.cyan")?,
                ansi.white.admit("ansi.white")?,
                ansi.bright_black.admit("ansi.bright_black")?,
                ansi.bright_red.admit("ansi.bright_red")?,
                ansi.bright_green.admit("ansi.bright_green")?,
                ansi.bright_yellow.admit("ansi.bright_yellow")?,
                ansi.bright_blue.admit("ansi.bright_blue")?,
                ansi.bright_magenta.admit("ansi.bright_magenta")?,
                ansi.bright_cyan.admit("ansi.bright_cyan")?,
                ansi.bright_white.admit("ansi.bright_white")?,
            ],
            source: Source {
                url: source.url,
                path: text("source.path", source.path)?,
                revision: text("source.revision", source.revision)?,
                license: text("source.license", source.license)?,
                copyright: text("source.copyright", source.copyright)?,
            },
        })
    }
}

/// Why a theme file was not admitted.
#[derive(Debug)]
pub(crate) enum ThemeFileError {
    /// Not TOML, a missing or unknown key, or a value of the wrong shape.
    Syntax(toml::de::Error),
    /// The named field holds a malformed color, identifier, text, URL, or a correction that
    /// changes nothing.
    Invalid(&'static str),
}

impl fmt::Display for ThemeFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => write!(formatter, "{error}"),
            Self::Invalid(field) => write!(formatter, "invalid value for `{field}`"),
        }
    }
}

fn color(field: &'static str, text: &str) -> Result<Rgb, ThemeFileError> {
    Rgb::parse_hex(text).ok_or(ThemeFileError::Invalid(field))
}

fn is_kebab_case(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    id: String,
    name: String,
    family: String,
    polarity: PolarityFile,
    source: SourceFile,
    canvas: CanvasFile,
    ansi: AnsiFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PolarityFile {
    Light,
    Dark,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    url: String,
    path: String,
    revision: String,
    license: String,
    copyright: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CanvasFile {
    background: String,
    foreground: String,
    cursor: PaletteColorFile,
    cursor_text: String,
    selection_background: Option<PaletteColorFile>,
    selection_foreground: Option<PaletteColorFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsiFile {
    black: PaletteColorFile,
    red: PaletteColorFile,
    green: PaletteColorFile,
    yellow: PaletteColorFile,
    blue: PaletteColorFile,
    magenta: PaletteColorFile,
    cyan: PaletteColorFile,
    white: PaletteColorFile,
    bright_black: PaletteColorFile,
    bright_red: PaletteColorFile,
    bright_green: PaletteColorFile,
    bright_yellow: PaletteColorFile,
    bright_blue: PaletteColorFile,
    bright_magenta: PaletteColorFile,
    bright_cyan: PaletteColorFile,
    bright_white: PaletteColorFile,
}

/// A plain `"#rrggbb"`, or `{ value, original }` when a gated correction replaced the source value.
#[derive(Deserialize)]
#[serde(untagged)]
enum PaletteColorFile {
    Plain(String),
    Corrected(CorrectedFile),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrectedFile {
    value: String,
    original: String,
}

impl PaletteColorFile {
    fn admit(self, field: &'static str) -> Result<PaletteColor, ThemeFileError> {
        match self {
            Self::Plain(value) => Ok(PaletteColor {
                value: color(field, &value)?,
                original: None,
            }),
            Self::Corrected(corrected) => {
                let value = color(field, &corrected.value)?;
                let original = color(field, &corrected.original)?;
                if value == original {
                    return Err(ThemeFileError::Invalid(field));
                }
                Ok(PaletteColor {
                    value,
                    original: Some(original),
                })
            }
        }
    }
}
