//! `orchbee --desktop-machines`: the desktop window's saved machines, for its Main over pipes.
//! Main writes one JSON object per line on standard input and reads the same on standard output;
//! the page's records travel inside them, never opened by Main. The process holds the machine hub
//! as the TUI does and ends when its standard input ends. A login answer passes through its
//! input: every buffer that held one is wiped once read, and no diagnostic prints a record.
//! `apps/orchbee/docs/machines.md#desktop-machines-process` owns the contract.
use crate::{
    cli::Output,
    desktop_machines::{DesktopMachines, parse_intent, parse_locale},
    localization::Locale,
    tui_capabilities::Capability,
    workbench_client::UiWakeup,
};
use orchbee_machine_protocol::wipe;
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// The longest line Main writes; a longer one is discarded up to its LF.
const MAIN_LINE: usize = 512 * 1024;
/// The longest page record: 65,536 UTF-16 code units are at most this many UTF-8 bytes.
const PAGE_RECORD: usize = 3 * 65_536;
/// The longest record this process sends the page.
const OWN_RECORD: usize = 1 << 20;
/// The largest request number a page may choose, JavaScript's largest safe integer.
const LARGEST_REQUEST: u64 = (1 << 53) - 1;
/// How long the process waits without a wake, so the hub's and the clock's news still arrive.
const IDLE: Duration = Duration::from_millis(250);
/// How long the machine observers and the hub holder may take to stop at the end.
const STOP: Duration = Duration::from_secs(1);

/// One line of Main's.
enum Line {
    /// Whether Desktop has a window, which alone shows the command logins the hub offers.
    Window(bool),
    Attach(String),
    Detach(String),
    Page {
        document: String,
        record: String,
    },
}

pub(crate) fn serve(locale: Locale) -> Output {
    let available = Capability::SavedMachines.require().is_ok();
    let mut out = io::stdout().lock();
    let started = write(
        &mut out,
        &json!({ "version": 1, "kind": "started", "available": available }),
    );
    if started.is_err() || !available {
        return finished(0);
    }
    let wakeup = UiWakeup::current();
    let (sender, lines) = mpsc::channel();
    let reader = wakeup.clone();
    if thread::Builder::new()
        .name("desktop-machines-input".to_owned())
        .spawn(move || read(&sender, &reader))
        .is_err()
    {
        return finished(1);
    }
    let mut machines = DesktopMachines::new(wakeup.clone(), locale);
    let mut page = Page::default();
    'serve: loop {
        let now = Instant::now();
        loop {
            match lines.try_recv() {
                Ok(line) => {
                    if page.receive(line, &mut machines, &mut out, now).is_err() {
                        break 'serve;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // Standard input ended: Main closed it, or Main is gone.
                Err(mpsc::TryRecvError::Disconnected) => break 'serve,
            }
        }
        let view = machines.tick(now);
        if page.offer(view, &mut out).is_err() {
            break;
        }
        #[cfg(unix)]
        wakeup.drain_wait();
        if !wakeup.take() {
            thread::park_timeout(
                machines
                    .due()
                    .saturating_duration_since(Instant::now())
                    .min(IDLE),
            );
        }
    }
    let _ = machines.stop_before(Instant::now() + STOP);
    finished(0)
}

fn finished(status: u8) -> Output {
    Output {
        status,
        stdout: Vec::new(),
        stderr: String::new(),
        finalization: None,
    }
}

/// The one attached document and the view it was sent.
#[derive(Default)]
struct Page {
    document: Option<String>,
    /// The number of the last view sent in the process's life.
    sequence: u64,
    /// The view the page has not acknowledged; the next waits for it.
    awaiting: Option<u64>,
    /// The last view sent to the attached document.
    sent: Option<Value>,
}

impl Page {
    fn receive(
        &mut self,
        line: Line,
        machines: &mut DesktopMachines,
        out: &mut impl Write,
        now: Instant,
    ) -> io::Result<()> {
        match line {
            Line::Window(exists) => machines.set_window(exists),
            // A new document replaces the attached one and is sent a whole view.
            Line::Attach(document) => {
                self.document = Some(document);
                self.awaiting = None;
                self.sent = None;
                machines.watch_capacity(false);
            }
            Line::Detach(document) => {
                if self.document.as_deref() == Some(document.as_str()) {
                    self.document = None;
                    self.awaiting = None;
                    self.sent = None;
                    machines.watch_capacity(false);
                }
            }
            // A record of a document no longer attached is never carried out.
            Line::Page { document, record } => {
                let carried = if self.document.as_deref() == Some(document.as_str()) {
                    self.record(&record, machines, out, now)
                } else {
                    Ok(())
                };
                wipe_text(record);
                return carried;
            }
        }
        Ok(())
    }

    fn record(
        &mut self,
        record: &str,
        machines: &mut DesktopMachines,
        out: &mut impl Write,
        now: Instant,
    ) -> io::Result<()> {
        let Some(mut record) = serde_json::from_str::<Value>(record)
            .ok()
            .filter(|record| record.get("version").and_then(Value::as_u64) == Some(1))
        else {
            eprintln!("desktop-machines: dropped a page record of another shape");
            return Ok(());
        };
        match record.get("kind").and_then(Value::as_str) {
            Some("ack") => {
                if record.get("sequence").and_then(Value::as_u64) == self.awaiting {
                    self.awaiting = None;
                }
            }
            Some("locale") => {
                if let Some(locale) = record
                    .get("locale")
                    .and_then(Value::as_str)
                    .and_then(parse_locale)
                {
                    machines.set_locale(locale);
                }
            }
            Some("watch") => {
                if let Some(watching) = record.get("capacity").and_then(Value::as_bool) {
                    machines.watch_capacity(watching);
                }
            }
            Some("intent") => {
                let Some(request) = record
                    .get("request")
                    .and_then(Value::as_u64)
                    .filter(|request| (1..=LARGEST_REQUEST).contains(request))
                else {
                    eprintln!("desktop-machines: dropped an intent without a request number");
                    return Ok(());
                };
                let outcome = match record.get_mut("intent").and_then(parse_intent) {
                    Some(intent) => {
                        machines.carry(intent, now);
                        json!({ "result": "done" })
                    }
                    None => json!({ "result": "refused", "reason": "malformed" }),
                };
                self.send(
                    out,
                    &json!({ "version": 1, "kind": "outcome", "request": request, "outcome": outcome }),
                )?;
            }
            _ => eprintln!("desktop-machines: dropped a page record of an unknown kind"),
        }
        Ok(())
    }

    /// Sends `view` when the page holds no unacknowledged one and it changed since the last.
    fn offer(&mut self, view: Value, out: &mut impl Write) -> io::Result<()> {
        if self.document.is_none() || self.awaiting.is_some() || self.sent.as_ref() == Some(&view) {
            return Ok(());
        }
        self.sequence += 1;
        self.send(
            out,
            &json!({ "version": 1, "kind": "view", "sequence": self.sequence, "view": view }),
        )?;
        self.awaiting = Some(self.sequence);
        self.sent = Some(view);
        Ok(())
    }

    fn send(&self, out: &mut impl Write, record: &Value) -> io::Result<()> {
        let Some(document) = &self.document else {
            return Ok(());
        };
        let record = record.to_string();
        if record.len() > OWN_RECORD {
            eprintln!("desktop-machines: dropped a page record over its bound");
            return Ok(());
        }
        write(
            out,
            &json!({ "version": 1, "kind": "page", "document": document, "record": record }),
        )
    }
}

fn write(out: &mut impl Write, line: &Value) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(line)?;
    bytes.push(b'\n');
    out.write_all(&bytes)?;
    out.flush()
}

/// Reads Main's lines until standard input ends; dropping `sender` then says so.
fn read(sender: &mpsc::Sender<Line>, wakeup: &UiWakeup) {
    let mut input = io::stdin().lock();
    let mut buffer = Vec::new();
    loop {
        wipe(&mut buffer);
        buffer.clear();
        match read_line(&mut input, &mut buffer) {
            Ok(Some(true)) => match parse(&buffer) {
                Some(line) => {
                    if sender.send(line).is_err() {
                        return;
                    }
                    wakeup.signal();
                }
                None => eprintln!("desktop-machines: dropped a line of another shape"),
            },
            Ok(Some(false)) => eprintln!("desktop-machines: dropped a line over its bound"),
            Ok(None) | Err(_) => break,
        }
    }
    wipe(&mut buffer);
    wakeup.signal();
}

/// Wipes `text`, which may hold a login answer, as it goes.
fn wipe_text(text: String) {
    wipe(&mut text.into_bytes());
}

/// One LF-ended line of at most [`MAIN_LINE`] bytes, `Some(false)` for a longer one, which is
/// discarded up to its LF, and `None` at the end; an unended last line is no line.
fn read_line(input: &mut impl BufRead, buffer: &mut Vec<u8>) -> io::Result<Option<bool>> {
    let mut fits = true;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(None);
        }
        let end = available.iter().position(|&byte| byte == b'\n');
        let take = end.unwrap_or(available.len());
        if fits && buffer.len() + take <= MAIN_LINE {
            buffer.extend_from_slice(&available[..take]);
        } else {
            fits = false;
            wipe(buffer);
            buffer.clear();
        }
        input.consume(end.map_or(take, |end| end + 1));
        if end.is_some() {
            return Ok(Some(fits));
        }
    }
}

fn parse(bytes: &[u8]) -> Option<Line> {
    let mut line: Value = serde_json::from_slice(bytes).ok()?;
    if line.get("version")?.as_u64()? != 1 {
        return None;
    }
    let kind = line.get("kind")?.as_str()?.to_owned();
    let text = |name: &str| line.get(name)?.as_str().map(str::to_owned);
    match kind.as_str() {
        "window" => line.get("exists")?.as_bool().map(Line::Window),
        "attach" => Some(Line::Attach(text("document")?)),
        "detach" => Some(Line::Detach(text("document")?)),
        // The record is taken out of the line, so no second copy of an answer stays behind.
        "page" => {
            let document = text("document")?;
            let Value::String(record) = line.get_mut("record")?.take() else {
                return None;
            };
            if record.len() > PAGE_RECORD {
                wipe_text(record);
                return None;
            }
            Some(Line::Page { document, record })
        }
        _ => None,
    }
}
