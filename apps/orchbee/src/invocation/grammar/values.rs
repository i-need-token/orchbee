//! Declared closed values and discoverable key examples.

pub(crate) const ORDER_VALUES: &[&str] = &["previous", "next", "first", "last"];
pub(crate) const SEARCH_DIRECTION_VALUES: &[&str] = &["forward", "backward"];
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
pub(crate) const PROVIDER_VALUES: &[&str] = &[
    "claude-code",
    "codex",
    "gemini",
    "github-copilot",
    "grok",
    "cline",
    "opencode",
    "kilo",
    "cursor",
    "qodercli",
    "antigravity",
    "mastracode",
    "droid",
    "kimi",
    "letta",
    "devin",
    "hermes",
    "pi",
    "omp",
    "qwen-code",
];
pub(crate) const COPY_MOTION_VALUES: &[&str] = &[
    "line-end",
    "first-non-blank",
    "next-word-start",
    "previous-word-start",
    "next-word-end",
    "next-big-word-start",
    "previous-big-word-start",
    "next-big-word-end",
    "previous-paragraph",
    "next-paragraph",
];
pub(crate) const SHELLS: &[&str] = &["bash", "elvish", "fish", "powershell", "zsh"];
pub(crate) const FORMAT_VALUES: &[&str] = &["text", "json"];
