mod client_control;
mod creation;
mod panel_input;
pub(crate) use creation::{CreationCommand, CreationDirectory, CreationKind};
mod schema_export;
mod terminal_output;
mod terminal_run;
pub(crate) use client_control::{
    ClientCommand, ClientLayoutTarget, ClientNavigationTarget, LayoutQuery, PanelSwapCommand,
    PanelSwapDestination, ZoomMode,
};
pub(crate) use terminal_output::{OutputSelection, OutputSource, OutputTarget};
mod workspace_group_close;
pub(crate) use workspace_group_close::WorkspaceGroupCloseCommand;
mod worktree;
#[cfg(windows)]
mod worktree_path_windows;
use std::{ffi::OsString, path::PathBuf};
pub(crate) use worktree::{
    WorktreeCommand, WorktreeQueryCommand, WorktreeStartCommand, WorktreeStartKind,
};

use orchbee_agent_observation_protocol::{
    AgentListCursor, AgentName, AgentSafeInteger, AgentSessionId, DEFAULT_LIST_LIMIT,
    MAXIMUM_LIST_LIMIT, MAXIMUM_SAFE_INTEGER,
};
use orchbee_workbench_protocol::{
    LaunchEnvironment, LaunchOptions, LaunchProfileName, LaunchProfileSelector, LaunchSelection,
    WorkbenchClientId, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchPanelSplitDirection, WorkbenchProjectId, WorkbenchRequestId,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabInsertIndex, WorkbenchTabMoveDirection,
    WorkbenchTerminalGeometry, WorkbenchWorkspaceId,
};

/// Legacy three-variant invocation consumed by `presentation::present`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Invocation {
    Help,
    Tui,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsageFailure;

const MAXIMUM_ARGUMENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

/// A `get`/filter target validated at parse time. `Id` carries the canonical
/// 32-lowercase-hex text; `Label` carries an exact case-sensitive label;
/// `Selector` carries an explicit `@<kind>:<suffix>` token whose kind was
/// already checked against the operand's resource — resolution reads only
/// the suffix, since the candidate set is always kind-scoped by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Id(String),
    Label(String),
    Selector(String),
}

impl Target {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Id(value) | Self::Label(value) | Self::Selector(value) => value,
        }
    }

    /// The validated lowercase-hex tail of a `@<kind>:<suffix>` selector; the
    /// parser admitted the token, so the split is structural. Non-selector
    /// targets have no suffix.
    pub(crate) fn suffix(&self) -> Option<&str> {
        match self {
            Self::Selector(token) => token.split_once(':').map(|(_, suffix)| suffix),
            _ => None,
        }
    }
}

/// Parsed Agent targets keep Workbench Panel selectors out of OBAP DTOs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentTarget {
    Session { session_id: AgentSessionId },
    Name { name: AgentName },
    Auto { target: AgentSessionId },
    Panel { target: Target },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Resource {
    Project,
    Workspace,
    Tab,
    Panel,
    Terminal,
    Schema,
}

impl Resource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Tab => "tab",
            Self::Panel => "panel",
            Self::Terminal => "terminal",
            Self::Schema => "schema",
        }
    }

    /// The recognized `@<kind>:` selector prefix. `Schema` has none; any
    /// other token is not selector syntax and keeps its label reading.
    fn selector_token(token: &str) -> Option<Self> {
        Some(match token {
            "project" => Self::Project,
            "workspace" => Self::Workspace,
            "tab" => Self::Tab,
            "panel" => Self::Panel,
            "terminal" => Self::Terminal,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HelpTopic {
    Family(Family),
    Command(CommandPath),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Family {
    Client,
    Agent,
    /// `agent integration`: the provider-onboarding sub-family under Agent.
    AgentIntegration,
    Project,
    Workspace,
    Tab,
    Panel,
    Layout,
    Terminal,
    Worktree,
    LaunchProfile,
    Site,
    Api,
    Config,
    Completion,
}

impl Family {
    /// The family's real help-path spelling, as `help_topic` recognizes it.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Agent => "agent",
            Self::AgentIntegration => "agent integration",
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Tab => "tab",
            Self::Panel => "panel",
            Self::Layout => "layout",
            Self::Terminal => "terminal",
            Self::Worktree => "worktree",
            Self::LaunchProfile => "launch-profile",
            Self::Site => "site",
            Self::Api => "api",
            Self::Config => "config",
            Self::Completion => "completion",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandPath {
    ClientList,
    ClientGet,
    ClientControlStatus,
    WorkspaceFocus,
    TabFocus,
    PanelFocus,
    PanelResize,
    LayoutSetSplitRatio,
    PanelZoom,
    PanelLayout,
    PanelNeighbor,
    PanelEdges,

    Tui,
    Status,
    StatusServer,
    StatusClient,
    AgentList,
    AgentChanges,
    AgentGet,
    AgentFocus,
    AgentAttention,
    AgentRead,
    AgentStart,
    AgentPrompt,
    AgentSendKeys,
    AgentExplain,
    AgentWait,
    AgentDoctor,
    AgentNames,
    AgentResolve,
    AgentRename,
    AgentIntegrationList,
    AgentIntegrationExplain,
    AgentIntegrationEnable,
    AgentIntegrationDisable,
    ProjectList,
    ProjectGet,
    ProjectCreate,
    ProjectRename,
    WorkspaceList,
    WorkspaceGet,
    WorkspaceCreate,
    WorkspaceRename,
    WorkspaceAcceptDirectory,
    WorkspaceClose,
    WorkspaceCloseStatus,
    WorkspaceCloseResume,
    TabList,
    TabGet,
    TabCreate,
    TabRename,
    PanelRename,
    PanelInput,
    PanelSwap,
    TabMove,
    TabClose,
    PanelList,
    PanelCurrent,
    PanelGet,
    PanelProcessInfo,
    PanelSplit,
    PanelClose,
    PanelMove,
    TerminalList,
    TerminalGet,
    TerminalRead,
    TerminalWaitOutput,
    TerminalSend,
    TerminalRun,
    WorktreeList,
    WorktreeCreate,
    WorktreeOpen,
    WorktreeRemove,
    WorktreeStatus,
    WorktreeResume,
    LaunchProfileList,
    LaunchProfileGet,
    LaunchProfileCreate,
    LaunchProfileDelete,

    ApiSnapshot,
    ApiSchema,
    ConfigCheck,
    SiteStart,
    SiteDiagnose,
    SiteStop,
    SiteStopStatus,
}

impl CommandPath {
    /// The command's real help-path spelling, as `help_topic` recognizes it.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ClientList => "client list",
            Self::ClientGet => "client get",
            Self::ClientControlStatus => "client control-status",
            Self::WorkspaceFocus => "workspace focus",
            Self::TabFocus => "tab focus",
            Self::PanelFocus => "panel focus",
            Self::PanelResize => "panel resize",
            Self::LayoutSetSplitRatio => "layout set-split-ratio",
            Self::PanelZoom => "panel zoom",
            Self::PanelLayout => "panel layout",
            Self::PanelNeighbor => "panel neighbor",
            Self::PanelEdges => "panel edges",

            Self::Tui => "tui",
            Self::Status => "status",
            Self::StatusServer => "status server",
            Self::StatusClient => "status client",
            Self::AgentList => "agent list",
            Self::AgentChanges => "agent changes",
            Self::AgentGet => "agent get",
            Self::AgentFocus => "agent focus",
            Self::AgentAttention => "agent attention",
            Self::AgentRead => "agent read",
            Self::AgentStart => "agent start",
            Self::AgentPrompt => "agent prompt",
            Self::AgentSendKeys => "agent send-keys",
            Self::AgentExplain => "agent explain",
            Self::AgentWait => "agent wait",
            Self::AgentDoctor => "agent doctor",
            Self::AgentNames => "agent names",
            Self::AgentResolve => "agent resolve",
            Self::AgentRename => "agent rename",
            Self::AgentIntegrationList => "agent integration list",
            Self::AgentIntegrationExplain => "agent integration explain",
            Self::AgentIntegrationEnable => "agent integration enable",
            Self::AgentIntegrationDisable => "agent integration disable",
            Self::ProjectList => "project list",
            Self::ProjectGet => "project get",
            Self::ProjectCreate => "project create",
            Self::ProjectRename => "project rename",
            Self::WorkspaceList => "workspace list",
            Self::WorkspaceGet => "workspace get",
            Self::WorkspaceCreate => "workspace create",
            Self::WorkspaceRename => "workspace rename",
            Self::WorkspaceAcceptDirectory => "workspace accept-directory",
            Self::WorkspaceClose => "workspace close",
            Self::WorkspaceCloseStatus => "workspace close-status",
            Self::WorkspaceCloseResume => "workspace close-resume",
            Self::TabList => "tab list",
            Self::TabGet => "tab get",
            Self::TabCreate => "tab create",
            Self::TabRename => "tab rename",
            Self::PanelRename => "panel rename",
            Self::PanelInput => "panel input",
            Self::PanelSwap => "panel swap",
            Self::TabMove => "tab move",
            Self::TabClose => "tab close",
            Self::PanelList => "panel list",
            Self::PanelCurrent => "panel current",
            Self::PanelGet => "panel get",
            Self::PanelProcessInfo => "panel process-info",
            Self::PanelSplit => "panel split",
            Self::PanelClose => "panel close",
            Self::PanelMove => "panel move",
            Self::TerminalList => "terminal list",
            Self::TerminalGet => "terminal get",
            Self::TerminalRead => "terminal read",
            Self::TerminalWaitOutput => "terminal wait-output",
            Self::TerminalSend => "terminal send",
            Self::TerminalRun => "terminal run",
            Self::WorktreeList => "worktree list",
            Self::WorktreeCreate => "worktree create",
            Self::WorktreeOpen => "worktree open",
            Self::WorktreeRemove => "worktree remove",
            Self::WorktreeStatus => "worktree status",
            Self::WorktreeResume => "worktree resume",
            Self::LaunchProfileList => "launch-profile list",
            Self::LaunchProfileGet => "launch-profile get",
            Self::LaunchProfileCreate => "launch-profile create",
            Self::LaunchProfileDelete => "launch-profile delete",

            Self::ApiSnapshot => "api snapshot",
            Self::ApiSchema => "api schema",
            Self::ConfigCheck => "config check",
            Self::SiteStart => "site start",
            Self::SiteDiagnose => "site diagnose",
            Self::SiteStop => "site stop",
            Self::SiteStopStatus => "site stop-status",
        }
    }
}

/// One canonical `agent wait` condition: the fourteen-value vocabulary the
/// CLI wait schema admits — eleven fresh-gated status conditions plus
/// the three direct freshness conditions. `as_str` is the canonical
/// kebab-case name used on the command line, inside `matchedConditions`,
/// and in the emitted wait document's `until` echo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentWaitCondition {
    Blocked,
    Done,
    NeedsAttention,
    Failed,
    Succeeded,
    Canceled,
    Interrupted,
    Working,
    Idle,
    Unknown,
    Ended,
    Fresh,
    Stale,
    Disconnected,
}

impl AgentWaitCondition {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::NeedsAttention => "needs-attention",
            Self::Failed => "failed",
            Self::Succeeded => "succeeded",
            Self::Canceled => "canceled",
            Self::Interrupted => "interrupted",
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Unknown => "unknown",
            Self::Ended => "ended",
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Disconnected => "disconnected",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "blocked" => Self::Blocked,
            "done" => Self::Done,
            "needs-attention" => Self::NeedsAttention,
            "failed" => Self::Failed,
            "succeeded" => Self::Succeeded,
            "canceled" => Self::Canceled,
            "interrupted" => Self::Interrupted,
            "working" => Self::Working,
            "idle" => Self::Idle,
            "unknown" => Self::Unknown,
            "ended" => Self::Ended,
            "fresh" => Self::Fresh,
            "stale" => Self::Stale,
            "disconnected" => Self::Disconnected,
            _ => return None,
        })
    }
}

/// The canonical default condition set `agent wait` applies when `--until`
/// is absent: the seven conditions, verbatim and in canonical order.
#[cfg(not(windows))]
pub(crate) const DEFAULT_AGENT_WAIT_CONDITIONS: [AgentWaitCondition; 7] = [
    AgentWaitCondition::NeedsAttention,
    AgentWaitCondition::Failed,
    AgentWaitCondition::Succeeded,
    AgentWaitCondition::Canceled,
    AgentWaitCondition::Interrupted,
    AgentWaitCondition::Idle,
    AgentWaitCondition::Ended,
];

#[cfg(windows)]
pub(crate) const DEFAULT_AGENT_WAIT_CONDITIONS: [AgentWaitCondition; 3] = [
    AgentWaitCondition::Idle,
    AgentWaitCondition::Done,
    AgentWaitCondition::Blocked,
];

pub(crate) const DEFAULT_AGENT_WAIT_TIMEOUT: Option<u32> =
    if cfg!(windows) { None } else { Some(600_000) };

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PanelQueryTarget {
    Current,
    Panel(Target),
    Client(WorkbenchClientId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QueryCommand {
    Status,
    AgentChanges {
        limit: u64,
        cursor: Option<orchbee_agent_observation_protocol::AgentChangesCursor>,
    },
    AgentList {
        include_ended: bool,
        limit: u64,
        cursor: Option<AgentListCursor>,
    },
    AgentGet(AgentTarget),
    AgentClientGet {
        target: AgentTarget,
        client: WorkbenchClientId,
    },
    AgentRead {
        target: AgentTarget,
        history: Option<u16>,
        unwrap: bool,
        lines: Option<u16>,
    },
    AgentExplain(AgentTarget),
    /// `agent wait`: one held Client Session polls the canonical session
    /// document until a requested condition matches, the fence refuses, or
    /// the shared absolute deadline lands.
    AgentWait {
        session: AgentTarget,
        /// The effective condition list — `DEFAULT_AGENT_WAIT_CONDITIONS`
        /// verbatim when `--until` is absent. Requested order is preserved
        /// for matching and the emitted echo.
        until: Vec<AgentWaitCondition>,
        /// The optional global-revision fence (`--after-revision`) — a
        /// caller-supplied bound, never a turn-scoped causal promise.
        after_revision: Option<AgentSafeInteger>,
        /// The total wait budget in milliseconds measured from command
        /// start, 1..=3600000, defaulting to 600000 — never `wait-output`'s
        /// 30000. None is explicit --forever.
        timeout_ms: Option<u32>,
    },
    /// `agent doctor`: the offline provider-diagnostics read — validated
    /// provider selectors in flag order and supplied asset roots in flag
    /// order; it never opens a session.
    AgentDoctor {
        providers: Vec<String>,
        asset_roots: Vec<String>,
    },
    ProjectList,
    ProjectGet(Target),
    WorkspaceList {
        project: Option<Target>,
    },
    WorkspaceGet(Target),
    /// `workspace close-status --project <id> --workspace <id>`: the
    /// retained Workspace Close record read from stable IDs — it stays
    /// answerable after the Workspace leaves the active projection.
    WorkspaceCloseStatus {
        project: WorkbenchProjectId,
        workspace: WorkbenchWorkspaceId,
    },
    TabList {
        workspace: Option<Target>,
        standalone: bool,
    },
    TabGet(Target),
    PanelList {
        tab: Option<Target>,
    },
    PanelCurrent(PanelQueryTarget),
    PanelGet(Target),
    PanelProcessInfo(PanelQueryTarget),
    TerminalList,
    TerminalGet(Target),
    TerminalRead {
        target: Target,
        /// `Some(n)` selects the `--history --rows n` form: retained
        /// scrollback rows instead of the visible frame.
        history: Option<u16>,
        /// `--unwrap`: present joined logical lines instead of physical
        /// rows; a read-only projection of the same emitted rows.
        unwrap: bool,
        /// `--lines <n>`: present only the last n emitted entries — logical
        /// lines under `--unwrap`, physical rows otherwise.
        lines: Option<u16>,
    },
    TerminalOutput {
        target: OutputTarget,
        selection: OutputSelection,
        plain: bool,
        ansi: bool,
    },
    TerminalSourceWait {
        target: Target,
        matcher: WaitPattern,
        selection: OutputSelection,
        timeout_ms: Option<u32>,
    },
    TerminalWaitOutput {
        target: Target,
        /// The searched pattern: the literal `--match` text or the
        /// pre-compiled `--regex`, exactly one admitted by the grammar.
        matcher: WaitPattern,
        /// The searched retained-row window (visible + history as one
        /// range), 1..=1000 — `--rows` defaults to 1000.
        rows: u16,
        /// Total wait budget in milliseconds from command start, 1..=3600000.
        timeout_ms: u32,
    },
    /// `worktree list --project <t> --directory <id>`: both selectors are
    /// mandatory; the directory is a strict nonzero 32-lowercase-hex Workbench
    /// directory ID, never a filesystem path or label.
    WorktreeList {
        project: Target,
        directory: WorkbenchDirectoryId,
    },
    /// `launch-profile list`: the whole ephemeral catalog as one bounded
    /// typed read — no selector, no snapshot resolution.
    LaunchProfileList,
    /// `launch-profile get <selector>`: one catalog entry resolved
    /// Server-side by the same selector rule get/delete/use share.
    LaunchProfileGet(LaunchProfileSelector),
    ApiSnapshot,
    ApiSchema {
        schema_id: Option<String>,
    },
    ConfigCheck,
}

/// The `terminal wait-output` search input the grammar admits: the literal
/// `--match` text, or the `--regex` pattern already compiled inside the
/// declared size budgets — the compiled program exists before any session,
/// socket, or viewer effect runs.
#[derive(Clone, Debug)]
pub(crate) enum WaitPattern {
    /// `--match <text>`: a literal, case-sensitive substring.
    Literal(String),
    /// `--regex <pattern>`: a linear-time `regex` program compiled once at
    /// parse; equality compares the source pattern.
    Regex(regex::Regex),
}

impl PartialEq for WaitPattern {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Literal(left), Self::Literal(right)) => left == right,
            (Self::Regex(left), Self::Regex(right)) => left.as_str() == right.as_str(),
            _ => false,
        }
    }
}

impl Eq for WaitPattern {}

/// The `tab create` context selector: exactly one Workspace context or the
/// standalone Terminal Context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TabContext {
    Workspace(Target),
    Standalone,
}

/// The `terminal send` payload the grammar admits: literal UTF-8 text, an
/// ordered repeatable key/combo sequence, or a declared local source
/// materialized once into one bounded record. The parser stores the
/// validated choice so the executor never re-reads argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalSendInput {
    /// Literal UTF-8 bytes; `enter` folds one trailing CR into the same
    /// single input record, never a second request.
    Text { text: String, enter: bool },
    /// The ordered `--key` sequence — every element is a named semantic key
    /// or an explicit modifier combination, in argv order.
    Keys(Vec<ParsedSendKey>),
    /// A declared regular file read once into one bounded record. `enter`
    /// folds one trailing CR; `paste` selects the Host-bracketed paste
    /// record instead of a literal write.
    File {
        path: String,
        enter: bool,
        paste: bool,
    },
    /// A declared noninteractive standard-input stream read once into one
    /// bounded record; `enter`/`paste` follow `File`.
    Stdin { enter: bool, paste: bool },
    /// Materialized-only variant: the literal bytes a declared
    /// `--file`/`--stdin` source produced, with `enter` folding one trailing
    /// CR into the same record. The parser never emits this.
    Bytes { bytes: Vec<u8>, enter: bool },
    /// Opaque command text from `terminal run`, or materialized content a declared
    /// `--paste --file`/`--stdin` source produced; `enter` is a separate
    /// trailing Enter record the executor appends, never folded into the
    /// paste content.
    Paste { content: Vec<u8>, enter: bool },
}

/// One `--key` argument's parsed value: a named semantic key or an explicit
/// `ctrl`/`alt`/`shift` combination over the closed base vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ParsedSendKey {
    Key(TerminalSendKey),
    Combo(TerminalSendCombo),
}

/// An explicit modifier combination: at least one modifier beside the base
/// key — a modifier-only or bare-base value is rejected by the grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalSendCombo {
    pub(crate) control: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) base: TerminalComboBase,
}

/// The closed combo base vocabulary: every named semantic key except the
/// already-modified `ctrl-c`/`ctrl-d`, plus `insert`, `f1` through `f12`,
/// `space`, and single ASCII letters and digits under their physical
/// `KeyX`/`DigitN` identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalComboBase {
    Key(TerminalSendKey),
    Insert,
    Function(u8),
    Space,
    Letter(u8),
    Digit(u8),
}

/// The closed `terminal send --key` vocabulary. `CtrlC`/`CtrlD` carry the
/// Control modifier on their letter; every other key is modifier-neutral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalSendKey {
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    CtrlC,
    CtrlD,
}

impl TerminalSendKey {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "enter" => Self::Enter,
            "escape" => Self::Escape,
            "tab" => Self::Tab,
            "backspace" => Self::Backspace,
            "delete" => Self::Delete,
            "up" => Self::Up,
            "down" => Self::Down,
            "left" => Self::Left,
            "right" => Self::Right,
            "home" => Self::Home,
            "end" => Self::End,
            "page-up" => Self::PageUp,
            "page-down" => Self::PageDown,
            "ctrl-c" => Self::CtrlC,
            "ctrl-d" => Self::CtrlD,
            _ => return None,
        })
    }
}

/// Parse one `--key` value: a bare named key, or `modifier+...+base` with at
/// least one of `ctrl`/`alt`/`shift` — order-insensitive, each at most once.
/// The base admits the named vocabulary except `ctrl-c`/`ctrl-d` (a
/// combination over an already-modified key is ambiguous) plus `insert`,
/// `f1`..=`f12`, `space`, one ASCII letter, or one ASCII digit. Everything
/// else — empty segments, unknown bases, bare `f5`-style bases with no
/// modifier — is refused before any effect.
fn parse_send_key(value: &str) -> Option<ParsedSendKey> {
    if !value.contains('+') {
        return TerminalSendKey::parse(value).map(ParsedSendKey::Key);
    }
    let mut control = false;
    let mut alt = false;
    let mut shift = false;
    let mut parts = value.split('+').peekable();
    loop {
        let part = parts.next()?;
        if part.is_empty() {
            return None;
        }
        if parts.peek().is_none() {
            let base = combo_base(part)?;
            if !(control || alt || shift) {
                return None;
            }
            return Some(ParsedSendKey::Combo(TerminalSendCombo {
                control,
                alt,
                shift,
                base,
            }));
        }
        let slot = match part {
            "ctrl" => &mut control,
            "alt" => &mut alt,
            "shift" => &mut shift,
            _ => return None,
        };
        if *slot {
            return None;
        }
        *slot = true;
    }
}

/// The combo base spelling: the named-key vocabulary minus the
/// already-modified pair, then the physical-code names.
fn combo_base(part: &str) -> Option<TerminalComboBase> {
    if let Some(key) = TerminalSendKey::parse(part) {
        return match key {
            TerminalSendKey::CtrlC | TerminalSendKey::CtrlD => None,
            key => Some(TerminalComboBase::Key(key)),
        };
    }
    if let Some(number) = part
        .strip_prefix('f')
        .and_then(|suffix| suffix.parse::<u8>().ok())
        && (1..=12).contains(&number)
        && part.len() == if number < 10 { 2 } else { 3 }
    {
        return Some(TerminalComboBase::Function(number));
    }
    Some(match part {
        "insert" => TerminalComboBase::Insert,
        "space" => TerminalComboBase::Space,
        _ => {
            let bytes = part.as_bytes();
            match bytes {
                [letter @ b'a'..=b'z'] => TerminalComboBase::Letter(*letter),
                [digit @ b'0'..=b'9'] => TerminalComboBase::Digit(*digit),
                _ => return None,
            }
        }
    })
}

/// One effectful command admitted by the grammar. Every field is a
/// wire-ready value already checked against protocol bounds, so the executor
/// never re-validates argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MutationCommand {
    ProjectCreate {
        label: WorkbenchLabel,
    },
    ProjectRename {
        target: Target,
        label: WorkbenchLabel,
    },
    WorkspaceCreate {
        project: Target,
        label: WorkbenchLabel,
    },
    WorkspaceLaunch {
        project: Target,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
        launch: Option<LaunchSelection>,
        profile: Option<LaunchProfileSelector>,
        options: LaunchOptions,
    },
    WorkspaceRename {
        target: Target,
        label: WorkbenchLabel,
    },
    WorkspaceAcceptDirectory {
        target: Target,
        path: WorkbenchDirectoryPath,
    },
    /// `workspace close <target> --confirm`: close every terminal in the
    /// Workspace's Terminal Context, then remove the Workspace and Context.
    /// The Project, checkout, branch and directories survive the close.
    WorkspaceClose {
        target: Target,
    },
    /// `workspace close-resume --project <id> --workspace <id> --request
    /// <close-id> --generation <n> --confirm`: one fresh attempt window for
    /// a resumable retained Workspace Close, fenced by the explicit
    /// attempt-generation the caller last observed.
    WorkspaceCloseResume {
        project: WorkbenchProjectId,
        workspace: WorkbenchWorkspaceId,
        request: WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
    },
    TabCreate {
        automatic_label: bool,
        context: TabContext,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
        launch: Option<LaunchSelection>,
        /// `--profile <selector>`: a catalog launch resolved Server-side;
        /// the grammar admits it only when `--exec`/`--arg` are absent.
        profile: Option<LaunchProfileSelector>,
        options: LaunchOptions,
    },
    PanelInput {
        target: Option<Target>,
        right_click: orchbee_workbench_protocol::PanelRightClickTarget,
    },
    PanelRename {
        target: Target,
        label: orchbee_workbench_protocol::PanelLabelUpdate,
    },
    TabRename {
        target: Target,
        label: WorkbenchLabel,
    },
    TabMove {
        target: Target,
        direction: WorkbenchTabMoveDirection,
    },
    TabMoveIndex {
        target: Target,
        insert_index: WorkbenchTabInsertIndex,
    },
    TabClose {
        target: Target,
    },
    PanelSplit {
        target: Target,
        right_click: orchbee_workbench_protocol::PanelRightClickTarget,
        direction: WorkbenchPanelSplitDirection,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
        launch: Option<LaunchSelection>,
        /// `--profile <selector>`: a catalog launch resolved Server-side;
        /// the grammar admits it only when `--exec`/`--arg` are absent.
        profile: Option<LaunchProfileSelector>,
        options: LaunchOptions,
    },
    PanelClose {
        target: Target,
    },
    PanelMove {
        target: Target,
        destination: PanelMoveDestination,
    },
    /// `launch-profile create --name <n> --exec <abs> [--arg <v>]...`: one
    /// new ephemeral catalog entry; the launch selection passes the same
    /// closed validation as the explicit `--exec`/`--arg` form.
    LaunchProfileCreate {
        name: LaunchProfileName,
        launch: LaunchSelection,
    },
    /// `launch-profile delete <selector>`: removes the entry the Server
    /// resolves under the shared selector rule.
    LaunchProfileDelete {
        selector: LaunchProfileSelector,
    },
    AgentPrompt {
        target: AgentTarget,
        text: String,
        wait: Option<AgentPromptWait>,
    },
    AgentSendKeys {
        target: AgentTarget,
        keys: Vec<ParsedSendKey>,
    },
    TerminalSend {
        target: Target,
        input: TerminalSendInput,
    },
}

/// The three exclusive `panel move` destinations. `Adjacent` keeps the
/// original same-Tab `--target-panel --direction` grammar; `Tab` and
/// `NewTab` are the cross-Tab relocation destinations, bounded to the
/// source TerminalContext.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PanelMoveDestination {
    Adjacent {
        into: Target,
        direction: WorkbenchPanelSplitDirection,
    },
    Tab {
        tab: Target,
        direction: WorkbenchPanelSplitDirection,
    },
    NewTab {
        label: WorkbenchLabel,
    },
}

/// One `agent integration` command admitted by the grammar — a local
/// provider-onboarding operation that never touches a Site. `provider` is
/// optional on `list` (a filter) and required on every other verb;
/// `adapter_source` is the development/test override already validated as
/// a literal absolute path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentIntegrationCommand {
    List {
        provider: Option<crate::agent_integrations::IntegrationId>,
        adapter_source: Option<PathBuf>,
    },
    Explain {
        provider: crate::agent_integrations::IntegrationId,
        adapter_source: Option<PathBuf>,
    },
    Enable {
        provider: crate::agent_integrations::IntegrationId,
        adapter_source: Option<PathBuf>,
    },
    Disable {
        provider: crate::agent_integrations::IntegrationId,
        adapter_source: Option<PathBuf>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentPromptWait {
    pub(crate) until: Vec<AgentWaitCondition>,
    pub(crate) timeout_ms: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentStartCommand {
    pub(crate) name: AgentName,
    pub(crate) provider: String,
    pub(crate) panel: Option<Target>,
    pub(crate) args: Vec<String>,
    pub(crate) timeout_ms: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentNamesCommand {
    List,
    Resolve(AgentTarget),
    Assign {
        target: AgentTarget,
        name: AgentName,
    },
    Release(AgentTarget),
}

/// Parsed stop/status intent; destructive confirmation and flag conflicts are checked before use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SiteStopCommand {
    Stop {
        id: Option<orchbee_local_client_protocol::StopId>,
        retry: bool,
        detach: bool,
        timeout_ms: u32,
    },
    Status {
        id: orchbee_local_client_protocol::StopId,
    },
}

fn stop_id(
    value: &str,
    context: UsageContext,
    option: &'static str,
    envelope: FailureEnvelope,
) -> Result<orchbee_local_client_protocol::StopId, ParseFailure> {
    let invalid = || {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.to_owned(),
                expected: ExpectedValue::SiteStopId,
            },
            envelope,
        )
    };
    if !valid_typed_id(value) || value == ZERO_IDENTITY {
        return Err(invalid());
    }
    let mut bytes = [0; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| invalid())?;
    }
    orchbee_local_client_protocol::StopId::new(bytes).map_err(|_| invalid())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeStatusScope {
    Server,
    Client,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Request {
    RuntimeStatus {
        scope: RuntimeStatusScope,
        format: OutputFormat,
    },
    Creation {
        command: CreationCommand,
        format: OutputFormat,
    },
    SchemaExport {
        schema_id: Option<String>,
        path: String,
        format: OutputFormat,
    },
    Client {
        command: ClientCommand,
        format: OutputFormat,
    },
    WorkspaceGroupClose {
        command: WorkspaceGroupCloseCommand,
        format: OutputFormat,
    },
    WorktreeQuery {
        command: WorktreeQueryCommand,
        format: OutputFormat,
    },
    Worktree {
        command: WorktreeCommand,
        format: OutputFormat,
    },
    SiteStop {
        command: SiteStopCommand,
        format: OutputFormat,
    },
    /// `mouse` is the `--mouse off|on` selection: whether host pointer
    /// reporting is armed for the session. Only the explicit `tui` path
    /// admits the option; a bare `orchbee` always runs with it on.
    Tui {
        mouse: bool,
    },
    Version,
    /// The embedded, locale-selected Agent guide; no runtime capabilities.
    Skill,
    RootHelp,
    Help(HelpTopic),
    Completion(CompletionShell),
    Query {
        command: QueryCommand,
        format: OutputFormat,
    },
    Mutation {
        command: MutationCommand,
        format: OutputFormat,
    },
    /// `site start`: the headless connect-or-activate command — neither a
    /// Workbench query nor a mutation; it runs the shared activation core.
    SiteStart {
        format: OutputFormat,
    },
    /// `site diagnose`: the read-only runtime observation command —
    /// neither a Workbench query nor a mutation; it never connects.
    SiteDiagnose {
        format: OutputFormat,
    },
    /// `agent integration`: the local provider-onboarding surface — runs
    /// entirely off ambient PATH and the cohort sidecar, never contacts a
    /// Site.
    AgentStart {
        command: AgentStartCommand,
        format: OutputFormat,
    },
    AgentNames {
        command: AgentNamesCommand,
        format: OutputFormat,
    },
    AgentIntegration {
        command: AgentIntegrationCommand,
        format: OutputFormat,
    },
}

/// How a parse failure is allowed to answer on stdout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureEnvelope {
    /// argv envelope or format-selection failure: stdout stays empty.
    Silent,
    /// A unique valid `--format json` selection preceded this failure.
    Json,
    /// No valid json selection: stdout stays empty.
    Text,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParseFailureKind {
    InvalidUsage,
    InvalidIdentifier { resource: Resource, target: String },
}

/// The invocation context a usage diagnostic is anchored to. `Path` carries a
/// real help path ("project list"), so its hint points at `orchbee <path>
/// --help`; `Literal` carries display text with no such spelling
/// ("orchbee --version"), so its hint falls back to the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsageContext {
    Root,
    Path(&'static str),
    Literal(&'static str),
}

/// The bounded vocabulary of value shapes an `invalid-usage` diagnostic may
/// expect; rendered per locale, never serialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedValue {
    RightClick,
    PanelLabel,
    OutputFilePath,
    ClientId,
    ClientOperationId,
    WorktreeReference,
    WorktreeOperationId,
    WorktreeAttempt,
    Format,
    Label,
    Dimension,
    TabDirection,
    TabInsertIndex,
    PanelDirection,
    LayoutDirection,
    FiniteNumber,
    SplitPath,
    Shell,
    HistoryRows,
    DirectoryPath,
    WorktreePath,
    /// The `--directory` worktree-list selector: a nonzero 32-character
    /// lowercase hexadecimal Workbench directory ID.
    DirectoryId,
    MatchText,
    /// The `terminal wait-output --regex` operand: a nonempty UTF-8 pattern
    /// of at most 1024 bytes that compiles inside the declared budgets.
    RegexPattern,
    /// The `terminal read --lines` operand: the presentation tail length.
    TailLines,
    OutputSource,
    OutputLines,
    WaitTimeout,
    AgentStartTimeout,
    AgentStartProvider,
    AgentStartArgument,
    SiteStopId,
    SendText,
    SendKey,
    /// The repeatable `--key` sequence bound: at most 64 values per request.
    SendKeySequence,
    PromptText,
    CommandText,
    MouseMode,
    Executable,
    LaunchArguments,
    LaunchEnvironment,
    AgentSessionId,
    AgentName,
    AgentListLimit,
    AgentCursor,
    /// The `agent wait --until` operand: one of the fourteen
    /// condition names.
    AgentWaitUntil,
    /// The `agent wait --after-revision` operand: a nonnegative
    /// safe-integer global runtime revision.
    AgentAfterRevision,
    /// The `agent doctor --provider` value: one canonical provider id.
    ProviderId,
    /// The `agent doctor --asset-root` value: a nonempty absolute path.
    AssetRootPath,
    /// The `launch-profile create --name` value: a required non-unique
    /// catalog name, 1..=512 UTF-8 bytes.
    ProfileName,
    /// A `launch-profile` selector or `--profile` value: id-first exact
    /// then name cardinality, 1..=512 UTF-8 bytes.
    ProfileSelector,
    /// `agent integration --provider <id>`: the closed provider vocabulary.
    Provider,
    /// `agent integration --adapter-source <abs>`: a literal absolute path.
    AdapterSource,
    /// The `workspace close-status|close-resume --project` operand: a
    /// nonzero 32-character lowercase hexadecimal Project ID.
    ProjectId,
    /// The `workspace close-status|close-resume --workspace` operand: a
    /// nonzero 32-character lowercase hexadecimal Workspace ID.
    WorkspaceId,
    /// The `workspace close-resume --request` operand: the close operation's
    /// own nonzero 32-character lowercase hexadecimal request ID.
    CloseRequestId,
    /// The `workspace close-resume --generation` operand: the caller's
    /// last-observed nonzero attempt generation, canonical decimal text.
    CloseGeneration,
}

/// Why an `invalid-usage` failure was reached. Carried for the localized
/// human stderr diagnostic only — never serialized into the machine envelope
/// and never a second parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UsageReason {
    /// Reserved fallback for a real failure without a more precise
    /// classification; every current exit names its concrete reason.
    #[expect(
        dead_code,
        reason = "fallback kept for exits that cannot honestly name a cause"
    )]
    Invalid,
    /// Non-Unicode argv or argv over the total byte bound; nothing echoes.
    ArgumentEnvelope,
    UnknownCommand {
        context: UsageContext,
        word: String,
    },
    UnknownOption {
        option: String,
    },
    UnexpectedOption {
        context: UsageContext,
        option: &'static str,
    },
    DuplicateOption {
        option: &'static str,
    },
    ConflictingOptions {
        context: UsageContext,
        first: &'static str,
        second: &'static str,
    },
    MissingValue {
        option: &'static str,
    },
    /// `requirement` is the display-ready required form: `'--label'` or
    /// `<tab>` — flag spellings and operand placeholders are locale-neutral.
    MissingRequired {
        context: UsageContext,
        requirement: &'static str,
    },
    /// Missing or invalid Windows caller identity, before runtime discovery.
    CallerContextUnavailable,
    UnexpectedArgument {
        context: UsageContext,
        value: String,
    },
    MissingCommand {
        context: UsageContext,
    },
    UnknownHelpTopic {
        path: String,
    },
    MisplacedFlag {
        context: UsageContext,
        flag: &'static str,
    },
    InvalidValue {
        context: UsageContext,
        option: &'static str,
        value: String,
        expected: ExpectedValue,
    },
    InvalidGeometry {
        context: UsageContext,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    },
    /// A declared `--file`/`--stdin` source failed local validation before
    /// any runtime effect; `cause` names the closed local cause set and
    /// `value` echoes only the `--file` operand — never payload bytes.
    SendSource {
        option: &'static str,
        value: Option<String>,
        cause: SendSourceCause,
    },
}

/// The closed causes a declared `terminal send` source can fail with before
/// any byte reaches the wire; rendered per locale, never serialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SendSourceCause {
    /// `--file` did not resolve to a regular file: a directory, FIFO,
    /// device, or a nonregular replacement opened at that path.
    NotRegularFile,
    /// `--stdin` was a terminal; declared sources never read interactive
    /// input.
    InteractiveTerminal,
    /// `--stdin` was a descriptor kind the source cannot read, such as a
    /// socket.
    UnsupportedStream,
    /// The source produced no bytes.
    Empty,
    /// The source exceeded the declared form's byte bound — the literal
    /// byte record bound including the folded `--enter` CR, or the paste
    /// content bound under `--paste`.
    Oversize { limit: usize },
    /// The source could not be opened or read.
    Unreadable,
}

#[derive(Clone, Debug, Eq)]
pub(crate) struct ParseFailure {
    pub(crate) kind: ParseFailureKind,
    pub(crate) envelope: FailureEnvelope,
    /// The stderr-only usage classification for `InvalidUsage` failures;
    /// `None` for other kinds and equality ignores it — the machine contract
    /// is `kind` plus `envelope`.
    pub(crate) usage: Option<UsageReason>,
}

impl PartialEq for ParseFailure {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.envelope == other.envelope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    HelpFlag(&'static str),
    VersionFlag,
    SkillFlag,
}

#[derive(Default)]
struct Options {
    output: Option<String>,
    client: Option<String>,
    focus: bool,
    no_focus: bool,
    zoom_on: bool,
    zoom_off: bool,
    zoom_toggle: bool,
    group: bool,
    branch: Option<String>,
    base: Option<String>,
    force: bool,
    trust_repository: bool,
    request_id: Option<String>,
    detach: bool,
    retry: bool,
    format: Option<OutputFormat>,
    project: Option<String>,
    directory: Option<String>,
    workspace: Option<String>,
    tab: Option<String>,
    standalone: bool,
    label: Option<String>,
    direction: Option<String>,
    insert_index: Option<String>,
    amount: Option<String>,
    ratio: Option<String>,
    right_click: Option<String>,
    cols: Option<String>,
    rows: Option<String>,
    cell_width: Option<String>,
    cell_height: Option<String>,
    confirm: bool,
    panel: Option<String>,
    current: bool,
    target_panel: Option<String>,
    swap_target: Option<String>,
    /// `panel move --into-tab <t>`: the sibling-Tab relocation target.
    into_tab: Option<String>,
    /// `panel move --new-tab`: the new-Tab relocation destination flag.
    new_tab: bool,
    history: bool,
    /// `terminal read --unwrap`: join verified soft-wrap rows into logical
    /// lines for presentation only.
    unwrap: bool,
    /// `terminal read --lines <n>`: presentation tail over emitted entries.
    lines: Option<String>,
    output_source: Option<String>,
    plain: bool,
    ansi: bool,
    path: Option<String>,
    match_text: Option<String>,
    /// `terminal wait-output --regex <pattern>`: the XOR alternative to the
    /// literal `--match` value; compiled once before any runtime effect.
    regex: Option<String>,
    timeout: Option<String>,
    text: Option<String>,
    enter: bool,
    /// Every `--key` occurrence in argv order: the repeatable value option
    /// builds the ordered key/combo record sequence.
    keys: Vec<String>,
    file: Option<String>,
    stdin: bool,
    /// `--paste`: the declared `--file`/`--stdin` source rides the
    /// Host-selected bracketed-paste record instead of a literal write.
    paste: bool,
    mouse: Option<bool>,
    exec: Option<String>,
    cwd: Option<String>,
    environment: Vec<String>,
    args: Vec<String>,
    include_ended: bool,
    limit: Option<String>,
    cursor: Option<String>,
    /// `agent wait --until <condition>`: the repeatable condition value —
    /// every occurrence appends one raw argv value in order, validated and
    /// deduplicated when the command binds it.
    until: Vec<String>,
    /// `agent wait --after-revision <n>`: the revision-fence value option.
    after_revision: Option<String>,
    /// `agent doctor --provider <id>`: repeatable, append-ordered.
    providers: Vec<String>,
    /// `agent doctor --asset-root <path>`: repeatable, append-ordered.
    asset_roots: Vec<String>,
    name: Option<String>,
    session: Option<String>,
    clear: bool,
    prompt_wait: bool,
    forever: bool,
    profile: Option<String>,
    provider: Option<String>,
    kind: Option<String>,
    adapter_source: Option<String>,
    /// `workspace close-resume --request <id>`: the close operation's own
    /// request ID — the operation identity, never the resume's new
    /// request identity.
    request: Option<String>,
    /// `workspace close-resume --generation <n>`: the explicit attempt
    /// fence; a stale value is a settled `stale` answer, never an effect.
    generation: Option<String>,
}

/// Every named option that owns the next argv token as its value, used by
/// both the format-envelope walk and the option scan so ownership is decided
/// identically in the two passes.
const VALUE_FLAGS: &[&str] = &[
    "--output",
    "--client",
    "--branch",
    "--base",
    "--request-id",
    "--adapter-source",
    "--after-revision",
    "--arg",
    "--asset-root",
    "--cell-height",
    "--cell-width",
    "--cols",
    "--cursor",
    "--direction",
    "--insert-index",
    "--amount",
    "--ratio",
    "--right-click",
    "--directory",
    "--exec",
    "--cwd",
    "--env",
    "--file",
    "--generation",
    "--into-tab",
    "--with",
    "--key",
    "--kind",
    "--label",
    "--limit",
    "--source",
    "--lines",
    "--path",
    "--match",
    "--mouse",
    "--name",
    "--session",
    "--project",
    "--provider",
    "--profile",
    "--regex",
    "--request",
    "--rows",
    "--tab",
    "--panel",
    "--target-panel",
    "--timeout",
    "--text",
    "--until",
    "--workspace",
];

const MAXIMUM_LABEL_SCALARS: usize = 512;
const ZERO_IDENTITY: &str = "00000000000000000000000000000000";

fn lowercase_hex(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn valid_typed_id(value: &str) -> bool {
    value.len() == 32 && lowercase_hex(value.as_bytes())
}

/// A selector suffix is one to thirty-one lowercase hex characters — a tail
/// of a member's typed identifier, never the full 32-character id shape.
fn valid_selector_suffix(suffix: &str) -> bool {
    (1..32).contains(&suffix.len()) && lowercase_hex(suffix.as_bytes())
}

fn valid_label(value: &str) -> bool {
    let scalars = value.chars().count();
    (1..=MAXIMUM_LABEL_SCALARS).contains(&scalars)
}

fn target(
    value: String,
    resource: Resource,
    envelope: FailureEnvelope,
) -> Result<Target, ParseFailure> {
    let identifier = |target: String| ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
        usage: None,
    };
    // A recognized `@<kind>:` prefix claims selector syntax wherever it
    // appears: the kind must equal the operand's resource and the suffix must
    // be a short lowercase-hex tail — both are parse-time `invalid-identifier`
    // failures. An `@` token without a recognized prefix falls through to the
    // label rule unchanged.
    let selector = value
        .strip_prefix('@')
        .and_then(|rest| rest.split_once(':'))
        .and_then(|(kind, suffix)| {
            Resource::selector_token(kind).map(|kind| (kind, valid_selector_suffix(suffix)))
        });
    if let Some((kind, admitted)) = selector {
        if !admitted || kind != resource {
            return Err(identifier(value));
        }
        return Ok(Target::Selector(value));
    }
    if valid_typed_id(&value) {
        if value == ZERO_IDENTITY {
            return Err(identifier(value));
        }
        return Ok(Target::Id(value));
    }
    match resource {
        Resource::Panel | Resource::Terminal => Err(identifier(value)),
        _ if valid_label(&value) => Ok(Target::Label(value)),
        _ => Err(identifier(value)),
    }
}

/// An `invalid-usage` failure carrying the concrete reason the original exit
/// reached; the reason rides the same envelope untouched.
fn usage_with(reason: UsageReason, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope,
        usage: Some(reason),
    }
}

fn identifier(resource: Resource, target: String, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
        usage: None,
    }
}

fn selection_envelope(format: Option<OutputFormat>) -> FailureEnvelope {
    match format {
        Some(OutputFormat::Json) => FailureEnvelope::Json,
        Some(OutputFormat::Text) | None => FailureEnvelope::Text,
    }
}

/// Parse the full argv tail into a typed request. The argv envelope (UTF-8 and
/// total byte budget) is validated before any option interpretation.
pub(crate) fn parse_request(
    arguments: impl Iterator<Item = OsString>,
) -> Result<Request, ParseFailure> {
    let mut tokens = Vec::new();
    let mut total_bytes = 0_usize;
    for argument in arguments {
        let Some(text) = argument.to_str() else {
            return Err(usage_with(
                UsageReason::ArgumentEnvelope,
                FailureEnvelope::Silent,
            ));
        };
        total_bytes += text.len();
        if total_bytes > MAXIMUM_ARGUMENT_BYTES {
            return Err(usage_with(
                UsageReason::ArgumentEnvelope,
                FailureEnvelope::Silent,
            ));
        }
        tokens.push(text.to_owned());
    }
    scan(tokens)
}

/// Phase A: scan the complete pre-`--` format envelope before any grammar
/// work. A missing, unknown, empty, or repeated selection anywhere in the
/// envelope is a silent format failure, and one valid unique selection shapes
/// every later grammar failure regardless of its position. Filter flags own
/// the next token even when it resembles a selector, so this walk applies the
/// same left-to-right ownership as the option scan below; a bare `--` is never
/// consumed as a value.
fn format_envelope(tokens: &[String]) -> Result<Option<OutputFormat>, ParseFailure> {
    let missing = || {
        usage_with(
            UsageReason::MissingValue { option: "--format" },
            FailureEnvelope::Silent,
        )
    };
    let invalid = |value: &str| {
        usage_with(
            UsageReason::InvalidValue {
                context: UsageContext::Root,
                option: "--format",
                value: value.to_owned(),
                expected: ExpectedValue::Format,
            },
            FailureEnvelope::Silent,
        )
    };
    let duplicate = || {
        usage_with(
            UsageReason::DuplicateOption { option: "--format" },
            FailureEnvelope::Silent,
        )
    };
    let mut format = None;
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        match token {
            "--" => break,
            "--format" => {
                let Some(value) = tokens.get(index + 1).filter(|value| value.as_str() != "--")
                else {
                    return Err(missing());
                };
                let selected = match value.as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(invalid(value)),
                };
                if format.replace(selected).is_some() {
                    return Err(duplicate());
                }
                index += 2;
            }
            _ if token.starts_with("--format=") => {
                let selected = match &token["--format=".len()..] {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(invalid(&token["--format=".len()..])),
                };
                if format.replace(selected).is_some() {
                    return Err(duplicate());
                }
                index += 1;
            }
            _ if VALUE_FLAGS.contains(&token) => {
                index += match tokens.get(index + 1) {
                    Some(value) if value != "--" => 2,
                    _ => 1,
                };
            }
            _ => index += 1,
        }
    }
    Ok(format)
}

fn scan(tokens: Vec<String>) -> Result<Request, ParseFailure> {
    let mut options = Options {
        format: format_envelope(&tokens)?,
        ..Options::default()
    };
    let envelope = selection_envelope(options.format);
    let mut positionals: Vec<Token> = Vec::new();
    let mut ended = false;
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if ended {
            positionals.push(Token::Word(token.clone()));
            index += 1;
            continue;
        }
        match token.as_str() {
            "--" => {
                ended = true;
                index += 1;
            }
            // The envelope phase already proved every selector is valid and
            // unique; the option walk only applies token ownership.
            "--format" => index += 2,
            _ if token.starts_with("--format=") => index += 1,
            // `--mouse` is a TUI-scoped on/off switch; it parses eagerly so a
            // malformed value fails here rather than at command resolution.
            // Like every declared value option it admits both spellings —
            // `--mouse off|on` and `--mouse=off|on` — but its value is the
            // closed {off,on} vocabulary validated here, so the joined form
            // enters this arm rather than the generic business-flag one.
            _ if token == "--mouse" || token.starts_with("--mouse=") => {
                let (value, width) = if token == "--mouse" {
                    match tokens.get(index + 1) {
                        Some(value) if value != "--" => (value.as_str(), 2),
                        _ => {
                            return Err(usage_with(
                                UsageReason::MissingValue { option: "--mouse" },
                                envelope,
                            ));
                        }
                    }
                } else {
                    (&token["--mouse=".len()..], 1)
                };
                let Some(selected) = mouse_option(value) else {
                    return Err(usage_with(
                        UsageReason::InvalidValue {
                            context: UsageContext::Root,
                            option: "--mouse",
                            value: value.to_owned(),
                            expected: ExpectedValue::MouseMode,
                        },
                        envelope,
                    ));
                };
                if options.mouse.replace(selected).is_some() {
                    return Err(usage_with(
                        UsageReason::DuplicateOption { option: "--mouse" },
                        envelope,
                    ));
                }
                index += width;
            }
            _ if VALUE_FLAGS.contains(&token.as_str()) => {
                // The guard admitted this spelling, so the static name exists.
                let option = VALUE_FLAGS
                    .iter()
                    .copied()
                    .find(|flag| *flag == token.as_str())
                    .expect("flag admitted by guard");
                let value = match tokens.get(index + 1) {
                    Some(value) if value != "--" => value.clone(),
                    _ => {
                        return Err(usage_with(UsageReason::MissingValue { option }, envelope));
                    }
                };
                apply_value_flag(&mut options, option, value, envelope)?;
                index += 2;
            }
            // The joined spelling `--name=<value>` is the declared twin of
            // `--name <value>` for every value option (command-surface
            // Options.Values): the token splits at the FIRST `=` and the
            // remainder is the verbatim value — embedded `=`, spaces,
            // Unicode, and leading `-` are all data, so `--arg=--` keeps
            // its literal terminator value and `--arg=` keeps the empty
            // argv element. `--mouse=<off|on>` is already claimed by its
            // own arm above and `--format=` is consumed by the envelope
            // phase; any other `--x=y` falls through to the unknown-option
            // rejection below.
            _ if joined_value_option(token).is_some() => {
                let option = joined_value_option(token).expect("admitted by guard");
                let value = token[option.len() + 1..].to_owned();
                apply_value_flag(&mut options, option, value, envelope)?;
                index += 1;
            }
            "--group" | "--standalone" | "--confirm" | "--history" | "--enter" | "--stdin"
            | "--include-ended" | "--new-tab" | "--paste" | "--unwrap" | "--current"
            | "--plain" | "--ansi" | "--clear" | "--wait" | "--forever" | "--detach"
            | "--retry" | "--force" | "--trust-repository" | "--on" | "--off" | "--toggle"
            | "--focus" | "--no-focus" => {
                let (seen, option) = if token == "--plain" {
                    (&mut options.plain, "--plain")
                } else if token == "--ansi" {
                    (&mut options.ansi, "--ansi")
                } else if token == "--focus" {
                    (&mut options.focus, "--focus")
                } else if token == "--no-focus" {
                    (&mut options.no_focus, "--no-focus")
                } else if token == "--on" {
                    (&mut options.zoom_on, "--on")
                } else if token == "--off" {
                    (&mut options.zoom_off, "--off")
                } else if token == "--toggle" {
                    (&mut options.zoom_toggle, "--toggle")
                } else if token == "--force" {
                    (&mut options.force, "--force")
                } else if token == "--trust-repository" {
                    (&mut options.trust_repository, "--trust-repository")
                } else if token == "--standalone" {
                    (&mut options.standalone, "--standalone")
                } else if token == "--current" {
                    (&mut options.current, "--current")
                } else if token == "--forever" {
                    (&mut options.forever, "--forever")
                } else if token == "--wait" {
                    (&mut options.prompt_wait, "--wait")
                } else if token == "--clear" {
                    (&mut options.clear, "--clear")
                } else if token == "--group" {
                    (&mut options.group, "--group")
                } else if token == "--confirm" {
                    (&mut options.confirm, "--confirm")
                } else if token == "--detach" {
                    (&mut options.detach, "--detach")
                } else if token == "--retry" {
                    (&mut options.retry, "--retry")
                } else if token == "--history" {
                    (&mut options.history, "--history")
                } else if token == "--stdin" {
                    (&mut options.stdin, "--stdin")
                } else if token == "--unwrap" {
                    (&mut options.unwrap, "--unwrap")
                } else if token == "--include-ended" {
                    (&mut options.include_ended, "--include-ended")
                } else if token == "--new-tab" {
                    (&mut options.new_tab, "--new-tab")
                } else if token == "--paste" {
                    (&mut options.paste, "--paste")
                } else {
                    (&mut options.enter, "--enter")
                };
                if *seen {
                    return Err(usage_with(
                        UsageReason::DuplicateOption { option },
                        envelope,
                    ));
                }
                *seen = true;
                index += 1;
            }
            "--help" | "-h" => {
                positionals.push(Token::HelpFlag(if token == "--help" {
                    "--help"
                } else {
                    "-h"
                }));
                index += 1;
            }
            "--version" => {
                positionals.push(Token::VersionFlag);
                index += 1;
            }
            "--skill" => {
                positionals.push(Token::SkillFlag);
                index += 1;
            }
            _ if token.starts_with('-') && token.len() > 1 => {
                return Err(usage_with(
                    UsageReason::UnknownOption {
                        option: token.clone(),
                    },
                    envelope,
                ));
            }
            _ => {
                positionals.push(Token::Word(token.clone()));
                index += 1;
            }
        }
    }
    resolve(positionals, options)
}

/// The joined-spelling counterpart of `VALUE_FLAGS`: returns the declared
/// value flag when `token` is `--<flag>=<value>` — the `=` immediately
/// follows the flag name so a name that is merely a flag prefix
/// (`--argx=v`) never matches. `--mouse` is excluded because its own
/// earlier arm already claims `--mouse=<off|on>` for its closed-vocabulary
/// validation; it must never fall into a generic slot. `--format` is
/// absent from `VALUE_FLAGS` by construction, so the envelope-phase
/// spelling never reaches here.
fn joined_value_option(token: &str) -> Option<&'static str> {
    if !token.starts_with("--") {
        return None;
    }
    VALUE_FLAGS.iter().copied().find(|flag| {
        *flag != "--mouse"
            && token.len() > flag.len()
            && token.starts_with(flag)
            && token.as_bytes()[flag.len()] == b'='
    })
}

/// Apply one collected value-option occurrence from either spelling: the
/// repeatable options append one verbatim value each and every other slot
/// is single-use — a second occurrence in either spelling is the same
/// duplicate usage failure. The repeatable value options: every occurrence
/// appends one verbatim argv value in order — `--arg` builds the launch
/// argv; `--provider`/`--asset-root` build the doctor's two declared
/// selector lists. `--provider` is additionally mirrored into the
/// single-use `provider` slot read by the `agent integration` commands,
/// whose single-use contract is enforced at resolve time from the list's
/// length. Value validation itself belongs to resolve; this layer only
/// assigns.
fn apply_value_flag(
    options: &mut Options,
    option: &'static str,
    value: String,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    if matches!(option, "--arg" | "--provider" | "--asset-root" | "--env") {
        match option {
            "--arg" => options.args.push(value),
            "--env" => options.environment.push(value),
            "--provider" => {
                options.provider = Some(value.clone());
                options.providers.push(value);
            }
            _ => options.asset_roots.push(value),
        }
        return Ok(());
    }
    if option == "--key" {
        options.keys.push(value);
        return Ok(());
    }
    if option == "--until" {
        options.until.push(value);
        return Ok(());
    }
    let slot = match option {
        "--output" => &mut options.output,
        "--branch" => &mut options.branch,
        "--base" => &mut options.base,
        "--project" => &mut options.project,
        "--directory" => &mut options.directory,
        "--workspace" => &mut options.workspace,
        "--tab" => &mut options.tab,
        "--label" => &mut options.label,
        "--direction" => &mut options.direction,
        "--insert-index" => &mut options.insert_index,
        "--amount" => &mut options.amount,
        "--ratio" => &mut options.ratio,
        "--right-click" => &mut options.right_click,
        "--cols" => &mut options.cols,
        "--rows" => &mut options.rows,
        "--cell-width" => &mut options.cell_width,
        "--cell-height" => &mut options.cell_height,
        "--path" => &mut options.path,
        "--match" => &mut options.match_text,
        "--timeout" => &mut options.timeout,
        "--kind" => &mut options.kind,
        "--text" => &mut options.text,
        "--file" => &mut options.file,
        "--exec" => &mut options.exec,
        "--cwd" => &mut options.cwd,
        "--limit" => &mut options.limit,
        "--cursor" => &mut options.cursor,
        "--after-revision" => &mut options.after_revision,
        "--name" => &mut options.name,
        "--session" => &mut options.session,
        "--profile" => &mut options.profile,
        "--adapter-source" => &mut options.adapter_source,
        "--regex" => &mut options.regex,
        "--request" => &mut options.request,
        "--request-id" => &mut options.request_id,
        "--client" => &mut options.client,
        "--generation" => &mut options.generation,
        "--source" => &mut options.output_source,
        "--lines" => &mut options.lines,
        "--panel" => &mut options.panel,
        "--target-panel" => &mut options.target_panel,
        "--into-tab" => &mut options.into_tab,
        "--with" => &mut options.swap_target,

        // Every reachable `option` is a `VALUE_FLAGS` member and the match
        // is total over that list — `--mouse` is the sole member diverted
        // to its own validated arm upstream. A future member missing its
        // slot arm must fail loudly here, not assign into an unrelated
        // field.
        _ => unreachable!("value option {option} lacks a slot arm"),
    };
    if slot.replace(value).is_some() {
        return Err(usage_with(
            UsageReason::DuplicateOption { option },
            envelope,
        ));
    }
    Ok(())
}

fn resolve(positionals: Vec<Token>, options: Options) -> Result<Request, ParseFailure> {
    let envelope = selection_envelope(options.format);
    if positionals.len() == 1 && matches!(positionals[0], Token::SkillFlag) {
        reject_extras(&options, UsageContext::Literal("orchbee --skill"), envelope)?;
        return Ok(Request::Skill);
    }
    // `--version` is legal only as the sole root invocation.
    if positionals.len() == 1 && matches!(positionals[0], Token::VersionFlag) {
        reject_extras(
            &options,
            UsageContext::Literal("orchbee --version"),
            envelope,
        )?;
        return Ok(Request::Version);
    }
    // A trailing -h/--help applies to a recognized help path only.
    let (path, suffix_help) = match positionals.last() {
        Some(Token::HelpFlag(_)) => (&positionals[..positionals.len() - 1], true),
        _ => (&positionals[..], false),
    };
    if suffix_help {
        return resolve_help_suffix(path, &options, envelope);
    }
    if let Some(flag) = path.iter().find_map(flag_literal) {
        let words: Vec<&str> = path.iter().filter_map(word_literal).collect();
        return Err(usage_with(
            UsageReason::MisplacedFlag {
                context: help_context(&words),
                flag,
            },
            envelope,
        ));
    }
    let words: Vec<&str> = path
        .iter()
        .map(|token| match token {
            Token::Word(word) => word.as_str(),
            Token::HelpFlag(_) | Token::VersionFlag | Token::SkillFlag => {
                unreachable!("flags excluded above")
            }
        })
        .collect();
    resolve_words(&words, &options, envelope)
}

fn flag_literal(token: &Token) -> Option<&'static str> {
    match token {
        Token::HelpFlag(literal) => Some(literal),
        Token::VersionFlag => Some("--version"),
        Token::SkillFlag => Some("--skill"),
        Token::Word(_) => None,
    }
}

fn word_literal(token: &Token) -> Option<&str> {
    match token {
        Token::Word(word) => Some(word.as_str()),
        Token::HelpFlag(_) | Token::VersionFlag | Token::SkillFlag => None,
    }
}

/// The help-path context of already-recognized words; unrecognized paths fall
/// back to the root so a hint never points at a nonexistent topic.
fn help_context(words: &[&str]) -> UsageContext {
    match help_topic(words) {
        Some(HelpTopic::Family(family)) => UsageContext::Path(family.as_str()),
        Some(HelpTopic::Command(command)) => UsageContext::Path(command.as_str()),
        None => UsageContext::Root,
    }
}

fn resolve_help_suffix(
    tokens: &[Token],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let words: Vec<&str> = tokens.iter().filter_map(word_literal).collect();
    let context = help_context(&words);
    // -h/--help must be the sole suffix of a recognized help path; other flag
    // tokens, a selected format, filters, or business arguments make it usage.
    reject_extras(options, context, envelope)?;
    if let Some(flag) = tokens.iter().find_map(flag_literal) {
        return Err(usage_with(
            UsageReason::MisplacedFlag { context, flag },
            envelope,
        ));
    }
    if words.is_empty() {
        return Ok(Request::RootHelp);
    }
    help_topic(&words).map(Request::Help).ok_or_else(|| {
        usage_with(
            UsageReason::UnknownHelpTopic {
                path: words.join(" "),
            },
            envelope,
        )
    })
}

fn resolve_words(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    match words {
        ["client", ..]
        | ["layout", ..]
        | ["agent", "focus" | "attention", ..]
        | ["workspace" | "tab", "focus", ..]
        | [
            "panel",
            "focus" | "zoom" | "layout" | "neighbor" | "edges" | "resize" | "swap",
            ..,
        ] => client_control::parse(words, options, envelope),

        [] => {
            // Bare invocation keeps TUI; a selected format or any option
            // (including `--mouse`) is usage — the flag exists only on the
            // explicit `tui` path.
            if options.format.is_some()
                || options.project.is_some()
                || options.workspace.is_some()
                || options.tab.is_some()
                || options.standalone
                || options.mouse.is_some()
                || business_options(options)
            {
                return Err(missing_command(UsageContext::Root, envelope));
            }
            Ok(Request::Tui { mouse: true })
        }
        ["tui"] => reject_extras_scope(options, TUI_SCOPE, UsageContext::Path("tui"), envelope)
            .map(|()| Request::Tui {
                mouse: options.mouse.unwrap_or(true),
            }),
        ["help", rest @ ..] => resolve_help_path(rest, options, envelope),
        ["status"] => {
            reject_filters(options, UsageContext::Path("status"), envelope)?;
            Ok(query(QueryCommand::Status, options))
        }
        ["status", scope @ ("server" | "client")] => {
            let (scope, path) = if *scope == "server" {
                (RuntimeStatusScope::Server, "status server")
            } else {
                (RuntimeStatusScope::Client, "status client")
            };
            reject_filters(options, UsageContext::Path(path), envelope)?;
            Ok(Request::RuntimeStatus {
                scope,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "changes"] => {
            let context = UsageContext::Path("agent changes");
            check_scope(options, AGENT_CHANGES_SCOPE, context, envelope)?;
            let cursor = agent_cursor_option(options, context, envelope)?.map(|value| {
                orchbee_agent_observation_protocol::AgentChangesCursor::parse(value.as_str())
                    .expect("shared bounded cursor encoding")
            });
            Ok(query(
                QueryCommand::AgentChanges {
                    limit: agent_limit_option(options, context, envelope)?,
                    cursor,
                },
                options,
            ))
        }
        ["agent", "list"] => {
            check_scope(
                options,
                AGENT_LIST_SCOPE,
                UsageContext::Path("agent list"),
                envelope,
            )?;
            let context = UsageContext::Path("agent list");
            Ok(query(
                QueryCommand::AgentList {
                    include_ended: options.include_ended,
                    limit: agent_limit_option(options, context, envelope)?,
                    cursor: agent_cursor_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["agent", verb @ ("get" | "explain"), rest @ ..] => {
            let context = UsageContext::Path(if *verb == "get" {
                "agent get"
            } else {
                "agent explain"
            });
            check_scope(
                options,
                Scope {
                    client: *verb == "get",
                    ..AGENT_TARGET_SCOPE
                },
                context,
                envelope,
            )?;
            let target = agent_selector(rest, options, context, envelope)?;
            Ok(query(
                if *verb == "get" {
                    if let Some(client) = client_control::client_id(options, context, envelope)? {
                        QueryCommand::AgentClientGet { target, client }
                    } else {
                        QueryCommand::AgentGet(target)
                    }
                } else {
                    QueryCommand::AgentExplain(target)
                },
                options,
            ))
        }
        ["agent", "read", rest @ ..] => {
            let context = UsageContext::Path("agent read");
            check_scope(
                options,
                Scope {
                    name: true,
                    session: true,
                    panel: true,
                    ..TERMINAL_READ_SCOPE
                },
                context,
                envelope,
            )?;
            if options.output_source.is_some() || options.plain || options.ansi {
                return terminal_output::request(
                    OutputTarget::Agent(agent_selector(rest, options, context, envelope)?),
                    options,
                    context,
                    envelope,
                );
            }
            let history = match (options.history, options.rows.is_some()) {
                (true, true) => Some(history_rows_option(options, context, envelope)?),
                (true, false) => return Err(missing_required(context, "'--rows'", envelope)),
                (false, true) => return Err(missing_required(context, "'--history'", envelope)),
                (false, false) => None,
            };
            Ok(query(
                QueryCommand::AgentRead {
                    target: agent_selector(rest, options, context, envelope)?,
                    history,
                    unwrap: options.unwrap,
                    lines: tail_lines_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["agent", "start", rest @ ..] => {
            let context = UsageContext::Path("agent start");
            check_scope(
                options,
                Scope {
                    panel: true,
                    current: true,
                    kind: true,
                    timeout: true,
                    args: true,
                    ..NO_OPTIONS
                },
                context,
                envelope,
            )?;
            let invalid = |option, value: &str, expected| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option,
                        value: value.to_owned(),
                        expected,
                    },
                    envelope,
                )
            };
            let Some(name) = rest.first() else {
                return Err(missing_required(context, "<name>", envelope));
            };
            let name = agent_name(name, context, envelope)?;
            let provider = options
                .kind
                .as_deref()
                .ok_or_else(|| missing_required(context, "'--kind'", envelope))?;
            if crate::agent_provider::executable(provider).is_none() {
                return Err(invalid(
                    "--kind",
                    provider,
                    ExpectedValue::AgentStartProvider,
                ));
            }
            let timeout_ms = match options.timeout.as_deref() {
                None => 30000,
                Some(value) => value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (3001..=300000).contains(n))
                    .ok_or_else(|| invalid("--timeout", value, ExpectedValue::AgentStartTimeout))?,
            };
            if options.current && options.panel.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--current",
                        second: "--panel",
                    },
                    envelope,
                ));
            }
            if rest.len() > 1 && !options.args.is_empty() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--arg",
                        second: "<args>",
                    },
                    envelope,
                ));
            }
            let args = if options.args.is_empty() {
                rest[1..].iter().map(|v| (*v).to_owned()).collect()
            } else {
                options.args.clone()
            };
            for arg in &args {
                if arg.chars().any(char::is_control) {
                    return Err(invalid("<args>", arg, ExpectedValue::AgentStartArgument));
                }
            }
            let panel = options
                .panel
                .as_deref()
                .map(|value| target(value.to_owned(), Resource::Panel, envelope))
                .transpose()?;
            Ok(Request::AgentStart {
                command: AgentStartCommand {
                    name,
                    provider: provider.to_owned(),
                    panel,
                    args,
                    timeout_ms,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "prompt", rest @ ..] => {
            let context = UsageContext::Path("agent prompt");
            check_scope(
                options,
                Scope {
                    prompt_wait: true,
                    forever: true,
                    until: true,
                    timeout: true,
                    ..AGENT_TARGET_SCOPE
                },
                context,
                envelope,
            )?;
            let selector_len = usize::from(
                options.name.is_none()
                    && options.session.is_none()
                    && options.panel.is_none()
                    && !rest.is_empty(),
            );
            let target = agent_selector(&rest[..selector_len], options, context, envelope)?;
            let values = &rest[selector_len..];
            let Some(text) = values.first() else {
                return Err(missing_required(context, "<text>", envelope));
            };
            if values.len() > 1 {
                return Err(usage_with(
                    UsageReason::UnexpectedArgument {
                        context,
                        value: values[1].to_owned(),
                    },
                    envelope,
                ));
            }
            if text.is_empty() || text.len() > 256 * 1024 {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<text>",
                        value: String::new(),
                        expected: ExpectedValue::PromptText,
                    },
                    envelope,
                ));
            }
            if !options.prompt_wait
                && (!options.until.is_empty() || options.timeout.is_some() || options.forever)
            {
                return Err(missing_required(context, "'--wait'", envelope));
            }
            let wait = if options.prompt_wait {
                Some(AgentPromptWait {
                    until: agent_until_option(options, context, envelope)?,
                    timeout_ms: agent_wait_timeout_option(options, context, envelope)?,
                })
            } else {
                None
            };
            Ok(mutation(
                MutationCommand::AgentPrompt {
                    target,
                    text: (*text).to_owned(),
                    wait,
                },
                options,
            ))
        }
        ["agent", "send-keys", rest @ ..] => {
            let context = UsageContext::Path("agent send-keys");
            check_scope(options, AGENT_TARGET_SCOPE, context, envelope)?;
            let selector_len = usize::from(
                options.name.is_none()
                    && options.session.is_none()
                    && options.panel.is_none()
                    && !rest.is_empty(),
            );
            let target = agent_selector(&rest[..selector_len], options, context, envelope)?;
            let values = &rest[selector_len..];
            if values.is_empty() {
                return Err(missing_required(context, "<key>...", envelope));
            }
            if values.len() > MAXIMUM_SEND_RECORDS {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<key>",
                        value: values[MAXIMUM_SEND_RECORDS].to_owned(),
                        expected: ExpectedValue::SendKeySequence,
                    },
                    envelope,
                ));
            }
            let keys = values
                .iter()
                .map(|value| {
                    parse_send_key(value).ok_or_else(|| {
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "<key>",
                                value: (*value).to_owned(),
                                expected: ExpectedValue::SendKey,
                            },
                            envelope,
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(mutation(
                MutationCommand::AgentSendKeys { target, keys },
                options,
            ))
        }
        ["agent", "wait", rest @ ..] => {
            let context = UsageContext::Path("agent wait");
            check_scope(options, AGENT_WAIT_SCOPE, context, envelope)?;
            Ok(query(
                QueryCommand::AgentWait {
                    session: agent_selector(rest, options, context, envelope)?,
                    until: agent_until_option(options, context, envelope)?,
                    after_revision: agent_after_revision_option(options, context, envelope)?,
                    timeout_ms: agent_wait_timeout_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["agent", "names"] => {
            reject_filters(options, UsageContext::Path("agent names"), envelope)?;
            Ok(Request::AgentNames {
                command: AgentNamesCommand::List,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "resolve", rest @ ..] => {
            let context = UsageContext::Path("agent resolve");
            check_scope(options, AGENT_TARGET_SCOPE, context, envelope)?;
            Ok(Request::AgentNames {
                command: AgentNamesCommand::Resolve(agent_selector(
                    rest, options, context, envelope,
                )?),
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "rename", rest @ ..] => {
            let context = UsageContext::Path("agent rename");
            check_scope(
                options,
                Scope {
                    clear: true,
                    ..AGENT_TARGET_SCOPE
                },
                context,
                envelope,
            )?;
            let command = if options.clear {
                AgentNamesCommand::Release(agent_selector(rest, options, context, envelope)?)
            } else {
                let Some((value, selectors)) = rest.split_last() else {
                    return Err(usage_with(
                        UsageReason::MissingRequired {
                            context,
                            requirement: "<target> <new-name> or --clear",
                        },
                        envelope,
                    ));
                };
                AgentNamesCommand::Assign {
                    target: agent_selector(selectors, options, context, envelope)?,
                    name: agent_name(value, context, envelope)?,
                }
            };
            Ok(Request::AgentNames {
                command,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "doctor"] => {
            let context = UsageContext::Path("agent doctor");
            check_scope(options, AGENT_DOCTOR_SCOPE, context, envelope)?;
            Ok(query(
                QueryCommand::AgentDoctor {
                    providers: doctor_providers_option(options, context, envelope)?,
                    asset_roots: doctor_asset_roots_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["agent", "integration", "list"] => {
            let context = UsageContext::Path("agent integration list");
            check_scope(options, INTEGRATION_SCOPE, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::List {
                    provider: provider_option(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "integration", "explain"] => {
            let context = UsageContext::Path("agent integration explain");
            check_scope(options, INTEGRATION_SCOPE, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::Explain {
                    provider: required_provider(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "integration", "enable"] => {
            let context = UsageContext::Path("agent integration enable");
            check_scope(options, INTEGRATION_SCOPE, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::Enable {
                    provider: required_provider(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "integration", "disable"] => {
            let context = UsageContext::Path("agent integration disable");
            check_scope(options, INTEGRATION_SCOPE, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::Disable {
                    provider: required_provider(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["project", "list"] => {
            reject_filters(options, UsageContext::Path("project list"), envelope)?;
            Ok(query(QueryCommand::ProjectList, options))
        }
        ["project", "get", value] => {
            reject_filters(options, UsageContext::Path("project get"), envelope)?;
            Ok(query(
                QueryCommand::ProjectGet(target((*value).to_owned(), Resource::Project, envelope)?),
                options,
            ))
        }
        ["project", "create"] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("project create"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::ProjectCreate {
                    label: label_option(options, UsageContext::Path("project create"), envelope)?,
                },
                options,
            ))
        }
        ["project", "rename", value] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("project rename"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::ProjectRename {
                    target: target((*value).to_owned(), Resource::Project, envelope)?,
                    label: label_option(options, UsageContext::Path("project rename"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "list"] => {
            check_scope(
                options,
                WORKSPACE_LIST_SCOPE,
                UsageContext::Path("workspace list"),
                envelope,
            )?;
            let project = options
                .project
                .clone()
                .map(|value| target(value, Resource::Project, envelope))
                .transpose()?;
            Ok(query(QueryCommand::WorkspaceList { project }, options))
        }
        ["workspace", "get", value] => {
            reject_filters(options, UsageContext::Path("workspace get"), envelope)?;
            Ok(query(
                QueryCommand::WorkspaceGet(target(
                    (*value).to_owned(),
                    Resource::Workspace,
                    envelope,
                )?),
                options,
            ))
        }
        ["workspace", "create"] => {
            let context = UsageContext::Path("workspace create");
            if options.cwd.is_some() {
                check_scope(options, WORKSPACE_LAUNCH_SCOPE, context, envelope)?;
                let project = target(
                    required(&options.project, context, "'--project'", envelope)?,
                    Resource::Project,
                    envelope,
                )?;
                let label = label_option(options, context, envelope)?;
                let geometry = geometry_option(options, context, envelope)?;
                reject_launch_profile_conflict(options, context, envelope)?;
                let launch = launch_option(options, context, envelope)?;
                let profile = profile_option(options, context, envelope)?;
                let launch_options = launch_options(options, context, envelope)?;
                Ok(mutation(
                    MutationCommand::WorkspaceLaunch {
                        project,
                        label,
                        geometry,
                        launch,
                        profile,
                        options: launch_options,
                    },
                    options,
                ))
            } else {
                check_scope(options, WORKSPACE_CREATE_SCOPE, context, envelope)?;
                Ok(mutation(
                    MutationCommand::WorkspaceCreate {
                        project: target(
                            required(&options.project, context, "'--project'", envelope)?,
                            Resource::Project,
                            envelope,
                        )?,
                        label: label_option(options, context, envelope)?,
                    },
                    options,
                ))
            }
        }
        ["workspace", "rename", value] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("workspace rename"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::WorkspaceRename {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    label: label_option(options, UsageContext::Path("workspace rename"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "accept-directory", value] => {
            check_scope(
                options,
                ACCEPT_DIRECTORY_SCOPE,
                UsageContext::Path("workspace accept-directory"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::WorkspaceAcceptDirectory {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    path: path_option(
                        options,
                        UsageContext::Path("workspace accept-directory"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["workspace", "close", value] if options.group => {
            workspace_group_close::start(value, options, envelope)
        }
        ["workspace", verb @ ("close-status" | "close-resume")] if options.group => {
            workspace_group_close::retained(verb, options, envelope)
        }
        ["workspace", "close", value] => {
            check_scope(
                options,
                CONFIRM_SCOPE,
                UsageContext::Path("workspace close"),
                envelope,
            )?;
            if !options.confirm {
                return Err(missing_required(
                    UsageContext::Path("workspace close"),
                    "'--confirm'",
                    envelope,
                ));
            }
            Ok(mutation(
                MutationCommand::WorkspaceClose {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                },
                options,
            ))
        }
        ["workspace", "close-status"] => {
            check_scope(
                options,
                WORKSPACE_CLOSE_STATUS_SCOPE,
                UsageContext::Path("workspace close-status"),
                envelope,
            )?;
            Ok(query(
                QueryCommand::WorkspaceCloseStatus {
                    project: project_id_option(
                        options,
                        UsageContext::Path("workspace close-status"),
                        envelope,
                    )?,
                    workspace: workspace_id_option(
                        options,
                        UsageContext::Path("workspace close-status"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["workspace", "close-resume"] => {
            check_scope(
                options,
                WORKSPACE_CLOSE_RESUME_SCOPE,
                UsageContext::Path("workspace close-resume"),
                envelope,
            )?;
            let context = UsageContext::Path("workspace close-resume");
            let project = project_id_option(options, context, envelope)?;
            let workspace = workspace_id_option(options, context, envelope)?;
            let request = close_request_option(options, context, envelope)?;
            let generation = close_generation_option(options, context, envelope)?;
            if !options.confirm {
                return Err(missing_required(context, "'--confirm'", envelope));
            }
            Ok(mutation(
                MutationCommand::WorkspaceCloseResume {
                    project,
                    workspace,
                    request,
                    generation,
                },
                options,
            ))
        }
        ["tab", "list"] => {
            check_scope(
                options,
                TAB_LIST_SCOPE,
                UsageContext::Path("tab list"),
                envelope,
            )?;
            reject_conflict(
                options.standalone && options.workspace.is_some(),
                UsageContext::Path("tab list"),
                envelope,
            )?;
            let workspace = options
                .workspace
                .clone()
                .map(|value| target(value, Resource::Workspace, envelope))
                .transpose()?;
            Ok(query(
                QueryCommand::TabList {
                    workspace,
                    standalone: options.standalone,
                },
                options,
            ))
        }
        ["tab", "get", value] => {
            reject_filters(options, UsageContext::Path("tab get"), envelope)?;
            Ok(query(
                QueryCommand::TabGet(target((*value).to_owned(), Resource::Tab, envelope)?),
                options,
            ))
        }
        ["tab", "create"] => creation::parse(None, options, envelope),
        ["panel", "input", operands @ ..] => panel_input::parse(operands, options, envelope),
        ["panel", "rename", value] => {
            let context = UsageContext::Path("panel rename");
            check_scope(
                options,
                Scope {
                    clear: true,
                    ..LABEL_SCOPE
                },
                context,
                envelope,
            )?;
            reject_conflict(options.clear && options.label.is_some(), context, envelope)?;
            if !options.clear && options.label.is_none() {
                return Err(missing_required(
                    context,
                    "'--label' or '--clear'",
                    envelope,
                ));
            }
            let label =
                orchbee_workbench_protocol::PanelLabelUpdate::parse(options.label.as_deref())
                    .map_err(|_| {
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "--label",
                                value: options.label.clone().unwrap_or_default(),
                                expected: ExpectedValue::PanelLabel,
                            },
                            envelope,
                        )
                    })?;
            Ok(mutation(
                MutationCommand::PanelRename {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    label,
                },
                options,
            ))
        }
        ["tab", "rename", value] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("tab rename"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::TabRename {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    label: label_option(options, UsageContext::Path("tab rename"), envelope)?,
                },
                options,
            ))
        }
        ["tab", "move", value] => {
            check_scope(
                options,
                TAB_MOVE_SCOPE,
                UsageContext::Path("tab move"),
                envelope,
            )?;
            Ok(mutation(
                tab_move_command(value, options, envelope)?,
                options,
            ))
        }
        ["tab", "close", value] => {
            check_scope(
                options,
                CONFIRM_SCOPE,
                UsageContext::Path("tab close"),
                envelope,
            )?;
            if !options.confirm {
                return Err(missing_required(
                    UsageContext::Path("tab close"),
                    "'--confirm'",
                    envelope,
                ));
            }
            Ok(mutation(
                MutationCommand::TabClose {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                },
                options,
            ))
        }
        ["panel", "list"] => {
            check_scope(
                options,
                PANEL_LIST_SCOPE,
                UsageContext::Path("panel list"),
                envelope,
            )?;
            let tab = options
                .tab
                .clone()
                .map(|value| target(value, Resource::Tab, envelope))
                .transpose()?;
            Ok(query(QueryCommand::PanelList { tab }, options))
        }
        ["panel", "process-info", operands @ ..] => {
            let context = UsageContext::Path("panel process-info");
            check_scope(options, PANEL_CURRENT_SCOPE, context, envelope)?;
            if let [_, extra, ..] = operands {
                return Err(unexpected_argument(context, extra, envelope));
            }
            if (options.panel.is_some() && options.current)
                || (!operands.is_empty() && (options.panel.is_some() || options.current))
            {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: if operands.is_empty() {
                            "--panel"
                        } else {
                            "panel-id"
                        },
                        second: if options.current {
                            "--current"
                        } else {
                            "--panel"
                        },
                    },
                    envelope,
                ));
            }
            let panel = operands
                .first()
                .map(|value| (*value).to_owned())
                .or_else(|| options.panel.clone())
                .map(|value| target(value, Resource::Panel, envelope))
                .transpose()?;
            let target = panel_query_target(panel, options, context, envelope)?;
            Ok(query(QueryCommand::PanelProcessInfo(target), options))
        }
        ["panel", "current"] => {
            let context = UsageContext::Path("panel current");
            check_scope(options, PANEL_CURRENT_SCOPE, context, envelope)?;
            if options.panel.is_some() && options.current {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--panel",
                        second: "--current",
                    },
                    envelope,
                ));
            }
            let panel = options
                .panel
                .clone()
                .map(|value| target(value, Resource::Panel, envelope))
                .transpose()?;
            let target = panel_query_target(panel, options, context, envelope)?;
            Ok(query(QueryCommand::PanelCurrent(target), options))
        }
        ["panel", "current", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel current"),
            extra,
            envelope,
        )),
        ["panel", "get", value] => {
            reject_filters(options, UsageContext::Path("panel get"), envelope)?;
            Ok(query(
                QueryCommand::PanelGet(target((*value).to_owned(), Resource::Panel, envelope)?),
                options,
            ))
        }
        ["panel", "split", rest @ ..] => creation::parse(Some(rest), options, envelope),
        ["panel", "close", value] => {
            check_scope(
                options,
                CONFIRM_SCOPE,
                UsageContext::Path("panel close"),
                envelope,
            )?;
            if !options.confirm {
                return Err(missing_required(
                    UsageContext::Path("panel close"),
                    "'--confirm'",
                    envelope,
                ));
            }
            Ok(mutation(
                MutationCommand::PanelClose {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                },
                options,
            ))
        }
        ["panel", "move", value] => {
            check_scope(
                options,
                PANEL_MOVE_SCOPE,
                UsageContext::Path("panel move"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::PanelMove {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    destination: panel_move_destination(options, envelope)?,
                },
                options,
            ))
        }
        ["terminal", "list"] => {
            reject_filters(options, UsageContext::Path("terminal list"), envelope)?;
            Ok(query(QueryCommand::TerminalList, options))
        }
        ["terminal", "get", value] => {
            reject_filters(options, UsageContext::Path("terminal get"), envelope)?;
            Ok(query(
                QueryCommand::TerminalGet(target(
                    (*value).to_owned(),
                    Resource::Terminal,
                    envelope,
                )?),
                options,
            ))
        }
        ["terminal", "read", value] => {
            check_scope(
                options,
                TERMINAL_READ_SCOPE,
                UsageContext::Path("terminal read"),
                envelope,
            )?;
            let context = UsageContext::Path("terminal read");
            if options.output_source.is_some() || options.plain || options.ansi {
                return terminal_output::request(
                    OutputTarget::Terminal(target(
                        (*value).to_owned(),
                        Resource::Terminal,
                        envelope,
                    )?),
                    options,
                    context,
                    envelope,
                );
            }
            // `--history` and `--rows` are admitted as a pair only: each
            // without the other is a missing-requirement, never a default.
            let history = match (options.history, options.rows.is_some()) {
                (true, true) => Some(history_rows_option(options, context, envelope)?),
                (true, false) => {
                    return Err(missing_required(context, "'--rows'", envelope));
                }
                (false, true) => {
                    return Err(missing_required(context, "'--history'", envelope));
                }
                (false, false) => None,
            };
            Ok(query(
                QueryCommand::TerminalRead {
                    target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                    history,
                    unwrap: options.unwrap,
                    lines: tail_lines_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["terminal", "wait-output", value] => {
            check_scope(
                options,
                TERMINAL_WAIT_OUTPUT_SCOPE,
                UsageContext::Path("terminal wait-output"),
                envelope,
            )?;
            let context = UsageContext::Path("terminal wait-output");
            if options.output_source.is_some() || options.lines.is_some() || options.forever {
                return terminal_output::wait_request(
                    target((*value).to_owned(), Resource::Terminal, envelope)?,
                    options,
                    context,
                    envelope,
                );
            }
            // Exactly one of `--match`/`--regex` is required; `--rows` and
            // `--timeout` carry their documented defaults, and a regex
            // compiles here — everything resolves before any cohort,
            // socket, session, viewer, or terminal effect.
            Ok(query(
                QueryCommand::TerminalWaitOutput {
                    target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                    matcher: wait_matcher_option(options, context, envelope)?,
                    rows: wait_rows_option(options, context, envelope)?,
                    timeout_ms: wait_timeout_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["terminal", "run", operands @ ..] => terminal_run::request(operands, options, envelope),
        ["terminal", "send", value] => {
            check_scope(
                options,
                TERMINAL_SEND_SCOPE,
                UsageContext::Path("terminal send"),
                envelope,
            )?;
            let context = UsageContext::Path("terminal send");
            Ok(mutation(
                MutationCommand::TerminalSend {
                    target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                    input: send_input(options, context, envelope)?,
                },
                options,
            ))
        }
        ["worktree", verb @ ("create" | "open" | "remove")] => {
            worktree::start(verb, options, envelope)
        }
        ["worktree", verb @ ("status" | "resume"), id] => {
            worktree::observe(verb, id, options, envelope)
        }
        ["worktree", "status"] => Err(missing_required(
            UsageContext::Path("worktree status"),
            "<operation-id>",
            envelope,
        )),
        ["worktree", "resume"] => Err(missing_required(
            UsageContext::Path("worktree resume"),
            "<operation-id>",
            envelope,
        )),
        ["worktree", "list"] => worktree::list(options, envelope),
        ["launch-profile", "list"] => {
            reject_filters(options, UsageContext::Path("launch-profile list"), envelope)?;
            Ok(query(QueryCommand::LaunchProfileList, options))
        }
        ["launch-profile", "get", value] => {
            reject_filters(options, UsageContext::Path("launch-profile get"), envelope)?;
            Ok(query(
                QueryCommand::LaunchProfileGet(profile_selector(
                    value,
                    UsageContext::Path("launch-profile get"),
                    envelope,
                )?),
                options,
            ))
        }
        ["launch-profile", "create"] => {
            let context = UsageContext::Path("launch-profile create");
            check_scope(options, LAUNCH_PROFILE_CREATE_SCOPE, context, envelope)?;
            Ok(mutation(
                MutationCommand::LaunchProfileCreate {
                    name: profile_name_option(options, context, envelope)?,
                    launch: required_launch_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["launch-profile", "delete", value] => {
            reject_filters(
                options,
                UsageContext::Path("launch-profile delete"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::LaunchProfileDelete {
                    selector: profile_selector(
                        value,
                        UsageContext::Path("launch-profile delete"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["api", "snapshot"] => {
            reject_filters(options, UsageContext::Path("api snapshot"), envelope)?;
            Ok(query(QueryCommand::ApiSnapshot, options))
        }
        ["api", "schema"] => schema_export::request(None, options, envelope),
        ["api", "schema", schema_id] => schema_export::request(Some(schema_id), options, envelope),
        ["config", "check"] => {
            reject_filters(options, UsageContext::Path("config check"), envelope)?;
            Ok(query(QueryCommand::ConfigCheck, options))
        }
        ["site", "start"] => {
            reject_filters(options, UsageContext::Path("site start"), envelope)?;
            Ok(Request::SiteStart {
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["site", "diagnose"] => {
            reject_filters(options, UsageContext::Path("site diagnose"), envelope)?;
            Ok(Request::SiteDiagnose {
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["site", "stop"] => {
            let context = UsageContext::Path("site stop");
            check_scope(
                options,
                Scope {
                    confirm: true,
                    request_id: true,
                    detach: true,
                    retry: true,
                    timeout: true,
                    ..NO_OPTIONS
                },
                context,
                envelope,
            )?;
            if !options.confirm {
                return Err(missing_required(context, "'--confirm'", envelope));
            }
            if options.retry && options.request_id.is_none() {
                return Err(missing_required(context, "'--request-id'", envelope));
            }
            let id = options
                .request_id
                .as_deref()
                .map(|value| stop_id(value, context, "--request-id", envelope))
                .transpose()?;
            let timeout_ms = if options.timeout.is_some() {
                wait_timeout_option(options, context, envelope)?
            } else {
                65_000
            };
            Ok(Request::SiteStop {
                command: SiteStopCommand::Stop {
                    id,
                    retry: options.retry,
                    detach: options.detach,
                    timeout_ms,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["site", "stop-status", id] => {
            let context = UsageContext::Path("site stop-status");
            reject_filters(options, context, envelope)?;
            Ok(Request::SiteStop {
                command: SiteStopCommand::Status {
                    id: stop_id(id, context, "<request-id>", envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["site", "stop-status"] => Err(missing_required(
            UsageContext::Path("site stop-status"),
            "<request-id>",
            envelope,
        )),
        ["completion", shell] => {
            reject_extras(options, UsageContext::Path("completion"), envelope)?;
            let shell = match *shell {
                "bash" => CompletionShell::Bash,
                "zsh" => CompletionShell::Zsh,
                "fish" => CompletionShell::Fish,
                "powershell" => CompletionShell::PowerShell,
                "elvish" => CompletionShell::Elvish,
                _ => {
                    return Err(usage_with(
                        UsageReason::InvalidValue {
                            context: UsageContext::Path("completion"),
                            option: "<shell>",
                            value: (*shell).to_owned(),
                            expected: ExpectedValue::Shell,
                        },
                        envelope,
                    ));
                }
            };
            Ok(Request::Completion(shell))
        }
        // Command-scoped catch-alls classify the real failure — an extra
        // positional, a missing operand, a missing subcommand, or an
        // unrecognized word — after every legal arm has kept precedence.
        ["tui", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tui"),
            extra,
            envelope,
        )),
        ["status", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("status"),
            extra,
            envelope,
        )),
        ["agent"] => Err(missing_command(UsageContext::Path("agent"), envelope)),
        ["agent", "changes", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent changes"),
            extra,
            envelope,
        )),
        ["agent", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent list"),
            extra,
            envelope,
        )),
        ["agent", "doctor", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent doctor"),
            extra,
            envelope,
        )),
        ["agent", "integration"] => Err(missing_command(
            UsageContext::Path("agent integration"),
            envelope,
        )),
        ["agent", "integration", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent integration list"),
            extra,
            envelope,
        )),
        [
            "agent",
            "integration",
            verb @ ("explain" | "enable" | "disable"),
            extra,
            ..,
        ] => Err(unexpected_argument(
            UsageContext::Path(match *verb {
                "explain" => "agent integration explain",
                "enable" => "agent integration enable",
                _ => "agent integration disable",
            }),
            extra,
            envelope,
        )),
        ["agent", "integration", verb, ..] => Err(unknown_command(
            UsageContext::Path("agent integration"),
            verb,
            envelope,
        )),
        ["agent", verb, ..] => Err(unknown_command(UsageContext::Path("agent"), verb, envelope)),
        ["project"] => Err(missing_command(UsageContext::Path("project"), envelope)),
        ["project", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project list"),
            extra,
            envelope,
        )),
        ["project", "get"] => Err(missing_required(
            UsageContext::Path("project get"),
            "<project>",
            envelope,
        )),
        ["project", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project get"),
            extra,
            envelope,
        )),
        ["project", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project create"),
            extra,
            envelope,
        )),
        ["project", "rename"] => Err(missing_required(
            UsageContext::Path("project rename"),
            "<project>",
            envelope,
        )),
        ["project", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project rename"),
            extra,
            envelope,
        )),
        ["project", verb, ..] => Err(unknown_command(
            UsageContext::Path("project"),
            verb,
            envelope,
        )),
        ["workspace"] => Err(missing_command(UsageContext::Path("workspace"), envelope)),
        ["workspace", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace list"),
            extra,
            envelope,
        )),
        ["workspace", "get"] => Err(missing_required(
            UsageContext::Path("workspace get"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace get"),
            extra,
            envelope,
        )),
        ["workspace", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace create"),
            extra,
            envelope,
        )),
        ["workspace", "rename"] => Err(missing_required(
            UsageContext::Path("workspace rename"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace rename"),
            extra,
            envelope,
        )),
        ["workspace", "accept-directory"] => Err(missing_required(
            UsageContext::Path("workspace accept-directory"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "accept-directory", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace accept-directory"),
            extra,
            envelope,
        )),
        ["workspace", "close"] => Err(missing_required(
            UsageContext::Path("workspace close"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace close"),
            extra,
            envelope,
        )),
        ["workspace", "close-status", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace close-status"),
            extra,
            envelope,
        )),
        ["workspace", "close-resume", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace close-resume"),
            extra,
            envelope,
        )),
        ["workspace", verb, ..] => Err(unknown_command(
            UsageContext::Path("workspace"),
            verb,
            envelope,
        )),
        ["tab"] => Err(missing_command(UsageContext::Path("tab"), envelope)),
        ["tab", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab list"),
            extra,
            envelope,
        )),
        ["tab", "get"] => Err(missing_required(
            UsageContext::Path("tab get"),
            "<tab>",
            envelope,
        )),
        ["tab", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab get"),
            extra,
            envelope,
        )),
        ["tab", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab create"),
            extra,
            envelope,
        )),
        ["tab", "rename"] => Err(missing_required(
            UsageContext::Path("tab rename"),
            "<tab>",
            envelope,
        )),
        ["tab", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab rename"),
            extra,
            envelope,
        )),
        ["tab", "move"] => Err(missing_required(
            UsageContext::Path("tab move"),
            "<tab>",
            envelope,
        )),
        ["tab", "move", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab move"),
            extra,
            envelope,
        )),
        ["tab", "close"] => Err(missing_required(
            UsageContext::Path("tab close"),
            "<tab>",
            envelope,
        )),
        ["tab", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab close"),
            extra,
            envelope,
        )),
        ["tab", verb, ..] => Err(unknown_command(UsageContext::Path("tab"), verb, envelope)),
        ["panel"] => Err(missing_command(UsageContext::Path("panel"), envelope)),
        ["panel", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel list"),
            extra,
            envelope,
        )),
        ["panel", "get"] => Err(missing_required(
            UsageContext::Path("panel get"),
            "<panel>",
            envelope,
        )),
        ["panel", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel get"),
            extra,
            envelope,
        )),
        ["panel", "close"] => Err(missing_required(
            UsageContext::Path("panel close"),
            "<panel>",
            envelope,
        )),
        ["panel", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel close"),
            extra,
            envelope,
        )),
        ["panel", "move"] => Err(missing_required(
            UsageContext::Path("panel move"),
            "<panel>",
            envelope,
        )),
        ["panel", "move", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel move"),
            extra,
            envelope,
        )),
        ["panel", verb, ..] => Err(unknown_command(UsageContext::Path("panel"), verb, envelope)),
        ["terminal"] => Err(missing_command(UsageContext::Path("terminal"), envelope)),
        ["terminal", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal list"),
            extra,
            envelope,
        )),
        ["terminal", "get"] => Err(missing_required(
            UsageContext::Path("terminal get"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal get"),
            extra,
            envelope,
        )),
        ["terminal", "read"] => Err(missing_required(
            UsageContext::Path("terminal read"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "read", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal read"),
            extra,
            envelope,
        )),
        ["terminal", "wait-output"] => Err(missing_required(
            UsageContext::Path("terminal wait-output"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "wait-output", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal wait-output"),
            extra,
            envelope,
        )),
        ["terminal", "send"] => Err(missing_required(
            UsageContext::Path("terminal send"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "send", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal send"),
            extra,
            envelope,
        )),
        ["terminal", verb, ..] => Err(unknown_command(
            UsageContext::Path("terminal"),
            verb,
            envelope,
        )),
        ["worktree"] => Err(missing_command(UsageContext::Path("worktree"), envelope)),
        ["worktree", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("worktree list"),
            extra,
            envelope,
        )),
        ["worktree", verb @ ("create" | "open" | "remove"), extra, ..] => Err(unexpected_argument(
            UsageContext::Path(match *verb {
                "create" => "worktree create",
                "open" => "worktree open",
                _ => "worktree remove",
            }),
            extra,
            envelope,
        )),
        ["worktree", verb @ ("status" | "resume"), _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path(if *verb == "status" {
                "worktree status"
            } else {
                "worktree resume"
            }),
            extra,
            envelope,
        )),
        ["worktree", verb, ..] => Err(unknown_command(
            UsageContext::Path("worktree"),
            verb,
            envelope,
        )),
        ["launch-profile"] => Err(missing_command(
            UsageContext::Path("launch-profile"),
            envelope,
        )),
        ["launch-profile", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile list"),
            extra,
            envelope,
        )),
        ["launch-profile", "get"] => Err(missing_required(
            UsageContext::Path("launch-profile get"),
            "<profile>",
            envelope,
        )),
        ["launch-profile", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile get"),
            extra,
            envelope,
        )),
        ["launch-profile", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile create"),
            extra,
            envelope,
        )),
        ["launch-profile", "delete"] => Err(missing_required(
            UsageContext::Path("launch-profile delete"),
            "<profile>",
            envelope,
        )),
        ["launch-profile", "delete", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile delete"),
            extra,
            envelope,
        )),
        ["launch-profile", verb, ..] => Err(unknown_command(
            UsageContext::Path("launch-profile"),
            verb,
            envelope,
        )),
        ["api"] => Err(missing_command(UsageContext::Path("api"), envelope)),
        ["api", "snapshot", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("api snapshot"),
            extra,
            envelope,
        )),
        ["api", "schema", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("api schema"),
            extra,
            envelope,
        )),
        ["api", verb, ..] => Err(unknown_command(UsageContext::Path("api"), verb, envelope)),
        ["config"] => Err(missing_command(UsageContext::Path("config"), envelope)),
        ["config", "check", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("config check"),
            extra,
            envelope,
        )),
        ["config", verb, ..] => Err(unknown_command(
            UsageContext::Path("config"),
            verb,
            envelope,
        )),
        ["site"] => Err(missing_command(UsageContext::Path("site"), envelope)),
        ["site", "start", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("site start"),
            extra,
            envelope,
        )),
        ["site", "diagnose", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("site diagnose"),
            extra,
            envelope,
        )),
        ["site", "stop", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("site stop"),
            extra,
            envelope,
        )),
        ["site", "stop-status", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("site stop-status"),
            extra,
            envelope,
        )),
        ["site", verb, ..] => Err(unknown_command(UsageContext::Path("site"), verb, envelope)),
        ["completion"] => Err(missing_required(
            UsageContext::Path("completion"),
            "<shell>",
            envelope,
        )),
        ["completion", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("completion"),
            extra,
            envelope,
        )),
        [word, ..] => Err(unknown_command(UsageContext::Root, word, envelope)),
    }
}

fn resolve_help_path(
    rest: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    // A recognized topic anchors its own help path; an unrecognized one keeps
    // the literal `orchbee help` display so no hint points at a fake topic.
    let context = match help_topic(rest) {
        Some(HelpTopic::Family(family)) => UsageContext::Path(family.as_str()),
        Some(HelpTopic::Command(command)) => UsageContext::Path(command.as_str()),
        None => UsageContext::Literal("orchbee help"),
    };
    reject_extras(options, context, envelope)?;
    if rest.is_empty() {
        return Ok(Request::RootHelp);
    }
    help_topic(rest).map(Request::Help).ok_or_else(|| {
        usage_with(
            UsageReason::UnknownHelpTopic {
                path: rest.join(" "),
            },
            envelope,
        )
    })
}

/// The one recognized-help-path table shared by `help <path>` and a trailing
/// `-h`/`--help`: families map to their overview, complete command paths to
/// their command help. Anything else is not a help path at all.
fn help_topic(path: &[&str]) -> Option<HelpTopic> {
    Some(match path {
        ["client"] => HelpTopic::Family(Family::Client),
        ["client", "list"] => HelpTopic::Command(CommandPath::ClientList),
        ["client", "get"] => HelpTopic::Command(CommandPath::ClientGet),
        ["client", "control-status"] => HelpTopic::Command(CommandPath::ClientControlStatus),
        ["workspace", "focus"] => HelpTopic::Command(CommandPath::WorkspaceFocus),
        ["tab", "focus"] => HelpTopic::Command(CommandPath::TabFocus),
        ["panel", "focus"] => HelpTopic::Command(CommandPath::PanelFocus),
        ["panel", "resize"] => HelpTopic::Command(CommandPath::PanelResize),
        ["layout"] => HelpTopic::Family(Family::Layout),
        ["layout", "set-split-ratio"] => HelpTopic::Command(CommandPath::LayoutSetSplitRatio),
        ["panel", "zoom"] => HelpTopic::Command(CommandPath::PanelZoom),
        ["panel", "layout"] => HelpTopic::Command(CommandPath::PanelLayout),
        ["panel", "neighbor"] => HelpTopic::Command(CommandPath::PanelNeighbor),
        ["panel", "edges"] => HelpTopic::Command(CommandPath::PanelEdges),

        ["tui"] => HelpTopic::Command(CommandPath::Tui),
        ["status"] => HelpTopic::Command(CommandPath::Status),
        ["status", "server"] => HelpTopic::Command(CommandPath::StatusServer),
        ["status", "client"] => HelpTopic::Command(CommandPath::StatusClient),
        ["agent"] => HelpTopic::Family(Family::Agent),
        ["agent", "list"] => HelpTopic::Command(CommandPath::AgentList),
        ["agent", "changes"] => HelpTopic::Command(CommandPath::AgentChanges),
        ["agent", "get"] => HelpTopic::Command(CommandPath::AgentGet),
        ["agent", "focus"] => HelpTopic::Command(CommandPath::AgentFocus),
        ["agent", "attention"] => HelpTopic::Command(CommandPath::AgentAttention),
        ["agent", "read"] => HelpTopic::Command(CommandPath::AgentRead),
        ["agent", "start"] => HelpTopic::Command(CommandPath::AgentStart),
        ["agent", "prompt"] => HelpTopic::Command(CommandPath::AgentPrompt),
        ["agent", "send-keys"] => HelpTopic::Command(CommandPath::AgentSendKeys),
        ["agent", "explain"] => HelpTopic::Command(CommandPath::AgentExplain),
        ["agent", "wait"] => HelpTopic::Command(CommandPath::AgentWait),
        ["agent", "doctor"] => HelpTopic::Command(CommandPath::AgentDoctor),
        ["agent", "names"] => HelpTopic::Command(CommandPath::AgentNames),
        ["agent", "resolve"] => HelpTopic::Command(CommandPath::AgentResolve),
        ["agent", "rename"] => HelpTopic::Command(CommandPath::AgentRename),
        ["agent", "integration"] => HelpTopic::Family(Family::AgentIntegration),
        ["agent", "integration", "list"] => HelpTopic::Command(CommandPath::AgentIntegrationList),
        ["agent", "integration", "explain"] => {
            HelpTopic::Command(CommandPath::AgentIntegrationExplain)
        }
        ["agent", "integration", "enable"] => {
            HelpTopic::Command(CommandPath::AgentIntegrationEnable)
        }
        ["agent", "integration", "disable"] => {
            HelpTopic::Command(CommandPath::AgentIntegrationDisable)
        }
        ["project"] => HelpTopic::Family(Family::Project),
        ["workspace"] => HelpTopic::Family(Family::Workspace),
        ["tab"] => HelpTopic::Family(Family::Tab),
        ["panel"] => HelpTopic::Family(Family::Panel),
        ["terminal"] => HelpTopic::Family(Family::Terminal),
        ["api"] => HelpTopic::Family(Family::Api),
        ["site"] => HelpTopic::Family(Family::Site),
        ["config"] => HelpTopic::Family(Family::Config),
        ["completion"] => HelpTopic::Family(Family::Completion),
        ["project", "list"] => HelpTopic::Command(CommandPath::ProjectList),
        ["project", "get"] => HelpTopic::Command(CommandPath::ProjectGet),
        ["project", "create"] => HelpTopic::Command(CommandPath::ProjectCreate),
        ["project", "rename"] => HelpTopic::Command(CommandPath::ProjectRename),
        ["workspace", "list"] => HelpTopic::Command(CommandPath::WorkspaceList),
        ["workspace", "get"] => HelpTopic::Command(CommandPath::WorkspaceGet),
        ["workspace", "create"] => HelpTopic::Command(CommandPath::WorkspaceCreate),
        ["workspace", "rename"] => HelpTopic::Command(CommandPath::WorkspaceRename),
        ["workspace", "accept-directory"] => {
            HelpTopic::Command(CommandPath::WorkspaceAcceptDirectory)
        }
        ["workspace", "close"] => HelpTopic::Command(CommandPath::WorkspaceClose),
        ["workspace", "close-status"] => HelpTopic::Command(CommandPath::WorkspaceCloseStatus),
        ["workspace", "close-resume"] => HelpTopic::Command(CommandPath::WorkspaceCloseResume),
        ["tab", "list"] => HelpTopic::Command(CommandPath::TabList),
        ["tab", "get"] => HelpTopic::Command(CommandPath::TabGet),
        ["tab", "create"] => HelpTopic::Command(CommandPath::TabCreate),
        ["tab", "rename"] => HelpTopic::Command(CommandPath::TabRename),
        ["panel", "rename"] => HelpTopic::Command(CommandPath::PanelRename),
        ["panel", "input"] => HelpTopic::Command(CommandPath::PanelInput),
        ["panel", "swap"] => HelpTopic::Command(CommandPath::PanelSwap),
        ["tab", "move"] => HelpTopic::Command(CommandPath::TabMove),
        ["tab", "close"] => HelpTopic::Command(CommandPath::TabClose),
        ["panel", "list"] => HelpTopic::Command(CommandPath::PanelList),
        ["panel", "current"] => HelpTopic::Command(CommandPath::PanelCurrent),
        ["panel", "get"] => HelpTopic::Command(CommandPath::PanelGet),
        ["panel", "process-info"] => HelpTopic::Command(CommandPath::PanelProcessInfo),
        ["panel", "split"] => HelpTopic::Command(CommandPath::PanelSplit),
        ["panel", "close"] => HelpTopic::Command(CommandPath::PanelClose),
        ["panel", "move"] => HelpTopic::Command(CommandPath::PanelMove),
        ["terminal", "list"] => HelpTopic::Command(CommandPath::TerminalList),
        ["terminal", "get"] => HelpTopic::Command(CommandPath::TerminalGet),
        ["terminal", "read"] => HelpTopic::Command(CommandPath::TerminalRead),
        ["terminal", "wait-output"] => HelpTopic::Command(CommandPath::TerminalWaitOutput),
        ["terminal", "send"] => HelpTopic::Command(CommandPath::TerminalSend),
        ["terminal", "run"] => HelpTopic::Command(CommandPath::TerminalRun),

        ["worktree"] => HelpTopic::Family(Family::Worktree),
        ["worktree", "list"] => HelpTopic::Command(CommandPath::WorktreeList),
        ["worktree", "create"] => HelpTopic::Command(CommandPath::WorktreeCreate),
        ["worktree", "open"] => HelpTopic::Command(CommandPath::WorktreeOpen),
        ["worktree", "remove"] => HelpTopic::Command(CommandPath::WorktreeRemove),
        ["worktree", "status"] => HelpTopic::Command(CommandPath::WorktreeStatus),
        ["worktree", "resume"] => HelpTopic::Command(CommandPath::WorktreeResume),

        ["launch-profile"] => HelpTopic::Family(Family::LaunchProfile),
        ["launch-profile", "list"] => HelpTopic::Command(CommandPath::LaunchProfileList),
        ["launch-profile", "get"] => HelpTopic::Command(CommandPath::LaunchProfileGet),
        ["launch-profile", "create"] => HelpTopic::Command(CommandPath::LaunchProfileCreate),
        ["launch-profile", "delete"] => HelpTopic::Command(CommandPath::LaunchProfileDelete),

        ["api", "snapshot"] => HelpTopic::Command(CommandPath::ApiSnapshot),
        ["api", "schema"] => HelpTopic::Command(CommandPath::ApiSchema),
        ["config", "check"] => HelpTopic::Command(CommandPath::ConfigCheck),
        ["site", "start"] => HelpTopic::Command(CommandPath::SiteStart),
        ["site", "diagnose"] => HelpTopic::Command(CommandPath::SiteDiagnose),
        ["site", "stop"] => HelpTopic::Command(CommandPath::SiteStop),
        ["site", "stop-status"] => HelpTopic::Command(CommandPath::SiteStopStatus),
        _ => return None,
    })
}

fn query(command: QueryCommand, options: &Options) -> Request {
    Request::Query {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    }
}

fn panel_query_target(
    panel: Option<Target>,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<PanelQueryTarget, ParseFailure> {
    if panel.is_some() && options.client.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "panel-id/--panel",
                second: "--client",
            },
            envelope,
        ));
    }
    if let Some(panel) = panel {
        return Ok(PanelQueryTarget::Panel(panel));
    }
    Ok(client_control::client_id(options, context, envelope)?
        .map_or(PanelQueryTarget::Current, PanelQueryTarget::Client))
}

fn mutation(command: MutationCommand, options: &Options) -> Request {
    Request::Mutation {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    }
}

/// The named-option set one command admits; anything seen outside it is usage.
/// `geometry` covers all four dimension flags together: a terminal geometry
/// is only meaningful whole.
#[derive(Clone, Copy)]
struct Scope {
    output: bool,
    client: bool,
    focus: bool,
    no_focus: bool,
    zoom_on: bool,
    zoom_off: bool,
    zoom_toggle: bool,
    group: bool,
    branch: bool,
    base: bool,
    force: bool,
    trust_repository: bool,
    request_id: bool,
    detach: bool,
    retry: bool,
    project: bool,
    /// `worktree list --directory <id>`: admits the strict directory-ID
    /// selector; it is an ID flag, never a path.
    directory: bool,
    workspace: bool,
    tab: bool,
    standalone: bool,
    label: bool,
    direction: bool,
    insert_index: bool,
    amount: bool,
    ratio: bool,
    right_click: bool,
    geometry: bool,
    confirm: bool,
    panel: bool,
    current: bool,
    target_panel: bool,
    swap_target: bool,
    /// `panel move --into-tab <t>`: admits the sibling-Tab relocation
    /// destination selector.
    into_tab: bool,
    /// `panel move --new-tab`: admits the new-Tab relocation destination
    /// flag.
    new_tab: bool,
    /// `terminal read --history --rows <n>`: admits the flag and the
    /// history-row meaning of the shared `--rows` value option.
    history: bool,
    /// `terminal read --unwrap`: admits the logical-line presentation flag.
    unwrap: bool,
    /// `terminal read --lines <n>`: admits the presentation tail option.
    lines: bool,
    output_source: bool,
    plain: bool,
    ansi: bool,
    /// `workspace accept-directory --path <p>`: admits the literal directory
    /// path value option.
    path: bool,
    /// `terminal wait-output --match <t> [--rows <n>] [--timeout <ms>]`:
    /// admits all three flags and the wait-window meaning of `--rows`.
    wait: bool,
    /// `terminal wait-output --regex <pattern>`: admits the compiled-pattern
    /// alternative to `--match`.
    regex: bool,
    /// `terminal send --text <t>`: the literal input-bytes value option.
    text: bool,
    /// `terminal send --enter`: the text form's same-record CR modifier.
    enter: bool,
    /// `terminal send --key <name>`: the semantic-key value option.
    key: bool,
    /// `terminal send --file <path>`: the declared regular-file source.
    file: bool,
    /// `terminal send --stdin`: the declared noninteractive input stream.
    stdin: bool,
    /// `tui --mouse off|on`: admits the TUI-only pointer-capture switch.
    mouse: bool,
    /// `tab create|panel split --exec <abs>`: the explicit launch executable.
    exec: bool,
    cwd: bool,
    environment: bool,
    /// `tab create|panel split --arg <v>`: the repeatable launch-argv option.
    args: bool,
    /// `agent list --include-ended`: admits the lifecycle filter flag.
    include_ended: bool,
    /// `agent list --limit <n>`: admits the page-size value option.
    limit: bool,
    /// `agent list --cursor <c>`: admits the page cursor value option.
    cursor: bool,
    /// `agent wait --until <c>`: admits the repeatable wait-condition value
    /// option.
    until: bool,
    /// `agent wait --after-revision <n>`: admits the revision-fence value
    /// option.
    after_revision: bool,
    /// `agent wait --timeout <ms>`: admits the shared `--timeout` flag under
    /// the wait meaning alone — never `terminal wait-output`'s `--match`
    /// or `--rows` peers.
    timeout: bool,
    /// `agent doctor --provider <id>` / `agent integration * --provider
    /// <id>`: the provider selector — repeatable on doctor; a filter on
    /// integration `list` and a required selector on explain/enable/disable.
    provider: bool,
    kind: bool,
    /// `agent doctor --asset-root <path>`: admits the repeatable declared
    /// asset-root selector.
    asset_root: bool,
    /// `launch-profile create --name <n>`: the required catalog name.
    name: bool,
    session: bool,
    clear: bool,
    prompt_wait: bool,
    forever: bool,
    /// `tab create|panel split --profile <sel>`: the catalog-launch
    /// selector, exclusive with the explicit `--exec`/`--arg` form.
    profile: bool,
    /// `agent integration * --adapter-source <abs>`: the development/test
    /// capsule-source override (literal absolute path).
    adapter_source: bool,
    /// `workspace close-resume --request <id>`: the close operation's own
    /// request ID.
    request: bool,
    /// `workspace close-resume --generation <n>`: the attempt fence.
    generation: bool,
}

const NO_OPTIONS: Scope = Scope {
    output: false,
    client: false,
    focus: false,
    no_focus: false,
    zoom_on: false,
    zoom_off: false,
    zoom_toggle: false,
    group: false,
    branch: false,
    base: false,
    force: false,
    trust_repository: false,
    request_id: false,
    detach: false,
    retry: false,
    project: false,
    directory: false,
    workspace: false,
    tab: false,
    standalone: false,
    label: false,
    direction: false,
    insert_index: false,
    amount: false,
    ratio: false,
    right_click: false,
    geometry: false,
    confirm: false,
    panel: false,
    current: false,
    target_panel: false,
    swap_target: false,
    into_tab: false,
    new_tab: false,
    history: false,
    unwrap: false,
    lines: false,
    output_source: false,
    plain: false,
    ansi: false,
    path: false,
    wait: false,
    regex: false,
    text: false,
    enter: false,
    key: false,
    file: false,
    stdin: false,
    mouse: false,
    exec: false,
    cwd: false,
    environment: false,
    args: false,
    include_ended: false,
    limit: false,
    cursor: false,
    until: false,
    after_revision: false,
    timeout: false,
    provider: false,
    kind: false,
    asset_root: false,
    name: false,
    session: false,
    clear: false,
    prompt_wait: false,
    forever: false,
    profile: false,
    adapter_source: false,
    request: false,
    generation: false,
};

/// The explicit `tui` route: only `--mouse` is admitted on top of the
/// shared option grammar.
const TUI_SCOPE: Scope = Scope {
    mouse: true,
    ..NO_OPTIONS
};

/// `agent list`: the three pagination/filter options of the read grammar.
const AGENT_TARGET_SCOPE: Scope = Scope {
    panel: true,
    name: true,
    session: true,
    ..NO_OPTIONS
};

const AGENT_CHANGES_SCOPE: Scope = Scope {
    limit: true,
    cursor: true,
    ..NO_OPTIONS
};

const AGENT_LIST_SCOPE: Scope = Scope {
    include_ended: true,
    limit: true,
    cursor: true,
    ..NO_OPTIONS
};

/// `agent wait`: the condition list, the revision fence, and the shared
/// timeout flag under the canonical wait meaning — the sole Agent scope
/// that admits `--timeout`.
const AGENT_WAIT_SCOPE: Scope = Scope {
    panel: true,
    forever: true,
    name: true,
    session: true,
    until: true,
    after_revision: true,
    timeout: true,
    ..NO_OPTIONS
};

/// `agent doctor`: the two repeatable declared selectors — provider ids and
/// supplied asset roots.
const AGENT_DOCTOR_SCOPE: Scope = Scope {
    provider: true,
    asset_root: true,
    ..NO_OPTIONS
};

const WORKSPACE_LIST_SCOPE: Scope = Scope {
    project: true,
    ..NO_OPTIONS
};

const TAB_LIST_SCOPE: Scope = Scope {
    workspace: true,
    standalone: true,
    ..NO_OPTIONS
};

const PANEL_CURRENT_SCOPE: Scope = Scope {
    panel: true,
    current: true,
    client: true,
    ..NO_OPTIONS
};

const PANEL_LIST_SCOPE: Scope = Scope {
    tab: true,
    ..NO_OPTIONS
};

const WORKSPACE_LAUNCH_SCOPE: Scope = Scope {
    project: true,
    label: true,
    geometry: true,
    exec: true,
    cwd: true,
    environment: true,
    args: true,
    profile: true,
    ..NO_OPTIONS
};

const WORKSPACE_CREATE_SCOPE: Scope = Scope {
    project: true,
    label: true,
    ..NO_OPTIONS
};

const LABEL_SCOPE: Scope = Scope {
    label: true,
    ..NO_OPTIONS
};

const ACCEPT_DIRECTORY_SCOPE: Scope = Scope {
    path: true,
    ..NO_OPTIONS
};

const TAB_CREATE_SCOPE: Scope = Scope {
    client: true,
    focus: true,
    no_focus: true,
    timeout: true,
    workspace: true,
    standalone: true,
    label: true,
    geometry: true,
    exec: true,
    cwd: true,
    environment: true,
    args: true,
    profile: true,
    ..NO_OPTIONS
};

const TAB_MOVE_SCOPE: Scope = Scope {
    direction: true,
    insert_index: true,
    ..NO_OPTIONS
};

const CONFIRM_SCOPE: Scope = Scope {
    confirm: true,
    ..NO_OPTIONS
};

/// `workspace close-status`: the stable-ID scope — the operation record is
/// addressed by the exact Project and Workspace IDs, never a positional
/// target.
const WORKSPACE_CLOSE_STATUS_SCOPE: Scope = Scope {
    project: true,
    workspace: true,
    ..NO_OPTIONS
};

/// `workspace close-resume`: the stable-ID scope plus the close operation's
/// own request ID, the attempt-generation fence, and the explicit confirm.
const WORKSPACE_CLOSE_RESUME_SCOPE: Scope = Scope {
    project: true,
    workspace: true,
    request: true,
    generation: true,
    confirm: true,
    ..NO_OPTIONS
};

const PANEL_SPLIT_SCOPE: Scope = Scope {
    right_click: true,
    panel: true,
    current: true,
    ratio: true,
    client: true,
    focus: true,
    no_focus: true,
    timeout: true,
    label: true,
    direction: true,
    geometry: true,
    exec: true,
    cwd: true,
    environment: true,
    args: true,
    profile: true,
    ..NO_OPTIONS
};

const PANEL_MOVE_SCOPE: Scope = Scope {
    direction: true,
    target_panel: true,
    into_tab: true,
    new_tab: true,
    label: true,
    ..NO_OPTIONS
};

/// `terminal read`: the paired history selector plus the two read-only
/// presentation transforms — logical-line joining and the entry tail.
const TERMINAL_READ_SCOPE: Scope = Scope {
    output_source: true,
    plain: true,
    ansi: true,
    history: true,
    unwrap: true,
    lines: true,
    ..NO_OPTIONS
};

const TERMINAL_WAIT_OUTPUT_SCOPE: Scope = Scope {
    output_source: true,
    lines: true,
    forever: true,
    wait: true,
    regex: true,
    ..NO_OPTIONS
};

const TERMINAL_SEND_SCOPE: Scope = Scope {
    text: true,
    enter: true,
    key: true,
    file: true,
    stdin: true,
    ..NO_OPTIONS
};

const WORKTREE_LIST_SCOPE: Scope = Scope {
    project: true,
    directory: true,
    ..NO_OPTIONS
};

/// `launch-profile create --name <n> --exec <abs> [--arg <v>]...`: the
/// catalog entry's name plus the same explicit launch selection grammar.
const LAUNCH_PROFILE_CREATE_SCOPE: Scope = Scope {
    name: true,
    exec: true,
    args: true,
    ..NO_OPTIONS
};

/// `agent integration *`: the provider selector plus the development/test
/// adapter-source override — the only integration-scoped options.
const INTEGRATION_SCOPE: Scope = Scope {
    provider: true,
    adapter_source: true,
    ..NO_OPTIONS
};

/// Any mutation-only option seen outside a mutation command.
fn business_options(options: &Options) -> bool {
    options.output.is_some()
        || options.client.is_some()
        || options.focus
        || options.no_focus
        || options.zoom_on
        || options.zoom_off
        || options.zoom_toggle
        || options.request_id.is_some()
        || options.detach
        || options.retry
        || options.directory.is_some()
        || options.label.is_some()
        || options.direction.is_some()
        || options.amount.is_some()
        || options.ratio.is_some()
        || options.cols.is_some()
        || options.rows.is_some()
        || options.cell_width.is_some()
        || options.cell_height.is_some()
        || options.confirm
        || options.group
        || options.target_panel.is_some()
        || options.swap_target.is_some()
        || options.into_tab.is_some()
        || options.new_tab
        || options.history
        || options.unwrap
        || options.output_source.is_some()
        || options.plain
        || options.ansi
        || options.lines.is_some()
        || options.path.is_some()
        || options.match_text.is_some()
        || options.regex.is_some()
        || options.timeout.is_some()
        || options.text.is_some()
        || options.enter
        || !options.keys.is_empty()
        || options.file.is_some()
        || options.stdin
        || options.paste
        || options.exec.is_some()
        || !options.args.is_empty()
        || options.include_ended
        || options.limit.is_some()
        || options.cursor.is_some()
        || !options.until.is_empty()
        || options.after_revision.is_some()
        || !options.providers.is_empty()
        || !options.asset_roots.is_empty()
        || options.name.is_some()
        || options.session.is_some()
        || options.clear
        || options.profile.is_some()
        || options.provider.is_some()
        || options.adapter_source.is_some()
        || options.request.is_some()
        || options.generation.is_some()
}

/// The first named option present but outside `scope`, in the fixed `Options`
/// declaration order — deterministic without any argv-order state.
fn first_out_of_scope(options: &Options, scope: Scope) -> Option<&'static str> {
    if options.right_click.is_some() && !scope.right_click {
        return Some("--right-click");
    }
    if options.amount.is_some() && !scope.amount {
        return Some("--amount");
    }
    if options.ratio.is_some() && !scope.ratio {
        return Some("--ratio");
    }
    if options.output.is_some() && !scope.output {
        return Some("--output");
    }
    if options.branch.is_some() && !scope.branch {
        return Some("--branch");
    }
    if options.base.is_some() && !scope.base {
        return Some("--base");
    }
    if options.force && !scope.force {
        return Some("--force");
    }
    if options.trust_repository && !scope.trust_repository {
        return Some("--trust-repository");
    }
    if options.client.is_some() && !scope.client {
        return Some("--client");
    }
    if options.focus && !scope.focus {
        return Some("--focus");
    }
    if options.no_focus && !scope.no_focus {
        return Some("--no-focus");
    }
    if options.zoom_on && !scope.zoom_on {
        return Some("--on");
    }
    if options.zoom_off && !scope.zoom_off {
        return Some("--off");
    }
    if options.zoom_toggle && !scope.zoom_toggle {
        return Some("--toggle");
    }
    if options.request_id.is_some() && !scope.request_id {
        return Some("--request-id");
    }
    if options.detach && !scope.detach {
        return Some("--detach");
    }
    if options.retry && !scope.retry {
        return Some("--retry");
    }
    if options.project.is_some() && !scope.project {
        return Some("--project");
    }
    if options.directory.is_some() && !scope.directory {
        return Some("--directory");
    }
    if options.workspace.is_some() && !scope.workspace {
        return Some("--workspace");
    }
    if options.tab.is_some() && !scope.tab {
        return Some("--tab");
    }
    if options.standalone && !scope.standalone {
        return Some("--standalone");
    }
    if options.label.is_some() && !scope.label {
        return Some("--label");
    }
    if options.insert_index.is_some() && !scope.insert_index {
        return Some("--insert-index");
    }
    if options.direction.is_some() && !scope.direction {
        return Some("--direction");
    }
    if options.cols.is_some() && !scope.geometry {
        return Some("--cols");
    }
    // `--rows` is a geometry dimension under `geometry` scope, the paired
    // history row count under `history` scope, and the searched window under
    // `wait` scope; all three spellings share the flag.
    if options.rows.is_some() && !scope.geometry && !scope.history && !scope.wait {
        return Some("--rows");
    }
    if options.cell_width.is_some() && !scope.geometry {
        return Some("--cell-width");
    }
    if options.cell_height.is_some() && !scope.geometry {
        return Some("--cell-height");
    }
    if options.group && !scope.group {
        return Some("--group");
    }
    if options.confirm && !scope.confirm {
        return Some("--confirm");
    }
    if options.panel.is_some() && !scope.panel {
        return Some("--panel");
    }
    if options.current && !scope.current {
        return Some("--current");
    }
    if options.target_panel.is_some() && !scope.target_panel {
        return Some("--target-panel");
    }
    if options.swap_target.is_some() && !scope.swap_target {
        return Some("--with");
    }
    if options.into_tab.is_some() && !scope.into_tab {
        return Some("--into-tab");
    }
    if options.new_tab && !scope.new_tab {
        return Some("--new-tab");
    }
    if options.history && !scope.history {
        return Some("--history");
    }
    if options.unwrap && !scope.unwrap {
        return Some("--unwrap");
    }
    if options.output_source.is_some() && !scope.output_source {
        return Some("--source");
    }
    if options.ansi && !scope.ansi {
        return Some("--ansi");
    }
    if options.plain && !scope.plain {
        return Some("--plain");
    }
    if options.lines.is_some() && !scope.lines {
        return Some("--lines");
    }
    if options.path.is_some() && !scope.path {
        return Some("--path");
    }
    if options.match_text.is_some() && !scope.wait {
        return Some("--match");
    }
    if options.regex.is_some() && !scope.regex {
        return Some("--regex");
    }
    // `--timeout` belongs to `terminal wait-output` through `wait` scope and
    // to `agent wait` through its own `timeout` scope; the two never share
    // their other flags.
    if options.timeout.is_some() && !scope.wait && !scope.timeout {
        return Some("--timeout");
    }
    if options.text.is_some() && !scope.text {
        return Some("--text");
    }
    if options.enter && !scope.enter {
        return Some("--enter");
    }
    if !options.keys.is_empty() && !scope.key {
        return Some("--key");
    }
    if options.file.is_some() && !scope.file {
        return Some("--file");
    }
    if options.stdin && !scope.stdin {
        return Some("--stdin");
    }
    if options.paste && !scope.file && !scope.stdin {
        return Some("--paste");
    }
    if options.mouse.is_some() && !scope.mouse {
        return Some("--mouse");
    }
    if options.cwd.is_some() && !scope.cwd {
        return Some("--cwd");
    }
    if !options.environment.is_empty() && !scope.environment {
        return Some("--env");
    }
    if options.exec.is_some() && !scope.exec {
        return Some("--exec");
    }
    if !options.args.is_empty() && !scope.args {
        return Some("--arg");
    }
    if options.include_ended && !scope.include_ended {
        return Some("--include-ended");
    }
    if options.limit.is_some() && !scope.limit {
        return Some("--limit");
    }
    if options.cursor.is_some() && !scope.cursor {
        return Some("--cursor");
    }
    if !options.until.is_empty() && !scope.until {
        return Some("--until");
    }
    if options.after_revision.is_some() && !scope.after_revision {
        return Some("--after-revision");
    }
    if !options.providers.is_empty() && !scope.provider {
        return Some("--provider");
    }
    if !options.asset_roots.is_empty() && !scope.asset_root {
        return Some("--asset-root");
    }
    if options.session.is_some() && !scope.session {
        return Some("--session");
    }
    if options.forever && !scope.forever {
        return Some("--forever");
    }
    if options.prompt_wait && !scope.prompt_wait {
        return Some("--wait");
    }
    if options.clear && !scope.clear {
        return Some("--clear");
    }
    if options.name.is_some() && !scope.name {
        return Some("--name");
    }
    if options.profile.is_some() && !scope.profile {
        return Some("--profile");
    }
    if options.kind.is_some() && !scope.kind {
        return Some("--kind");
    }
    if options.provider.is_some() && !scope.provider {
        return Some("--provider");
    }
    if options.adapter_source.is_some() && !scope.adapter_source {
        return Some("--adapter-source");
    }
    if options.request.is_some() && !scope.request {
        return Some("--request");
    }
    if options.generation.is_some() && !scope.generation {
        return Some("--generation");
    }
    None
}

/// Reject every named option outside the command's admitted set, naming the
/// first out-of-scope flag. `--format` is orthogonal and never scope-checked.
fn check_scope(
    options: &Options,
    scope: Scope,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    match first_out_of_scope(options, scope) {
        Some(option) => Err(usage_with(
            UsageReason::UnexpectedOption { context, option },
            envelope,
        )),
        None => Ok(()),
    }
}

fn required(
    slot: &Option<String>,
    context: UsageContext,
    requirement: &'static str,
    envelope: FailureEnvelope,
) -> Result<String, ParseFailure> {
    slot.clone()
        .ok_or_else(|| missing_required(context, requirement, envelope))
}

/// `--mouse` accepts exactly `off`/`on` as a separate token; anything else —
/// including the `--mouse=<value>` equals form — is a usage failure, never a
/// guessed default.
fn mouse_option(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// Per-launch overrides are independent of the program/profile selection.
fn launch_options(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchOptions, ParseFailure> {
    launch_options_with_directory(options, options.cwd.as_deref(), context, envelope)
}

fn launch_options_with_directory(
    options: &Options,
    directory: Option<&str>,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchOptions, ParseFailure> {
    let cwd = directory
        .map(WorkbenchDirectoryPath::parse)
        .transpose()
        .map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--cwd",
                    value: options.cwd.clone().unwrap_or_default(),
                    expected: ExpectedValue::DirectoryPath,
                },
                envelope,
            )
        })?;
    // Environment values can contain secrets. Neither the parse failure nor its
    // localized diagnostic retains the supplied value, even for malformed input.
    let invalid_environment = || {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--env",
                value: String::new(),
                expected: ExpectedValue::LaunchEnvironment,
            },
            envelope,
        )
    };
    let entries = options
        .environment
        .iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .ok_or_else(invalid_environment)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let environment = LaunchEnvironment::new(entries).map_err(|_| invalid_environment())?;
    Ok(LaunchOptions { cwd, environment })
}

/// `--exec`/`--arg` build the optional explicit launch selection: `--exec`
/// once with a non-empty absolute NUL-free executable, `--arg` repeat-ordered
/// verbatim argv values, at most the protocol argument count and aggregate
/// byte bound. `--arg` without `--exec` is a missing requirement — an
/// argument list has no launch to modify.
fn launch_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<LaunchSelection>, ParseFailure> {
    let Some(executable) = options.exec.clone() else {
        if options.args.is_empty() {
            return Ok(None);
        }
        return Err(missing_required(context, "'--exec'", envelope));
    };
    if WorkbenchDirectoryPath::parse(&executable).is_err() {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--exec",
                value: executable,
                expected: ExpectedValue::Executable,
            },
            envelope,
        ));
    }
    LaunchSelection::new(executable, options.args.clone())
        .map(Some)
        .map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--arg",
                    value: options.args.join(" "),
                    expected: ExpectedValue::LaunchArguments,
                },
                envelope,
            )
        })
}

/// A `launch-profile` positional selector: id-first exact then name
/// cardinality, validated only against the shared selector bound — the
/// Server owns resolution, the CLI never pre-reads the catalog.
fn profile_selector(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchProfileSelector, ParseFailure> {
    LaunchProfileSelector::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<profile>",
                value: value.to_owned(),
                expected: ExpectedValue::ProfileSelector,
            },
            envelope,
        )
    })
}

/// The `--profile <selector>` launch option on `tab create`/`panel split`:
/// the same selector grammar as the positional form, named by its flag.
fn profile_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<LaunchProfileSelector>, ParseFailure> {
    let Some(value) = options.profile.clone() else {
        return Ok(None);
    };
    LaunchProfileSelector::parse(&value).map(Some).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--profile",
                value,
                expected: ExpectedValue::ProfileSelector,
            },
            envelope,
        )
    })
}

/// `--profile` and the explicit `--exec`/`--arg` launch are mutually
/// exclusive — the wire decodes the pair as one rejection, so the grammar
/// refuses it first with the concrete flag pair named.
fn reject_launch_profile_conflict(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    if options.profile.is_none() {
        return Ok(());
    }
    let explicit = if options.exec.is_some() {
        Some("--exec")
    } else {
        (!options.args.is_empty()).then_some("--arg")
    };
    match explicit {
        Some(first) => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first,
                second: "--profile",
            },
            envelope,
        )),
        None => Ok(()),
    }
}

/// `launch-profile create --name <n>`: the required, non-unique catalog
/// name under the protocol's 512-byte bound.
fn profile_name_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchProfileName, ParseFailure> {
    let value = required(&options.name, context, "'--name'", envelope)?;
    LaunchProfileName::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--name",
                value,
                expected: ExpectedValue::ProfileName,
            },
            envelope,
        )
    })
}

/// `launch-profile create` stores exactly one explicit launch selection:
/// `--exec` is required here even though it is optional on tab/panel
/// launches — a profile has no default-shell meaning.
fn required_launch_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchSelection, ParseFailure> {
    launch_option(options, context, envelope)?
        .ok_or_else(|| missing_required(context, "'--exec'", envelope))
}

/// `--label` carries a wire-ready Workbench label: non-empty and at most the
/// protocol's 512-scalar bound.
fn label_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchLabel, ParseFailure> {
    let value = required(&options.label, context, "'--label'", envelope)?;
    WorkbenchLabel::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--label",
                value,
                expected: ExpectedValue::Label,
            },
            envelope,
        )
    })
}

/// `--path` carries a wire-ready Workbench directory path: the literal
/// absolute local-platform path, at most the protocol's 4,096-byte bound. The CLI
/// never expands `~`/`$`, never creates the directory, and never probes a
/// filesystem — a malformed value is a usage failure at this boundary.
fn path_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchDirectoryPath, ParseFailure> {
    let value = required(&options.path, context, "'--path'", envelope)?;
    WorkbenchDirectoryPath::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--path",
                value,
                expected: ExpectedValue::DirectoryPath,
            },
            envelope,
        )
    })
}

/// `--directory` carries a wire-ready Workbench directory ID: the strict
/// nonzero 32-lowercase-hex selector the `worktree list` grammar admits. It
/// is an ID only — never a filesystem path or label — and membership stays
/// the Server's authority; a malformed value is a generic usage failure with
/// no resource details at this boundary.
fn directory_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchDirectoryId, ParseFailure> {
    let value = required(&options.directory, context, "'--directory'", envelope)?;
    WorkbenchDirectoryId::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--directory",
                value,
                expected: ExpectedValue::DirectoryId,
            },
            envelope,
        )
    })
}

/// `workspace close-status|close-resume --project <id>`: the strict stable
/// Project ID — an ID only, never a label or selector, because the retained
/// close record outlives the Workspace's active-projection membership.
fn project_id_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchProjectId, ParseFailure> {
    let value = required(&options.project, context, "'--project'", envelope)?;
    WorkbenchProjectId::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--project",
                value,
                expected: ExpectedValue::ProjectId,
            },
            envelope,
        )
    })
}

/// `workspace close-status|close-resume --workspace <id>`: the strict
/// stable Workspace ID under the same stable-identity rule as `--project`.
fn workspace_id_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchWorkspaceId, ParseFailure> {
    let value = required(&options.workspace, context, "'--workspace'", envelope)?;
    WorkbenchWorkspaceId::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--workspace",
                value,
                expected: ExpectedValue::WorkspaceId,
            },
            envelope,
        )
    })
}

/// `workspace close-resume --request <id>`: the close operation's own
/// request ID — the operation identity the Server re-verifies, never the
/// resume request's fresh identity.
fn close_request_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchRequestId, ParseFailure> {
    let value = required(&options.request, context, "'--request'", envelope)?;
    WorkbenchRequestId::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--request",
                value,
                expected: ExpectedValue::CloseRequestId,
            },
            envelope,
        )
    })
}

/// `workspace close-resume --generation <n>`: the explicit attempt fence —
/// the protocol's nonzero generation spelling; a stale-but-valid value is
/// the Server's settled `stale` answer, while a malformed value is usage.
fn close_generation_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTabCloseAttemptGeneration, ParseFailure> {
    let value = required(&options.generation, context, "'--generation'", envelope)?;
    WorkbenchTabCloseAttemptGeneration::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--generation",
                value,
                expected: ExpectedValue::CloseGeneration,
            },
            envelope,
        )
    })
}

/// The four geometry flags are an all-or-none group. With all four absent the
/// command requests the documented default: an 80x24 logical grid with the
/// protocol's paired-zero (unknown) cell pixel dimensions — a CLI request
/// policy, never a probed display fact. With any flag present all four are
/// required; each parses as a u16 dimension and must satisfy the protocol
/// geometry bounds (columns/rows 1..=4096, cell pixel dimensions 0..=4096 and
/// both zero or both nonzero, grid ≤ 262144 cells, pixel extents ≤ 65535).
fn geometry_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTerminalGeometry, ParseFailure> {
    if options.cols.is_none()
        && options.rows.is_none()
        && options.cell_width.is_none()
        && options.cell_height.is_none()
    {
        return WorkbenchTerminalGeometry::new(80, 24, 0, 0).map_err(|_| {
            usage_with(
                UsageReason::InvalidGeometry {
                    context,
                    cols: 80,
                    rows: 24,
                    cell_width: 0,
                    cell_height: 0,
                },
                envelope,
            )
        });
    }
    let dimension = |slot: &Option<String>, flag: &'static str| -> Result<u16, ParseFailure> {
        let value = required(
            slot,
            context,
            match flag {
                "--cols" => "'--cols'",
                "--rows" => "'--rows'",
                "--cell-width" => "'--cell-width'",
                _ => "'--cell-height'",
            },
            envelope,
        )?;
        value.parse::<u16>().map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: flag,
                    value,
                    expected: ExpectedValue::Dimension,
                },
                envelope,
            )
        })
    };
    let cols = dimension(&options.cols, "--cols")?;
    let rows = dimension(&options.rows, "--rows")?;
    let cell_width = dimension(&options.cell_width, "--cell-width")?;
    let cell_height = dimension(&options.cell_height, "--cell-height")?;
    WorkbenchTerminalGeometry::new(cols, rows, cell_width, cell_height).map_err(|_| {
        usage_with(
            UsageReason::InvalidGeometry {
                context,
                cols,
                rows,
                cell_width,
                cell_height,
            },
            envelope,
        )
    })
}

/// The `--rows` value under `--history`: a bounded retained-row count, not a
/// geometry dimension. 1..=1000 rows per the read-history contract.
fn history_rows_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u16, ParseFailure> {
    let value = required(&options.rows, context, "'--rows'", envelope)?;
    match value.parse::<u16>() {
        Ok(rows) if (1..=1000).contains(&rows) => Ok(rows),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--rows",
                value,
                expected: ExpectedValue::HistoryRows,
            },
            envelope,
        )),
    }
}

/// `terminal read --lines <n>`: the presentation tail length over the
/// already-emitted entries — logical lines under `--unwrap`, physical rows
/// otherwise. 1..=65535; it never widens the fetched window.
fn tail_lines_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<u16>, ParseFailure> {
    let Some(value) = options.lines.clone() else {
        return Ok(None);
    };
    match value.parse::<u16>() {
        Ok(lines) if lines >= 1 => Ok(Some(lines)),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--lines",
                value,
                expected: ExpectedValue::TailLines,
            },
            envelope,
        )),
    }
}

/// The `--regex` pattern bound: a nonempty UTF-8 pattern of at most this
/// many bytes, compiled once before any effect.
pub(crate) const MAXIMUM_WAIT_PATTERN_BYTES: usize = 1024;

/// The compiled-program bound the `regex` builder enforces at compile time.
const REGEX_SIZE_LIMIT: usize = 1024 * 1024;

/// The regex engine's lazy DFA cache bound during matching.
const REGEX_DFA_SIZE_LIMIT: usize = 1024 * 1024;

/// `wait-output` admits exactly one search input: `--match` keeps the
/// literal, case-sensitive substring semantics (nonempty, ≤4096 bytes —
/// nothing is interpreted); `--regex` is a nonempty ≤1024-byte pattern
/// compiled once here under the declared program and DFA budgets, so an
/// invalid or oversized program is a usage failure before any runtime
/// access. Both present is a conflict; neither is a missing requirement.
fn wait_matcher_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WaitPattern, ParseFailure> {
    match (&options.match_text, &options.regex) {
        (Some(_), Some(_)) => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--match",
                second: "--regex",
            },
            envelope,
        )),
        (None, None) => Err(missing_required(
            context,
            "'--match' or '--regex'",
            envelope,
        )),
        (Some(text), None) => {
            if text.is_empty() || text.len() > 4096 {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--match",
                        value: text.clone(),
                        expected: ExpectedValue::MatchText,
                    },
                    envelope,
                ));
            }
            Ok(WaitPattern::Literal(text.clone()))
        }
        (None, Some(pattern)) => {
            if pattern.is_empty() || pattern.len() > MAXIMUM_WAIT_PATTERN_BYTES {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--regex",
                        value: pattern.clone(),
                        expected: ExpectedValue::RegexPattern,
                    },
                    envelope,
                ));
            }
            let compiled = regex::RegexBuilder::new(pattern)
                .size_limit(REGEX_SIZE_LIMIT)
                .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
                .build()
                .map_err(|_| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option: "--regex",
                            value: pattern.clone(),
                            expected: ExpectedValue::RegexPattern,
                        },
                        envelope,
                    )
                })?;
            Ok(WaitPattern::Regex(compiled))
        }
    }
}

/// `--rows` under `wait-output`: the searched retained-row window spanning
/// visible plus scrollback as one range. 1..=1000, defaulting to 1000.
fn wait_rows_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u16, ParseFailure> {
    match options.rows.is_some() {
        true => history_rows_option(options, context, envelope),
        false => Ok(1000),
    }
}

/// `--timeout` under `wait-output`: the total wait budget in milliseconds
/// measured from command start. 1..=3600000, defaulting to 30000.
fn wait_timeout_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u32, ParseFailure> {
    let Some(value) = options.timeout.clone() else {
        return Ok(30_000);
    };
    match value.parse::<u32>() {
        Ok(timeout) if (1..=3_600_000).contains(&timeout) => Ok(timeout),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--timeout",
                value,
                expected: ExpectedValue::WaitTimeout,
            },
            envelope,
        )),
    }
}

/// The wire's per-record text bound: literal bytes plus the optional
/// folded CR must fit one `text` record.
pub(crate) const MAXIMUM_SEND_TEXT_BYTES: usize = 4096;

/// The wire's per-record bound for one literal `bytes` record — the
/// materialized `--file`/`--stdin` payload plus its folded `--enter` CR.
pub(crate) const MAXIMUM_SEND_BYTES_BYTES: usize = 1024 * 1024;

/// The wire's per-record bound for one `paste` record's content under
/// `--paste`; the Host's own bracket markers are additional.
pub(crate) const MAXIMUM_SEND_PASTE_BYTES: usize = 256 * 1024;

/// The wire's sequence bound: an ordered `--key` run submits at most 64
/// records in one request.
pub(crate) const MAXIMUM_SEND_RECORDS: usize = 64;

/// `terminal send` admits exactly one source family: `--text`, one or more
/// repeatable `--key` values, `--file`, or `--stdin`. `--enter` is the
/// text-bearing modifier — legal under `--text`, `--file`, and `--stdin`,
/// never under `--key`. `--paste` selects the Host-bracketed paste record
/// and is legal only beside a declared `--file`/`--stdin` source. The text
/// byte budget counts the payload the wire actually carries — the literal
/// bytes plus the appended CR — so an over-limit record is refused before
/// any connection. `--file` and `--stdin` are declarations only: the parser
/// stores the source untouched and the materializer reads it later under
/// the shared invocation deadline.
fn send_input(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<TerminalSendInput, ParseFailure> {
    // Exactly one source family: a second declaration is the conflict the
    // grammar rejects, named in a fixed order regardless of argv order.
    let declared: Vec<&'static str> = [
        ("--text", options.text.is_some()),
        ("--key", !options.keys.is_empty()),
        ("--file", options.file.is_some()),
        ("--stdin", options.stdin),
    ]
    .into_iter()
    .filter_map(|(flag, seen)| seen.then_some(flag))
    .collect();
    if declared.len() > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: declared[0],
                second: declared[1],
            },
            envelope,
        ));
    }
    if let Some(text) = &options.text {
        if options.paste {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: "--paste",
                },
                envelope,
            ));
        }
        let bytes = text.len() + usize::from(options.enter);
        if text.is_empty() || bytes > MAXIMUM_SEND_TEXT_BYTES {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--text",
                    value: text.clone(),
                    expected: ExpectedValue::SendText,
                },
                envelope,
            ));
        }
        return Ok(TerminalSendInput::Text {
            text: text.clone(),
            enter: options.enter,
        });
    }
    if !options.keys.is_empty() {
        if options.enter {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: "--enter",
                },
                envelope,
            ));
        }
        if options.paste {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: "--paste",
                },
                envelope,
            ));
        }
        if options.keys.len() > MAXIMUM_SEND_RECORDS {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--key",
                    value: options.keys[MAXIMUM_SEND_RECORDS].clone(),
                    expected: ExpectedValue::SendKeySequence,
                },
                envelope,
            ));
        }
        // Whole-sequence prevalidation: every value parses before the
        // request exists, so a bad later key never reaches the wire after
        // earlier keys.
        let mut keys = Vec::with_capacity(options.keys.len());
        for value in &options.keys {
            let Some(key) = parse_send_key(value) else {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--key",
                        value: value.clone(),
                        expected: ExpectedValue::SendKey,
                    },
                    envelope,
                ));
            };
            keys.push(key);
        }
        return Ok(TerminalSendInput::Keys(keys));
    }
    if options.file.is_some() || options.stdin {
        return Ok(if let Some(path) = &options.file {
            TerminalSendInput::File {
                path: path.clone(),
                enter: options.enter,
                paste: options.paste,
            }
        } else {
            TerminalSendInput::Stdin {
                enter: options.enter,
                paste: options.paste,
            }
        });
    }
    if options.paste {
        return Err(usage_with(
            UsageReason::UnexpectedOption {
                context,
                option: "--paste",
            },
            envelope,
        ));
    }
    if options.enter {
        return Err(usage_with(
            UsageReason::UnexpectedOption {
                context,
                option: "--enter",
            },
            envelope,
        ));
    }
    Err(missing_required(
        context,
        "'--text', '--key', '--file', or '--stdin'",
        envelope,
    ))
}

/// `agent list --limit`: the bounded page size, 1..=100, defaulting to the
/// wire contract's 50.
fn agent_limit_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u64, ParseFailure> {
    let Some(value) = options.limit.clone() else {
        return Ok(DEFAULT_LIST_LIMIT);
    };
    match value.parse::<u64>() {
        Ok(limit) if (1..=MAXIMUM_LIST_LIMIT).contains(&limit) => Ok(limit),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--limit",
                value,
                expected: ExpectedValue::AgentListLimit,
            },
            envelope,
        )),
    }
}

/// `agent list --cursor`: the minted `v1.<59>.<43>` page cursor, carried
/// verbatim — authenticity is verified Server-side, never here.
fn agent_cursor_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<AgentListCursor>, ParseFailure> {
    match &options.cursor {
        Some(value) => AgentListCursor::parse(value).map(Some).map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--cursor",
                    value: value.clone(),
                    expected: ExpectedValue::AgentCursor,
                },
                envelope,
            )
        }),
        None => Ok(None),
    }
}

/// `agent integration --provider <id>`: the closed provider vocabulary —
/// an unknown spelling is a usage failure, never a fuzzy match. `list`
/// admits it as an optional filter.
fn provider_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<crate::agent_integrations::IntegrationId>, ParseFailure> {
    // Single-use on this family: the shared repeatable arm records every
    // `--provider` into `providers`; a second occurrence is a duplicate here.
    if options.providers.len() > 1 {
        return Err(usage_with(
            UsageReason::DuplicateOption {
                option: "--provider",
            },
            envelope,
        ));
    }
    match &options.provider {
        Some(value) => crate::agent_integrations::IntegrationId::parse(value)
            .map(Some)
            .ok_or_else(|| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--provider",
                        value: value.clone(),
                        expected: ExpectedValue::Provider,
                    },
                    envelope,
                )
            }),
        None => Ok(None),
    }
}

/// The required `--provider` on explain/enable/disable.
fn required_provider(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<crate::agent_integrations::IntegrationId, ParseFailure> {
    provider_option(options, context, envelope)?
        .ok_or_else(|| missing_required(context, "'--provider'", envelope))
}

/// `agent integration --adapter-source <abs>`: a literal absolute path —
/// never expanded, created, or probed at parse time.
fn adapter_source_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<PathBuf>, ParseFailure> {
    match &options.adapter_source {
        Some(value)
            if value.is_empty()
                || !PathBuf::from(value).is_absolute()
                || value.as_bytes().contains(&0) =>
        {
            Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--adapter-source",
                    value: value.clone(),
                    expected: ExpectedValue::AdapterSource,
                },
                envelope,
            ))
        }
        Some(value) => Ok(Some(PathBuf::from(value))),
        None => Ok(None),
    }
}

/// The `agent get|explain|wait` operand: a wire-ready Agent session identity —
/// opaque `1..=128` `[A-Za-z0-9._:-]+`, never an id/label split.
fn agent_session_id(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<AgentSessionId, ParseFailure> {
    AgentSessionId::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<session>",
                value: value.to_owned(),
                expected: ExpectedValue::AgentSessionId,
            },
            envelope,
        )
    })
}

/// `agent wait --until <condition>`: the repeatable canonical condition
/// vocabulary — every value must be one of the fourteen names, a repeated
/// value is a duplicate flag, and an absent flag means the canonical
/// default seven in canonical order.
fn agent_until_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Vec<AgentWaitCondition>, ParseFailure> {
    if options.until.is_empty() {
        return Ok(DEFAULT_AGENT_WAIT_CONDITIONS.to_vec());
    }
    let mut conditions = Vec::with_capacity(options.until.len());
    for value in &options.until {
        let Some(condition) = AgentWaitCondition::parse(value) else {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--until",
                    value: value.clone(),
                    expected: ExpectedValue::AgentWaitUntil,
                },
                envelope,
            ));
        };
        if conditions.contains(&condition) {
            return Err(usage_with(
                UsageReason::DuplicateOption { option: "--until" },
                envelope,
            ));
        }
        conditions.push(condition);
    }
    Ok(conditions)
}

/// `agent wait --after-revision <n>`: the optional global-revision fence —
/// a nonnegative safe integer echoed verbatim into the emitted document.
fn agent_after_revision_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<AgentSafeInteger>, ParseFailure> {
    match &options.after_revision {
        Some(value) => match value.parse::<u64>() {
            Ok(revision) if revision <= MAXIMUM_SAFE_INTEGER => {
                Ok(Some(AgentSafeInteger::new(revision)))
            }
            _ => Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--after-revision",
                    value: value.clone(),
                    expected: ExpectedValue::AgentAfterRevision,
                },
                envelope,
            )),
        },
        None => Ok(None),
    }
}

/// `agent wait --timeout`: the total wait budget in milliseconds measured
/// from command start. Explicit values are 1..=3600000. Windows defaults to
/// no total timeout; existing non-Windows defaults remain 600000.
fn agent_wait_timeout_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<u32>, ParseFailure> {
    if options.forever {
        if options.timeout.is_some() {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: "--forever",
                    second: "--timeout",
                },
                envelope,
            ));
        }
        return Ok(None);
    }
    let Some(value) = options.timeout.clone() else {
        return Ok(DEFAULT_AGENT_WAIT_TIMEOUT);
    };
    match value.parse::<u32>() {
        Ok(timeout) if (1..=3_600_000).contains(&timeout) => Ok(Some(timeout)),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--timeout",
                value,
                expected: ExpectedValue::WaitTimeout,
            },
            envelope,
        )),
    }
}

/// `agent doctor --provider <id>`: each repeatable value must be one
/// canonical provider id — an unknown id is a typed parse failure, never a
/// silently empty selection.
fn doctor_providers_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Vec<String>, ParseFailure> {
    options
        .providers
        .iter()
        .map(|value| {
            if crate::agent_doctor::PROVIDER_IDS.contains(&value.as_str()) {
                Ok(value.clone())
            } else {
                Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--provider",
                        value: value.clone(),
                        expected: ExpectedValue::ProviderId,
                    },
                    envelope,
                ))
            }
        })
        .collect()
}

/// `agent doctor --asset-root <path>`: each repeatable value must be a
/// nonempty absolute NUL-free path — relative spellings are usage failures
/// at this boundary, never `~`/`$`-expanded and never probed as guesses.
/// At most four supplied roots may be declared.
fn doctor_asset_roots_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Vec<String>, ParseFailure> {
    for value in &options.asset_roots {
        if value.is_empty()
            || !std::path::Path::new(value).is_absolute()
            || value.as_bytes().contains(&0)
        {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--asset-root",
                    value: value.clone(),
                    expected: ExpectedValue::AssetRootPath,
                },
                envelope,
            ));
        }
    }
    if options.asset_roots.len() > 4 {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--asset-root",
                value: options.asset_roots.join(" "),
                expected: ExpectedValue::AssetRootPath,
            },
            envelope,
        ));
    }
    Ok(options.asset_roots.clone())
}

fn tab_move_command(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<MutationCommand, ParseFailure> {
    let context = UsageContext::Path("tab move");
    if options.direction.is_some() && options.insert_index.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--direction",
                second: "--insert-index",
            },
            envelope,
        ));
    }
    let target = target(value.to_owned(), Resource::Tab, envelope)?;
    if let Some(value) = &options.insert_index {
        let index = (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| value.parse::<usize>().ok())
            .flatten()
            .and_then(|value| WorkbenchTabInsertIndex::new(value).ok())
            .ok_or_else(|| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--insert-index",
                        value: value.clone(),
                        expected: ExpectedValue::TabInsertIndex,
                    },
                    envelope,
                )
            })?;
        return Ok(MutationCommand::TabMoveIndex {
            target,
            insert_index: index,
        });
    }
    if options.direction.is_none() {
        return Err(missing_required(
            context,
            "'--direction' or '--insert-index'",
            envelope,
        ));
    }
    Ok(MutationCommand::TabMove {
        target,
        direction: tab_direction(options, context, envelope)?,
    })
}

fn tab_direction(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTabMoveDirection, ParseFailure> {
    let value = required(&options.direction, context, "'--direction'", envelope)?;
    match value.as_str() {
        "previous" => Ok(WorkbenchTabMoveDirection::Previous),
        "next" => Ok(WorkbenchTabMoveDirection::Next),
        "first" => Ok(WorkbenchTabMoveDirection::First),
        "last" => Ok(WorkbenchTabMoveDirection::Last),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--direction",
                value,
                expected: ExpectedValue::TabDirection,
            },
            envelope,
        )),
    }
}

/// `panel move` admits exactly one destination selector: `--target-panel`
/// (the original same-Tab adjacent insert), `--into-tab` (an existing
/// sibling Tab), or `--new-tab` (a fresh Tab appended to the source
/// TerminalContext). Each form pins its own required and forbidden peers
/// before any target resolution runs; a stray `--label`/`--direction`
/// outside its owning form is a usage conflict, never silently ignored.
fn panel_move_destination(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<PanelMoveDestination, ParseFailure> {
    let context = UsageContext::Path("panel move");
    let declared = [
        options.target_panel.is_some().then_some("--target-panel"),
        options.into_tab.is_some().then_some("--into-tab"),
        options.new_tab.then_some("--new-tab"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    match declared[..] {
        ["--target-panel"] => {
            if options.label.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--target-panel",
                        second: "--label",
                    },
                    envelope,
                ));
            }
            Ok(PanelMoveDestination::Adjacent {
                into: target(
                    required(&options.target_panel, context, "'--target-panel'", envelope)?,
                    Resource::Panel,
                    envelope,
                )?,
                direction: panel_direction(options, context, envelope)?,
            })
        }
        ["--into-tab"] => {
            if options.label.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--into-tab",
                        second: "--label",
                    },
                    envelope,
                ));
            }
            Ok(PanelMoveDestination::Tab {
                tab: target(
                    required(&options.into_tab, context, "'--into-tab'", envelope)?,
                    Resource::Tab,
                    envelope,
                )?,
                direction: panel_direction(options, context, envelope)?,
            })
        }
        ["--new-tab"] => {
            if options.direction.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--new-tab",
                        second: "--direction",
                    },
                    envelope,
                ));
            }
            Ok(PanelMoveDestination::NewTab {
                label: label_option(options, context, envelope)?,
            })
        }
        [] => Err(missing_required(
            context,
            "'--target-panel', '--into-tab', or '--new-tab'",
            envelope,
        )),
        _ => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: declared[0],
                second: declared[1],
            },
            envelope,
        )),
    }
}

fn panel_direction(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchPanelSplitDirection, ParseFailure> {
    let value = required(&options.direction, context, "'--direction'", envelope)?;
    match value.as_str() {
        "above" => Ok(WorkbenchPanelSplitDirection::Above),
        "below" => Ok(WorkbenchPanelSplitDirection::Below),
        "left" => Ok(WorkbenchPanelSplitDirection::Left),
        "right" => Ok(WorkbenchPanelSplitDirection::Right),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--direction",
                value,
                expected: ExpectedValue::PanelDirection,
            },
            envelope,
        )),
    }
}

/// Reject the `--workspace`/`--standalone` combination when it actually fires.
fn reject_conflict(
    conflict: bool,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    if conflict {
        Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--workspace",
                second: "--standalone",
            },
            envelope,
        ))
    } else {
        Ok(())
    }
}

fn missing_required(
    context: UsageContext,
    requirement: &'static str,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::MissingRequired {
            context,
            requirement,
        },
        envelope,
    )
}

fn missing_command(context: UsageContext, envelope: FailureEnvelope) -> ParseFailure {
    usage_with(UsageReason::MissingCommand { context }, envelope)
}

fn unknown_command(context: UsageContext, word: &str, envelope: FailureEnvelope) -> ParseFailure {
    usage_with(
        UsageReason::UnknownCommand {
            context,
            word: word.to_owned(),
        },
        envelope,
    )
}

fn unexpected_argument(
    context: UsageContext,
    value: &str,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::UnexpectedArgument {
            context,
            value: value.to_owned(),
        },
        envelope,
    )
}

/// Reject any option/filter/format on commands that accept none, naming the
/// first rejected flag in the fixed declaration order (`--format` last).
fn reject_extras(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    reject_extras_scope(options, NO_OPTIONS, context, envelope)
}

/// `reject_extras` against an explicit admitted set: `--format` stays
/// rejected while the scope's own flags (today only `tui`'s `--mouse`)
/// pass through.
fn reject_extras_scope(
    options: &Options,
    scope: Scope,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    match first_out_of_scope(options, scope)
        .or_else(|| options.format.is_some().then_some("--format"))
    {
        Some(option) => Err(usage_with(
            UsageReason::UnexpectedOption { context, option },
            envelope,
        )),
        None => Ok(()),
    }
}

/// Reject the named filters; `--format` remains legal on data commands.
fn reject_filters(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    check_scope(options, NO_OPTIONS, context, envelope)
}

#[cfg(test)]
mod tests;

fn agent_name(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<AgentName, ParseFailure> {
    AgentName::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "name",
                value: value.to_owned(),
                expected: ExpectedValue::AgentName,
            },
            envelope,
        )
    })
}

fn agent_selector(
    words: &[&str],
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<AgentTarget, ParseFailure> {
    if words.len() > 1 {
        return Err(usage_with(
            UsageReason::UnexpectedArgument {
                context,
                value: words[1].to_owned(),
            },
            envelope,
        ));
    }
    let selected = usize::from(!words.is_empty())
        + usize::from(options.session.is_some())
        + usize::from(options.name.is_some())
        + usize::from(options.panel.is_some());
    if selected > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "<target>",
                second: "--session / --name / --panel",
            },
            envelope,
        ));
    }
    if let Some(value) = &options.panel {
        return Ok(AgentTarget::Panel {
            target: target(value.clone(), Resource::Panel, envelope)?,
        });
    }
    if let Some(value) = words.first() {
        if value.starts_with("@panel:") {
            return Ok(AgentTarget::Panel {
                target: target((*value).to_owned(), Resource::Panel, envelope)?,
            });
        }
        return Ok(AgentTarget::Auto {
            target: agent_session_id(value, context, envelope)?,
        });
    }
    if let Some(value) = &options.session {
        return Ok(AgentTarget::Session {
            session_id: agent_session_id(value, context, envelope)?,
        });
    }
    if let Some(value) = &options.name {
        return Ok(AgentTarget::Name {
            name: agent_name(value, context, envelope)?,
        });
    }
    Err(usage_with(
        UsageReason::MissingRequired {
            context,
            requirement: "<target> or --session <id> or --name <alias> or --panel <id|selector>",
        },
        envelope,
    ))
}
