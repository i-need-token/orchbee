//! Text markers any shell prints around one command, and the output found between them.
//!
//! The submitted text spells each marker in pieces the shell joins only when it runs, so the
//! echoed input never holds a whole marker; only a line the shell printed can match one.
use crate::cli::terminal_output::OutputRow;
use crate::invocation::CaptureShell;

/// The most captured text one run returns; longer output keeps its newest lines.
pub(super) const MAXIMUM_OUTPUT_BYTES: usize = 1024 * 1024;

/// Longer than any marker line, so a longer line is never compared.
const MAXIMUM_MARKER_BYTES: usize = 256;

/// The text submitted for `command`: the begin marker, the command, then the end marker with
/// its status. The command travels as one quoted string the shell parses only when it runs it,
/// so a syntax error in it is reported as its failure and the end marker still follows. The end
/// marker starts with a line break, so it begins its own line after output that ends without
/// one; output that ends with a line break leaves one empty line above it.
pub(super) fn wrapped(shell: CaptureShell, run_id: &str, command: &str) -> String {
    match shell {
        // The leading space keeps the line out of history where the shell ignores such lines.
        CaptureShell::Posix => format!(
            " printf '%s%s\\n' '[orchbee' ' run {run_id}]'; eval '{}'; printf '\\n%s%s %s\\n' '[orchbee' ' end {run_id}]' \"$?\"",
            command.replace('\'', "'\\''")
        ),
        CaptureShell::Fish => format!(
            " printf '%s%s\\n' '[orchbee' ' run {run_id}]'; eval '{}'; printf '\\n%s%s %s\\n' '[orchbee' ' end {run_id}]' $status",
            command.replace('\\', "\\\\").replace('\'', "\\'")
        ),
        // The command runs in the current scope. A native command's nonzero exit code is its
        // status, any other failure is 1, and the first terminating error ends the command.
        CaptureShell::PowerShell => format!(
            "'[orchbee'+' run {run_id}]'; $global:LASTEXITCODE=0; $__orchbee=0; try {{ . ([scriptblock]::Create('{}'+\"`n\"+'$__orchbee=$(if ($?) {{0}} elseif ($global:LASTEXITCODE) {{$global:LASTEXITCODE}} else {{1}})')) }} catch {{ $_; $__orchbee=1 }}; \"`n[orchbee\"+\" end {run_id}] $__orchbee\"; Remove-Variable __orchbee",
            powershell_quoted(command)
        ),
        // cmd has no quoted evaluation and parses the whole line before running any of it.
        // `(call )` clears ERRORLEVEL, a caret splits each marker, and `call` expands the
        // status after the command ran rather than when the line was read. `if` and `for`
        // take the rest of their line as their own command, so a line that uses one runs as a
        // parenthesized block; any other line runs bare, where a `)` in its text or in a
        // variable it expands, such as `C:\Program Files (x86)`, stays literal.
        CaptureShell::Cmd => {
            let command = if takes_rest_of_line(command) {
                format!("({command})")
            } else {
                command.to_owned()
            };
            format!(
                "(call )& echo [orch^bee run {run_id}]& {command}& echo.& call echo [orch^bee end {run_id}] %^ERRORLEVEL%"
            )
        }
    }
}

/// Whether a cmd line uses `if` or `for`: either word outside double quotes, carets removed.
fn takes_rest_of_line(command: &str) -> bool {
    let mut quoted = false;
    let mut escaped = false;
    let mut word = String::new();
    for character in command.chars().chain([' ']) {
        let literal = std::mem::take(&mut escaped);
        if character == '^' && !quoted && !literal {
            escaped = true;
            continue;
        }
        if character == '"' && !literal {
            quoted = !quoted;
        }
        if character.is_ascii_alphanumeric() && !quoted {
            word.push(character.to_ascii_lowercase());
            continue;
        }
        if word == "if" || word == "for" {
            return true;
        }
        word.clear();
    }
    false
}

/// `text` inside a PowerShell single-quoted string, which each typographic single quote also
/// closes; a doubled quote stays literal.
fn powershell_quoted(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len());
    for character in text.chars() {
        quoted.push(character);
        if matches!(
            character,
            '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}'
        ) {
            quoted.push(character);
        }
    }
    quoted
}

/// One run's markers, compared without spaces: a soft wrap can fall on a space the screen does
/// not keep, and no space distinguishes one marker from another.
pub(super) struct Markers {
    begin: String,
    end: String,
    /// The run ID in pieces short enough that a soft wrap inside the ID leaves one of them
    /// whole on a row. Only this run's echoed input and markers hold them.
    pieces: Vec<String>,
}

enum Marker {
    Begin,
    End(i32),
}

impl Markers {
    pub(super) fn new(run_id: &str) -> Self {
        Self {
            begin: format!("[orchbeerun{run_id}]"),
            end: format!("[orchbeeend{run_id}]"),
            pieces: run_id
                .as_bytes()
                .chunks(8)
                .map(|piece| String::from_utf8_lossy(piece).into_owned())
                .collect(),
        }
    }

    /// Whether `row` holds part of this run's ID, as its echoed input does.
    fn mentioned_in(&self, row: &OutputRow) -> bool {
        self.pieces
            .iter()
            .any(|piece| row.text.contains(piece.as_str()))
    }

    /// The marker that starts at the first of `rows` and how many rows it takes. The shell
    /// prints each marker after a line break, so a marker starts a row and wraps only onto the
    /// rows that continue it; it is matched from any row, even one that seems to continue the
    /// row above, and its last row may seem to continue onto the next. An end marker takes
    /// every digit of its status.
    fn starting(&self, rows: &[OutputRow]) -> Option<(Marker, usize)> {
        if !rows.first()?.text.trim_start().starts_with('[') {
            return None;
        }
        let mut text = String::new();
        let mut found = None;
        for (index, row) in rows.iter().enumerate() {
            if index > 0 && !row.from_previous {
                break;
            }
            text.extend(row.text.chars().filter(|character| *character != ' '));
            if text.len() > MAXIMUM_MARKER_BYTES {
                break;
            }
            match self.classify(&text) {
                Some(Marker::Begin) => return Some((Marker::Begin, index + 1)),
                Some(end) => found = Some((end, index + 1)),
                None if found.is_some() => break,
                None => {}
            }
        }
        found
    }

    /// Which of this run's markers `text`, without its spaces, is. An end marker carries the
    /// decimal status the shells print.
    fn classify(&self, text: &str) -> Option<Marker> {
        if text == self.begin {
            return Some(Marker::Begin);
        }
        let status = text.strip_prefix(self.end.as_str())?;
        let digits = status.strip_prefix('-').unwrap_or(status);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        status.parse().ok().map(Marker::End)
    }

    /// Whether this run's end marker is among `rows`, one sample in row order.
    pub(super) fn ended_in(&self, rows: &[OutputRow]) -> bool {
        (0..rows.len())
            .any(|first| matches!(self.starting(&rows[first..]), Some((Marker::End(_), _))))
    }
}

/// What one scan found above the bottom of the terminal.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Captured {
    /// The end marker's status; `None` while the command still runs.
    pub(super) status: Option<i32>,
    /// The text between the markers, or down to the last printed row while the command runs;
    /// `None` when no row can be attributed to the command.
    pub(super) output: Option<String>,
    /// Only the newest lines are kept: the start of the output is no longer retained, or the
    /// output exceeded `MAXIMUM_OUTPUT_BYTES`.
    pub(super) truncated: bool,
}

/// Rows taken from the bottom upward one window at a time, until the begin marker. Rows from the
/// end marker down are the prompt and are dropped once it is found.
pub(super) struct Scan<'a> {
    markers: &'a Markers,
    status: Option<i32>,
    /// Kept rows, newest window first, each window in row order.
    kept: Vec<Vec<OutputRow>>,
    kept_bytes: usize,
    truncated: bool,
    /// A kept row holds part of the run ID, so this run's input is still retained.
    echoed: bool,
    /// Rows at the top of the newer windows that may end a marker starting in an older window.
    pending: Vec<OutputRow>,
}

impl<'a> Scan<'a> {
    pub(super) const fn new(markers: &'a Markers) -> Self {
        Self {
            markers,
            status: None,
            kept: Vec::new(),
            kept_bytes: 0,
            truncated: false,
            echoed: false,
            pending: Vec::new(),
        }
    }

    /// Take the window just above every row taken so far; `history_start` when its first row is
    /// the oldest retained row. Answers once the begin marker or the history start is reached,
    /// or once a finished command's kept output reaches its bound.
    pub(super) fn push(
        &mut self,
        mut window: Vec<OutputRow>,
        history_start: bool,
    ) -> Option<Captured> {
        window.append(&mut self.pending);
        let mut cut = window.len();
        for first in (0..window.len()).rev() {
            match self.markers.starting(&window[first..]) {
                Some((Marker::Begin, rows)) => {
                    // A line break follows the marker, so only a redraw leaves its row wrapped.
                    let redrawn = window[first + rows - 1].into_next;
                    let output = window.split_off(first + rows);
                    let count = cut.saturating_sub(first + rows);
                    self.keep(output.into_iter().take(count).collect());
                    return Some(self.finish(Some(redrawn)));
                }
                Some((Marker::End(status), _)) if self.status.is_none() => {
                    self.status = Some(status);
                    self.kept.clear();
                    self.kept_bytes = 0;
                    self.truncated = false;
                    self.echoed = false;
                    cut = first;
                }
                _ => {}
            }
        }
        // The first rows may end a marker that starts in the older window, so they wait for it.
        let waiting = if history_start {
            0
        } else {
            continuing(&window).min(cut)
        };
        let output = window.split_off(waiting);
        self.keep(output.into_iter().take(cut - waiting).collect());
        self.pending = window;
        (history_start || (self.truncated && self.status.is_some())).then(|| self.finish(None))
    }

    /// Put `rows`, the rows just above the kept ones, in front of them within the output bound.
    fn keep(&mut self, mut rows: Vec<OutputRow>) {
        if self.truncated {
            return;
        }
        let mut first = rows.len();
        while first > 0 {
            let cost = rows[first - 1].text.len() + 1;
            if self.kept_bytes + cost > MAXIMUM_OUTPUT_BYTES {
                self.truncated = true;
                break;
            }
            self.kept_bytes += cost;
            first -= 1;
        }
        let rows = rows.split_off(first);
        self.echoed |= rows.iter().any(|row| self.markers.mentioned_in(row));
        self.kept.push(rows);
    }

    /// `begun` holds whether the begin marker's row seems to wrap once that marker is found.
    fn finish(&mut self, begun: Option<bool>) -> Captured {
        // A finished command printed its begin marker after its echoed input. When neither is
        // among the kept rows, the history, a clear or the output bound dropped them, and
        // every kept row is the newest part of its output. While the command runs, or with its
        // input still in view, a missing begin marker leaves no row that is surely its output.
        // Only rows kept from just below the begin marker can be the ones a redraw left.
        let redrawn = begun == Some(true) && !self.truncated;
        let begun = begun.is_some();
        let attributed = begun || (self.status.is_some() && !self.echoed);
        let truncated = attributed && (self.truncated || !begun);
        let output = attributed.then(|| {
            let mut rows = std::mem::take(&mut self.kept)
                .into_iter()
                .rev()
                .flatten()
                .collect::<Vec<_>>();
            if truncated {
                let continued = rows.iter().take_while(|row| row.from_previous).count();
                rows.drain(..continued);
            }
            // While the command runs, the blank rows below its output are not output.
            if self.status.is_none() {
                while rows.last().is_some_and(|row| row.text.trim().is_empty()) {
                    rows.pop();
                }
            }
            text(&rows, redrawn)
        });
        Captured {
            status: self.status,
            output,
            truncated,
        }
    }
}

/// How many of the first rows continue the row above them within a marker's length: the rows
/// that may end a marker starting above them.
fn continuing(rows: &[OutputRow]) -> usize {
    let mut bytes = 0;
    rows.iter()
        .take_while(|row| {
            bytes += row.text.len();
            row.from_previous && bytes <= MAXIMUM_MARKER_BYTES
        })
        .count()
}

/// Rows as text: one line break between printed lines, none across a soft wrap. A line keeps
/// the spaces it wraps on but not the blank cells after its last character, and the last row,
/// which no output row follows, keeps none.
///
/// `redrawn` says the begin marker's row seems to wrap into the first row. Under ConPTY, every
/// row that PSReadLine's prediction of a longer earlier line took below the input stays marked
/// as wrapping, padded with spaces to the edge, after the shell prints over it; so while the
/// rows below that marker also seem to wrap after a blank cell, each ends its line.
fn text(rows: &[OutputRow], mut redrawn: bool) -> String {
    let mut text = String::new();
    let mut wraps = false;
    for (index, row) in rows.iter().enumerate() {
        if index > 0 && !(wraps && row.from_previous) {
            text.push('\n');
        }
        redrawn &= row.into_next && row.text.chars().next_back().is_none_or(|last| last == ' ');
        wraps = row.into_next && !redrawn && index + 1 < rows.len();
        text.push_str(if wraps {
            &row.text
        } else {
            row.text.trim_end_matches(' ')
        });
    }
    text
}

#[cfg(test)]
mod tests;
