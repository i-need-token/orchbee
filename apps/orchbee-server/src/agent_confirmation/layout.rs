//! Bundled confirmation layouts. A layout describes one dialog of one Agent exactly as that
//! Agent draws it, from the dialog's top border to the last row of the screen; whatever it does
//! not describe is a mismatch. Layouts are compiled in and never read from a file a person or a
//! download can change: a rule that can press a key ships only inside a release.
use orchbee_agent_observation_protocol::AgentReportProvider;
use regex::{Regex, RegexBuilder};
use serde::Deserialize;

use super::Kind;

const SCREEN_BYTES: usize = 64 * 1024;
const PATTERN_BYTES: usize = 8 * 1024;
const COMPILED_BYTES: usize = 1 << 20;
/// Rows of command or path text one dialog may hold; the command check bounds them again.
const TEXT_ROWS: usize = 16;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    /// Matches a whole row: the dialog's top border.
    top: String,
    layout: Vec<RawLayout>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayout {
    name: String,
    kind: String,
    accept: String,
    /// One pattern per dialog row or run of rows, each ending in its line break. Joined, they
    /// must match the region from the top border to the last row.
    rows: Vec<String>,
    /// Alternatives for one row of the `text` capture; the first that matches every row gives
    /// each row's text in its own `text` capture. Absent, a row is its text.
    #[serde(default)]
    text: Vec<String>,
    /// Matches the `notes` capture when the Agent itself marks the request as dangerous.
    #[serde(default)]
    danger: Option<String>,
}

struct Layout {
    name: String,
    kind: Kind,
    pattern: Regex,
    text: Vec<Regex>,
    danger: Option<Regex>,
}

struct Agent {
    provider: AgentReportProvider,
    top: Regex,
    layouts: Vec<Layout>,
}

/// A confirmation dialog read from one screen: which layout drew it and the text it asks about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Dialog {
    pub(crate) layout: String,
    pub(crate) kind: Kind,
    /// The command's rows without the dialog's gutter, or the path as one row.
    pub(crate) text: Vec<String>,
    /// The Agent drew its own danger marker.
    pub(crate) marked: bool,
}

/// Every bundled layout, compiled once.
pub(crate) struct Layouts(Vec<Agent>);

fn compile(pattern: &str) -> Result<Regex, String> {
    if pattern.len() > PATTERN_BYTES {
        return Err("Confirmation layout pattern exceeds bound".into());
    }
    RegexBuilder::new(pattern)
        .size_limit(COMPILED_BYTES)
        .build()
        .map_err(|error| format!("Confirmation layout pattern is invalid: {error}"))
}

fn parse(provider: AgentReportProvider, source: &str) -> Result<Agent, String> {
    let raw: RawFile = toml::from_str(source)
        .map_err(|error| format!("Confirmation layout is invalid: {error}"))?;
    let mut layouts = Vec::with_capacity(raw.layout.len());
    for layout in raw.layout {
        let kind = match layout.kind.as_str() {
            "command" => Kind::Command,
            "file-create" => Kind::FileCreate,
            "file-edit" => Kind::FileEdit,
            _ => {
                return Err(format!(
                    "Confirmation layout {} has an unknown kind",
                    layout.name
                ));
            }
        };
        // Enter is the only key the Terminal Host's screen-gated message can carry.
        if layout.accept != "enter" {
            return Err(format!(
                "Confirmation layout {} names an unknown key",
                layout.name
            ));
        }
        let pattern = compile(&format!(r"\A(?:{})\z", layout.rows.concat()))?;
        if !pattern.capture_names().flatten().any(|name| name == "text") {
            return Err(format!(
                "Confirmation layout {} captures no text",
                layout.name
            ));
        }
        let text = layout
            .text
            .iter()
            .map(|pattern| compile(&format!(r"\A(?:{pattern})\z")))
            .collect::<Result<Vec<_>, _>>()?;
        if text
            .iter()
            .any(|pattern| !pattern.capture_names().flatten().any(|name| name == "text"))
        {
            return Err(format!(
                "Confirmation layout {} has a row without text",
                layout.name
            ));
        }
        layouts.push(Layout {
            name: layout.name,
            kind,
            pattern,
            text,
            danger: layout.danger.as_deref().map(compile).transpose()?,
        });
    }
    Ok(Agent {
        provider,
        top: compile(&format!(r"\A(?:{})\z", raw.top))?,
        layouts,
    })
}

/// The dialog region: from the last row the top border matches to the last row that is not
/// blank, every row without trailing spaces and ended by one line break.
fn region(top: &Regex, screen: &str) -> Option<String> {
    let rows: Vec<&str> = screen
        .lines()
        .map(|row| row.trim_end_matches(' '))
        .collect();
    let end = rows.iter().rposition(|row| !row.is_empty())? + 1;
    let start = rows[..end].iter().rposition(|row| top.is_match(row))?;
    let mut region = String::with_capacity(screen.len());
    for row in &rows[start..end] {
        region.push_str(row);
        region.push('\n');
    }
    Some(region)
}

impl Layout {
    fn read(&self, region: &str) -> Option<Dialog> {
        let captures = self.pattern.captures(region)?;
        let rows: Vec<&str> = captures.name("text")?.as_str().lines().collect();
        if rows.is_empty() || rows.len() > TEXT_ROWS {
            return None;
        }
        let text = if self.text.is_empty() {
            rows.iter().map(|row| (*row).to_owned()).collect()
        } else {
            self.text.iter().find_map(|pattern| {
                rows.iter()
                    .map(|row| Some(pattern.captures(row)?.name("text")?.as_str().to_owned()))
                    .collect::<Option<Vec<String>>>()
            })?
        };
        let marked = match (&self.danger, captures.name("notes")) {
            (Some(danger), Some(notes)) => danger.is_match(notes.as_str()),
            _ => false,
        };
        Some(Dialog {
            layout: self.name.clone(),
            kind: self.kind,
            text,
            marked,
        })
    }
}

impl Layouts {
    /// Parses every bundled layout; one bad layout fails them all, so nothing is pressed with
    /// half a rule set.
    pub(crate) fn bundled() -> Result<Self, String> {
        macro_rules! bundle {
            ($($provider:ident => $file:literal),* $(,)?) => {
                [$((
                    AgentReportProvider::$provider,
                    include_str!(concat!("../../resources/agent-confirmation/", $file, ".toml")),
                )),*]
            };
        }
        bundle!(Claude => "claude")
            .into_iter()
            .map(|(provider, source)| parse(provider, source))
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Whether any layout of the Agent is bundled, which is what makes an Agent type supported.
    pub(crate) fn supports(&self, provider: AgentReportProvider) -> bool {
        self.0.iter().any(|agent| agent.provider == provider)
    }

    /// Reads the Agent's confirmation dialog from one screen whose lines are screen rows.
    /// `None` means no bundled layout describes the screen completely.
    pub(crate) fn read(&self, provider: AgentReportProvider, screen: &str) -> Option<Dialog> {
        if screen.len() > SCREEN_BYTES {
            return None;
        }
        let agent = self.0.iter().find(|agent| agent.provider == provider)?;
        let region = region(&agent.top, screen)?;
        agent.layouts.iter().find_map(|layout| layout.read(&region))
    }
}
