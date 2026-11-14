//! Structured usage failures: the failure vocabulary, the stdout envelope a failure may answer on,
//! and the constructors every parser shares. A usage reason is stderr-only classification; the
//! machine contract is the failure kind plus its envelope.

use super::{OutputFormat, Resource};

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
    DeclaredValue,
    GraphicsSpecification,
    EventsSpecification,
    TerminalViewportPoint,
    TerminalCopyMotion,
    TerminalCopyQuery,
    TerminalCopyPrevious,
    TerminalCopyDirection,
    TerminalSelectionPoint,
    TerminalSelectionSpan,
    TerminalContentRevision,
    TerminalContentGeneration,
    AgentViewSpec,
    AgentViewSource,
    MachineSelector,
    MachineLabel,
    SshTarget,
    PluginId,
    PluginActionId,
    ConfiguredCommandId,
    PluginContext,
    PluginLogLimit,
    PluginPanePlacement,
    PluginPopupSize,
    PluginPaneDirection,
    PluginGithubSource,
    PluginGitReference,
    WindowTitle,
    NotificationTitle,
    NotificationPosition,
    NotificationSound,
    PresentationId,
    ReportAgent,
    ReportState,
    ReportSize,
    ReportSource,
    MetadataAgent,
    MetadataStateLabel,
    MetadataPresentation,
    MetadataSource,
    MetadataToken,
    MetadataPatchSize,
    MetadataSequence,
    MetadataTtl,
    SessionName,
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
    WorkspaceInsertIndex,
    LayoutSavedAt,
    WorkspaceBlockSize,
    PanelDirection,
    LayoutDirection,
    FiniteNumber,
    SplitPath,
    Shell,
    DirectoryPath,
    WorktreePath,
    /// The `--directory` worktree-list selector: a nonzero 32-character
    /// lowercase hexadecimal Workbench directory ID.
    DirectoryId,
    MatchText,
    /// The `terminal wait-output --regex` operand: a nonempty UTF-8 pattern
    /// of at most 1024 bytes that compiles inside the declared budgets.
    RegexPattern,
    OutputSource,
    OutputLines,
    WaitTimeout,
    AgentStartTimeout,
    AgentStartProvider,
    AgentStartArgument,
    ServerStopRequestId,
    ManifestOperationId,
    SendText,
    SendKey,
    /// The repeatable `--key` sequence bound: at most 64 values per request.
    SendKeySequence,
    PromptText,
    CommandText,
    MouseMode,
    Executable,
    ExecutableName,
    ExecutableOnPath,
    LaunchArguments,
    LaunchEnvironment,
    AgentSessionId,
    /// A positional Agent target: session ID or name, Panel ID or selector.
    AgentTarget,
    AgentName,
    AgentListLimit,
    AgentCursor,
    OutputCursor,
    /// The `agent wait --until` operand: one of the fourteen
    /// condition names.
    AgentWaitUntil,
    /// The `agent wait --after-revision` operand: a nonnegative
    /// safe-integer global runtime revision.
    AgentAfterRevision,
    /// The `agent doctor --provider` value: one provider with an Orchbee integration.
    ProviderId,
    /// An integration-scoped `--provider` named a canonical Agent that Orchbee
    /// can only start: it has no integration to check or manage.
    StartOnlyProvider,
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
    /// The `<family> status|resume <request-id>` operand: the operation's
    /// own nonzero 32-character lowercase hexadecimal request ID.
    CloseRequestId,
    /// The `<family> resume --generation` operand: the caller's
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
    /// `server connect` on a terminal: its protocol bytes need a program at both ends.
    RelayRequiresPipes,
    UnknownCommand {
        context: UsageContext,
        word: String,
    },
    UnknownOption {
        context: UsageContext,
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
#[cfg_attr(
    not(any(windows, target_os = "linux")),
    expect(
        dead_code,
        reason = "only the Linux and Windows source adapters read a declared source"
    )
)]
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

/// An `invalid-usage` failure carrying the concrete reason the original exit
/// reached; the reason rides the same envelope untouched.
pub(crate) fn usage_with(reason: UsageReason, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope,
        usage: Some(reason),
    }
}

pub(super) fn identifier(
    resource: Resource,
    target: String,
    envelope: FailureEnvelope,
) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
        usage: None,
    }
}

pub(crate) fn selection_envelope(format: Option<OutputFormat>) -> FailureEnvelope {
    match format {
        Some(OutputFormat::Json) => FailureEnvelope::Json,
        Some(OutputFormat::Text) | None => FailureEnvelope::Text,
    }
}

pub(super) fn missing_required(
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

pub(super) fn missing_command(context: UsageContext, envelope: FailureEnvelope) -> ParseFailure {
    usage_with(UsageReason::MissingCommand { context }, envelope)
}

pub(super) fn unknown_command(
    context: UsageContext,
    word: &str,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::UnknownCommand {
            context,
            word: word.to_owned(),
        },
        envelope,
    )
}

pub(super) fn unexpected_argument(
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
