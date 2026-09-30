//! Strict first-match text/numeric rules; presentation never alters the matched value.
use super::{WindowsSidebarTokenStyle, color};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
enum Condition {
    Equals(String),
    Contains(String),
    StartsWith(String),
    GreaterThan(f64),
    LessThan(f64),
}
// Admission rejects nonfinite numbers, including from canonical profiles.
impl Eq for Condition {}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rule {
    condition: Condition,
    ignore_case: bool,
    style: WindowsSidebarTokenStyle,
    hide: bool,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawRule {
    equals: Option<String>,
    contains: Option<String>,
    starts_with: Option<String>,
    gt: Option<f64>,
    lt: Option<f64>,
    ignore_case: Option<bool>,
    fg: Option<String>,
    bold: Option<bool>,
    dim: Option<bool>,
    hide: Option<bool>,
}
impl Rule {
    pub(super) fn admit(raw: RawRule) -> Result<Self, &'static str> {
        if [
            raw.equals.is_some(),
            raw.contains.is_some(),
            raw.starts_with.is_some(),
            raw.gt.is_some(),
            raw.lt.is_some(),
        ]
        .into_iter()
        .filter(|v| *v)
        .count()
            != 1
        {
            return Err("invalid-rules");
        }
        let condition = match (raw.equals, raw.contains, raw.starts_with, raw.gt, raw.lt) {
            (Some(s), ..) => Condition::Equals(s),
            (_, Some(s), ..) => Condition::Contains(s),
            (_, _, Some(s), ..) => Condition::StartsWith(s),
            (_, _, _, Some(n), _) => {
                if raw.ignore_case.is_some() {
                    return Err("invalid-rules");
                }
                Condition::GreaterThan(n)
            }
            (_, _, _, _, Some(n)) => {
                if raw.ignore_case.is_some() {
                    return Err("invalid-rules");
                }
                Condition::LessThan(n)
            }
            _ => return Err("invalid-rules"),
        };
        let rule = Self {
            condition,
            ignore_case: raw.ignore_case.unwrap_or(false),
            hide: raw.hide.unwrap_or(false),
            style: WindowsSidebarTokenStyle {
                fg: color(raw.fg)?,
                bold: raw.bold,
                dim: raw.dim,
            },
        };
        rule.validate()?;
        Ok(rule)
    }
    pub(super) fn validate(&self) -> Result<(), &'static str> {
        let valid = match &self.condition {
            Condition::Equals(s) | Condition::Contains(s) | Condition::StartsWith(s) => {
                s.len() <= 4096
            }
            Condition::GreaterThan(n) | Condition::LessThan(n) => {
                n.is_finite() && !self.ignore_case
            }
        };
        if valid { Ok(()) } else { Err("invalid-rules") }
    }
    pub(super) fn matches(&self, value: &str) -> bool {
        match &self.condition {
            Condition::Equals(expected) => {
                if self.ignore_case {
                    value.eq_ignore_ascii_case(expected)
                } else {
                    value == expected
                }
            }
            Condition::Contains(expected) => {
                if self.ignore_case {
                    expected.is_empty()
                        || value
                            .as_bytes()
                            .windows(expected.len())
                            .any(|part| part.eq_ignore_ascii_case(expected.as_bytes()))
                } else {
                    value.contains(expected)
                }
            }
            Condition::StartsWith(expected) => {
                if self.ignore_case {
                    value
                        .as_bytes()
                        .get(..expected.len())
                        .is_some_and(|part| part.eq_ignore_ascii_case(expected.as_bytes()))
                } else {
                    value.starts_with(expected)
                }
            }
            Condition::GreaterThan(n) => value
                .parse::<f64>()
                .ok()
                .is_some_and(|v| v.is_finite() && v > *n),
            Condition::LessThan(n) => value
                .parse::<f64>()
                .ok()
                .is_some_and(|v| v.is_finite() && v < *n),
        }
    }
    pub(super) fn style(&self, base: WindowsSidebarTokenStyle) -> Option<WindowsSidebarTokenStyle> {
        (!self.hide).then_some(WindowsSidebarTokenStyle {
            fg: self.style.fg.or(base.fg),
            bold: self.style.bold.or(base.bold),
            dim: self.style.dim.or(base.dim),
        })
    }
}
