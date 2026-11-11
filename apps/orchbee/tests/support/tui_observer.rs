//! Platform-independent observer for completed native TUI frames.
//!
//! Controlled Linux process tests feed this observer from a real PTY while the
//! renderer has healthy output. The separate cross-platform integration target
//! exercises the same strict frame grammar without pretending to provide PTY
//! or process evidence.

use std::{io, time::Instant};

use unicode_segmentation::UnicodeSegmentation;

const MAX_OUTPUT: usize = 8 * 1024 * 1024;
// `FixedViewportBackend::draw` can emit several reset pairs inside one
// backend frame. On the healthy renderer path, its synchronized-update begin
// and the end written by `flush` form the process-side publication witness,
// even when the host terminal does not implement synchronized updates
// visually. Terminal restoration can emit the same end sequence; this bounded
// observer does not establish output-failure atomicity or restoration success.
const SYNCHRONIZED_UPDATE_BEGIN: &[u8] = b"\x1b[?2026h";
const SYNCHRONIZED_UPDATE_END: &[u8] = b"\x1b[?2026l";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WorkbenchHeader<'a> {
    location: &'a str,
    tab: Option<(usize, &'a str)>,
}

impl<'a> WorkbenchHeader<'a> {
    pub(super) const fn empty(location: &'a str) -> Self {
        Self {
            location,
            tab: None,
        }
    }

    pub(super) const fn active(location: &'a str, position: usize, label: &'a str) -> Self {
        Self {
            location,
            tab: Some((position, label)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModalRow<'a> {
    Exact(&'a str),
    FieldAtEnd(&'a str),
}

impl<'a> ModalRow<'a> {
    pub(super) const fn exact(value: &'a str) -> Self {
        Self::Exact(value)
    }

    pub(super) const fn field_at_end(value: &'a str) -> Self {
        Self::FieldAtEnd(value)
    }
}

/// A bounded observer for this backend's absolute cursor writes and CSI
/// modes. It is deliberately not a general-purpose terminal emulator.
pub(super) struct Screen {
    cells: Vec<Vec<String>>,
    x: usize,
    y: usize,
    escape: Option<Vec<u8>>,
    utf8: Vec<u8>,
    raw: Vec<u8>,
    synchronized_begin_prefix: usize,
    synchronized_end_prefix: usize,
    synchronized_frame_open: bool,
    latest_completed: Option<CompletedFrame>,
}

pub(super) struct CompletedFrame {
    cells: Vec<Vec<String>>,
    marker_offset: usize,
}

impl Screen {
    pub(super) fn new(columns: usize, rows: usize) -> Self {
        Self {
            cells: vec![vec![" ".to_owned(); columns]; rows],
            x: 0,
            y: 0,
            escape: None,
            utf8: Vec::new(),
            raw: Vec::new(),
            synchronized_begin_prefix: 0,
            synchronized_end_prefix: 0,
            synchronized_frame_open: false,
            latest_completed: None,
        }
    }

    pub(super) fn feed_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        for byte in bytes {
            self.raw.push(*byte);
            if self.raw.len() > MAX_OUTPUT {
                return Err(io::Error::other("TUI output exceeded witness bound"));
            }
            self.feed(*byte)?;
            if advance_sequence(
                &mut self.synchronized_begin_prefix,
                *byte,
                SYNCHRONIZED_UPDATE_BEGIN,
            ) {
                self.synchronized_frame_open = true;
            }
            let flush_completed = advance_sequence(
                &mut self.synchronized_end_prefix,
                *byte,
                SYNCHRONIZED_UPDATE_END,
            );
            if flush_completed && self.synchronized_frame_open {
                self.synchronized_frame_open = false;
                self.latest_completed = Some(CompletedFrame {
                    cells: self.cells.clone(),
                    marker_offset: self.raw.len(),
                });
            }
        }
        Ok(())
    }

    fn feed(&mut self, byte: u8) -> io::Result<()> {
        if let Some(escape) = self.escape.as_mut() {
            escape.push(byte);
            if escape.len() > 128 {
                return Err(io::Error::other("invalid backend escape"));
            }
            if escape.len() == 1 && byte != b'[' {
                self.escape = None;
            } else if escape.len() >= 2 && (0x40..=0x7e).contains(&byte) {
                let value = String::from_utf8_lossy(escape).into_owned();
                self.escape = None;
                self.apply_escape(&value);
            }
            return Ok(());
        }
        if !self.utf8.is_empty() || byte >= 0x80 {
            self.utf8.push(byte);
            let expected = utf8_length(self.utf8[0]);
            if expected == 0 || self.utf8.len() > expected {
                self.utf8.clear();
                self.put_char('�');
            } else if self.utf8.len() == expected {
                let decoded = std::str::from_utf8(&self.utf8)
                    .ok()
                    .and_then(|value| value.chars().next())
                    .unwrap_or('�');
                self.utf8.clear();
                self.put_char(decoded);
            }
        } else if byte == 0x1b {
            self.escape = Some(Vec::new());
        } else if byte == b'\r' {
            self.x = 0;
        } else if byte == b'\n' {
            self.y = self.y.saturating_add(1);
        } else if byte == 0x08 {
            self.x = self.x.saturating_sub(1);
        } else if byte == b'\t' {
            self.x = (self.x + 8) & !7;
        } else if byte.is_ascii_graphic() || byte == b' ' {
            self.put_char(char::from(byte));
        }
        Ok(())
    }

    fn apply_escape(&mut self, value: &str) {
        let final_byte = value.as_bytes().last().copied();
        let parameters = value
            .strip_prefix('[')
            .and_then(|value| value.get(..value.len().saturating_sub(1)))
            .unwrap_or_default();
        match final_byte {
            Some(b'H' | b'f') => {
                let mut coordinates = parameters.split(';');
                self.y = coordinates
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1);
                self.x = coordinates
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1);
            }
            Some(b'J') => self.clear_display(parameters),
            Some(b'K') => self.clear_line(parameters),
            _ => {}
        }
    }

    fn clear_display(&mut self, parameters: &str) {
        match parameters {
            "" | "0" => {
                self.clear_line_from(self.y, self.x);
                for row in self.cells.iter_mut().skip(self.y.saturating_add(1)) {
                    row.fill(" ".to_owned());
                }
            }
            "1" => {
                for row in self.cells.iter_mut().take(self.y) {
                    row.fill(" ".to_owned());
                }
                self.clear_line_to(self.y, self.x);
            }
            "2" | "3" => {
                for row in &mut self.cells {
                    row.fill(" ".to_owned());
                }
            }
            _ => {}
        }
    }

    fn clear_line(&mut self, parameters: &str) {
        match parameters {
            "" | "0" => self.clear_line_from(self.y, self.x),
            "1" => self.clear_line_to(self.y, self.x),
            "2" => {
                if let Some(row) = self.cells.get_mut(self.y) {
                    row.fill(" ".to_owned());
                }
            }
            _ => {}
        }
    }

    fn clear_line_from(&mut self, y: usize, x: usize) {
        if let Some(row) = self.cells.get_mut(y) {
            for cell in row.iter_mut().skip(x) {
                *cell = " ".to_owned();
            }
        }
    }

    fn clear_line_to(&mut self, y: usize, x: usize) {
        if let Some(row) = self.cells.get_mut(y) {
            for cell in row.iter_mut().take(x.saturating_add(1)) {
                *cell = " ".to_owned();
            }
        }
    }

    fn put_char(&mut self, value: char) {
        let symbol = value.to_string();
        let width = display_width(&symbol).max(1);
        if let Some(cell) = self
            .cells
            .get_mut(self.y)
            .and_then(|row| row.get_mut(self.x))
        {
            cell.clear();
            cell.push(value);
        }
        if let Some(row) = self.cells.get_mut(self.y) {
            for tail in 1..width {
                if let Some(cell) = row.get_mut(self.x.saturating_add(tail)) {
                    cell.clear();
                    cell.push(' ');
                }
            }
        }
        self.x = self.x.saturating_add(width);
    }

    pub(super) fn checkpoint(&self) -> usize {
        self.raw.len()
    }

    pub(super) fn latest_frame(&self) -> Option<&CompletedFrame> {
        self.latest_completed.as_ref()
    }

    pub(super) fn observe_completed_before<T>(
        &self,
        checkpoint: usize,
        deadline: Instant,
        predicate: &mut impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> Option<T> {
        let frame = self
            .latest_completed
            .as_ref()
            .filter(|frame| frame.marker_offset() > checkpoint)?;
        let value = predicate(frame)?;
        (Instant::now() < deadline).then_some(value)
    }

    pub(super) fn working_text(&self) -> String {
        displayed_rows(&self.cells).join("\n")
    }

    pub(super) fn raw_contains(&self, value: &[u8]) -> bool {
        self.raw.windows(value.len()).any(|part| part == value)
    }

    pub(super) fn raw_contains_after(&self, checkpoint: usize, value: &[u8]) -> bool {
        self.raw
            .get(checkpoint.min(self.raw.len())..)
            .is_some_and(|tail| tail.windows(value.len()).any(|part| part == value))
    }

    pub(super) fn raw_since(&self, checkpoint: usize) -> String {
        let start = checkpoint.min(self.raw.len());
        let start = start.max(self.raw.len().saturating_sub(4096));
        format!("{:?}", String::from_utf8_lossy(&self.raw[start..]))
    }
}

impl CompletedFrame {
    /// Finds a dialog read in order: `expected` starts with the exact heading on its top edge,
    /// `┌─ Heading ─…─ [x] ─┐`, and continues with every row between its side borders, trimmed.
    pub(super) fn has_bordered_modal_content(&self, expected: &[ModalRow<'_>]) -> bool {
        let Some((ModalRow::Exact(heading), expected)) = expected.split_first() else {
            return false;
        };
        for (top, row) in self.cells.iter().enumerate() {
            for (left, cell) in row.iter().enumerate() {
                if cell != "┌" {
                    continue;
                }
                let Some(right) = (left + 1..row.len()).find(|index| row[*index] == "┐") else {
                    continue;
                };
                if !dialog_top_edge_matches(&displayed_row(&row[left + 1..right]), heading) {
                    continue;
                }
                let Some(bottom) = (top + 1..self.cells.len()).find(|index| {
                    let row = &self.cells[*index];
                    row.get(left).is_some_and(|cell| cell == "└")
                        && row.get(right).is_some_and(|cell| cell == "┘")
                        && row[left + 1..right].iter().all(|cell| cell == "─")
                }) else {
                    continue;
                };
                let interior = &self.cells[top + 1..bottom];
                if interior.len() != expected.len()
                    || !interior.iter().all(|row| {
                        row.get(left).is_some_and(|cell| cell == "│")
                            && row.get(right).is_some_and(|cell| cell == "│")
                    })
                {
                    continue;
                }
                if interior.iter().zip(expected).all(|(row, expected)| {
                    let text = displayed_row(&row[left + 1..right]);
                    modal_row_matches(text.trim(), *expected)
                }) {
                    return true;
                }
            }
        }
        false
    }

    /// The strip shows exactly these Tabs in order, with the current one at `active_position`.
    /// A label equal to its position names an automatic Tab; see [`shown_label`].
    pub(super) fn has_ordered_tabs(&self, labels: &[&str], active_position: usize) -> bool {
        if labels.is_empty() || !(1..=labels.len()).contains(&active_position) {
            return false;
        }
        self.strip_tokens().is_some_and(|tokens| {
            tokens.len() == labels.len()
                && tokens
                    .iter()
                    .zip(labels)
                    .enumerate()
                    .all(|(index, (token, label))| {
                        token.label == shown_label(index + 1, label)
                            && token.active == (index + 1 == active_position)
                    })
        })
    }

    pub(super) fn has_ordered_navigator_rows(
        &self,
        labels: &[&str],
        active_position: usize,
    ) -> bool {
        if labels.is_empty() || !(1..=labels.len()).contains(&active_position) {
            return false;
        }
        let rows = self.displayed_rows();
        let mut previous = None;
        for (index, label) in labels.iter().enumerate() {
            let position = index + 1;
            let current = if position == active_position {
                '*'
            } else {
                ' '
            };
            let navigation = if position == active_position {
                '›'
            } else {
                ' '
            };
            let token = format!("{navigation} {current} {position} {label}");
            let mut matches = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.contains(&token));
            let Some((row, _)) = matches.next() else {
                return false;
            };
            if matches.next().is_some() || previous.is_some_and(|previous| previous >= row) {
                return false;
            }
            previous = Some(row);
        }
        true
    }

    /// The sidebar names every segment of the location in order, and the strip shows the expected
    /// current Tab, or no Tab at all.
    pub(super) fn has_native_header(&self, expected: WorkbenchHeader<'_>) -> bool {
        let sidebar = self
            .displayed_rows()
            .iter()
            .filter_map(|row| row.split_once('│').map(|(sidebar, _)| sidebar.to_owned()))
            .collect::<Vec<_>>()
            .join("\n");
        let mut rest = sidebar.as_str();
        for segment in expected.location.split(" / ") {
            let Some(index) = rest.find(segment) else {
                return false;
            };
            rest = &rest[index + segment.len()..];
        }
        match expected.tab {
            Some((position, label)) => self.has_active_tab(position, label),
            None => self.strip_tokens().is_none_or(|tokens| tokens.is_empty()),
        }
    }

    /// The strip's current Tab is the one at `position` and shows this label.
    pub(super) fn has_active_tab(&self, position: usize, label: &str) -> bool {
        self.strip_tokens().is_some_and(|tokens| {
            tokens.iter().filter(|token| token.active).count() == 1
                && tokens.get(position.wrapping_sub(1)).is_some_and(|token| {
                    token.active && token.label == shown_label(position, label)
                })
        })
    }

    /// The Tabs of the first row that ends in the New Tab button, in strip order.
    fn strip_tokens(&self) -> Option<Vec<StripToken>> {
        self.displayed_rows()
            .iter()
            .find_map(|row| strip_tokens(row.trim_end()))
    }

    pub(super) fn displayed_rows(&self) -> Vec<String> {
        displayed_rows(&self.cells)
    }

    pub(super) fn prefix_active(&self) -> bool {
        self.displayed_rows()
            .iter()
            .any(|row| row.contains("PREFIX Ctrl+b") || row.contains("前缀 Ctrl+b"))
    }

    /// First `(column, row)` cell whose row tail starts with `needle`.
    pub(super) fn cell_position(&self, needle: &str) -> Option<(u16, u16)> {
        self.cells.iter().enumerate().find_map(|(row, cells)| {
            (0..cells.len())
                .find(|column| cells[*column..].concat().starts_with(needle))
                .map(|column| (column as u16, row as u16))
        })
    }

    pub(super) fn rows(&self) -> Vec<String> {
        self.displayed_rows()
    }

    pub(super) fn text(&self) -> String {
        self.rows().join("\n")
    }

    pub(super) fn marker_offset(&self) -> usize {
        self.marker_offset
    }

    pub(super) fn cells(&self) -> &[Vec<String>] {
        &self.cells
    }
}

/// The name an automatic Tab shows for the fixtures' `/bin/sh` terminal. Journeys name an
/// automatic Tab by its position, which is what such a Tab showed before Tabs took their
/// program's name.
const AUTOMATIC_TAB_LABEL: &str = "sh";
/// The strip's trailing New Tab button and the pad cell before it.
const NEW_TAB_BUTTON: &str = " [+]";
/// The current Tab's pad cell and the close control in its reserved three-cell track.
const CURRENT_TRACK: &str = " [x]";
/// Any other Tab's pad cell and its blank close track.
const BLANK_TRACK: &str = "    ";

/// The label a Tab shows: an automatic Tab, named by its position, shows the fixture shell.
pub(super) fn shown_label(position: usize, label: &str) -> &str {
    if label == position.to_string() {
        AUTOMATIC_TAB_LABEL
    } else {
        label
    }
}

#[derive(Debug, Eq, PartialEq)]
struct StripToken {
    label: String,
    active: bool,
}

/// Parses one strip row: an optional sidebar ending in `│`, then each Tab as a pad cell, its
/// label and its track, one gap cell between Tabs, then the New Tab button. A row that does not
/// parse completely is not a strip.
fn strip_tokens(row: &str) -> Option<Vec<StripToken>> {
    let strip = row.strip_suffix(NEW_TAB_BUTTON)?;
    let mut rest = strip.rsplit_once('│').map_or(strip, |(_, strip)| strip);
    let mut tokens = Vec::new();
    while !rest.is_empty() {
        if !tokens.is_empty() {
            rest = rest.strip_prefix(' ')?;
        }
        rest = rest.strip_prefix(' ')?;
        let current = rest.find(CURRENT_TRACK);
        let blank = rest.find(BLANK_TRACK);
        let (end, active) = match (current, blank) {
            (Some(current), Some(blank)) if blank < current => (blank, false),
            (Some(current), _) => (current, true),
            (None, Some(blank)) => (blank, false),
            (None, None) => return None,
        };
        let label = &rest[..end];
        if label.is_empty() || label.starts_with(' ') {
            return None;
        }
        tokens.push(StripToken {
            label: label.to_owned(),
            active,
        });
        rest = &rest[end + CURRENT_TRACK.len()..];
    }
    Some(tokens)
}

fn advance_sequence(prefix: &mut usize, byte: u8, sequence: &[u8]) -> bool {
    debug_assert!(!sequence.is_empty());
    debug_assert!(*prefix < sequence.len());
    if byte == sequence[*prefix] {
        *prefix += 1;
    } else {
        *prefix = usize::from(byte == sequence[0]);
    }
    if *prefix == sequence.len() {
        *prefix = 0;
        true
    } else {
        false
    }
}

/// The cells between a dialog's top corners: `─ `, the heading, a blank, at least one rule cell,
/// and the spaced close control ` [x] ─`.
fn dialog_top_edge_matches(edge: &str, heading: &str) -> bool {
    edge.strip_prefix("─ ")
        .and_then(|rest| rest.strip_prefix(heading))
        .and_then(|rest| rest.strip_prefix(' '))
        .and_then(|rest| rest.strip_suffix(" [x] ─"))
        .is_some_and(|rule| !rule.is_empty() && rule.chars().all(|cell| cell == '─'))
}

fn modal_row_matches(actual: &str, expected: ModalRow<'_>) -> bool {
    match expected {
        ModalRow::Exact(expected) => actual == expected,
        ModalRow::FieldAtEnd(value) => {
            let Some(content) = actual
                .strip_prefix("[ ")
                .and_then(|actual| actual.strip_suffix(" ]"))
            else {
                return false;
            };
            let width = display_width(content);
            if width == 0 {
                return false;
            }
            let window = editor_end_window(value, width);
            let padding = " ".repeat(width.saturating_sub(display_width(window)));
            content == format!("{window}{padding}")
        }
    }
}

fn editor_end_window(value: &str, width: usize) -> &str {
    let mut scroll = 0;
    while scroll < value.len() && display_width(&value[scroll..]) >= width {
        scroll = boundary_after(value, scroll);
    }
    let mut used = 0;
    let mut end = scroll;
    for (offset, cluster) in value[scroll..].grapheme_indices(true) {
        let cluster_width = display_width(cluster);
        if used + cluster_width > width {
            break;
        }
        used += cluster_width;
        end = scroll + offset + cluster.len();
    }
    &value[scroll..end]
}

fn boundary_after(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    text[index..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(text.len(), |(offset, _)| index + offset)
}

fn displayed_rows(cells: &[Vec<String>]) -> Vec<String> {
    cells
        .iter()
        .map(|row| displayed_row(row).trim_end().to_owned())
        .collect()
}

fn displayed_row(cells: &[String]) -> String {
    let mut text = String::new();
    let mut column = 0;
    while let Some(symbol) = cells.get(column) {
        text.push_str(symbol);
        column += display_width(symbol).max(1);
    }
    text
}

fn display_width(value: &str) -> usize {
    ratatui::text::Line::raw(value).width()
}

fn utf8_length(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first & 0xe0 == 0xc0 {
        2
    } else if first & 0xf0 == 0xe0 {
        3
    } else if first & 0xf8 == 0xf0 {
        4
    } else {
        0
    }
}
