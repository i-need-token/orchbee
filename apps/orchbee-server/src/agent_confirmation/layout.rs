//! Bundled confirmation layouts. A layout describes one dialog of one Agent exactly as that
//! Agent draws it, from the dialog's top border to the last row of the screen; whatever it does
//! not describe is a mismatch. Layouts are compiled in and never read from a file a person or a
//! download can change: a rule that can press a key ships only inside a release.
use std::ops::Range;

use orchbee_agent_observation_protocol::AgentReportProvider;
use regex::{Captures, Regex, RegexBuilder};
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
    /// Matches a whole row: the dialog's top border, or its title where the Agent draws none.
    top: String,
    /// Matches a whole row that text a model supplies cannot draw. An Agent whose `top` such
    /// text can draw names one; see [`region`].
    #[serde(default)]
    fence: Option<String>,
    /// The row `top` matches must be the only one on the screen. An Agent whose `top` such text
    /// can draw, and whose dialog is always whole at its top, names it; see [`region`].
    #[serde(default)]
    only: bool,
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
    /// The capture of the option the accept key chooses, for an Agent that tells the chosen
    /// option only by how it is drawn. The dialog is read only while that option is drawn in one
    /// style and every capture in `unselected` in one other style.
    #[serde(default)]
    selected: Option<String>,
    /// The captures of the other options. Two at least: with one, two options drawn apart do not
    /// tell which of them is chosen.
    #[serde(default)]
    unselected: Vec<String>,
}

struct Selection {
    selected: String,
    unselected: Vec<String>,
}

struct Layout {
    name: String,
    kind: Kind,
    pattern: Regex,
    text: Vec<Regex>,
    danger: Option<Regex>,
    selection: Option<Selection>,
}

struct Agent {
    provider: AgentReportProvider,
    top: Regex,
    fence: Option<Regex>,
    only: bool,
    layouts: Vec<Layout>,
}

/// One screen row as the Terminal Host draws it: its text, and for each cell where the cell's
/// text starts in it and the cell's style, which is compared only for equality.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct StyledRow {
    pub(crate) text: String,
    pub(crate) cells: Vec<(usize, u128)>,
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
        let captures = |name: &str| pattern.capture_names().flatten().any(|found| found == name);
        if !captures("text") {
            return Err(format!(
                "Confirmation layout {} captures no text",
                layout.name
            ));
        }
        let selection = match (layout.selected, layout.unselected.len()) {
            (None, 0) => None,
            (Some(selected), 2..)
                if captures(&selected) && layout.unselected.iter().all(|name| captures(name)) =>
            {
                Some(Selection {
                    selected,
                    unselected: layout.unselected,
                })
            }
            _ => {
                return Err(format!(
                    "Confirmation layout {} names its options wrongly",
                    layout.name
                ));
            }
        };
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
            selection,
        });
    }
    Ok(Agent {
        provider,
        top: compile(&format!(r"\A(?:{})\z", raw.top))?,
        fence: raw
            .fence
            .map(|fence| compile(&format!(r"\A(?:{fence})\z")))
            .transpose()?,
        only: raw.only,
        layouts,
    })
}

/// The dialog's rows of one screen, each without trailing spaces and ended by one line break,
/// with the styled rows they were read from when the screen was read with styles.
struct Region<'a> {
    text: String,
    /// Where each row starts in `text`.
    starts: Vec<usize>,
    styles: Option<&'a [StyledRow]>,
}

/// The dialog region: from the last row `top` matches to the last row that is not blank.
///
/// Under a fence or `only` that row must be the only one `top` matches. Under a fence a fence
/// row must lie above it too: the fence row is not the dialog's, so the dialog starts on the
/// screen and its own top row is there. An Agent names `only` when its dialog cannot start above
/// the screen. Either way, being the only one, the matched row is the dialog's own and not text
/// drawn to look like it.
fn region<'a>(agent: &Agent, screen: &str, styles: Option<&'a [StyledRow]>) -> Option<Region<'a>> {
    let rows: Vec<&str> = screen
        .lines()
        .map(|row| row.trim_end_matches(' '))
        .collect();
    let end = rows.iter().rposition(|row| !row.is_empty())? + 1;
    let start = rows[..end]
        .iter()
        .rposition(|row| agent.top.is_match(row))?;
    let above = &rows[..start];
    if (agent.only || agent.fence.is_some()) && above.iter().any(|row| agent.top.is_match(row)) {
        return None;
    }
    if let Some(fence) = &agent.fence
        && !above.iter().any(|row| fence.is_match(row))
    {
        return None;
    }
    let mut text = String::with_capacity(screen.len());
    let mut starts = Vec::with_capacity(end - start);
    for row in &rows[start..end] {
        starts.push(text.len());
        text.push_str(row);
        text.push('\n');
    }
    Some(Region {
        text,
        starts,
        styles: styles.and_then(|styles| styles.get(start..end)),
    })
}

impl Region<'_> {
    /// The one style every visible cell of `range`, which lies in one row, is drawn in. `None`
    /// when the cells differ, when the range holds no visible cell, or when the styled row is not
    /// the row the text was read from.
    fn style(&self, range: Range<usize>) -> Option<u128> {
        let row = self.starts.partition_point(|start| *start <= range.start) - 1;
        let start = self.starts[row];
        let text = &self.text[start..self.text[start..].find('\n')? + start];
        let styled = self.styles?.get(row)?;
        if styled.text.trim_end_matches(' ') != text || range.end > start + text.len() {
            return None;
        }
        let range = range.start - start..range.end - start;
        let mut found = None;
        for (index, (cell, style)) in styled.cells.iter().enumerate() {
            let next = styled
                .cells
                .get(index + 1)
                .map_or(styled.text.len(), |(next, _)| *next);
            if !range.contains(cell) || styled.text.get(*cell..next)?.trim().is_empty() {
                continue;
            }
            if found.is_some_and(|found| found != *style) {
                return None;
            }
            found = Some(*style);
        }
        found
    }
}

impl Selection {
    /// Whether the option the accept key chooses is drawn apart from the others, which are all
    /// drawn alike.
    fn holds(&self, region: &Region<'_>, captures: &Captures<'_>) -> bool {
        let style = |name: &str| region.style(captures.name(name)?.range());
        let Some(selected) = style(&self.selected) else {
            return false;
        };
        let mut unselected = self.unselected.iter().map(|name| style(name));
        let Some(Some(other)) = unselected.next() else {
            return false;
        };
        other != selected && unselected.all(|style| style == Some(other))
    }
}

impl Layout {
    fn read(&self, region: &Region<'_>) -> Option<Dialog> {
        let captures = self.pattern.captures(&region.text)?;
        if let Some(selection) = &self.selection
            && !selection.holds(region, &captures)
        {
            return None;
        }
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
        bundle!(
            Claude => "claude",
            Codex => "codex",
            Gemini => "gemini",
            OpenCode => "opencode",
        )
        .into_iter()
        .map(|(provider, source)| parse(provider, source))
        .collect::<Result<Vec<_>, _>>()
        .map(Self)
    }

    /// Whether any layout of the Agent is bundled, which is what makes an Agent type supported.
    pub(crate) fn supports(&self, provider: AgentReportProvider) -> bool {
        self.0.iter().any(|agent| agent.provider == provider)
    }

    /// Whether reading the Agent's dialog from this screen needs the screen's styles: a layout
    /// of the Agent tells its chosen option by style, and the screen shows a dialog region.
    pub(crate) fn needs_styles(&self, provider: AgentReportProvider, screen: &str) -> bool {
        screen.len() <= SCREEN_BYTES
            && self.0.iter().any(|agent| {
                agent.provider == provider
                    && agent
                        .layouts
                        .iter()
                        .any(|layout| layout.selection.is_some())
                    && region(agent, screen, None).is_some()
            })
    }

    /// Reads the Agent's confirmation dialog from one screen whose lines are screen rows, with
    /// the styled rows read at the same frame when there are any. `None` means no bundled layout
    /// describes the screen completely.
    pub(crate) fn read(
        &self,
        provider: AgentReportProvider,
        screen: &str,
        styles: Option<&[StyledRow]>,
    ) -> Option<Dialog> {
        if screen.len() > SCREEN_BYTES {
            return None;
        }
        let agent = self.0.iter().find(|agent| agent.provider == provider)?;
        let region = region(agent, screen, styles)?;
        agent.layouts.iter().find_map(|layout| layout.read(&region))
    }
}
