//! Declared option conflicts shared by admission and every completion adapter.
use super::{CommandPath, CommandSpec};

struct Conflict {
    commands: &'static [CommandPath],
    flags: &'static [&'static str],
    incompatible: &'static [&'static str],
}

const CONFLICTS: &[Conflict] = &[
    Conflict {
        commands: &[CommandPath::MachineList],
        flags: &["--json"],
        incompatible: &["--format"],
    },
    Conflict {
        commands: &[CommandPath::MachineList],
        flags: &["--format"],
        incompatible: &["--json"],
    },
    Conflict {
        commands: &[CommandPath::PluginLink],
        flags: &["--enabled"],
        incompatible: &["--disabled"],
    },
    Conflict {
        commands: &[CommandPath::PluginLink],
        flags: &["--disabled"],
        incompatible: &["--enabled"],
    },
    Conflict {
        commands: &[CommandPath::PanelCreateTerminal],
        flags: &["--child"],
        incompatible: &["--presentation"],
    },
    Conflict {
        commands: &[CommandPath::PanelCreateTerminal],
        flags: &["--presentation"],
        incompatible: &["--child", "--label"],
    },
    Conflict {
        commands: &[CommandPath::PanelCreateTerminal],
        flags: &["--label"],
        incompatible: &["--presentation"],
    },
    Conflict {
        commands: &[CommandPath::PanelCreateTerminal],
        flags: &["--profile"],
        incompatible: &["--exec", "--arg"],
    },
    Conflict {
        commands: &[CommandPath::PanelCreateTerminal],
        flags: &["--exec", "--arg"],
        incompatible: &["--profile"],
    },
    Conflict {
        commands: &[CommandPath::PanelReportMetadata],
        flags: &["--title"],
        incompatible: &["--clear-title"],
    },
    Conflict {
        commands: &[CommandPath::PanelReportMetadata],
        flags: &["--clear-title"],
        incompatible: &["--title"],
    },
    Conflict {
        commands: &[CommandPath::PanelReportMetadata],
        flags: &["--display-agent"],
        incompatible: &["--clear-display-agent"],
    },
    Conflict {
        commands: &[CommandPath::PanelReportMetadata],
        flags: &["--clear-display-agent"],
        incompatible: &["--display-agent"],
    },
    Conflict {
        commands: &[CommandPath::PanelReportMetadata],
        flags: &["--state-label"],
        incompatible: &["--clear-state-labels"],
    },
    Conflict {
        commands: &[CommandPath::PanelReportMetadata],
        flags: &["--clear-state-labels"],
        incompatible: &["--state-label"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--file"],
        incompatible: &["--stdin"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--stdin"],
        incompatible: &["--file"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--tab"],
        incompatible: &["--workspace"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--workspace"],
        incompatible: &["--tab", "--confirm"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--confirm"],
        incompatible: &["--workspace"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--focus"],
        incompatible: &["--no-focus"],
    },
    Conflict {
        commands: &[CommandPath::LayoutApply],
        flags: &["--no-focus"],
        incompatible: &["--focus"],
    },
    Conflict {
        commands: &[CommandPath::LayoutExport],
        flags: &["--tab"],
        incompatible: &["--panel", "--current"],
    },
    Conflict {
        commands: &[CommandPath::LayoutExport],
        flags: &["--panel"],
        incompatible: &["--tab", "--current"],
    },
    Conflict {
        commands: &[CommandPath::LayoutExport],
        flags: &["--current"],
        incompatible: &["--tab", "--panel"],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--into-tab"],
        incompatible: &[
            "--new-tab",
            "--new-workspace",
            "--workspace",
            "--project",
            "--label",
            "--tab-label",
        ],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--new-tab"],
        incompatible: &[
            "--into-tab",
            "--new-workspace",
            "--target-panel",
            "--direction",
            "--ratio",
            "--project",
            "--tab-label",
        ],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--new-workspace"],
        incompatible: &[
            "--into-tab",
            "--new-tab",
            "--target-panel",
            "--direction",
            "--ratio",
            "--workspace",
        ],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--target-panel", "--direction", "--ratio"],
        incompatible: &["--new-tab", "--new-workspace"],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--label"],
        incompatible: &["--into-tab"],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--workspace"],
        incompatible: &["--into-tab", "--new-workspace", "--project", "--tab-label"],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--project", "--tab-label"],
        incompatible: &["--into-tab", "--new-tab", "--workspace"],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--focus"],
        incompatible: &["--no-focus"],
    },
    Conflict {
        commands: &[CommandPath::PanelMove],
        flags: &["--no-focus"],
        incompatible: &["--focus"],
    },
    Conflict {
        commands: &[CommandPath::AgentAttach],
        flags: &["--name"],
        incompatible: &["--agent-session", "--panel"],
    },
    Conflict {
        commands: &[CommandPath::AgentAttach],
        flags: &["--agent-session"],
        incompatible: &["--name", "--panel"],
    },
    Conflict {
        commands: &[CommandPath::AgentAttach],
        flags: &["--panel"],
        incompatible: &["--name", "--agent-session"],
    },
    Conflict {
        commands: &[CommandPath::WorktreeCreate, CommandPath::WorktreeOpen],
        flags: &["--focus"],
        incompatible: &["--no-focus", "--detach"],
    },
    Conflict {
        commands: &[CommandPath::WorktreeCreate, CommandPath::WorktreeOpen],
        flags: &["--no-focus", "--detach"],
        incompatible: &["--focus"],
    },
    Conflict {
        commands: &[CommandPath::WorktreeCreate, CommandPath::WorktreeOpen],
        flags: &["--project"],
        incompatible: &["--workspace"],
    },
    Conflict {
        commands: &[CommandPath::WorktreeCreate, CommandPath::WorktreeOpen],
        flags: &["--workspace"],
        incompatible: &["--project", "--cwd"],
    },
    Conflict {
        commands: &[CommandPath::WorktreeList],
        flags: &["--client"],
        incompatible: &["--directory"],
    },
    Conflict {
        commands: &[CommandPath::WorktreeList],
        flags: &["--project", "--directory"],
        incompatible: &["--client"],
    },
    Conflict {
        commands: &[CommandPath::PanelCurrent, CommandPath::PanelProcessInfo],
        flags: &["--panel"],
        incompatible: &["--current", "--client"],
    },
    Conflict {
        commands: &[CommandPath::PanelCurrent, CommandPath::PanelProcessInfo],
        flags: &["--client", "--current"],
        incompatible: &["--panel"],
    },
    Conflict {
        commands: &[CommandPath::TerminalWaitOutput],
        flags: &["--forever"],
        incompatible: &["--timeout"],
    },
    Conflict {
        commands: &[CommandPath::TerminalWaitOutput],
        flags: &["--timeout"],
        incompatible: &["--forever"],
    },
    Conflict {
        commands: &[CommandPath::TerminalRead, CommandPath::AgentRead],
        flags: &["--plain"],
        incompatible: &["--ansi"],
    },
    Conflict {
        commands: &[CommandPath::TerminalRead, CommandPath::AgentRead],
        flags: &["--ansi"],
        incompatible: &["--plain"],
    },
    Conflict {
        commands: &[
            CommandPath::TabCreate,
            CommandPath::PanelSplit,
            CommandPath::WorkspaceCreate,
        ],
        flags: &["--focus"],
        incompatible: &["--no-focus"],
    },
    Conflict {
        commands: &[
            CommandPath::TabCreate,
            CommandPath::PanelSplit,
            CommandPath::WorkspaceCreate,
        ],
        flags: &["--no-focus"],
        incompatible: &["--focus"],
    },
    Conflict {
        commands: &[CommandPath::TabCreate, CommandPath::PanelSplit],
        flags: &["--profile"],
        incompatible: &["--exec", "--arg"],
    },
    Conflict {
        commands: &[CommandPath::TabCreate, CommandPath::PanelSplit],
        flags: &["--exec", "--arg"],
        incompatible: &["--profile"],
    },
    Conflict {
        commands: &[CommandPath::PanelInput, CommandPath::PanelSplit],
        flags: &["--panel"],
        incompatible: &["--current"],
    },
    Conflict {
        commands: &[CommandPath::PanelInput, CommandPath::PanelSplit],
        flags: &["--current"],
        incompatible: &["--panel"],
    },
    Conflict {
        commands: &[CommandPath::TabMove],
        flags: &["--order"],
        incompatible: &["--insert-index"],
    },
    Conflict {
        commands: &[CommandPath::TabMove],
        flags: &["--insert-index"],
        incompatible: &["--order"],
    },
    Conflict {
        commands: &[CommandPath::PanelSwap],
        flags: &["--with"],
        incompatible: &["--direction"],
    },
    Conflict {
        commands: &[CommandPath::PanelSwap],
        flags: &["--direction"],
        incompatible: &["--with"],
    },
    Conflict {
        commands: &[CommandPath::PanelRename],
        flags: &["--label"],
        incompatible: &["--clear"],
    },
    Conflict {
        commands: &[CommandPath::PanelRename],
        flags: &["--clear"],
        incompatible: &["--label"],
    },
    Conflict {
        commands: &[CommandPath::LayoutSetSplitRatio],
        flags: &["--tab"],
        incompatible: &["--panel", "--current"],
    },
    Conflict {
        commands: &[CommandPath::LayoutSetSplitRatio],
        flags: &["--panel"],
        incompatible: &["--tab", "--current"],
    },
    Conflict {
        commands: &[CommandPath::LayoutSetSplitRatio],
        flags: &["--current"],
        incompatible: &["--tab", "--panel"],
    },
    Conflict {
        commands: &[CommandPath::PanelZoom],
        flags: &["--on"],
        incompatible: &["--off", "--toggle"],
    },
    Conflict {
        commands: &[CommandPath::PanelZoom],
        flags: &["--off"],
        incompatible: &["--on", "--toggle"],
    },
    Conflict {
        commands: &[CommandPath::PanelZoom],
        flags: &["--toggle"],
        incompatible: &["--on", "--off"],
    },
    Conflict { commands: &[CommandPath::AgentPrompt, CommandPath::AgentWait, CommandPath::TerminalWaitOutput], flags: &["--timeout"], incompatible: &["--forever"] },
    Conflict { commands: &[CommandPath::AgentPrompt, CommandPath::AgentWait, CommandPath::TerminalWaitOutput], flags: &["--forever"], incompatible: &["--timeout"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--agent-session"], incompatible: &["--name"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--agent-session"], incompatible: &["--panel"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--agent-session"], incompatible: &["--terminal"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--name"], incompatible: &["--agent-session"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--name"], incompatible: &["--panel"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--name"], incompatible: &["--terminal"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--panel"], incompatible: &["--agent-session"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--panel"], incompatible: &["--name"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--panel"], incompatible: &["--terminal"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--terminal"], incompatible: &["--agent-session"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--terminal"], incompatible: &["--name"] },
    Conflict { commands: &[CommandPath::AgentAttach, CommandPath::AgentAttention, CommandPath::AgentExplain, CommandPath::AgentFocus, CommandPath::AgentGet, CommandPath::AgentPrompt, CommandPath::AgentRead, CommandPath::AgentRename, CommandPath::AgentResolve, CommandPath::AgentSendKeys, CommandPath::AgentWait], flags: &["--terminal"], incompatible: &["--panel"] },
    Conflict { commands: &[CommandPath::AgentStart, CommandPath::LayoutExport, CommandPath::LayoutSetSplitRatio], flags: &["--panel"], incompatible: &["--current"] },
    Conflict { commands: &[CommandPath::AgentStart, CommandPath::LayoutExport, CommandPath::LayoutSetSplitRatio], flags: &["--current"], incompatible: &["--panel"] },
    Conflict { commands: &[CommandPath::PanelCreateTerminal, CommandPath::PanelSplit, CommandPath::TabCreate, CommandPath::WorkspaceCreate], flags: &["--exec"], incompatible: &["--profile"] },
    Conflict { commands: &[CommandPath::PanelCreateTerminal, CommandPath::PanelSplit, CommandPath::TabCreate, CommandPath::WorkspaceCreate], flags: &["--profile"], incompatible: &["--exec"] },
    Conflict { commands: &[CommandPath::PanelCreateTerminal, CommandPath::PanelSplit, CommandPath::TabCreate, CommandPath::WorkspaceCreate], flags: &["--arg"], incompatible: &["--profile"] },
    Conflict { commands: &[CommandPath::PanelCreateTerminal, CommandPath::PanelSplit, CommandPath::TabCreate, CommandPath::WorkspaceCreate], flags: &["--profile"], incompatible: &["--arg"] },
    Conflict { commands: &[CommandPath::PanelMove], flags: &["--into-tab"], incompatible: &["--new-tab"] },
    Conflict { commands: &[CommandPath::PanelMove], flags: &["--new-tab"], incompatible: &["--into-tab"] },
    Conflict { commands: &[CommandPath::PanelMove], flags: &["--target-panel"], incompatible: &["--new-tab"] },
    Conflict { commands: &[CommandPath::PanelMove], flags: &["--new-tab"], incompatible: &["--target-panel"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--text"], incompatible: &["--file"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--text"], incompatible: &["--stdin"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--text"], incompatible: &["--key"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--file"], incompatible: &["--text"] },
    Conflict { commands: &[CommandPath::LayoutApply, CommandPath::TerminalSend], flags: &["--file"], incompatible: &["--stdin"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--file"], incompatible: &["--key"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--stdin"], incompatible: &["--text"] },
    Conflict { commands: &[CommandPath::LayoutApply, CommandPath::TerminalSend], flags: &["--stdin"], incompatible: &["--file"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--stdin"], incompatible: &["--key"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--key"], incompatible: &["--text"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--key"], incompatible: &["--file"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--key"], incompatible: &["--stdin"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--key"], incompatible: &["--enter"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--enter"], incompatible: &["--key"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--key"], incompatible: &["--paste"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--paste"], incompatible: &["--key"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--text"], incompatible: &["--paste"] },
    Conflict { commands: &[CommandPath::TerminalSend], flags: &["--paste"], incompatible: &["--text"] },
    Conflict { commands: &[CommandPath::EventsSubscribe, CommandPath::EventsWait, CommandPath::TerminalWaitOutput], flags: &["--match"], incompatible: &["--regex"] },
    Conflict { commands: &[CommandPath::EventsSubscribe, CommandPath::EventsWait, CommandPath::TerminalWaitOutput], flags: &["--regex"], incompatible: &["--match"] },
];

static PROJECTED_CONFLICTS: std::sync::LazyLock<Vec<(CommandPath, &'static str, Vec<&'static str>)>> = std::sync::LazyLock::new(|| {
    super::COMMANDS.iter().flat_map(|command| command.flags().iter().map(move |flag| {
        let mut conflicts = CONFLICTS.iter().filter(|rule| rule.commands.contains(&command.id) && rule.flags.contains(flag))
            .flat_map(|rule| rule.incompatible.iter().copied()).collect::<Vec<_>>();
        conflicts.sort_unstable(); conflicts.dedup(); (command.id, *flag, conflicts)
    })).collect()
});

pub(super) fn conflicts(command: &CommandSpec, flag: &str) -> &'static [&'static str] {
    PROJECTED_CONFLICTS.iter().find(|(id, spelling, _)| *id == command.id && *spelling == flag).map_or(&[], |(_, _, values)| values.as_slice())
}

pub(crate) struct OperandRule {
    commands: &'static [CommandPath],
    pub(crate) reductions: &'static [&'static [&'static str]],
    pub(crate) selectors: &'static [&'static str],
    pub(crate) values: &'static [&'static str],
    pub(crate) value_offset: usize,
}
const OPERANDS: &[OperandRule] = &[
    OperandRule { commands: &[CommandPath::PanelCurrent, CommandPath::PanelFocus, CommandPath::PanelZoom, CommandPath::PanelLayout, CommandPath::PanelNeighbor, CommandPath::PanelEdges, CommandPath::PanelResize, CommandPath::PanelSwap, CommandPath::PanelInput, CommandPath::PanelSplit], reductions: &[&["--panel", "--current"]], selectors: &["--panel", "--current"], values: &[], value_offset: 0 },
    OperandRule { commands: &[CommandPath::PanelProcessInfo, CommandPath::PanelCreateTerminal], reductions: &[&["--panel", "--current", "--client"]], selectors: &["--panel", "--current", "--client"], values: &[], value_offset: 0 },
    OperandRule { commands: &[CommandPath::TerminalRead, CommandPath::TerminalSend, CommandPath::TerminalWaitOutput, CommandPath::TerminalRun], reductions: &[&["--panel"]], selectors: &["--panel"], values: &[], value_offset: 0 },
    OperandRule { commands: &[CommandPath::AgentAttach, CommandPath::AgentGet, CommandPath::AgentFocus, CommandPath::AgentRead, CommandPath::AgentPrompt, CommandPath::AgentExplain, CommandPath::AgentWait, CommandPath::AgentResolve], reductions: &[&["--agent-session", "--name", "--panel", "--terminal"]], selectors: &["--agent-session", "--name", "--panel", "--terminal"], values: &[], value_offset: 0 },
    OperandRule { commands: &[CommandPath::AgentRename], reductions: &[&["--agent-session", "--name", "--panel", "--terminal"], &["--clear"]], selectors: &["--agent-session", "--name", "--panel", "--terminal"], values: &[], value_offset: 0 },
    OperandRule { commands: &[CommandPath::AgentSendKeys], reductions: &[&["--agent-session", "--name", "--panel", "--terminal"]], selectors: &["--agent-session", "--name", "--panel", "--terminal"], values: super::values::KEY_VALUES, value_offset: 1 },
 ];
const ORDINARY_OPERANDS: OperandRule = OperandRule { commands: &[], reductions: &[], selectors: &[], values: &[], value_offset: 0 };

pub(super) fn operands(command: &CommandSpec) -> &'static OperandRule {
    OPERANDS.iter().find(|rule| rule.commands.contains(&command.id)).unwrap_or(&ORDINARY_OPERANDS)
}

pub(crate) struct Requirement {
    commands: &'static [CommandPath],
    pub(crate) flag: &'static str,
    pub(crate) any: &'static [&'static str],
}
const REQUIREMENTS: &[Requirement] = &[
    Requirement { commands: &[CommandPath::LaunchProfileCreate, CommandPath::PanelCreateTerminal, CommandPath::PanelSplit, CommandPath::TabCreate, CommandPath::WorkspaceCreate], flag: "--arg", any: &["--exec"] },
    Requirement { commands: &[CommandPath::TerminalSend], flag: "--enter", any: &["--text", "--file", "--stdin"] },
 ];
pub(super) fn requirements(command: &CommandSpec) -> impl Iterator<Item = &'static Requirement> {
    REQUIREMENTS.iter().filter(move |rule| rule.commands.contains(&command.id))
}
