//! Declared closed values and discoverable key examples.

pub(crate) const ORDER_VALUES: &[&str] = &["previous", "next", "first", "last"];
pub(crate) const CWD_VALUES: &[&str] = &["follow", "home", "current", "workspace"];
pub(crate) const OUTPUT_SOURCES: &[&str] = &["visible", "recent", "recent-unwrapped", "detection"];
pub(crate) const RIGHT_CLICK_VALUES: &[&str] = &["orchbee", "panel"];
pub(crate) const KEY_VALUES: &[&str] = &[
    "enter",
    "escape",
    "tab",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "page-up",
    "page-down",
    "ctrl+c",
    "ctrl+d",
    "space",
    "insert",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
    "f13",
    "f14",
    "f15",
    "f16",
    "f17",
    "f18",
    "f19",
    "f20",
    "f21",
    "f22",
    "f23",
    "f24",
    "f25",
    "plus",
    "minus",
    "semicolon",
    "slash",
    "ctrl+shift+a",
    "super+c",
    "alt+enter",
];
macro_rules! wait_conditions {
    ($($variant:ident => $word:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) enum AgentWaitCondition { $($variant),+ }
        impl AgentWaitCondition {
            pub(crate) const fn as_str(self) -> &'static str { match self { $(Self::$variant => $word),+ } }
            pub(crate) fn parse(value: &str) -> Option<Self> { match value { $($word => Some(Self::$variant),)+ _ => None } }
        }
        pub(crate) const UNTIL_VALUES: &[&str] = &[$($word),+];
    };
}
wait_conditions! {
    Blocked => "blocked",
    Done => "done",
    NeedsAttention => "needs-attention",
    Failed => "failed",
    Succeeded => "succeeded",
    Canceled => "canceled",
    Interrupted => "interrupted",
    Working => "working",
    Idle => "idle",
    Unknown => "unknown",
    Ended => "ended",
    Fresh => "fresh",
    Stale => "stale",
    Disconnected => "disconnected",
}
pub(crate) const PROVIDER_VALUES: &[&str] = &{
    use crate::agent_integrations::IntegrationId;
    let mut values = [""; IntegrationId::ALL.len()];
    let mut index = 0;
    while index < values.len() {
        values[index] = IntegrationId::ALL[index].provider();
        index += 1;
    }
    values
};
/// A canonical Agent outside the integration vocabulary: Orchbee can start
/// it but has no integration to check or manage for it.
pub(crate) fn start_only_provider(value: &str) -> bool {
    crate::agent_provider::CANONICAL_IDS.contains(&value) && !PROVIDER_VALUES.contains(&value)
}
/// Agent doctor probe-table identities in canonical probe order. CLI provider spellings map
/// through `IntegrationId` before this check; `agent_doctor` proves its probe table matches.
pub(crate) const DOCTOR_PROVIDER_IDS: &[&str] = &[
    "claude-code",
    "codex",
    "gemini",
    "github-copilot",
    "grok",
    "cline",
    "opencode",
    "kilo",
    "mastracode",
    "droid",
    "kimi",
    "letta",
    "devin",
    "hermes",
    "antigravity-cli",
    "qodercli",
    "cursor",
    "pi",
    "omp",
    "qwen-code",
];
macro_rules! mapped_values {
    ($values:ident, $parse:ident, $type:ty, {$($word:literal => $value:expr),+ $(,)?}) => {
        pub(crate) const $values: &[&str] = &[$($word),+];
        pub(crate) fn $parse(value: &str) -> Option<$type> {
            match value { $($word => Some($value),)+ _ => None }
        }
    };
}
use orchbee_workbench_protocol::{NotificationPosition, NotificationSound, PanelAgentState};
mapped_values!(PANEL_AGENT_STATES, panel_agent_state, PanelAgentState, {
    "idle" => PanelAgentState::Idle, "working" => PanelAgentState::Working,
    "blocked" => PanelAgentState::Blocked, "done" => PanelAgentState::Done,
    "error" => PanelAgentState::Error, "unknown" => PanelAgentState::Unknown,
});
mapped_values!(NOTIFICATION_POSITIONS, notification_position, NotificationPosition, {
    "top-left" => NotificationPosition::TopLeft, "top-right" => NotificationPosition::TopRight,
    "bottom-left" => NotificationPosition::BottomLeft, "bottom-right" => NotificationPosition::BottomRight,
});
mapped_values!(NOTIFICATION_SOUNDS, notification_sound, NotificationSound, {
    "none" => NotificationSound::None, "done" => NotificationSound::Done, "request" => NotificationSound::Request,
});
pub(crate) const SHELLS: &[&str] = &["bash", "elvish", "fish", "powershell", "zsh"];
pub(crate) const FORMAT_VALUES: &[&str] = &["text", "json"];
