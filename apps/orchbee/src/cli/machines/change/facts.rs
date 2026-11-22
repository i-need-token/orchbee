//! What closing the Workspace where it runs would stop, as that machine's `workspace close
//! --dry-run` reads it: every terminal, each with whether it runs a program. An older Orchbee there
//! lists the terminals without that, and every one of them counts as unknown, never idle.
use super::Answer;
use crate::{cli::handles, cli_text::Stops, localization::Locale, terminal_title::display_name};
use serde_json::{Value, json};

pub(super) struct Facts {
    /// The terminals closing the Workspace ends, as `orchbee.workspace.close-preview` lists them.
    terminals: Vec<Value>,
    /// Whether the machine read what runs in them.
    read: bool,
    /// Whether it read that none of them runs a program.
    idle: bool,
}

impl Facts {
    /// The facts in a `workspace close --dry-run` answer, or `None` for an answer that is neither
    /// that document nor the terminal list an older Orchbee answers instead.
    pub(super) fn of(answer: &Answer) -> Option<Self> {
        let mut terminals = answer.at("/result/terminals")?.as_array()?.clone();
        match answer.at("/schema")?.as_str()? {
            "orchbee.workspace.close-preview" => {
                let read = answer.at("/result/evidence")?.as_str()? == "read";
                let idle = answer.at("/result/idle")?.as_bool()?;
                Some(Self {
                    terminals,
                    read,
                    idle: read && idle,
                })
            }
            "orchbee.terminal.list" => {
                for terminal in &mut terminals {
                    terminal
                        .as_object_mut()?
                        .insert("activity".to_owned(), json!("unknown"));
                }
                Some(Self {
                    terminals,
                    read: false,
                    idle: false,
                })
            }
            _ => None,
        }
    }

    /// The `terminals`, `idle` and `evidence` members of the change's documents.
    pub(super) fn members(facts: Option<&Self>) -> [(&'static str, Value); 3] {
        [
            (
                "terminals",
                facts.map_or(Value::Null, |facts| json!(facts.terminals)),
            ),
            ("idle", facts.map_or(Value::Null, |facts| json!(facts.idle))),
            (
                "evidence",
                facts.map_or(Value::Null, |facts| {
                    json!(if facts.read { "read" } else { "unavailable" })
                }),
            ),
        ]
    }

    /// The names of the terminals that may run a program: the Agent, else the title as the text
    /// table shows it, else the terminal's handle.
    pub(super) fn names(&self, locale: Locale) -> Vec<String> {
        self.terminals
            .iter()
            .filter(|terminal| terminal["activity"] != "idle")
            .map(|terminal| {
                let observation = &terminal["observation"];
                observation["agent"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| {
                        observation["title"]
                            .as_str()
                            .and_then(|title| display_name(title, locale))
                    })
                    .unwrap_or_else(|| {
                        let id = terminal["terminalId"].as_str().unwrap_or_default();
                        handles::stable("terminal", &[id]).remove(0)
                    })
            })
            .collect()
    }

    /// What the move would stop, given `names` from [`Self::names`].
    pub(super) fn stops<'a>(&self, names: &'a [String]) -> Stops<'a> {
        match (self.read, self.idle) {
            (true, true) => Stops::Idle,
            (true, false) if !names.is_empty() => Stops::Busy(names),
            _ => Stops::Unread,
        }
    }
}
