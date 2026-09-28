//! Live prompt regions adapted from pinned Herdr; byte ranges use real UTF-8 line boundaries, including CRLF.
//! Attribution and Apache-2.0 license: ../../resources/agent-detection/NOTICE and LICENSE.
use super::ScreenInput;
#[derive(Clone, Copy, Debug)]
pub(super) enum Region {
    Whole,
    Title,
    Progress,
    AfterPrompt,
    BeforePrompt,
    WithoutPrompt,
    BlockMarker,
    AfterBlock,
    BoxBody,
    AboveBox,
    LastAboveBox,
    AfterRule,
    Bottom(usize),
    BottomNonEmpty(usize),
    TopNonEmpty(usize),
}
impl Region {
    pub(super) fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        Ok(match spec {
            "whole_recent" => Self::Whole,
            "osc_title" => Self::Title,
            "osc_progress" => Self::Progress,
            "after_last_prompt_marker" => Self::AfterPrompt,
            "before_current_prompt_marker" => Self::BeforePrompt,
            "whole_recent_without_current_prompt_marker" => Self::WithoutPrompt,
            "current_prompt_block_marker" => Self::BlockMarker,
            "after_current_prompt_block_marker" => Self::AfterBlock,
            "prompt_box_body" => Self::BoxBody,
            "above_prompt_box" => Self::AboveBox,
            "last_non_empty_above_prompt_box" => Self::LastAboveBox,
            "after_last_horizontal_rule" => Self::AfterRule,
            _ => {
                let mut found = None;
                for (name, kind) in [
                    ("bottom_lines", 0),
                    ("bottom_non_empty_lines", 1),
                    ("top_non_empty_lines", 2),
                ] {
                    if let Some(raw) = spec
                        .strip_prefix(name)
                        .and_then(|v| v.strip_prefix('('))
                        .and_then(|v| v.strip_suffix(')'))
                    {
                        let count = raw
                            .parse::<usize>()
                            .ok()
                            .filter(|n| *n <= usize::from(u16::MAX));
                        if kind == 2
                            && (raw.starts_with('0') || !raw.bytes().all(|b| b.is_ascii_digit()))
                        {
                            break;
                        }
                        if let Some(n) = count {
                            found = Some(match kind {
                                0 => Self::Bottom(n),
                                1 => Self::BottomNonEmpty(n),
                                _ => Self::TopNonEmpty(n),
                            });
                        }
                        break;
                    }
                }
                found.ok_or_else(|| format!("Invalid screen region: {spec}"))?
            }
        })
    }
    pub(super) fn select<'a>(self, input: ScreenInput<'a>) -> &'a str {
        match self {
            Self::Whole => return input.screen,
            Self::Title => return input.title,
            Self::Progress => return input.progress,
            _ => {}
        }
        let text = input.screen;
        let lines = Lines::new(text);
        let current = lines.current_prompt();
        match self {
            Self::AfterPrompt => lines
                .rows
                .iter()
                .rposition(|r| prompt(r.1))
                .map_or(text, |n| lines.from(n + 1)),
            Self::BeforePrompt => current.map_or(text, |n| lines.before(n)),
            Self::WithoutPrompt => {
                if current.is_some() {
                    ""
                } else {
                    text
                }
            }
            Self::BlockMarker | Self::AfterBlock => current
                .and_then(|n| lines.rows[..n].iter().rposition(|r| block(r.1)))
                .map_or("", |n| {
                    if matches!(self, Self::BlockMarker) {
                        lines.rows[n].1
                    } else {
                        lines.from(n)
                    }
                }),
            Self::BoxBody => lines.box_top().map_or("", |n| {
                let end = lines.rows[n + 1..]
                    .iter()
                    .position(|r| horizontal(r.1))
                    .map_or(lines.rows.len(), |p| n + 1 + p);
                &text[lines.offset(n + 1)..lines.offset(end)]
            }),
            Self::AboveBox => lines.box_top().map_or(text, |n| lines.before(n)),
            Self::LastAboveBox => lines.rows[..lines.box_top().unwrap_or(lines.rows.len())]
                .iter()
                .rev()
                .find(|r| !r.1.trim().is_empty())
                .map_or("", |r| r.1),
            Self::AfterRule => lines
                .rows
                .iter()
                .rposition(|r| horizontal(r.1))
                .map_or(text, |n| lines.from(n + 1)),
            Self::Bottom(n) => lines.from(lines.rows.len().saturating_sub(n)),
            Self::BottomNonEmpty(n) => lines
                .rows
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, r)| !r.1.trim().is_empty())
                .take(n)
                .last()
                .map_or("", |(i, _)| lines.from(i)),
            Self::TopNonEmpty(n) => lines
                .rows
                .iter()
                .enumerate()
                .filter(|(_, r)| !r.1.trim().is_empty())
                .take(n)
                .last()
                .map_or("", |(i, _)| lines.before(i + 1)),
            Self::Whole | Self::Title | Self::Progress => {
                unreachable!("direct fields returned above")
            }
        }
    }
}
struct Lines<'a> {
    text: &'a str,
    rows: Vec<(usize, &'a str)>,
}
impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let mut offset = 0;
        let rows = text
            .split_inclusive('\n')
            .map(|chunk| {
                let line = chunk
                    .strip_suffix('\n')
                    .map_or(chunk, |s| s.strip_suffix('\r').unwrap_or(s));
                let row = (offset, line);
                offset += chunk.len();
                row
            })
            .collect();
        Self { text, rows }
    }
    fn offset(&self, n: usize) -> usize {
        self.rows.get(n).map_or(self.text.len(), |r| r.0)
    }
    fn from(&self, n: usize) -> &'a str {
        &self.text[self.offset(n)..]
    }
    fn before(&self, n: usize) -> &'a str {
        &self.text[..self.offset(n)]
    }
    fn current_prompt(&self) -> Option<usize> {
        let n = self.rows.iter().rposition(|r| prompt(r.1))?;
        (!self.rows[n + 1..].iter().any(|r| block(r.1))).then_some(n)
    }
    fn box_top(&self) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, r)| horizontal(r.1))
            .nth(1)
            .map(|(i, _)| i)
    }
}
fn prompt(s: &str) -> bool {
    s == "›" || s.starts_with("› ")
}
fn block(s: &str) -> bool {
    s.starts_with(['•', '■', '✗', '✓'])
}
fn horizontal(s: &str) -> bool {
    let s = s.trim();
    let count = s.chars().take_while(|c| *c == '─').count();
    count > 0 && (count >= 3 || s.trim_start_matches('─').trim().is_empty())
}
