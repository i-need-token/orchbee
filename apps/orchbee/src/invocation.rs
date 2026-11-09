//! Constructs typed domain requests and structured usage failures under the declared command
//! grammar. It owns operand construction; grammar metadata owns accepted syntax, and application
//! execution owners retain platform admission, live target resolution, and effects.
//!
//! This root declares the typed request vocabulary and the `Options` scan state that every parser
//! shares. `argv`, `scan`, and `dispatch` run one parse; feature children own their commands'
//! operands; the option and `failure` children hold the decoders and failures those parsers share.

mod agent_view_spec;
pub(crate) mod operation;
mod panel_agent;
mod panel_metadata;
pub(crate) mod panel_terminal_creation;
pub(crate) mod plugins;
mod report_target;
pub(crate) use panel_agent::PanelAgentCommand;
mod workspace_metadata;
pub(crate) use panel_metadata::PanelMetadataCommand;
pub(crate) use workspace_metadata::WorkspaceMetadataCommand;
mod client_control;
pub(crate) mod configured_commands;
pub(crate) mod events;
mod layouts;
mod named_sessions;
pub(crate) mod pane_graphics;
mod send_key;
pub(crate) use layouts::LayoutDestination;
pub(crate) use layouts::{LayoutApplyCommand, LayoutExportCommand};
pub(crate) use named_sessions::NamedSessionCommand;
pub(crate) mod machine_routing;
mod machines;
pub(crate) use machines::MachineCommand;
mod creation;
pub(crate) mod program;
pub(crate) mod workspace_creation;
pub(crate) use workspace_creation::ParsedWorkspaceCreationCommand;
pub(crate) use workspace_creation::WorkspaceCreationCommand;
mod panel_relocation;
pub(crate) use panel_relocation::{PanelRelocationCommand, PanelRelocationDestination};
mod panel_input;
pub(crate) use creation::{CreationCommand, CreationDirectory, CreationKind};
mod diagnostics;
mod schema_export;
mod terminal_output;
mod terminal_selection;
pub(crate) use terminal_selection::TerminalSelectionCommand;
mod terminal_buffer;
mod terminal_copy;
mod terminal_links;
pub(crate) use crate::terminal_copy::{
    CopyMotion, CopySearchDirection, TerminalTextPoint, TerminalTextRange,
};
pub(crate) use terminal_copy::TerminalCopyAction;
pub(crate) use terminal_copy::TerminalCopyCommand;
pub(crate) use terminal_links::TerminalLinkCommand;
mod terminal_run;
mod terminal_stream;
mod terminal_target;
pub(crate) use client_control::ClientLayoutTarget;
pub(crate) use client_control::{ClientCommand, LayoutQuery};
pub(crate) use client_control::{ClientNavigationTarget, ZoomMode};
pub(crate) use client_control::{PanelSwapCommand, PanelSwapDestination};
pub(crate) use terminal_output::{OutputSelection, OutputSource, OutputTarget};
mod output_cursor;
pub(crate) use output_cursor::{OutputAfter, OutputCursor};
pub(crate) use terminal_stream::AttachTarget;
pub(crate) use terminal_stream::{
    TerminalAttachCommand, TerminalControlCommand, TerminalObserveCommand,
};
mod workspace_group_close;
pub(crate) use workspace_group_close::WorkspaceGroupCloseCommand;
mod agent_options;
mod argv;
mod dispatch;
mod failure;
mod launch_options;
mod option_scope;
mod scan;
mod send_input;
mod server_stop;
mod target_syntax;
pub(crate) mod user_path;
pub(crate) mod user_path_windows;
mod wait_options;
mod workbench_options;
mod worktree;
use agent_options::{
    adapter_source_option, agent_after_revision_option, agent_limit_option, agent_name,
    agent_selector, agent_until_option, agent_wait_timeout_option, doctor_asset_roots_option,
    doctor_providers_option, provider_option, required_provider,
};
#[cfg(test)]
pub(crate) use argv::parse_request;
pub(crate) use argv::parse_selected_request;
use dispatch::resolve_words;
pub(crate) use failure::{
    ExpectedValue, FailureEnvelope, ParseFailure, ParseFailureKind, SendSourceCause, UsageContext,
    UsageReason, selection_envelope, usage_with,
};
use failure::{
    identifier, missing_command, missing_required, unexpected_argument, unknown_command,
};
use help::resolve_help_path;
pub(crate) use help::usage_option_candidates;
use launch_options::{
    launch_option, launch_options_with_directory, profile_name_option, profile_option,
    profile_selector, reject_launch_profile_conflict, required_launch_option,
};
use option_scope::{
    business_options, check_scope, reject_extras, reject_extras_scope, reject_filters, required,
};
use scan::{format_envelope, scan};
use send_input::{parse_send_key, send_input};
use server_stop::{confirm_option_required, stop_command, stop_id};
use std::path::PathBuf;
pub(crate) use target_syntax::label_requires_id;
use target_syntax::{ZERO_IDENTITY, target, valid_typed_id};
use wait_options::{close_wait, wait_matcher_option, wait_timeout_option};
use workbench_options::{
    close_generation_option, close_request_option, directory_option, geometry_option, label_option,
    manifest_operation_id, panel_direction, panel_move_destination, path_option, tab_move_command,
    workspace_id_option,
};
pub(crate) use worktree::WorktreeCommand;
pub(crate) use worktree::WorktreeQueryCommand;
pub(crate) use worktree::{ParsedWorktreeCommand, ParsedWorktreeQueryCommand};
pub(crate) use worktree::{WorktreeOpenSelector, WorktreeStartCommand, WorktreeStartKind};

use orchbee_agent_observation_protocol::{AgentName, AgentSafeInteger, AgentSessionId};
use orchbee_workbench_protocol::{
    LaunchOptions, LaunchProfileName, LaunchProfileSelector, LaunchSelection, WorkbenchClientId,
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchPanelSplitDirection,
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabInsertIndex,
    WorkbenchTabMoveDirection, WorkbenchTerminalGeometry, WorkbenchWorkspaceId,
};

/// Launch/version result consumed by `presentation::present`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Invocation {
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
/// `Selector` carries an exact compact `@<kind>:0x<hex>` identity.
/// The parser checks its resource kind before effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    /// Bare ID in the four input/output commands; resolution refuses cross-kind collisions.
    TerminalOrPanelId(String),
    Id(String),
    Label(String),
    Selector(String),
}

impl Target {
    /// Expand an exact compact selector, preserving the original token for diagnostics.
    pub(crate) fn selector_identity(&self) -> Option<String> {
        let Self::Selector(token) = self else {
            return None;
        };
        let (_, payload) = token.split_once(':')?;
        let hex = payload.strip_prefix("0x")?;
        Some(format!("{hex:0>32}"))
    }

    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Id(value)
            | Self::TerminalOrPanelId(value)
            | Self::Label(value)
            | Self::Selector(value) => value,
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
    Terminal { target: Target },
    Projection { selector: AgentSelector },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentSelector {
    Kind(String),
    Ordinal(usize),
}
impl AgentSelector {
    pub(crate) fn token(&self) -> String {
        match self {
            Self::Kind(kind) => format!("@agent:{kind}"),
            Self::Ordinal(n) => format!("@agent:{n}"),
        }
    }
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

mod help;
pub(crate) use help::{HelpPath, help_paths};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HelpTopic {
    Family(Family),
    Command(CommandPath),
}

pub(crate) mod grammar;
pub(crate) use grammar::{CommandPath, Family};

pub(crate) use grammar::values::AgentWaitCondition;

/// Explicit total wait intent; omission is resolved by the execution adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentWaitTimeout {
    Milliseconds(u32),
    Forever,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PanelQueryTarget {
    Current,
    Panel(Target),
    Client(WorkbenchClientId),
}

/// How `tab close` and `workspace close` settle: wait for the close to
/// finish within the deadline, return once accepted, or only preview the
/// terminals that would end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseWait {
    Wait { timeout_ms: u32 },
    Detach,
    DryRun,
}

/// `terminal list` placement filter; the parser admits at most one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalListFilter {
    Workspace(Target),
    Tab(Target),
    Panel(Target),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QueryCommand {
    Status,
    AgentChanges {
        limit: u64,
        cursor: Option<orchbee_agent_observation_protocol::AgentChangesCursor>,
    },
    AgentList {
        client: Option<WorkbenchClientId>,
        include_ended: bool,
        limit: u64,
        cursor: Option<String>,
    },
    AgentGet(AgentTarget),
    AgentClientGet {
        target: AgentTarget,
        client: WorkbenchClientId,
    },
    AgentExplain(AgentTarget),
    /// `agent wait`: one held Client Connection polls the canonical session
    /// document until a requested condition matches, the fence refuses, or
    /// the shared absolute deadline lands.
    AgentWait {
        session: AgentTarget,
        client: Option<WorkbenchClientId>,
        /// None preserves the execution owner's existing default conditions.
        until: Option<Vec<AgentWaitCondition>>,
        /// A global revision fence, never a turn identity.
        after_revision: Option<AgentSafeInteger>,
        /// None preserves the execution owner's existing total timeout.
        timeout_ms: Option<AgentWaitTimeout>,
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
        /// `--worktrees`: join each row's live Git worktree observation.
        worktrees: bool,
    },
    WorkspaceGet(Target),
    /// `workspace close-status --project <id> --workspace <id>`: the
    /// retained Workspace Close record read from stable IDs — it stays
    /// answerable after the Workspace leaves the active projection.
    WorkspaceCloseStatus {
        workspace: WorkbenchWorkspaceId,
    },
    TabList {
        workspace: Option<Target>,
    },
    TabGet(Target),
    PanelList {
        tab: Option<Target>,
        workspace: Option<Target>,
    },
    PanelCurrent(PanelQueryTarget),
    PanelGet(Target),
    PanelProcessInfo(PanelQueryTarget),
    TerminalList {
        filter: Option<TerminalListFilter>,
    },
    TerminalGet(Target),
    #[cfg(test)]
    TerminalRead {
        target: Target,
        /// Retained scrollback row count instead of the visible frame.
        history: Option<u16>,
        /// Present joined logical lines instead of physical rows.
        unwrap: bool,
        /// `--lines <n>`: present only the last n emitted entries — logical
        /// lines when joined, physical rows otherwise.
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
        after: Option<OutputAfter>,
    },
    /// `worktree list --project <t> --directory <id>`: both selectors are
    /// mandatory; the directory is a strict nonzero 32-lowercase-hex Workbench
    /// directory ID, never a filesystem path or label.
    WorktreeList {
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

/// The Workspace selector for a new Tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TabContext {
    Workspace(Target),
}

/// The `terminal send` payload the grammar admits: literal UTF-8 text, an
/// ordered repeatable semantic key sequence, or a declared local source
/// materialized once into one bounded record. The parser stores the
/// validated choice so the executor never re-reads argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalSendInput {
    /// Literal UTF-8 bytes; `enter` folds one trailing CR into the same
    /// single input record, never a second request.
    Text { text: String, enter: bool },
    /// The ordered `--key` sequence — every element is a named semantic key
    /// or an explicit modifier combination, in argv order.
    Keys(Vec<orchbee_client_server_protocol::SendKeyStroke>),
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

/// One effectful command admitted by the grammar. Every field is a
/// wire-ready value already checked against protocol bounds, so the executor
/// never re-validates argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MutationCommand {
    ProjectCreate {
        label: WorkbenchLabel,
    },
    ProjectDissolve {
        target: Target,
    },
    WorkspaceSetProject {
        target: Target,
        project: Option<Target>,
    },
    WorkspaceCreateProject {
        target: Target,
        label: WorkbenchLabel,
    },
    ProjectRename {
        target: Target,
        label: WorkbenchLabel,
    },
    WorkspaceLaunch {
        project: Option<Target>,
        label: WorkbenchLabel,
        /// A generated label follows the first terminal's directory until a rename.
        automatic_label: bool,
        geometry: WorkbenchTerminalGeometry,
        launch: Option<LaunchSelection>,
        profile: Option<LaunchProfileSelector>,
        options: LaunchOptions,
    },
    WorkspaceRename {
        target: Target,
        label: WorkbenchLabel,
    },
    WorkspaceMove {
        target: Target,
        insert_index: orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex,
    },
    WorkspaceMoveBlock {
        targets: Vec<Target>,
        before: Option<Target>,
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
        wait: CloseWait,
    },
    /// `workspace close-resume --project <id> --workspace <id> --request
    /// <close-id> --generation <n> --confirm`: one fresh attempt window for
    /// a resumable retained Workspace Close, fenced by the explicit
    /// attempt-generation the caller last observed.
    WorkspaceCloseResume {
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
        wait: CloseWait,
    },
    PanelSplit {
        target: Target,
        right_click: orchbee_workbench_protocol::PanelRightClickTarget,
        direction: WorkbenchPanelSplitDirection,
        label: WorkbenchLabel,
        /// The visible name `--label` gives the new Panel, as `panel rename`
        /// would set it; absent when the label is the automatic default.
        custom_label: Option<orchbee_workbench_protocol::PanelCustomLabel>,
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
        keys: Vec<orchbee_client_server_protocol::SendKeyStroke>,
    },
    TerminalSend {
        target: Target,
        input: TerminalSendInput,
    },
    TerminalRunWait {
        target: Target,
        command: String,
        timeout_ms: u32,
    },
}

/// Panel placement uses one existing-Tab form or a new Tab in the source TerminalContext.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PanelMoveDestination {
    Tab {
        tab: Target,
        panel: Option<Target>,
        direction: WorkbenchPanelSplitDirection,
    },
    NewTab {
        label: WorkbenchLabel,
    },
}

/// One `agent integration` command admitted by the grammar — a local
/// provider-onboarding operation that never touches a Session. `provider` is
/// optional on `list` (a filter) and required on every other verb;
/// `adapter_source` is the development/test override already validated as
/// a literal absolute path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentIntegrationCommand {
    List {
        outdated_only: bool,
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
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) until: Option<Vec<AgentWaitCondition>>,
    pub(crate) timeout_ms: Option<AgentWaitTimeout>,
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
pub(crate) enum ManifestCommand {
    Current,
    Update {
        operation_id: Option<WorkbenchRequestId>,
    },
    Reload {
        operation_id: Option<WorkbenchRequestId>,
    },
    Status {
        operation_id: WorkbenchRequestId,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ServerStopCommand {
    Stop {
        id: Option<orchbee_local_client_protocol::StopId>,
        /// Whether `--confirm` was given. A release Windows build parses a stop without it and
        /// refuses it at destructive admission instead.
        confirm: bool,
        retry: bool,
        detach: bool,
        timeout_ms: u32,
    },
    Status {
        id: orchbee_local_client_protocol::StopId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeStatusScope {
    Server,
    Client,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Request {
    RetainedClose {
        family: operation::CloseFamily,
        operation_id: WorkbenchRequestId,
        generation: Option<WorkbenchTabCloseAttemptGeneration>,
        format: OutputFormat,
    },
    ServerOperationStatus {
        operation_id: WorkbenchRequestId,
        format: OutputFormat,
    },
    PaneGraphics {
        command: pane_graphics::Command,
        format: OutputFormat,
    },
    Events {
        command: events::Command,
        format: OutputFormat,
    },
    ConfiguredCommand {
        command: configured_commands::Command,
        format: OutputFormat,
    },
    TerminalEditor {
        target: Target,
        client: Option<orchbee_workbench_protocol::WorkbenchClientId>,
        timeout_ms: u32,
        format: OutputFormat,
    },
    TerminalClear {
        target: Target,
        format: OutputFormat,
    },
    LayoutHistory {
        format: OutputFormat,
    },
    LayoutReopen {
        saved_at: u64,
        format: OutputFormat,
    },
    TerminalLink {
        command: TerminalLinkCommand,
        format: OutputFormat,
    },
    TerminalCopy {
        command: TerminalCopyCommand,
        format: OutputFormat,
    },
    TerminalSelection {
        command: TerminalSelectionCommand,
        format: OutputFormat,
    },
    AgentView {
        action: orchbee_workbench_protocol::AgentViewAction,
        format: OutputFormat,
    },
    MachineRoute(machine_routing::Route),
    MachineRelay,
    MachineTunnel,
    Machine {
        command: MachineCommand,
        format: OutputFormat,
    },
    PanelAgent {
        command: PanelAgentCommand,
        format: OutputFormat,
    },
    PanelMetadata {
        command: PanelMetadataCommand,
        format: OutputFormat,
    },
    WorkspaceMetadata {
        command: WorkspaceMetadataCommand,
        format: OutputFormat,
    },
    LayoutApply {
        command: LayoutApplyCommand,
        format: OutputFormat,
    },
    LayoutExport {
        command: LayoutExportCommand,
        format: OutputFormat,
    },
    NamedSession {
        command: NamedSessionCommand,
        format: OutputFormat,
    },
    TerminalAttach {
        command: TerminalAttachCommand,
    },
    TerminalObserve {
        command: TerminalObserveCommand,
    },
    TerminalControl {
        command: TerminalControlCommand,
    },
    PanelRelocation {
        command: PanelRelocationCommand,
        format: OutputFormat,
    },
    WorkspaceCreation {
        command: ParsedWorkspaceCreationCommand,
        format: OutputFormat,
    },
    RuntimeStatus {
        scope: RuntimeStatusScope,
        format: OutputFormat,
    },
    PanelTerminalCreation {
        command: panel_terminal_creation::PanelTerminalCreationCommand<program::ParsedLaunch>,
        format: OutputFormat,
    },
    Creation {
        command: CreationCommand<program::ParsedLaunch>,
        format: OutputFormat,
    },
    SchemaExport {
        schema_id: Option<String>,
        path: String,
        format: OutputFormat,
    },
    /// `diagnostics create`: one diagnostic bundle written to `path`.
    DiagnosticsCreate {
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
        command: ParsedWorktreeQueryCommand,
        format: OutputFormat,
    },
    Worktree {
        command: ParsedWorktreeCommand,
        format: OutputFormat,
    },
    ServerStop {
        command: ServerStopCommand,
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
    /// An invocation that reaches a command surface this release withholds; it is refused before
    /// any grammar or runtime work.
    Withheld(grammar::Withheld),
    /// The canonical Windows user configuration template; no runtime discovery.
    DefaultConfig,
    RootHelp,
    Help(HelpTopic),
    Completion(CompletionShell),
    ClientQuery {
        command: QueryCommand,
        client: WorkbenchClientId,
        format: OutputFormat,
    },
    Query {
        command: QueryCommand,
        format: OutputFormat,
    },
    Mutation {
        command: MutationCommand,
        format: OutputFormat,
    },
    /// `server start`: the headless connect-or-activate command — neither a
    /// Workbench query nor a mutation; it runs the shared activation core.
    ServerStart {
        format: OutputFormat,
    },
    /// `server connect`: relays one admitted Client connection over standard input and output.
    ServerConnect,
    Plugin {
        command: plugins::PluginCommand,
        format: OutputFormat,
    },
    ChannelShow {
        format: OutputFormat,
    },
    Update {
        channel: Option<orchbee_local_runtime::UpdateChannel>,
        format: OutputFormat,
    },
    ConfigResetKeys {
        format: OutputFormat,
    },
    /// Read or reload the running Session's configuration; never activates a Session.
    WindowTitle {
        title: Option<orchbee_workbench_protocol::WindowTitle>,
        format: OutputFormat,
    },
    Notification {
        message: orchbee_workbench_protocol::Notification,
        format: OutputFormat,
    },
    RuntimeConfiguration {
        action: orchbee_workbench_protocol::ConfigurationAction,
        format: OutputFormat,
    },
    AgentManifests {
        command: ManifestCommand,
        format: OutputFormat,
    },
    ServerDiagnose {
        format: OutputFormat,
    },
    AgentStart {
        command: AgentStartCommand,
        format: OutputFormat,
    },
    AgentNames {
        command: AgentNamesCommand,
        format: OutputFormat,
    },
    /// `agent integration`: provider onboarding — declares activation of the local Session and
    /// runs one Server-authorized job on its Worker.
    AgentIntegration {
        command: AgentIntegrationCommand,
        format: OutputFormat,
    },
}

/// Invocation-wide selection is separate from business targets such as Agent sessions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectedRequest {
    pub(crate) request: Request,
    pub(crate) availability: grammar::Availability,
    pub(crate) format: Option<OutputFormat>,
    pub(crate) session: Option<orchbee_local_runtime::SessionName>,
}

#[derive(Default)]
struct Options {
    seen: Vec<&'static str>,
    values: Vec<(&'static str, String)>,
    dry_run: bool,
    events: events::Options,
    link_offset: Option<String>,
    copy_motion: Option<String>,
    copy_query: Option<String>,
    copy_previous: Option<String>,
    selection_anchor: Option<String>,
    content_revision: Option<String>,
    agent_view_spec: Option<String>,
    remote_session: Option<String>,
    machine_json: bool,
    plugin_enabled: bool,
    plugin_disabled: bool,
    plugin_id: Option<String>,
    plugin_context: Option<String>,
    plugin_entrypoint: Option<String>,
    plugin_target: Option<String>,
    plugin_placement: Option<String>,
    plugin_width: Option<String>,
    plugin_height: Option<String>,
    plugin_yes: bool,
    plugin_ref: Option<String>,
    notification_body: Option<String>,
    notification_position: Option<String>,
    notification_sound: Option<String>,
    child: bool,
    report_presentation: Option<String>,
    panel_agent: panel_agent::Options,
    metadata: workspace_metadata::Options,
    panel_metadata: panel_metadata::Options,
    session: Option<String>,
    output: Option<String>,
    client: Option<String>,
    focus: bool,
    no_focus: bool,
    zoom_on: bool,
    zoom_off: bool,
    zoom_toggle: bool,
    branch: Option<String>,
    base: Option<String>,
    force: bool,
    takeover: bool,
    trust_repository: bool,
    request_id: Option<String>,
    detach: bool,
    retry: bool,
    format: Option<OutputFormat>,
    project: Option<String>,
    directory: Option<String>,
    workspace: Option<String>,
    tab: Option<String>,

    label: Option<String>,
    direction: Option<String>,
    order: Option<String>,
    search_direction: Option<String>,
    terminal: Option<String>,
    insert_index: Option<String>,
    before: Option<String>,
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
    /// `panel move --into-tab <t>`: the existing-Tab placement target.
    into_tab: Option<String>,
    /// `panel move --new-tab`: the new-Tab relocation destination flag.
    new_tab: bool,
    new_workspace: bool,
    tab_label: Option<String>,
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
    /// builds the ordered stroke record sequence.
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
    outdated_only: bool,
    include_ended: bool,
    worktrees: bool,
    limit: Option<String>,
    cursor: Option<String>,
    /// `agent wait --until <condition>`: the repeatable condition value —
    /// every occurrence appends one raw argv value in order, validated and
    /// deduplicated when the command binds it.
    until: Vec<String>,
    /// `agent wait --after-revision <n>`: the revision-fence value option.
    after_revision: Option<String>,
    output_after: Option<String>,
    /// `agent doctor --provider <id>`: repeatable, append-ordered.
    providers: Vec<String>,
    /// `agent doctor --asset-root <path>`: repeatable, append-ordered.
    asset_roots: Vec<String>,
    name: Option<String>,
    agent_session: Option<String>,
    clear: bool,
    prompt_wait: bool,
    forever: bool,
    profile: Option<String>,
    provider: Option<String>,
    adapter_source: Option<String>,
    /// `workspace close-resume --request <id>`: the close operation's own
    /// request ID — the operation identity, never the resume's new
    /// request identity.
    request: Option<String>,
    /// `workspace close-resume --generation <n>`: the explicit attempt
    /// fence; a stale value is a settled `stale` answer, never an effect.
    generation: Option<String>,
}

fn query(command: QueryCommand, options: &Options) -> Request {
    Request::Query {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    }
}

fn mutation(command: MutationCommand, options: &Options) -> Request {
    Request::Mutation {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
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

#[cfg(test)]
mod grammar_consistency_tests;
#[cfg(test)]
mod tests;
