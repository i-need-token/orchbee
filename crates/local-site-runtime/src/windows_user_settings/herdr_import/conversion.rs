//! Source fields become a sparse TOML tree through the existing settings admission owner.
use super::super::{
    WindowsDirectoryPolicy, WindowsKeySettings,
    WindowsSettingsDiagnosticDisposition as Disposition, WindowsUserSettings, keys, parse,
    unknown_field,
};
use super::{OsString, Path, ReportCollector, WindowsSettingsError};
use toml::{Table, Value};

pub(super) fn convert(
    input: &Table,
    source: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
    report: &mut ReportCollector,
) -> Result<Table, WindowsSettingsError> {
    let mut converter = Converter {
        source,
        env,
        report,
        output: Table::new(),
    };
    for (section, value) in input {
        match section.as_str() {
            "terminal" => converter.simple_section(
                section,
                value,
                &["default_shell", "shell_mode", "new_cwd"],
            )?,
            "worktrees" => converter.simple_section(section, value, &["directory"])?,
            "session" => converter.simple_section(section, value, &["resume_agents_on_restore"])?,
            "keys" => converter.keys(value),
            "ui" => converter.ui(value)?,
            "update" | "onboarding" | "theme" | "server" | "advanced" | "experimental"
            | "remote" => converter.skip(section, value, "not-migrated", Disposition::Unsupported),
            _ => converter.skip(
                &unknown_field("", section),
                value,
                "unknown-key",
                Disposition::Ignored,
            ),
        }
    }
    Ok(converter.output)
}

struct Converter<'a, F> {
    source: &'a Path,
    env: &'a F,
    report: &'a mut ReportCollector,
    output: Table,
}

impl<F: Fn(&str) -> Option<OsString>> Converter<'_, F> {
    fn skip_other(&mut self, parent: &str, key: &str, value: &Value) {
        let known = match parent {
            "terminal" => key == "kitty_graphics",
            "ui" => super::super::HERDR_ONLY_UI_FIELDS.contains(&key),
            "ui.toast" => key == "clipboard",
            "ui.sound" => matches!(key, "path" | "done_path" | "request_path"),
            "keys" => key == "command" || keys::is_herdr_only_action(key),
            _ => false,
        };
        self.skip(
            &unknown_field(parent, key),
            value,
            if known { "not-migrated" } else { "unknown-key" },
            if known {
                Disposition::Unsupported
            } else {
                Disposition::Ignored
            },
        );
    }
    fn skip(&mut self, field: &str, value: &Value, reason: &'static str, disposition: Disposition) {
        if let Some(table) = value.as_table().filter(|t| !t.is_empty()) {
            for (key, value) in table {
                self.skip(&unknown_field(field, key), value, reason, disposition);
            }
        } else {
            self.report.fields.push(field, reason, disposition);
        }
    }
    fn section<'v>(&mut self, field: &str, value: &'v Value) -> Option<&'v Table> {
        match value.as_table() {
            Some(table) => Some(table),
            None => {
                self.report
                    .fields
                    .push(field, "wrong-type", Disposition::Defaulted);
                None
            }
        }
    }
    fn simple_section(
        &mut self,
        section: &str,
        value: &Value,
        allowed: &[&str],
    ) -> Result<(), WindowsSettingsError> {
        let Some(table) = self.section(section, value) else {
            return Ok(());
        };
        for (key, value) in table {
            let field = unknown_field(section, key);
            if allowed.contains(&key.as_str()) {
                self.leaf(&field, value)?;
            } else {
                self.skip_other(section, key, value);
            }
        }
        Ok(())
    }
    fn admitted(
        &mut self,
        field: &str,
        value: &Value,
    ) -> Result<Option<WindowsUserSettings>, WindowsSettingsError> {
        let mut table = Table::new();
        put(&mut table, field, value.clone());
        let loaded = parse(&encode(&table), self.source, self.env)?;
        self.report.absorb(&loaded.diagnostics);
        Ok(loaded.diagnostics.is_empty().then_some(loaded.settings))
    }
    fn leaf(&mut self, field: &str, value: &Value) -> Result<(), WindowsSettingsError> {
        let Some(settings) = self.admitted(field, value)? else {
            return Ok(());
        };
        let converted = match field {
            "terminal.default_shell" => Value::String(settings.shell.executable().to_owned()),
            "terminal.shell_mode" => Value::String(settings.shell.mode().as_str().to_owned()),
            "terminal.new_cwd" => match settings.cwd {
                Some((WindowsDirectoryPolicy::Explicit, Some(path))) => Value::String(path),
                _ => Value::String(value.as_str().unwrap_or_default().trim().to_owned()),
            },
            "worktrees.directory" => {
                Value::String(settings.worktree_root.expect("valid explicit directory"))
            }
            _ => value.clone(),
        };
        put(&mut self.output, field, converted);
        self.report.mapped(field, &[field]);
        Ok(())
    }
    fn ui(&mut self, value: &Value) -> Result<(), WindowsSettingsError> {
        let Some(table) = self.section("ui", value) else {
            return Ok(());
        };
        for (key, value) in table {
            match key.as_str() {
                "window_title" => self.leaf("ui.window_title", value)?,
                "sidebar" => self.sidebar(value)?,
                "toast" => self.toast(value)?,
                "sound" => self.sound(value)?,
                _ => self.skip_other("ui", key, value),
            }
        }
        Ok(())
    }
    fn sidebar(&mut self, value: &Value) -> Result<(), WindowsSettingsError> {
        let Some(table) = self.section("ui.sidebar", value) else {
            return Ok(());
        };
        for (kind, value) in table {
            let field = unknown_field("ui.sidebar", kind);
            let allowed: &[&str] = match kind.as_str() {
                "agents" => &["rows", "rows_by_agent", "row_gap"],
                "spaces" => &["rows", "row_gap"],
                _ => {
                    self.skip(&field, value, "unknown-key", Disposition::Ignored);
                    continue;
                }
            };
            let Some(raw) = self.section(&field, value) else {
                continue;
            };
            let mut filtered = Table::new();
            for (key, value) in raw {
                if allowed.contains(&key.as_str()) {
                    filtered.insert(key.clone(), value.clone());
                } else {
                    self.skip(
                        &unknown_field(&field, key),
                        value,
                        "unknown-key",
                        Disposition::Ignored,
                    );
                }
            }
            if !filtered.is_empty() {
                self.leaf(&field, &Value::Table(filtered))?;
            }
        }
        Ok(())
    }
    fn toast(&mut self, value: &Value) -> Result<(), WindowsSettingsError> {
        let Some(table) = self.section("ui.toast", value) else {
            return Ok(());
        };
        let selected = table
            .get("delivery")
            .map(|v| ("ui.toast.delivery", v, false))
            .or_else(|| table.get("enabled").map(|v| ("ui.toast.enabled", v, true)));
        if let Some((field, value, legacy)) = selected {
            let delivery = if legacy {
                value.as_bool().map(|v| if v { "orchbee" } else { "off" })
            } else {
                match value.as_str() {
                    Some("herdr") => Some("orchbee"),
                    Some("off" | "terminal" | "system") => value.as_str(),
                    _ => None,
                }
            };
            if let Some(delivery) = delivery {
                for target in ["ui.toast.delivery", "ui.toast.agent_delivery"] {
                    put(&mut self.output, target, Value::String(delivery.to_owned()));
                }
                self.report
                    .mapped(field, &["ui.toast.delivery", "ui.toast.agent_delivery"]);
            } else {
                self.report.fields.push(
                    field,
                    if (legacy && !value.is_bool()) || (!legacy && !value.is_str()) {
                        "wrong-type"
                    } else {
                        "invalid-mode"
                    },
                    Disposition::Defaulted,
                );
            }
        }
        for (key, value) in table {
            match key.as_str() {
                "delivery" => {}
                "enabled" if table.contains_key("delivery") => {
                    self.report
                        .fields
                        .push("ui.toast.enabled", "shadowed", Disposition::Ignored)
                }
                "enabled" => {}
                "delay_seconds" => self.leaf("ui.toast.delay_seconds", value)?,
                "herdr" => self.simple_section("ui.toast.overlay", value, &["position"])?,
                _ => self.skip_other("ui.toast", key, value),
            }
        }
        Ok(())
    }
    fn sound(&mut self, value: &Value) -> Result<(), WindowsSettingsError> {
        let Some(table) = self.section("ui.sound", value) else {
            return Ok(());
        };
        for (key, value) in table {
            match key.as_str() {
                "enabled" => self.leaf("ui.sound.enabled", value)?,
                "agents" => {
                    let Some(agents) = self.section("ui.sound.agents", value) else {
                        continue;
                    };
                    for (agent, value) in agents {
                        let source_field = unknown_field("ui.sound.agents", agent);
                        let target = match agent.as_str() {
                            "open_code" => "opencode",
                            "github_copilot" => "copilot",
                            "pi" | "claude" | "codex" | "gemini" | "cursor" | "devin" | "agy"
                            | "cline" | "kimi" | "kiro" | "droid" | "amp" | "grok" | "hermes"
                            | "kilo" | "qodercli" | "qwen" | "letta" | "maki" | "muse" => agent,
                            _ => {
                                self.skip(
                                    &source_field,
                                    value,
                                    "unknown-key",
                                    Disposition::Ignored,
                                );
                                continue;
                            }
                        };
                        if matches!(value.as_str(), Some("default" | "on" | "off")) {
                            let target_field = format!("ui.sound.agents.{target}");
                            put(&mut self.output, &target_field, value.clone());
                            self.report.mapped(&source_field, &[&target_field]);
                        } else {
                            self.report.fields.push(
                                source_field,
                                if value.is_str() {
                                    "invalid-mode"
                                } else {
                                    "wrong-type"
                                },
                                Disposition::Defaulted,
                            );
                        }
                    }
                }
                _ => self.skip_other("ui.sound", key, value),
            }
        }
        Ok(())
    }
    fn keys(&mut self, value: &Value) {
        let Some(table) = self.section("keys", value) else {
            return;
        };
        let mut settings = WindowsKeySettings::default();
        if let Some(prefix) = table.get("prefix") {
            let mut single = Table::new();
            single.insert("prefix".into(), prefix.clone());
            let mut diagnostics = super::super::DiagnosticsCollector::default();
            settings = WindowsKeySettings::parse_lenient(&Value::Table(single), &mut diagnostics);
            let diagnostics = diagnostics.finish();
            self.report.absorb(&diagnostics);
            if diagnostics.is_empty() {
                put(&mut self.output, "keys.prefix", prefix.clone());
                self.report.mapped("keys.prefix", &["keys.prefix"]);
            }
        }
        for action in keys::ACTIONS {
            let (field, raw) = match (table.get(*action), *action) {
                (Some(raw), _) => (*action, Some(raw)),
                (None, "zoom") => ("fullscreen", table.get("fullscreen")),
                (None, _) => (*action, None),
            };
            let Some(raw) = raw else {
                continue;
            };
            let mut diagnostics = super::super::DiagnosticsCollector::default();
            keys::parse_action_lenient(action, field, raw, &mut settings, &mut diagnostics, 16);
            let diagnostics = diagnostics.finish();
            let source_field = format!("keys.{field}");
            let target_field = format!("keys.{action}");
            self.report.absorb(&diagnostics);
            if diagnostics.is_empty() {
                put(&mut self.output, &target_field, raw.clone());
                self.report.mapped(&source_field, &[&target_field]);
            } else {
                // Import disables the invalid source action as a whole; it cannot reserve a trigger.
                settings
                    .overrides
                    .last_mut()
                    .expect("action admission adds one override")
                    .triggers
                    .clear();
                put(&mut self.output, &target_field, Value::Array(Vec::new()));
                self.report.disabled_target(&source_field, &target_field);
            }
        }
        for (key, value) in table {
            if key == "prefix" || keys::ACTIONS.contains(&key.as_str()) {
                continue;
            }
            if key == "fullscreen" && !table.contains_key("zoom") {
                continue;
            }
            if key == "fullscreen" {
                self.skip("keys.fullscreen", value, "shadowed", Disposition::Ignored);
            } else {
                self.skip_other("keys", key, value);
            }
        }
    }
}

fn put(table: &mut Table, field: &str, value: Value) {
    if let Some((parent, rest)) = field.split_once('.') {
        let entry = table
            .entry(parent.to_owned())
            .or_insert_with(|| Value::Table(Table::new()));
        put(
            entry
                .as_table_mut()
                .expect("only table namespaces are inserted"),
            rest,
            value,
        );
    } else {
        table.insert(field.to_owned(), value);
    }
}

pub(super) fn encode(table: &Table) -> String {
    fn value(raw: &Value) -> toml_edit::Value {
        match raw {
            Value::String(s) => toml_edit::Value::from(s.as_str()),
            Value::Integer(n) => toml_edit::Value::from(*n),
            Value::Float(n) => toml_edit::Value::from(*n),
            Value::Boolean(b) => toml_edit::Value::from(*b),
            Value::Datetime(d) => toml_edit::Value::from(*d),
            Value::Array(values) => toml_edit::Value::Array(values.iter().map(value).collect()),
            Value::Table(table) => {
                let mut inline = toml_edit::InlineTable::new();
                for (key, raw) in table {
                    inline.insert(key, value(raw));
                }
                toml_edit::Value::InlineTable(inline)
            }
        }
    }
    fn items(raw: &Table) -> toml_edit::Table {
        let mut table = toml_edit::Table::new();
        for (key, raw) in raw {
            table.insert(
                key,
                match raw {
                    Value::Table(t) => toml_edit::Item::Table(items(t)),
                    _ => toml_edit::Item::Value(value(raw)),
                },
            );
        }
        table
    }
    let document = toml_edit::DocumentMut::from(items(table));
    format!("{}\n", document.to_string().trim_end_matches('\n'))
}
