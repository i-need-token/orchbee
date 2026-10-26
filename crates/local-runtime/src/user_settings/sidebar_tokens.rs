//! Common token style and raw-value rules for both Windows sidebar consumers.
mod rules;
use rules::{RawRule, Rule};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarTokenStyle {
    pub fg: Option<[u8; 3]>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Token {
    pub(super) name: String,
    style: SidebarTokenStyle,
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum RawToken {
    Plain(String),
    Styled(RawStyled),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawStyled {
    token: String,
    fg: Option<String>,
    bold: Option<bool>,
    dim: Option<bool>,
    #[serde(default)]
    rules: Vec<RawRule>,
}

pub(super) fn color(value: Option<String>) -> Result<Option<[u8; 3]>, &'static str> {
    value
        .map(|value| {
            let hex = value.strip_prefix('#').ok_or("invalid-color")?;
            if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("invalid-color");
            }
            let hex = if hex.len() == 3 {
                hex.chars().flat_map(|c| [c, c]).collect::<String>()
            } else {
                hex.into()
            };
            let component = |offset| {
                u8::from_str_radix(&hex[offset..offset + 2], 16).map_err(|_| "invalid-color")
            };
            Ok([component(0)?, component(2)?, component(4)?])
        })
        .transpose()
}
impl Token {
    pub(super) fn admit(raw: RawToken) -> Result<Self, &'static str> {
        let raw = match raw {
            RawToken::Plain(token) => RawStyled {
                token,
                fg: None,
                bold: None,
                dim: None,
                rules: Vec::new(),
            },
            RawToken::Styled(raw) => raw,
        };
        let token = Self {
            name: raw.token,
            style: SidebarTokenStyle {
                fg: color(raw.fg)?,
                bold: raw.bold,
                dim: raw.dim,
            },
            rules: raw
                .rules
                .into_iter()
                .map(Rule::admit)
                .collect::<Result<_, _>>()?,
        };
        token.validate(false)?;
        Ok(token)
    }
    pub(super) fn validate(&self, forbid_rules: bool) -> Result<(), &'static str> {
        if self.rules.len() > 16 || (forbid_rules && !self.rules.is_empty()) {
            return Err("invalid-rules");
        }
        for rule in &self.rules {
            rule.validate()?;
        }
        Ok(())
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Rules inspect the original value, before display sanitization. Icons have no text value.
    pub fn style_for_value(&self, value: Option<&str>) -> Option<SidebarTokenStyle> {
        if let Some(value) = value {
            for rule in &self.rules {
                if rule.matches(value) {
                    return rule.style(self.style);
                }
            }
        }
        Some(self.style)
    }
}
pub(super) fn custom_key(name: &str) -> Result<&str, &'static str> {
    let key = name.strip_prefix('$').ok_or("invalid-token")?;
    if key.is_empty()
        || key.len() > 32
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err("invalid-token");
    }
    Ok(key)
}
