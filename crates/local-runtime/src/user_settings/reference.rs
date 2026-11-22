//! The offline settings reference: every registered key as comment lines, derived from the registry.
use super::registry::{
    SETTING_KEYS, SETTING_PLATFORMS, SettingDefault, SettingEffect, SettingKey, SettingValue,
};
use std::fmt::Write as _;

/// One action and its current Client-registry defaults, supplied by the application owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultActionBindings {
    pub action: &'static str,
    /// Empty means the action has no default key binding.
    pub bindings: Vec<String>,
}

const HEADER: &str = "\
# Orchbee settings reference. Every line is a comment, so a copy of this file changes nothing.
# To change a setting, uncomment its table header and its line and edit the value; deleting the
# line restores the default. Comments are fixed English so the reference reads the same in every
# locale.
";

/// Return the settings reference without Client action rows.
pub fn default_user_settings_toml() -> String {
    default_user_settings_toml_with_actions(&[])
}

/// Render every registered key, and each Client action's current keys, as comment lines.
pub fn default_user_settings_toml_with_actions(actions: &[DefaultActionBindings]) -> String {
    let mut output = String::from(HEADER);
    let mut current: Option<String> = None;
    for setting in SETTING_KEYS.iter().filter(|setting| setting.reference) {
        let placement = Placement::of(setting);
        if current.as_deref() != Some(placement.header.as_str()) {
            output.push('\n');
            writeln!(output, "# {}", placement.header).expect("writing to a String cannot fail");
            current = Some(placement.header.clone());
        }
        push_wrapped_comment(&mut output, "", &purpose(setting));
        push_wrapped_comment(
            &mut output,
            "  ",
            &format!(
                "Default: {}. Values: {}.",
                default_text(setting),
                values(setting)
            ),
        );
        match placement.line {
            Line::Value(leaf) => {
                if let Some(literal) = literal(setting.default) {
                    writeln!(output, "# {leaf} = {literal}")
                        .expect("writing to a String cannot fail");
                }
            }
            Line::Whole => {
                if let SettingDefault::Unset { example, .. } = setting.default {
                    writeln!(output, "# {example}").expect("writing to a String cannot fail");
                }
            }
            Line::Platforms => {
                if let Some(literal) = literal(setting.default) {
                    for platform in SETTING_PLATFORMS {
                        writeln!(output, "# {platform} = {literal}")
                            .expect("writing to a String cannot fail");
                    }
                }
            }
            Line::Actions => push_action_lines(&mut output, actions),
            Line::None => {}
        }
    }
    output
}

/// Where one key appears: its table header and how its own line is written.
struct Placement {
    header: String,
    line: Line,
}

enum Line {
    /// `leaf = default`.
    Value(&'static str),
    /// The default's example is already a whole `name = value` line.
    Whole,
    /// One line per platform.
    Platforms,
    /// One line per registered Client action.
    Actions,
    /// The key is an array of tables; its header stands for it.
    None,
}

impl Placement {
    fn of(setting: &SettingKey) -> Self {
        let segments: Vec<&'static str> = setting.path.split('.').collect();
        let (leaf, parents) = segments
            .split_last()
            .expect("registered paths are never empty");
        let array = SETTING_KEYS
            .iter()
            .any(|other| other.path.starts_with(&format!("{}[].", setting.path)));
        if array {
            return Self {
                header: format!("[[{}]]", example_table(&segments)),
                line: Line::None,
            };
        }
        let element = parents.iter().any(|segment| segment.ends_with("[]"));
        let table = example_table(parents);
        let header = if element {
            format!("[[{table}]]")
        } else {
            format!("[{table}]")
        };
        let line = match *leaf {
            "*" => Line::Whole,
            "<platform>" => Line::Platforms,
            "<action>" => Line::Actions,
            leaf => Line::Value(leaf),
        };
        Self { header, line }
    }
}

/// A table path with placeholders replaced by the example the reference shows.
fn example_table(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| {
            segment
                .trim_end_matches("[]")
                .replace("<platform>", SETTING_PLATFORMS[0])
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn purpose(setting: &SettingKey) -> String {
    let mut text = setting.description.to_owned();
    if setting.path.contains("<platform>") {
        text.push_str(" Each platform has its own table: windows, macos or linux.");
    }
    match setting.effect {
        SettingEffect::Immediate => {}
        SettingEffect::NewTerminals => text.push_str(" Applies to terminals opened afterwards."),
        SettingEffect::Restart => text.push_str(" Applies after a restart."),
    }
    text
}

fn default_text(setting: &SettingKey) -> String {
    match setting.default {
        SettingDefault::Unset { meaning, example } if !example.is_empty() => {
            format!("unset ({meaning}); the line below is an example")
        }
        SettingDefault::Unset { meaning, .. } => format!("unset ({meaning})"),
        default => literal(default).unwrap_or_default(),
    }
}

fn literal(default: SettingDefault) -> Option<String> {
    match default {
        SettingDefault::Boolean(value) => Some(value.to_string()),
        SettingDefault::Integer(value) => Some(value.to_string()),
        SettingDefault::Text(value) => Some(toml_edit::Value::from(value).to_string()),
        SettingDefault::Toml(value) => Some(value.to_owned()),
        SettingDefault::Unset { example, .. } if !example.is_empty() => Some(example.to_owned()),
        SettingDefault::Unset { .. } => None,
    }
}

fn values(setting: &SettingKey) -> String {
    match setting.value {
        SettingValue::Boolean => "true or false".into(),
        SettingValue::Integer {
            minimum,
            maximum,
            step,
        } if step > 1 => format!("an integer from {minimum} through {maximum} in steps of {step}"),
        SettingValue::Integer {
            minimum, maximum, ..
        } => format!("an integer from {minimum} through {maximum}"),
        SettingValue::Choice(choices) => choices
            .iter()
            .map(|choice| format!("\"{choice}\""))
            .collect::<Vec<_>>()
            .join(", "),
        SettingValue::Text {
            maximum_bytes,
            shape,
        } => format!("{shape}, up to {maximum_bytes} bytes"),
        SettingValue::Color => "\"#rrggbb\" in lowercase hexadecimal".into(),
        SettingValue::Theme => {
            "a color-theme id of lowercase letters, digits and hyphens, up to 64 characters".into()
        }
        SettingValue::Path => {
            "an absolute folder, a folder under ~, or a folder relative to this file".into()
        }
        SettingValue::Structured(shape) => shape.into(),
    }
}

fn push_action_lines(output: &mut String, actions: &[DefaultActionBindings]) {
    for action in actions {
        let value = action
            .bindings
            .iter()
            .map(|binding| toml_edit::Value::from(binding.as_str()).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "# {} = [{value}]", action.action)
            .expect("writing to a String cannot fail");
    }
}

fn push_wrapped_comment(output: &mut String, indent: &str, value: &str) {
    let mut line = format!("#{indent}");
    let continuation = format!("#{indent}  ");
    let mut line_has_value = false;
    for word in value.split_whitespace() {
        if line_has_value && line.len() + 1 + word.len() > 100 {
            output.push_str(&line);
            output.push('\n');
            line.clone_from(&continuation);
            line.push_str(word);
        } else {
            line.push(' ');
            line.push_str(word);
        }
        line_has_value = true;
    }
    output.push_str(&line);
    output.push('\n');
}
