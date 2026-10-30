//! One bounded outer-terminal default-color probe; replies never enter pane or editor input.

use crate::host_appearance::{HostAppearance, HostColors};
pub(crate) use crate::terminal_events::appearance::{ColorReply, ColorTarget};
use std::fmt;
#[cfg(windows)]
use std::io;
use std::time::{Duration, Instant};

const QUERY_DEADLINE: Duration = Duration::from_millis(350);
/// Uses only an implemented host appearance adapter after the terminal probe expires.
pub(crate) fn system_fallback() -> HostAppearance {
    #[cfg(windows)]
    {
        from_windows_apps_use_light_theme(orchbee_local_runtime::read_windows_apps_use_light_theme())
    }
    #[cfg(not(windows))]
    {
        HostAppearance::Unknown
    }
}

/// The one ordered query for the host's default foreground and background colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Query;

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
