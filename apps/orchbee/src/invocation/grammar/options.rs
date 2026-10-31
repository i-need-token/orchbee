//! One record owns a spelling, its storage binding, multiplicity and command admission.

use super::{CommandPath, OptionRole, OptionSpec};

pub(crate) const OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        spelling: "--dry-run",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorkspaceClose, CommandPath::TabClose],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.dry_run = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--event",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::EventsSubscribe, CommandPath::EventsWait],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.events.events.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--filter",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::EventsSubscribe, CommandPath::EventsWait],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.events.filters.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--after-cursor",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::EventsSubscribe, CommandPath::EventsWait],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.events.after = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--offset-from-bottom",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::TerminalLinkResolve,
            CommandPath::TerminalLinkActivate,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.link_offset = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--motion",
        values: super::values::COPY_MOTION_VALUES,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::TerminalCopyMotion],
        role: OptionRole::Business,
        value: Some("copy_motion"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.copy_motion = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--query",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalCopySearch],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.copy_query = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--previous",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalCopySearch],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.copy_previous = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--anchor",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::CommandInvoke,
            CommandPath::TerminalSelectionRead,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.selection_anchor = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--content-revision",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::CommandInvoke,
            CommandPath::TerminalLinkResolve,
            CommandPath::TerminalLinkActivate,
            CommandPath::TerminalCopyMotion,
            CommandPath::TerminalCopySearch,
            CommandPath::TerminalSelectionRead,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.content_revision = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--spec",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PanelGraphicsSet,
            CommandPath::PanelGraphicsClear,
            CommandPath::PanelGraphicsStream,
            CommandPath::AgentViewSet,
        ],
        role: OptionRole::Business,
        value: Some("agent_view_spec"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.agent_view_spec = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--remote-session",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::MachineAdd],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.remote_session = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--json",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::MachineList],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.machine_json = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--yes",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginInstall],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.plugin_yes = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--entrypoint",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginPaneOpen],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_entrypoint = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--placement",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginPaneOpen],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_placement = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--width",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginPaneOpen],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_width = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--height",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginPaneOpen],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_height = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--target-pane",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginPaneOpen],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_target = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--ref",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginInstall],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_ref = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--plugin",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginList,
            CommandPath::PluginActionList,
            CommandPath::PluginActionInvoke,
            CommandPath::PluginPaneOpen,
            CommandPath::PluginLogList,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_id = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--context",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::CommandInvoke, CommandPath::PluginActionInvoke],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.plugin_context = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--enabled",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginLink],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.plugin_enabled = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--disabled",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PluginLink],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.plugin_disabled = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--body",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::NotificationShow],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.notification_body = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--position",
        values: super::values::NOTIFICATION_POSITIONS,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::NotificationShow],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.notification_position = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--sound",
        values: super::values::NOTIFICATION_SOUNDS,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::NotificationShow],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.notification_sound = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--child",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelCreateTerminal],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.child = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--presentation",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PanelGraphicsSet,
            CommandPath::PanelGraphicsClear,
            CommandPath::PanelGraphicsInfo,
            CommandPath::PanelGraphicsStream,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelReportAgent,
            CommandPath::PanelClearAgentAuthority,
            CommandPath::PanelReleaseAgent,
            CommandPath::PanelReportMetadata,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.report_presentation = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--agent",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PanelReportAgent,
            CommandPath::PanelReleaseAgent,
            CommandPath::PanelReportMetadata,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_metadata.agent = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--state",
        values: super::values::PANEL_AGENT_STATES,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::PanelReportAgent],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_agent.state = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--message",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportAgent],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_agent.message = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--applies-to-source",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_metadata.applies_to_source = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--title",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_metadata.title = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--display-agent",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_metadata.display_agent = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--state-label",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel_metadata.state_labels.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--clear-title",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.panel_metadata.clear_title = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--clear-display-agent",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.panel_metadata.clear_display_agent = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--clear-state-labels",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelReportMetadata],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.panel_metadata.clear_state_labels = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--token",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceReportMetadata,
            CommandPath::PanelReportMetadata,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.metadata.updates.push(("--token", value));
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--clear-token",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceReportMetadata,
            CommandPath::PanelReportMetadata,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.metadata.updates.push(("--clear-token", value));
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--seq",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceReportMetadata,
            CommandPath::PanelReportAgent,
            CommandPath::PanelClearAgentAuthority,
            CommandPath::PanelReleaseAgent,
            CommandPath::PanelReportMetadata,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.metadata.sequence = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--ttl-ms",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceReportMetadata,
            CommandPath::PanelReportMetadata,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.metadata.ttl = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--right-click",
        values: super::values::RIGHT_CLICK_VALUES,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::PanelInput, CommandPath::PanelSplit],
        role: OptionRole::Business,
        value: Some("right_click"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.right_click = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--focus",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::LayoutApply,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.focus = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--no-focus",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::LayoutApply,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.no_focus = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--amount",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelResize],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.amount = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--ratio",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::LayoutSetSplitRatio,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.ratio = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--output",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::ApiSchema],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.output = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--client",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PanelGraphicsInfo,
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::CommandInvoke,
            CommandPath::TerminalEditScrollback,
            CommandPath::PluginPaneFocus,
            CommandPath::PopupClose,
            CommandPath::PluginActionInvoke,
            CommandPath::PluginPaneOpen,
            CommandPath::LayoutApply,
            CommandPath::LayoutExport,
            CommandPath::PanelResize,
            CommandPath::LayoutSetSplitRatio,
            CommandPath::PanelLayout,
            CommandPath::PanelNeighbor,
            CommandPath::PanelEdges,
            CommandPath::WorkspaceFocus,
            CommandPath::TabFocus,
            CommandPath::PanelFocus,
            CommandPath::PanelZoom,
            CommandPath::AgentList,
            CommandPath::AgentGet,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::AgentPrompt,
            CommandPath::AgentWait,
            CommandPath::WorkspaceList,
            CommandPath::WorkspaceGet,
            CommandPath::WorkspaceCreate,
            CommandPath::TabList,
            CommandPath::TabGet,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSwap,
            CommandPath::PanelCurrent,
            CommandPath::PanelProcessInfo,
            CommandPath::PanelList,
            CommandPath::PanelGet,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
            CommandPath::WorktreeList,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.client = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--on",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelZoom],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.zoom_on = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--off",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelZoom],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.zoom_off = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--toggle",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelZoom],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.zoom_toggle = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--branch",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorktreeCreate, CommandPath::WorktreeOpen],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.branch = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--base",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorktreeCreate],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.base = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--force",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorktreeRemove],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.force = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--takeover",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::TerminalAttach,
            CommandPath::AgentAttach,
            CommandPath::TerminalControl,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.takeover = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--trust-repository",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorktreeList,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
            CommandPath::WorktreeRemove,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.trust_repository = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--request-id",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::SessionStop,
            CommandPath::PanelResize,
            CommandPath::LayoutSetSplitRatio,
            CommandPath::WorkspaceFocus,
            CommandPath::TabFocus,
            CommandPath::PanelFocus,
            CommandPath::PanelZoom,
            CommandPath::ServerReloadAgentManifests,
            CommandPath::ServerUpdateAgentManifests,
            CommandPath::ServerStop,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::ProjectCloseAll,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
            CommandPath::WorktreeRemove,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.request_id = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--retry",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::SessionStop, CommandPath::ServerStop],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.retry = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--detach",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::ProjectResume,
            CommandPath::SessionStop,
            CommandPath::PanelResize,
            CommandPath::WorkspaceFocus,
            CommandPath::TabFocus,
            CommandPath::PanelFocus,
            CommandPath::PanelZoom,
            CommandPath::ServerStop,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::ProjectCloseAll,
            CommandPath::ProjectCloseResume,
            CommandPath::WorkspaceClose,
            CommandPath::TabClose,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
            CommandPath::WorktreeRemove,
            CommandPath::WorktreeResume,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.detach = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--project",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceSetProject,
            CommandPath::WorkspaceList,
            CommandPath::WorkspaceCreate,
            CommandPath::PanelMove,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.project = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--workspace",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::CommandInvoke,
            CommandPath::PluginPaneOpen,
            CommandPath::LayoutApply,
            CommandPath::WorkspaceCreate,
            CommandPath::WorkspaceCloseStatus,
            CommandPath::WorkspaceCloseResume,
            CommandPath::TabList,
            CommandPath::TabCreate,
            CommandPath::PanelList,
            CommandPath::TerminalList,
            CommandPath::PanelMove,
            CommandPath::WorktreeList,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
            CommandPath::WorktreeRemove,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.workspace = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--tab",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::CommandInvoke,
            CommandPath::LayoutApply,
            CommandPath::LayoutExport,
            CommandPath::LayoutSetSplitRatio,
            CommandPath::PanelList,
            CommandPath::TerminalList,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.tab = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--label",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::MachineAdd,
            CommandPath::MachineRename,
            CommandPath::LayoutApply,
            CommandPath::ProjectCreate,
            CommandPath::WorkspaceCreateProject,
            CommandPath::ProjectRename,
            CommandPath::WorkspaceCreate,
            CommandPath::WorkspaceRename,
            CommandPath::TabCreate,
            CommandPath::TabRename,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelRename,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.label = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--direction",
        values: super::CARDINAL_DIRECTIONS,
        closed_values: true,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::PanelResize,
            CommandPath::PanelNeighbor,
            CommandPath::PanelFocus,
            CommandPath::PanelSwap,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
        ],
        role: OptionRole::Business,
        value: Some("direction"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.direction = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--order",
        values: super::values::ORDER_VALUES,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::TabMove],
        role: OptionRole::Business,
        value: Some("order"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.order = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--search-direction",
        values: super::values::SEARCH_DIRECTION_VALUES,
        closed_values: true,
        string_rule: None,
        commands: &[CommandPath::TerminalCopySearch],
        role: OptionRole::Business,
        value: Some("search_direction"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.search_direction = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--terminal",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::AgentAttach,
            CommandPath::AgentGet,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::AgentRead,
            CommandPath::AgentSendKeys,
            CommandPath::AgentPrompt,
            CommandPath::AgentExplain,
            CommandPath::AgentResolve,
            CommandPath::AgentRename,
            CommandPath::AgentWait,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.terminal = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--insert-index",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorkspaceMove, CommandPath::TabMove],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.insert_index = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--before",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorkspaceMoveBlock],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.before = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--cols",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::TerminalObserve,
            CommandPath::TerminalControl,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.cols = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--rows",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::TerminalObserve,
            CommandPath::TerminalControl,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.rows = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--cell-width",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.cell_width = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--cell-height",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.cell_height = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--confirm",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceResume,
            CommandPath::TabResume,
            CommandPath::PanelResume,
            CommandPath::ProjectResume,
            CommandPath::LayoutApply,
            CommandPath::SessionDelete,
            CommandPath::SessionStop,
            CommandPath::ServerStop,
            CommandPath::ProjectCloseAll,
            CommandPath::ProjectCloseResume,
            CommandPath::WorkspaceClose,
            CommandPath::WorkspaceCloseResume,
            CommandPath::TabClose,
            CommandPath::PanelClose,
            CommandPath::WorktreeRemove,
            CommandPath::WorktreeResume,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.confirm = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--panel",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::CommandInvoke,
            CommandPath::LayoutExport,
            CommandPath::AgentAttach,
            CommandPath::LayoutSetSplitRatio,
            CommandPath::AgentStart,
            CommandPath::AgentGet,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::AgentRead,
            CommandPath::AgentSendKeys,
            CommandPath::AgentPrompt,
            CommandPath::AgentExplain,
            CommandPath::AgentResolve,
            CommandPath::AgentRename,
            CommandPath::AgentWait,
            CommandPath::TerminalList,
            CommandPath::TerminalRead,
            CommandPath::TerminalSend,
            CommandPath::TerminalRun,
            CommandPath::TerminalWaitOutput,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.panel = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--current",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::LayoutExport,
            CommandPath::PanelResize,
            CommandPath::LayoutSetSplitRatio,
            CommandPath::PanelLayout,
            CommandPath::PanelNeighbor,
            CommandPath::PanelEdges,
            CommandPath::PanelFocus,
            CommandPath::PanelZoom,
            CommandPath::AgentStart,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelInput,
            CommandPath::PanelSwap,
            CommandPath::PanelCurrent,
            CommandPath::PanelProcessInfo,
            CommandPath::PanelSplit,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.current = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--with",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelSwap],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.swap_target = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--target-panel",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelMove],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.target_panel = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--into-tab",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelMove],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.into_tab = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--new-tab",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelMove],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.new_tab = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--new-workspace",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelMove],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.new_workspace = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--tab-label",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::PanelMove],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.tab_label = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--path",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::LayoutSetSplitRatio,
            CommandPath::WorkspaceAcceptDirectory,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.path = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--match",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::TerminalWaitOutput,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.match_text = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--regex",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::TerminalWaitOutput,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.regex = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--source",
        values: super::values::OUTPUT_SOURCES,
        closed_values: true,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::AgentViewClear,
            CommandPath::AgentRead,
            CommandPath::WorkspaceReportMetadata,
            CommandPath::PanelReportAgent,
            CommandPath::PanelClearAgentAuthority,
            CommandPath::PanelReleaseAgent,
            CommandPath::PanelReportMetadata,
            CommandPath::TerminalRead,
            CommandPath::TerminalWaitOutput,
        ],
        role: OptionRole::Business,
        value: Some("output_source"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.output_source = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--plain",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::AgentRead,
            CommandPath::TerminalRead,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.plain = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--ansi",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::AgentRead,
            CommandPath::TerminalRead,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.ansi = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--lines",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::AgentRead,
            CommandPath::TerminalRead,
            CommandPath::TerminalWaitOutput,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.lines = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--timeout",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::ProjectResume,
            CommandPath::PanelGraphicsSet,
            CommandPath::PanelGraphicsClear,
            CommandPath::PanelGraphicsInfo,
            CommandPath::PanelGraphicsStream,
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::CommandInvoke,
            CommandPath::TerminalEditScrollback,
            CommandPath::PluginPaneFocus,
            CommandPath::PluginPaneClose,
            CommandPath::PopupClose,
            CommandPath::PluginInstall,
            CommandPath::PluginPaneOpen,
            CommandPath::LayoutApply,
            CommandPath::LayoutExport,
            CommandPath::SessionStop,
            CommandPath::TerminalAttach,
            CommandPath::AgentAttach,
            CommandPath::TerminalObserve,
            CommandPath::TerminalControl,
            CommandPath::PanelResize,
            CommandPath::LayoutSetSplitRatio,
            CommandPath::PanelLayout,
            CommandPath::PanelNeighbor,
            CommandPath::PanelEdges,
            CommandPath::WorkspaceFocus,
            CommandPath::TabFocus,
            CommandPath::PanelFocus,
            CommandPath::PanelZoom,
            CommandPath::ServerStop,
            CommandPath::AgentStart,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::AgentPrompt,
            CommandPath::AgentWait,
            CommandPath::ProjectCloseAll,
            CommandPath::ProjectCloseResume,
            CommandPath::WorkspaceCreate,
            CommandPath::WorkspaceClose,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSwap,
            CommandPath::TabClose,
            CommandPath::PanelSplit,
            CommandPath::PanelMove,
            CommandPath::TerminalRun,
            CommandPath::TerminalWaitOutput,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
            CommandPath::WorktreeRemove,
            CommandPath::WorktreeResume,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.timeout = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--text",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalSend],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.text = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--enter",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalSend],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.enter = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--key",
        values: super::values::KEY_VALUES,
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalSend],
        role: OptionRole::Business,
        value: Some("key"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.keys.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--file",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::LayoutApply, CommandPath::TerminalSend],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.file = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--stdin",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::LayoutApply, CommandPath::TerminalSend],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.stdin = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--paste",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalSend],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.paste = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--directory",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::WorktreeList],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.directory = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--exec",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::LaunchProfileCreate,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.exec = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--cwd",
        values: super::values::CWD_VALUES,
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::WorktreeList,
            CommandPath::WorktreeCreate,
            CommandPath::WorktreeOpen,
        ],
        role: OptionRole::Business,
        value: Some("cwd"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.cwd = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--env",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginPaneOpen,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.environment.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--arg",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::AgentStart,
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
            CommandPath::LaunchProfileCreate,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.args.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--outdated-only",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::AgentIntegrationList],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.outdated_only = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--include-ended",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::AgentList],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.include_ended = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--limit",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::PluginLogList,
            CommandPath::AgentList,
            CommandPath::AgentChanges,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.limit = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--cursor",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::CommandInvoke,
            CommandPath::TerminalLinkResolve,
            CommandPath::TerminalLinkActivate,
            CommandPath::TerminalCopyMotion,
            CommandPath::TerminalCopySearch,
            CommandPath::AgentList,
            CommandPath::AgentChanges,
            CommandPath::TerminalSelectionRead,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.cursor = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--until",
        values: super::values::UNTIL_VALUES,
        closed_values: true,
        string_rule: None,
        commands: &[
            CommandPath::EventsSubscribe,
            CommandPath::EventsWait,
            CommandPath::AgentPrompt,
            CommandPath::AgentWait,
        ],
        role: OptionRole::Business,
        value: Some("until"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.until.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--after-revision",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::AgentWait],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.after_revision = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--after",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::TerminalWaitOutput],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.output_after = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--provider",
        values: super::values::PROVIDER_VALUES,
        closed_values: true,
        string_rule: None,
        commands: &[
            CommandPath::AgentStart,
            CommandPath::AgentDoctor,
            CommandPath::AgentIntegrationList,
            CommandPath::AgentIntegrationExplain,
            CommandPath::AgentIntegrationEnable,
            CommandPath::AgentIntegrationDisable,
        ],
        role: OptionRole::Business,
        value: Some("provider"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.provider = Some(value.clone());
            options.providers.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--asset-root",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::AgentDoctor],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: true,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.asset_roots.push(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--profile",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceCreate,
            CommandPath::TabCreate,
            CommandPath::PanelCreateTerminal,
            CommandPath::PanelSplit,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.profile = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--name",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::AgentAttach,
            CommandPath::AgentGet,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::AgentRead,
            CommandPath::AgentSendKeys,
            CommandPath::AgentPrompt,
            CommandPath::AgentExplain,
            CommandPath::AgentResolve,
            CommandPath::AgentRename,
            CommandPath::AgentWait,
            CommandPath::LaunchProfileCreate,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.name = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--agent-session",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::AgentAttach,
            CommandPath::AgentGet,
            CommandPath::AgentFocus,
            CommandPath::AgentAttention,
            CommandPath::AgentRead,
            CommandPath::AgentSendKeys,
            CommandPath::AgentPrompt,
            CommandPath::AgentExplain,
            CommandPath::AgentResolve,
            CommandPath::AgentRename,
            CommandPath::AgentWait,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.agent_session = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--clear",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::AgentRename, CommandPath::PanelRename],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.clear = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--wait",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[CommandPath::AgentPrompt, CommandPath::TerminalRun],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.prompt_wait = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--forever",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::AgentPrompt,
            CommandPath::AgentWait,
            CommandPath::TerminalWaitOutput,
        ],
        role: OptionRole::Business,
        value: None,
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, _, _| {
            options.forever = true;
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--adapter-source",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::AgentIntegrationList,
            CommandPath::AgentIntegrationExplain,
            CommandPath::AgentIntegrationEnable,
            CommandPath::AgentIntegrationDisable,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.adapter_source = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--request",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::ProjectCloseStatus,
            CommandPath::ProjectCloseResume,
            CommandPath::WorkspaceCloseResume,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.request = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--generation",
        values: &[],
        closed_values: false,
        string_rule: None,
        commands: &[
            CommandPath::WorkspaceResume,
            CommandPath::TabResume,
            CommandPath::PanelResume,
            CommandPath::ProjectResume,
            CommandPath::TerminalLinkResolve,
            CommandPath::TerminalLinkActivate,
            CommandPath::TerminalCopyMotion,
            CommandPath::TerminalCopySearch,
            CommandPath::ProjectCloseResume,
            CommandPath::WorkspaceCloseResume,
            CommandPath::TerminalSelectionRead,
            CommandPath::WorktreeResume,
        ],
        role: OptionRole::Business,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.generation = Some(value);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--mouse",
        // Default first, matching every usage line that names the choices.
        values: &["on", "off"],
        closed_values: true,
        string_rule: None,
        commands: &[
            CommandPath::SessionAttach,
            CommandPath::TerminalAttach,
            CommandPath::AgentAttach,
            CommandPath::Tui,
        ],
        role: OptionRole::Business,
        value: Some("mouse"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, envelope| {
            options.mouse = Some(mouse_option(&value).ok_or_else(|| {
                super::super::usage_with(
                    super::super::UsageReason::InvalidValue {
                        context: super::super::UsageContext::Root,
                        option: "--mouse",
                        value: value.clone(),
                        expected: super::super::ExpectedValue::MouseMode,
                    },
                    envelope,
                )
            })?);
            Ok(())
        },
    },
    OptionSpec {
        spelling: "--session",
        values: &[],
        closed_values: false,
        string_rule: Some(&super::SESSION_NAME_RULE),
        commands: &[],
        role: OptionRole::Global,
        value: Some("filter"),
        repeatable: false,
        joined: true,
        help: None,
        apply: |options, value, _| {
            options.session = Some(value);
            Ok(())
        },
    },
];

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
