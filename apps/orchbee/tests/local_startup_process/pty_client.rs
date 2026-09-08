use super::native_cohort::Fixture;
use nix::{
    fcntl::{FcntlArg, OFlag, fcntl, open},
    pty::{grantpt, posix_openpt, ptsname_r, unlockpt},
    sys::stat::Mode,
    sys::termios::{Termios, tcgetattr},
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::fd::{AsFd, OwnedFd},
    path::Path,
    process::{Child, ChildStderr, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const COLUMNS: usize = 120;
const ROWS: usize = 32;
const MAX_OUTPUT: usize = 8 * 1024 * 1024;
const COMPLETE_DRAW: &[u8] = b"\x1b[0m\x1b[0m";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadyMode {
    Controlling,
    ViewOnly,
}

impl ReadyMode {
    fn footer(self, tab_number: usize) -> String {
        let mode = match self {
            Self::Controlling => "controlling",
            Self::ViewOnly => "view only",
        };
        format!("Tab {tab_number} · Ready · {mode}")
    }
}

pub(super) struct Client {
    child: Child,
    master: File,
    slave: OwnedFd,
    original: Termios,
    stderr: ChildStderr,
    stderr_bytes: Vec<u8>,
    screen: Screen,
}

impl Client {
    pub(super) fn spawn(fixture: &Fixture, locale: &str) -> Self {
        Self::spawn_at(fixture, locale, &fixture.base)
    }

    pub(super) fn spawn_at(fixture: &Fixture, locale: &str, base: &Path) -> Self {
        fixture.register_runtime_base(base);
        let pty = open_outer_pty().unwrap();
        let original = tcgetattr(&pty.slave).unwrap();
        let mut child = Command::new(fixture.cohort.join("orchbee"))
            .env("ORCHBEE_RUNTIME_BASE", base)
            .env("LC_ALL", locale)
            .env("TERM", "xterm-256color")
            .env("SHELL", "/bin/sh")
            .stdin(Stdio::from(File::from(clone_cloexec(&pty.slave).unwrap())))
            .stdout(Stdio::from(File::from(clone_cloexec(&pty.slave).unwrap())))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = child.stderr.take().unwrap();
        fcntl(&stderr, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
        Self {
            child,
            master: File::from(pty.master),
            slave: pty.slave,
            original,
            stderr,
            stderr_bytes: Vec::new(),
            screen: Screen::new(COLUMNS, ROWS),
        }
    }

    pub(super) fn collect(&mut self) {
        let mut bytes = [0; 16_384];
        loop {
            match self.master.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => self.screen.feed_all(&bytes[..count]).unwrap(),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) if error.raw_os_error() == Some(nix::libc::EIO) => break,
                Err(error) => panic!("PTY read: {error}"),
            }
        }
        loop {
            match self.stderr.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    self.stderr_bytes.extend_from_slice(&bytes[..count]);
                    assert!(
                        self.stderr_bytes.len() <= MAX_OUTPUT,
                        "Client stderr exceeded bound"
                    );
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) => panic!("Client stderr read: {error}"),
            }
        }
    }

    pub(super) fn contains(&self, text: &str) -> bool {
        if text.starts_with('\x1b') {
            self.screen.raw_contains(text.as_bytes())
        } else {
            self.screen
                .latest_frame()
                .is_some_and(|frame| frame.text().contains(text))
        }
    }

    #[track_caller]
    pub(super) fn ready_before(&mut self, deadline: Instant) -> ReadyMode {
        self.observe_after(
            0,
            deadline,
            "first Tab and Ready frame",
            CompletedFrame::initial_ready_mode,
        )
    }

    pub(super) fn checkpoint(&mut self) -> usize {
        self.collect();
        self.screen.checkpoint()
    }

    #[track_caller]
    pub(super) fn wait_ready_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        tab_number: usize,
        safe_label: &str,
        mode: ReadyMode,
    ) {
        let active = format!("[*{tab_number} {safe_label}]");
        let footer = mode.footer(tab_number);
        self.observe_after(
            checkpoint,
            deadline,
            "selected Tab and Ready frame",
            |screen| {
                let text = screen.text();
                (text.contains(&active) && text.contains(&footer)).then_some(())
            },
        );
    }

    #[track_caller]
    pub(super) fn wait_navigator_refreshed_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
    ) -> u64 {
        self.wait_ordered_navigator_after(checkpoint, deadline, &["1", "2"], 1)
    }

    #[track_caller]
    pub(super) fn wait_ordered_navigator_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        labels: &[&str],
        active_position: usize,
    ) -> u64 {
        self.observe_after(
            checkpoint,
            deadline,
            "refreshed Workbench navigator",
            |screen| {
                let rows = screen.rows();
                if !screen.has_ordered_tabs(labels, active_position)
                    || !rows.iter().any(|row| row.contains("Workbench navigator"))
                {
                    return None;
                }
                let mut previous_row = None;
                for (index, label) in labels.iter().enumerate() {
                    let position = index + 1;
                    let marker = if position == active_position {
                        '*'
                    } else {
                        ' '
                    };
                    let token = format!("[{marker}{position}] {label}");
                    let mut matches = rows
                        .iter()
                        .enumerate()
                        .filter(|(_, row)| row.contains(&token));
                    let (row, _) = matches.next()?;
                    if matches.next().is_some()
                        || previous_row.is_some_and(|previous| previous >= row)
                    {
                        return None;
                    }
                    previous_row = Some(row);
                }
                rows.iter().find_map(|row| {
                    let suffix = row.split_once("Tabs refreshed · revision ")?.1;
                    let digits = suffix
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>();
                    (!digits.is_empty())
                        .then(|| digits.parse::<u64>().ok())
                        .flatten()
                })
            },
        )
    }

    #[track_caller]
    pub(super) fn wait_ordered_ready_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        labels: &[&str],
        active_position: usize,
        mode: ReadyMode,
    ) {
        let footer = mode.footer(active_position);
        self.observe_after(
            checkpoint,
            deadline,
            "ordered Tabs and Ready frame",
            |screen| {
                (screen.has_ordered_tabs(labels, active_position)
                    && screen.text().contains(&footer))
                .then_some(())
            },
        );
    }

    #[track_caller]
    pub(super) fn wait_terminal_output_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        output: &str,
    ) {
        self.observe_after(checkpoint, deadline, "fresh shell output frame", |screen| {
            screen.text().contains(output).then_some(())
        });
    }

    // Keep the observer's fixed 120x32 capacity; this witness only shrinks the viewport.
    pub(super) fn resize_rows(&mut self, rows: u16) {
        assert!(rows > 0 && usize::from(rows) <= ROWS);
        rustix::termios::tcsetwinsize(
            &self.slave,
            rustix::termios::Winsize {
                ws_col: COLUMNS as u16,
                ws_row: rows,
                ws_xpixel: 960,
                ws_ypixel: rows * 20,
            },
        )
        .unwrap();
    }

    #[track_caller]
    pub(super) fn wait_resized_ready_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        rows: u16,
        active_position: usize,
    ) {
        let footer = ReadyMode::Controlling.footer(active_position);
        self.observe_after(checkpoint, deadline, "resized Ready footer", |screen| {
            screen
                .rows()
                .get(usize::from(rows) - 1)
                .is_some_and(|row| row.contains(&footer))
                .then_some(())
        });
    }

    #[track_caller]
    pub(super) fn wait_navigator_closed_after(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        tab_number: usize,
        safe_label: &str,
        mode: ReadyMode,
    ) {
        let active = format!("[*{tab_number} {safe_label}]");
        let footer = mode.footer(tab_number);
        self.observe_after(
            checkpoint,
            deadline,
            "closed Workbench navigator",
            |screen| {
                let text = screen.text();
                (!text.contains("Workbench navigator")
                    && !text.contains("[*1] 1")
                    && !text.contains("[ 2] 2")
                    && !text.contains("Tabs refreshed · revision ")
                    && text.contains(&active)
                    && text.contains(&footer))
                .then_some(())
            },
        );
    }

    pub(super) fn current_row_containing(&mut self, needle: &str) -> String {
        self.collect();
        self.screen
            .latest_frame()
            .map(CompletedFrame::rows)
            .unwrap_or_default()
            .into_iter()
            .find(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("no current row contains {needle:?}; {}", self.diagnostic(0)))
    }

    pub(super) fn send(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).unwrap();
    }

    #[track_caller]
    pub(super) fn exit_message_before(&mut self, deadline: Instant) -> String {
        loop {
            self.collect();
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    Instant::now() < deadline,
                    "Client exited after deadline; {}",
                    self.diagnostic(0)
                );
                assert_eq!(status.code(), Some(1));
                self.assert_termios_restored();
                assert!(!self.contains("\x1b[?1049h"));
                return String::from_utf8_lossy(&self.stderr_bytes).into_owned();
            }
            assert!(
                Instant::now() < deadline,
                "Client did not exit; {}",
                self.diagnostic(0)
            );
            thread::yield_now();
        }
    }

    #[track_caller]
    pub(super) fn close_before(&mut self, deadline: Instant) {
        self.send(b"\x02q");
        loop {
            self.collect();
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    Instant::now() < deadline,
                    "Client exited after deadline; {}",
                    self.diagnostic(0)
                );
                assert!(
                    status.success(),
                    "Client exited {status}; {}",
                    self.diagnostic(0)
                );
                self.assert_termios_restored();
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Client did not exit; {}",
                self.diagnostic(0)
            );
            thread::yield_now();
        }
    }

    fn observe_after<T>(
        &mut self,
        checkpoint: usize,
        deadline: Instant,
        description: &str,
        mut predicate: impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> T {
        loop {
            self.collect();
            if let Some(value) =
                self.screen
                    .observe_completed_before(checkpoint, deadline, &mut predicate)
            {
                return value;
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                panic!(
                    "Client exited {status} before {description}; {}",
                    self.diagnostic(checkpoint)
                );
            }
            assert!(
                Instant::now() < deadline,
                "deadline waiting for {description}; {}",
                self.diagnostic(checkpoint)
            );
            thread::yield_now();
        }
    }

    fn diagnostic(&self, checkpoint: usize) -> String {
        format!(
            "latest completed screen:\n{}\nworking screen:\n{}\nstderr: {:?}\nraw since checkpoint: {}",
            self.screen
                .latest_frame()
                .map(CompletedFrame::text)
                .unwrap_or_default(),
            self.screen.working_text(),
            String::from_utf8_lossy(&self.stderr_bytes),
            self.screen.raw_since(checkpoint)
        )
    }

    fn assert_termios_restored(&self) {
        assert_eq!(tcgetattr(&self.slave).unwrap(), self.original);
    }

    fn status(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().unwrap()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.status().is_none() {
            let _result = self.child.kill();
        }
        let _result = self.child.wait();
    }
}

struct OuterPty {
    master: OwnedFd,
    slave: OwnedFd,
}

fn open_outer_pty() -> io::Result<OuterPty> {
    let master =
        posix_openpt(OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK)
            .map_err(io::Error::from)?;
    grantpt(&master).map_err(io::Error::from)?;
    unlockpt(&master).map_err(io::Error::from)?;
    let slave_name = ptsname_r(&master).map_err(io::Error::from)?;
    let slave = open(
        Path::new(&slave_name),
        OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    rustix::termios::tcsetwinsize(
        &slave,
        rustix::termios::Winsize {
            ws_col: COLUMNS as u16,
            ws_row: ROWS as u16,
            ws_xpixel: 960,
            ws_ypixel: 640,
        },
    )?;
    Ok(OuterPty {
        master: master.into(),
        slave,
    })
}

fn clone_cloexec(descriptor: &OwnedFd) -> io::Result<OwnedFd> {
    descriptor.as_fd().try_clone_to_owned()
}

pub(super) fn wait_file(path: &Path, clients: &mut [&mut Client], deadline: Instant) -> String {
    loop {
        for client in clients.iter_mut() {
            client.collect();
            if let Some(status) = client.status() {
                panic!(
                    "Client exited {status} before shell evidence {}; {}",
                    path.display(),
                    client.diagnostic(0)
                );
            }
        }
        if let Some(value) = nonempty_file_before(path, deadline) {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "missing shell evidence {}; {}",
            path.display(),
            clients
                .iter()
                .map(|client| client.diagnostic(0))
                .collect::<Vec<_>>()
                .join("\n")
        );
        thread::yield_now();
    }
}

fn nonempty_file_before(path: &Path, deadline: Instant) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    (!value.is_empty() && Instant::now() < deadline).then_some(value)
}

// A test-only observer for this backend's absolute cursor writes and CSI modes, not a product VT.
struct Screen {
    cells: Vec<Vec<String>>,
    x: usize,
    y: usize,
    escape: Option<Vec<u8>>,
    utf8: Vec<u8>,
    raw: Vec<u8>,
    complete_draw_prefix: usize,
    latest_completed: Option<CompletedFrame>,
}

struct CompletedFrame {
    cells: Vec<Vec<String>>,
    marker_offset: usize,
}

impl Screen {
    fn new(columns: usize, rows: usize) -> Self {
        Self {
            cells: vec![vec![" ".to_owned(); columns]; rows],
            x: 0,
            y: 0,
            escape: None,
            utf8: Vec::new(),
            raw: Vec::new(),
            complete_draw_prefix: 0,
            latest_completed: None,
        }
    }

    fn feed_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        for byte in bytes {
            self.raw.push(*byte);
            if self.raw.len() > MAX_OUTPUT {
                return Err(io::Error::other("TUI output exceeded witness bound"));
            }
            self.feed(*byte)?;
            if self.advance_complete_draw(*byte) {
                self.latest_completed = Some(CompletedFrame {
                    cells: self.cells.clone(),
                    marker_offset: self.raw.len(),
                });
            }
        }
        Ok(())
    }

    fn advance_complete_draw(&mut self, byte: u8) -> bool {
        if byte == COMPLETE_DRAW[self.complete_draw_prefix] {
            self.complete_draw_prefix += 1;
        } else {
            self.complete_draw_prefix = usize::from(byte == COMPLETE_DRAW[0]);
        }
        if self.complete_draw_prefix == COMPLETE_DRAW.len() {
            self.complete_draw_prefix = 0;
            true
        } else {
            false
        }
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
        if let Some(cell) = self
            .cells
            .get_mut(self.y)
            .and_then(|row| row.get_mut(self.x))
        {
            cell.clear();
            cell.push(value);
        }
        self.x = self.x.saturating_add(1);
    }

    fn checkpoint(&self) -> usize {
        self.raw.len()
    }

    fn latest_frame(&self) -> Option<&CompletedFrame> {
        self.latest_completed.as_ref()
    }

    fn observe_completed_before<T>(
        &self,
        checkpoint: usize,
        deadline: Instant,
        predicate: &mut impl FnMut(&CompletedFrame) -> Option<T>,
    ) -> Option<T> {
        let frame = self
            .latest_completed
            .as_ref()
            .filter(|frame| frame.marker_offset > checkpoint)?;
        let value = predicate(frame)?;
        (Instant::now() < deadline).then_some(value)
    }

    fn working_text(&self) -> String {
        rows(&self.cells).join("\n")
    }

    fn raw_contains(&self, value: &[u8]) -> bool {
        self.raw.windows(value.len()).any(|part| part == value)
    }

    fn raw_since(&self, checkpoint: usize) -> String {
        let start = checkpoint.min(self.raw.len());
        let start = start.max(self.raw.len().saturating_sub(4096));
        format!("{:?}", String::from_utf8_lossy(&self.raw[start..]))
    }
}

impl CompletedFrame {
    fn has_ordered_tabs(&self, labels: &[&str], active_position: usize) -> bool {
        let expected = labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let position = index + 1;
                let marker = if position == active_position {
                    '*'
                } else {
                    ' '
                };
                format!("[{marker}{position} {label}]")
            })
            .collect::<Vec<_>>()
            .join(" ");
        !labels.is_empty()
            && self.rows().iter().any(|row| {
                row.contains(&expected)
                    && row.matches('[').count() == labels.len()
                    && row.matches("[*").count() == 1
            })
    }

    fn initial_ready_mode(&self) -> Option<ReadyMode> {
        let text = self.text();
        if !text.contains("shortcuts") || !text.contains("[*1 1]") {
            return None;
        }
        [ReadyMode::Controlling, ReadyMode::ViewOnly]
            .into_iter()
            .find(|mode| text.contains(&mode.footer(1)))
    }

    fn rows(&self) -> Vec<String> {
        rows(&self.cells)
    }

    fn text(&self) -> String {
        self.rows().join("\n")
    }
}

fn rows(cells: &[Vec<String>]) -> Vec<String> {
    cells
        .iter()
        .map(|row| row.concat().trim_end().to_owned())
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{FcntlArg, FdFlag, OFlag, fcntl};

    #[test]
    fn ordered_tab_observer_rejects_stale_order_missing_members_and_extra_selection() {
        for (strip, expected) in [
            ("[ 1 3] [*2 1] [ 3 2]", true),
            ("[*1 1] [ 2 2] [ 3 3]", false),
            ("[ 1 3] [*2 1]", false),
            ("[ 1 3] [*2 1] [ 3 2] [*4 4]", false),
            ("[ 1 3] [*2 1] [ 3 2] [ 4 4]", false),
            ("[ 1 3] [ 2 1] [*3 2]", false),
        ] {
            let mut screen = Screen::new(COLUMNS, ROWS);
            screen
                .feed_all(format!("\x1b[1;1H{strip}\x1b[0m\x1b[0m").as_bytes())
                .unwrap();
            assert_eq!(
                screen
                    .latest_frame()
                    .unwrap()
                    .has_ordered_tabs(&["3", "1", "2"], 2),
                expected,
                "{strip}"
            );
        }
    }

    #[test]
    fn initial_ready_requires_the_first_active_tab_and_native_ready_mode() {
        for (active, footer, expected) in [
            (
                "[*1 1]",
                "Tab 1 · Ready · controlling",
                Some(ReadyMode::Controlling),
            ),
            (
                "[*1 1]",
                "Tab 1 · Ready · view only",
                Some(ReadyMode::ViewOnly),
            ),
            ("[*1 1]", "Terminal connected", None),
            ("[*1 1]", "Viewing only", None),
            ("[*1 1]", "Tab 1 · Connecting…", None),
            ("[*2 2]", "Tab 1 · Ready · controlling", None),
            ("[*1 1]", "Tab 2 · Ready · controlling", None),
        ] {
            let mut screen = Screen::new(COLUMNS, ROWS);
            screen
                .feed_all(
                    format!("\x1b[1;1H{active}\x1b[31;1Hshortcuts\x1b[32;1H{footer}\x1b[0m\x1b[0m")
                        .as_bytes(),
                )
                .unwrap();
            assert_eq!(
                screen.latest_frame().unwrap().initial_ready_mode(),
                expected,
                "{active}: {footer}"
            );
        }
    }

    #[test]
    fn fragmented_complete_draw_rejects_a_later_half_draw_hybrid() {
        let mut screen = Screen::new(COLUMNS, ROWS);
        let complete = concat!(
            "\x1b[1;1H[ 1 1] [*2 2]",
            "\x1b[32;1HTab 2 · Ready · controlling",
            "\x1b[0m\x1b[0m"
        )
        .as_bytes();
        for byte in complete {
            screen.feed_all(std::slice::from_ref(byte)).unwrap();
        }
        let checkpoint = screen.checkpoint();
        let frame = screen.latest_frame().unwrap();
        assert_eq!(frame.marker_offset, complete.len());
        assert!(frame.text().contains("[*2 2]"));
        assert!(frame.text().contains("Tab 2 · Ready · controlling"));

        let partial = concat!("\x1b[1;1H[*1 1] [ 2 2]", "\x1b[32;5H1").as_bytes();
        screen.feed_all(partial).unwrap();
        assert!(screen.working_text().contains("[*1 1]"));
        assert!(
            screen
                .working_text()
                .contains("Tab 1 · Ready · controlling")
        );
        let mut false_ready = |frame: &CompletedFrame| {
            let text = frame.text();
            (text.contains("[*1 1]") && text.contains("Tab 1 · Ready · controlling")).then_some(())
        };
        assert!(
            screen
                .observe_completed_before(
                    checkpoint,
                    Instant::now() + Duration::from_secs(1),
                    &mut false_ready,
                )
                .is_none()
        );
    }

    #[test]
    fn complete_frame_snapshot_ignores_next_partial_draw_in_the_same_batch() {
        let mut screen = Screen::new(COLUMNS, ROWS);
        let complete = concat!(
            "\x1b[1;1H[*1 1] [ 2 2]",
            "\x1b[32;1HTab 1 · Ready · controlling",
            "\x1b[0m\x1b[0m"
        );
        let partial = concat!("\x1b[1;1H[ 1 1] [*2 2]", "\x1b[32;5H2");
        screen
            .feed_all(format!("{complete}{partial}").as_bytes())
            .unwrap();

        let frame = screen.latest_frame().unwrap();
        assert_eq!(frame.marker_offset, complete.len());
        assert!(frame.text().contains("[*1 1]"));
        assert!(frame.text().contains("Tab 1 · Ready · controlling"));
        assert!(screen.working_text().contains("[*2 2]"));
        assert!(
            screen
                .working_text()
                .contains("Tab 2 · Ready · controlling")
        );

        screen
            .feed_all(b"\x1b[32;1H\x1b[2KTab 2 - Switching\x1b[0m\x1b[0m")
            .unwrap();
        let mut stale_ready = |frame: &CompletedFrame| {
            frame
                .text()
                .contains("Tab 1 · Ready · controlling")
                .then_some(())
        };
        assert!(
            screen
                .observe_completed_before(
                    0,
                    Instant::now() + Duration::from_secs(1),
                    &mut stale_ready,
                )
                .is_none(),
            "an older complete Ready frame must not beat the latest complete frame"
        );
    }

    #[test]
    fn expired_deadline_rejects_preloaded_screen_and_file_observations() {
        let mut screen = Screen::new(COLUMNS, ROWS);
        screen.feed_all(b"\x1b[1;1Hready\x1b[0m\x1b[0m").unwrap();
        let expired = Instant::now() - Duration::from_secs(1);
        let mut predicate = |frame: &CompletedFrame| frame.text().contains("ready").then_some(());
        assert!(
            screen
                .observe_completed_before(0, expired, &mut predicate)
                .is_none()
        );
        assert!(nonempty_file_before(Path::new("/proc/self/cmdline"), expired).is_none());
    }

    #[test]
    fn outer_pty_and_stdio_clones_are_cloexec_from_creation() {
        let pty = open_outer_pty().unwrap();
        let stdin = clone_cloexec(&pty.slave).unwrap();
        let stdout = clone_cloexec(&pty.slave).unwrap();
        for descriptor in [&pty.master, &pty.slave, &stdin, &stdout] {
            let flags = FdFlag::from_bits_truncate(fcntl(descriptor, FcntlArg::F_GETFD).unwrap());
            assert!(flags.contains(FdFlag::FD_CLOEXEC));
        }
        let master_flags =
            OFlag::from_bits_truncate(fcntl(&pty.master, FcntlArg::F_GETFL).unwrap());
        assert!(master_flags.contains(OFlag::O_NONBLOCK));
        assert_eq!(
            rustix::termios::tcgetwinsize(&pty.slave).unwrap(),
            rustix::termios::Winsize {
                ws_col: COLUMNS as u16,
                ws_row: ROWS as u16,
                ws_xpixel: 960,
                ws_ypixel: 640,
            }
        );
    }
}
