//! Deterministic Find in terminal state for one bar.
//!
//! The bar owns its query draft, the Match case choice, the delay before a search runs, the one
//! search it still expects and the last settled matches. Content reads, viewport movement,
//! notices and paint stay in the native UI adapter, which passes the clock in.

use std::{num::NonZeroU64, time::Duration, time::Instant};

use crate::{
    terminal_copy::{CopySearchCase, CopySearchDirection, CopySearchResult, TerminalTextRange},
    terminal_copy_mode::{QueryId, SearchEdit},
    terminal_text::{
        is_forbidden_terminal_scalar,
        line_editor::{LengthLimit, LineEditor},
    },
};

/// Typing waits this long after the last edit before the search runs.
pub(crate) const SEARCH_DELAY: Duration = Duration::from_millis(150);
const MAXIMUM_QUERY_SCALARS: usize = 512;

/// The query and Match case a Client remembers between bars until it exits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct FindMemory {
    pub(crate) query: String,
    pub(crate) match_case: bool,
}

/// The settled matches of one search, in absolute retained-buffer coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FindMatches {
    /// A bounded window of matches around the active one.
    pub(crate) ranges: Vec<TerminalTextRange>,
    /// The global index of `ranges[0]`.
    pub(crate) first: u64,
    pub(crate) total: u64,
    /// The active match, as an index into `ranges`.
    pub(crate) active: usize,
}

impl FindMatches {
    pub(crate) fn active_range(&self) -> Option<TerminalTextRange> {
        self.ranges.get(self.active).copied()
    }

    /// The active match's one-based position among all matches.
    pub(crate) fn position(&self) -> u64 {
        self.first + self.active as u64 + 1
    }

    fn from_result(result: &CopySearchResult) -> Option<Self> {
        let active = result.current?;
        let global = result.current_global?;
        if result.total == 0 || active >= result.matches.len() {
            return None;
        }
        Some(Self {
            ranges: result.matches.clone(),
            first: global.checked_sub(active as u64)?,
            total: result.total,
            active,
        })
    }

    /// Moves within the retained window; `false` when the step leaves it.
    fn step_locally(&mut self, forward: bool) -> bool {
        let global = self.first + self.active as u64;
        let target = if forward {
            (global + 1) % self.total
        } else {
            global.checked_sub(1).unwrap_or(self.total - 1)
        };
        let Some(local) = target.checked_sub(self.first) else {
            return false;
        };
        match usize::try_from(local) {
            Ok(local) if local < self.ranges.len() => {
                self.active = local;
                true
            }
            _ => false,
        }
    }
}

/// What the bar last learned from a search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FindOutcome {
    /// Nothing searched for the current query yet, or the query is empty.
    None,
    Matches(FindMatches),
    NoMatches,
    /// The search reached its time limit.
    Stopped,
    /// The retained output could not be searched.
    Failed,
}

/// The counter cell of the bar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FindCounter {
    Blank,
    Searching,
    Position { current: u64, total: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Fresh,
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Search {
    Idle,
    Due(Instant),
    Running {
        query: QueryId,
        step: Step,
        started: Instant,
    },
}

/// One read the adapter should admit for the bar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FindRead {
    pub(crate) query: QueryId,
    pub(crate) text: Box<str>,
    pub(crate) case: CopySearchCase,
    pub(crate) direction: CopySearchDirection,
    /// The match to step from; `None` starts from the newest end of the buffer.
    pub(crate) previous: Option<TerminalTextRange>,
}

/// What a next or previous request did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FindStep {
    /// The active match moved within the settled window.
    Moved,
    /// The step needs a read from the active match.
    Read(FindRead),
    /// Nothing to step through.
    None,
}

/// A refused edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FindRefusal {
    Limit,
    IdentityExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FindBarView<'a> {
    pub(crate) draft: &'a str,
    /// UTF-8 byte offset, always an extended-grapheme boundary.
    pub(crate) cursor: usize,
    pub(crate) selected: bool,
    pub(crate) match_case: bool,
    pub(crate) counter: FindCounter,
    /// Whether next and previous have matches to move through.
    pub(crate) can_step: bool,
}

pub(crate) struct FindBar {
    editor: LineEditor,
    match_case: bool,
    search: Search,
    outcome: FindOutcome,
    next_query: u64,
}

impl FindBar {
    /// Opens with the remembered query selected; a remembered query searches at once.
    pub(crate) fn new(memory: &FindMemory, now: Instant) -> Self {
        let editor = LineEditor::new(
            memory.query.clone(),
            LengthLimit::Scalars(MAXIMUM_QUERY_SCALARS),
        )
        .select_all();
        let search = if memory.query.is_empty() {
            Search::Idle
        } else {
            Search::Due(now)
        };
        Self {
            editor,
            match_case: memory.match_case,
            search,
            outcome: FindOutcome::None,
            next_query: 0,
        }
    }

    pub(crate) fn memory(&self) -> FindMemory {
        FindMemory {
            query: self.editor.text().to_owned(),
            match_case: self.match_case,
        }
    }

    pub(crate) fn view(&self) -> FindBarView<'_> {
        let counter = if self.editor.text().is_empty() {
            FindCounter::Blank
        } else if self.search != Search::Idle {
            FindCounter::Searching
        } else {
            match &self.outcome {
                FindOutcome::Matches(matches) => FindCounter::Position {
                    current: matches.position(),
                    total: matches.total,
                },
                FindOutcome::NoMatches => FindCounter::Position {
                    current: 0,
                    total: 0,
                },
                FindOutcome::None | FindOutcome::Stopped | FindOutcome::Failed => {
                    FindCounter::Blank
                }
            }
        };
        FindBarView {
            draft: self.editor.text(),
            cursor: self.editor.cursor(),
            selected: self.editor.selected(),
            match_case: self.match_case,
            counter,
            can_step: matches!(self.outcome, FindOutcome::Matches(_)),
        }
    }

    /// The settled matches to highlight; they stay while a newer search runs.
    pub(crate) fn matches(&self) -> Option<&FindMatches> {
        match &self.outcome {
            FindOutcome::Matches(matches) if !self.editor.text().is_empty() => Some(matches),
            _ => None,
        }
    }

    /// When the delayed search becomes due.
    pub(crate) fn wake(&self) -> Option<Instant> {
        match self.search {
            Search::Due(at) => Some(at),
            Search::Idle | Search::Running { .. } => None,
        }
    }

    /// The search in flight, if any.
    pub(crate) fn running(&self) -> Option<QueryId> {
        match self.search {
            Search::Running { query, .. } => Some(query),
            Search::Idle | Search::Due(_) => None,
        }
    }

    /// Applies one draft edit. `Ok(true)` means the query changed: any read in flight is
    /// superseded and the caller cancels it.
    pub(crate) fn edit(&mut self, edit: SearchEdit<'_>, now: Instant) -> Result<bool, FindRefusal> {
        let previous = self.editor.text().to_owned();
        let editor = &mut self.editor;
        match edit {
            SearchEdit::Insert(value) => {
                if value.chars().any(|character| {
                    character.is_control() || is_forbidden_terminal_scalar(character)
                }) {
                    return Err(FindRefusal::Limit);
                }
                editor.insert(value).map_err(|_| FindRefusal::Limit)?;
            }
            SearchEdit::Backspace => {
                editor.backspace();
            }
            SearchEdit::Delete => {
                editor.delete();
            }
            SearchEdit::Left => {
                editor.left();
            }
            SearchEdit::Right => {
                editor.right();
            }
            SearchEdit::Home => {
                editor.home();
            }
            SearchEdit::End => {
                editor.end();
            }
            SearchEdit::Line(key) => {
                editor.apply(key).map_err(|_| FindRefusal::Limit)?;
            }
        }
        let changed = self.editor.text() != previous;
        if changed {
            self.restart(now + SEARCH_DELAY);
        }
        Ok(changed)
    }

    /// Flips Match case; a nonempty query searches again at once.
    pub(crate) fn toggle_match_case(&mut self, now: Instant) {
        self.match_case = !self.match_case;
        self.restart(now);
    }

    fn restart(&mut self, at: Instant) {
        if self.editor.text().is_empty() {
            self.search = Search::Idle;
            self.outcome = FindOutcome::None;
        } else {
            self.search = Search::Due(at);
        }
    }

    const fn case(&self) -> CopySearchCase {
        if self.match_case {
            CopySearchCase::Sensitive
        } else {
            CopySearchCase::Insensitive
        }
    }

    /// The fresh search to admit now, when its delay has passed.
    pub(crate) fn take_due(&mut self, now: Instant) -> Result<Option<FindRead>, FindRefusal> {
        match self.search {
            Search::Due(at) if at <= now => self.begin(Step::Fresh, None, now).map(Some),
            _ => Ok(None),
        }
    }

    fn begin(
        &mut self,
        step: Step,
        previous: Option<TerminalTextRange>,
        now: Instant,
    ) -> Result<FindRead, FindRefusal> {
        self.next_query = self
            .next_query
            .checked_add(1)
            .ok_or(FindRefusal::IdentityExhausted)?;
        let query = NonZeroU64::new(self.next_query)
            .map(QueryId::new)
            .ok_or(FindRefusal::IdentityExhausted)?;
        self.search = Search::Running {
            query,
            step,
            started: now,
        };
        Ok(FindRead {
            query,
            text: self.editor.text().into(),
            case: self.case(),
            direction: match step {
                Step::Fresh | Step::Previous => CopySearchDirection::Backward,
                Step::Next => CopySearchDirection::Forward,
            },
            previous,
        })
    }

    /// Next (toward newer) or previous (toward older) match; both wrap. A search still waiting
    /// for its delay runs now instead.
    pub(crate) fn step(&mut self, forward: bool, now: Instant) -> Result<FindStep, FindRefusal> {
        match self.search {
            Search::Due(_) => return self.begin(Step::Fresh, None, now).map(FindStep::Read),
            Search::Running { .. } => return Ok(FindStep::None),
            Search::Idle => {}
        }
        let FindOutcome::Matches(matches) = &mut self.outcome else {
            return Ok(FindStep::None);
        };
        if matches.step_locally(forward) {
            return Ok(FindStep::Moved);
        }
        let previous = matches.active_range();
        let step = if forward { Step::Next } else { Step::Previous };
        self.begin(step, previous, now).map(FindStep::Read)
    }

    /// Adopts the result of the search in flight; `false` for any other identity.
    pub(crate) fn settle(&mut self, query: QueryId, result: &CopySearchResult) -> bool {
        if self.running() != Some(query) {
            return false;
        }
        self.search = Search::Idle;
        self.outcome =
            FindMatches::from_result(result).map_or(FindOutcome::NoMatches, FindOutcome::Matches);
        true
    }

    /// Records a failed search in flight. A search that ran past `limit` stopped at the time
    /// limit; any other failure could not read the output. `false` for any other identity.
    pub(crate) fn fail(&mut self, query: QueryId, now: Instant, limit: Duration) -> bool {
        let Search::Running { started, .. } = self.search else {
            return false;
        };
        if self.running() != Some(query) {
            return false;
        }
        self.search = Search::Idle;
        self.outcome = if now.saturating_duration_since(started) >= limit {
            FindOutcome::Stopped
        } else {
            FindOutcome::Failed
        };
        true
    }

    /// Drops the matches and searches again at once, as a changed terminal requires.
    pub(crate) fn invalidate(&mut self, now: Instant) {
        self.outcome = FindOutcome::None;
        self.restart(now);
    }

    pub(crate) const fn outcome(&self) -> &FindOutcome {
        &self.outcome
    }
}

#[cfg(test)]
mod tests;
