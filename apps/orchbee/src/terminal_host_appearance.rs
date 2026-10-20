//! One bounded outer-terminal default-color probe; replies never enter pane or editor input.

use std::fmt;
#[cfg(windows)]
use std::io;
use std::time::{Duration, Instant};

const QUERY_DEADLINE: Duration = Duration::from_millis(350);
const OSC_FRAME_LIMIT: usize = 96;
const OSC_DRAIN_LIMIT: usize = OSC_FRAME_LIMIT + 2;
const OSC_DRAIN_IDLE: Duration = Duration::from_millis(50);
const OSC_DRAIN_WINDOW: Duration = Duration::from_secs(1);

/// The outer host's observed background polarity, independent of the System theme fallback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum HostAppearance {
    Dark,
    Light,
    #[default]
    Unknown,
}

/// Colors captured from the outer terminal, never from a retained child frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostColors {
    pub(crate) foreground: Option<u32>,
    pub(crate) background: Option<u32>,
}

/// Which dynamic color a terminal returned for one OSC query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColorTarget {
    Foreground,
    Background,
}

/// One complete OSC color reply. `None` means the response was malformed or over-bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ColorReply {
    pub(crate) target: ColorTarget,
    pub(crate) rgb: Option<[u8; 3]>,
}

/// The one ordered query for the host's default foreground and background colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Query;

impl Query {
    /// Sends both default-color queries without changing the host terminal's state.
    pub(crate) const fn sequence(self) -> &'static str {
        "\x1b]10;?\x1b\\\x1b]11;?\x1b\\"
    }
}

impl crossterm::Command for Query {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }

    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(self.sequence())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeState {
    Unissued,
    Pending { deadline: Instant },
    FallbackPending,
    Settled(HostAppearance),
}

/// One admitted terminal endpoint's irreversible, bounded appearance observation.
#[derive(Debug)]
pub(crate) struct Probe {
    state: ProbeState,
    colors: HostColors,
    reply_deadline: Option<Instant>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            state: ProbeState::Unissued,
            colors: HostColors::default(),
            reply_deadline: None,
        }
    }
}

/// The query deadline expired without a valid OSC 11 background.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Timeout;

impl Probe {
    /// Begins this endpoint's single query. Later calls are permanent no-ops.
    pub(crate) fn begin(&mut self, now: Instant) -> Option<Query> {
        if self.state != ProbeState::Unissued {
            return None;
        }
        self.state = ProbeState::Pending {
            deadline: now + QUERY_DEADLINE,
        };
        self.reply_deadline = Some(now + QUERY_DEADLINE);
        Some(Query)
    }

    pub(crate) const fn colors(&self) -> HostColors {
        self.colors
    }

    /// Capture each queried component once inside the original bounded window.
    /// Background may arrive first; its polarity settles immediately, while a
    /// later foreground can complete the same observation without changing it.
    pub(crate) fn observe(&mut self, reply: ColorReply, now: Instant) -> Option<HostAppearance> {
        if !self.reply_deadline.is_some_and(|deadline| now < deadline) {
            return None;
        }
        let [red, green, blue] = reply.rgb?;
        let rgb = (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue);
        match reply.target {
            ColorTarget::Foreground if self.colors.foreground.is_none() => {
                self.colors.foreground = Some(rgb);
                match self.state {
                    ProbeState::Settled(appearance) => Some(appearance),
                    _ => None,
                }
            }
            ColorTarget::Background if self.colors.background.is_none() => {
                self.colors.background = Some(rgb);
                let appearance = classify_background(red, green, blue);
                self.state = ProbeState::Settled(appearance);
                Some(appearance)
            }
            _ => None,
        }
    }

    /// Closes the reply window at the absolute deadline; late replies remain consumable but inert.
    pub(crate) fn timeout(&mut self, now: Instant) -> Option<Timeout> {
        let ProbeState::Pending { deadline } = self.state else {
            return None;
        };
        if now < deadline {
            return None;
        }
        self.state = ProbeState::FallbackPending;
        self.reply_deadline = None;
        Some(Timeout)
    }

    /// Settles the one-shot platform fallback after the query deadline.
    pub(crate) fn settle_fallback(&mut self, appearance: HostAppearance) -> Option<HostAppearance> {
        if self.state != ProbeState::FallbackPending {
            return None;
        }
        self.state = ProbeState::Settled(appearance);
        Some(appearance)
    }
}

/// Maps the Windows current-user app-theme registry value; invalid or missing stays unknown.
#[cfg(any(windows, test))]
pub(crate) const fn from_windows_apps_use_light_theme(light: Option<bool>) -> HostAppearance {
    match light {
        Some(true) => HostAppearance::Light,
        Some(false) => HostAppearance::Dark,
        None => HostAppearance::Unknown,
    }
}

/// A bounded OSC prefix and color payload collected by either host input decoder.
#[derive(Debug)]
pub(crate) struct OscReplyFrame {
    selector: Vec<u8>,
    target: Option<ColorTarget>,
    payload: Vec<u8>,
    byte_count: usize,
    malformed: bool,
    escape_pending: bool,
    deadline: Instant,
}

/// One parser transition; `NotColorReply` bytes follow the introducer and must be decoded normally.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OscStep {
    Pending,
    NotColorReply(Vec<u8>),
    OverBound(ColorTarget),
    Complete(ColorReply),
}

/// A payload-free, bounded resynchronizer for a targeted OSC reply tail.
#[derive(Debug)]
pub(crate) struct OscDrain {
    escape_pending: bool,
    bytes_left: usize,
    idle_deadline: Instant,
    deadline: Instant,
}

/// One bounded drain transition; `Reprocess` leaves the current byte to ordinary input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OscDrainStep {
    Pending,
    Finished,
    Reprocess,
}

impl OscDrain {
    fn new(now: Instant, escape_pending: bool) -> Self {
        Self {
            escape_pending,
            bytes_left: OSC_DRAIN_LIMIT,
            idle_deadline: now + OSC_DRAIN_IDLE,
            deadline: now + OSC_DRAIN_WINDOW,
        }
    }

    /// Whether this bounded recovery window has ended.
    pub(crate) fn expired(&self, now: Instant) -> bool {
        now >= self.idle_deadline || now >= self.deadline
    }

    /// Consumes a possible tail terminator and returns fresh bytes for normal decoding.
    pub(crate) fn push(&mut self, byte: u8, now: Instant) -> OscDrainStep {
        if self.expired(now) {
            return OscDrainStep::Reprocess;
        }
        if self.bytes_left == 0 {
            return OscDrainStep::Reprocess;
        }
        if self.escape_pending && byte == b'\\' {
            self.bytes_left -= 1;
            return OscDrainStep::Finished;
        }
        if byte == 0x07 || byte == 0x9c {
            self.bytes_left -= 1;
            return OscDrainStep::Finished;
        }
        self.bytes_left -= 1;
        self.escape_pending = byte == 0x1b;
        self.idle_deadline = now + OSC_DRAIN_IDLE;
        if self.bytes_left == 0 && !self.escape_pending {
            OscDrainStep::Finished
        } else {
            OscDrainStep::Pending
        }
    }
}

impl OscReplyFrame {
    /// Starts one input string with a one-second absolute framing deadline.
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            selector: Vec::with_capacity(3),
            target: None,
            payload: Vec::with_capacity(24),
            byte_count: 0,
            malformed: false,
            escape_pending: false,
            deadline: now + Duration::from_secs(1),
        }
    }

    /// Whether the exact OSC selector has identified a queried color.
    pub(crate) const fn target(&self) -> Option<ColorTarget> {
        self.target
    }

    /// Transfers the targeted frame's terminator prefix into bounded recovery.
    pub(crate) fn drain(&self, now: Instant) -> OscDrain {
        OscDrain::new(now, self.escape_pending)
    }

    /// Advances one OSC byte; query replies are capped before payload retention.
    pub(crate) fn push(&mut self, byte: u8) -> OscStep {
        if self.target.is_none() {
            self.selector.push(byte);
            let foreground = b"10;";
            let background = b"11;";
            if foreground.starts_with(&self.selector) {
                if self.selector.as_slice() == foreground {
                    self.target = Some(ColorTarget::Foreground);
                    return OscStep::Pending;
                }
                return OscStep::Pending;
            }
            if background.starts_with(&self.selector) {
                if self.selector.as_slice() == background {
                    self.target = Some(ColorTarget::Background);
                    return OscStep::Pending;
                }
                return OscStep::Pending;
            }
            return OscStep::NotColorReply(std::mem::take(&mut self.selector));
        }

        if byte == 0x07 || byte == 0x9c {
            return self.finish();
        }
        if self.escape_pending {
            self.escape_pending = false;
            if byte == b'\\' {
                return self.finish();
            }
            self.malformed = true;
        }
        if byte == 0x1b {
            self.escape_pending = true;
            return OscStep::Pending;
        }
        self.byte_count += 1;
        if self.byte_count > OSC_FRAME_LIMIT {
            return OscStep::OverBound(self.target.expect("target set before payload"));
        } else {
            self.payload.push(byte);
        }
        if !byte.is_ascii_graphic() {
            self.malformed = true;
        }
        OscStep::Pending
    }

    /// Expires incomplete targeted strings while preserving an input-drain owner.
    pub(crate) fn expired(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    /// Finishes a target reply; malformed, over-bound and unsupported specs stay unknown.
    pub(crate) fn finish(&self) -> OscStep {
        let Some(target) = self.target else {
            return OscStep::NotColorReply(self.selector.clone());
        };
        let rgb = (!self.malformed)
            .then(|| parse_rgb(&self.payload))
            .flatten();
        OscStep::Complete(ColorReply { target, rgb })
    }

    /// Bytes buffered before the selector was recognized as a color reply.
    pub(crate) fn selector_bytes(&self) -> &[u8] {
        &self.selector
    }
}

/// Parses the bounded RGB forms reported for xterm dynamic colors.
pub(crate) fn parse_rgb(specification: &[u8]) -> Option<[u8; 3]> {
    if let Some(hex) = specification.strip_prefix(b"#") {
        if hex.len() != 6 {
            return None;
        }
        let mut rgb = [0; 3];
        for (component, bytes) in rgb.iter_mut().zip(hex.chunks_exact(2)) {
            *component = u8::from_str_radix(std::str::from_utf8(bytes).ok()?, 16).ok()?;
        }
        return Some(rgb);
    }
    let components = specification
        .strip_prefix(b"rgb:")?
        .split(|byte| *byte == b'/')
        .collect::<Vec<_>>();
    if components.len() != 3 {
        return None;
    }
    let mut rgb = [0; 3];
    for (component, bytes) in rgb.iter_mut().zip(components) {
        if bytes.is_empty() || bytes.len() > 4 || !bytes.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        let digits = u16::from_str_radix(std::str::from_utf8(bytes).ok()?, 16).ok()?;
        let maximum = (1_u32 << (bytes.len() * 4)) - 1;
        *component = u8::try_from((u32::from(digits) * 255 + maximum / 2) / maximum).ok()?;
    }
    Some(rgb)
}

fn classify_background(red: u8, green: u8, blue: u8) -> HostAppearance {
    let luminance =
        299_u32 * u32::from(red) + 587_u32 * u32::from(green) + 114_u32 * u32::from(blue);
    if luminance < 128_000 {
        HostAppearance::Dark
    } else {
        HostAppearance::Light
    }
}

#[cfg(test)]
mod tests;
