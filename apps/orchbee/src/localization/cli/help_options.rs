//! Option value labels and purposes. Membership comes from the completion grammar;
//! defaults and interactions that differ by command belong to its detailed copy.
use super::super::Locale;
use crate::invocation::CommandPath;

impl Locale {
    pub(crate) fn cli_help_option(
        self,
        path: CommandPath,
        flag: &str,
    ) -> (&'static str, &'static str) {
        if path == CommandPath::AgentStart && flag == "--provider" {
            return ("<provider>", self.cli_agent_start_provider_choices());
        }
        let (value, english, chinese) = match (path, flag) {
            (CommandPath::TerminalRun, "--wait") => (
                "",
                "Wait for this command's correlated managed-shell completion.",
                "等待受管 Shell 回传本次命令的关联完成结果。",
            ),
            (CommandPath::TerminalRun, "--timeout") => (
                "<ms>",
                "Total wait deadline: 1..3600000 ms; default: 30000; requires --wait.",
                "总等待时限：1..3600000 毫秒；默认 30000；需要 --wait。",
            ),
            (CommandPath::TerminalWaitOutput, "--after") => (
                "<cursor|now>",
                "Match new complete rows after this cursor; now captures the first observation.",
                "仅匹配游标之后的新完整行；now 以第一次观测为起点。",
            ),
            (
                CommandPath::PanelLayout | CommandPath::PanelNeighbor | CommandPath::PanelEdges,
                "--timeout",
            ) => (
                "<ms>",
                "Deadline: 1..3600000 ms; default: 30000.",
                "时限：1..3600000 毫秒；默认 30000。",
            ),
            (CommandPath::WorktreeResume, "--timeout") => (
                "<ms>",
                "Deadline: 1..3600000 ms; default: 120000.",
                "时限：1..3600000 毫秒；默认 120000。",
            ),
            #[cfg(windows)]
            (
                CommandPath::CommandInvoke
                | CommandPath::PluginPaneFocus
                | CommandPath::PluginPaneClose
                | CommandPath::PopupClose,
                "--timeout",
            ) => (
                "<ms>",
                "Deadline: 1..3600000 ms; default: 30000.",
                "时限：1..3600000 毫秒；默认 30000。",
            ),
            #[cfg(windows)]
            (CommandPath::EventsSubscribe, "--timeout") => (
                "<ms>",
                "Overall deadline: 0..3600000 ms; omitted by default.",
                "总时限：0..3600000 毫秒；默认不设置。",
            ),
            #[cfg(windows)]
            (CommandPath::PanelGraphicsSet, "--spec") => (
                "<json|@file|->",
                "Graphics input: JSON, a file, or noninteractive stdin (-).",
                "图像输入：JSON、文件或非交互标准输入（-）。",
            ),
            #[cfg(windows)]
            (CommandPath::EventsWait | CommandPath::EventsSubscribe, "--source") => (
                "<source>",
                "Output scope: recent, recent-unwrapped, visible or detection.",
                "输出范围：recent、recent-unwrapped、visible 或 detection。",
            ),
            (CommandPath::TerminalAttach | CommandPath::AgentAttach, "--format") => (
                "<text|json>",
                "Accepted with either value; successful output remains interactive.",
                "两种取值均接受；成功输出始终为交互屏幕。",
            ),
            (CommandPath::TerminalObserve | CommandPath::TerminalControl, "--format") => (
                "<text|json>",
                "Accepted with either value; successful output remains NDJSON.",
                "两种取值均接受；成功输出始终为 NDJSON。",
            ),
            (CommandPath::ApiSchema | CommandPath::ApiSnapshot, "--format") => (
                "<text|json>",
                "Success always emits JSON; select json for JSON errors too.",
                "成功时始终输出 JSON；选择 json 也以 JSON 输出错误。",
            ),
            (
                CommandPath::WorkspaceCreate
                | CommandPath::PanelSplit
                | CommandPath::TabCreate
                | CommandPath::WorktreeCreate
                | CommandPath::WorktreeOpen,
                "--rows",
            ) => (
                "<n>",
                "Terminal height in rows; give it with --cols, --cell-width and --cell-height.",
                "终端行数；须与 --cols、--cell-width 和 --cell-height 一起提供。",
            ),
            (CommandPath::TerminalRead | CommandPath::AgentRead, "--rows") => (
                "<n>",
                "Number of history rows to read with --history.",
                "配合 --history 读取的历史行数。",
            ),
            (CommandPath::TerminalWaitOutput, "--rows") => (
                "<n>",
                "Number of recent rows to search.",
                "要搜索的最近行数。",
            ),
            (CommandPath::PanelSplit, "--direction") => (
                "<direction>",
                "Side of the selected panel where the new panel opens.",
                "新面板在所选面板哪一侧打开。",
            ),
            (CommandPath::PanelMove, "--direction") => (
                "<direction>",
                "Side of the destination panel to place the panel on.",
                "将面板放到目标面板的哪一侧。",
            ),
            (CommandPath::TabMove, "--order") => (
                "<direction>",
                "Where to move the tab among its neighbors.",
                "将标签页移动到相邻标签页中的哪个位置。",
            ),
            (CommandPath::PanelFocus | CommandPath::PanelNeighbor, "--direction") => (
                "<direction>",
                "Direction of the neighboring panel.",
                "相邻面板所在的方向。",
            ),
            (CommandPath::PanelResize, "--direction") => {
                ("<direction>", "Divider edge to move.", "要移动的分隔边。")
            }
            (CommandPath::PanelSwap, "--direction") => (
                "<direction>",
                "Direction of the neighboring panel to swap with.",
                "要交换的相邻面板所在方向。",
            ),
            (CommandPath::AgentList | CommandPath::AgentChanges, "--cursor") => (
                "<cursor>",
                "Continue from the cursor a previous page returned.",
                "从上一页返回的游标继续。",
            ),
            (
                CommandPath::TabList
                | CommandPath::TabCreate
                | CommandPath::PanelList
                | CommandPath::TerminalList
                | CommandPath::PanelMove
                | CommandPath::WorkspaceCloseStatus
                | CommandPath::WorkspaceCloseResume,
                "--workspace",
            ) => ("<target>", "Select a workspace.", "选择工作区。"),
            (CommandPath::LaunchProfileCreate, "--name") => (
                "<name>",
                "Name of the saved launch profile.",
                "保存的启动配置名称。",
            ),
            (
                CommandPath::TerminalRead
                | CommandPath::AgentRead
                | CommandPath::TerminalWaitOutput,
                "--source",
            ) => (
                "<source>",
                "Output scope: recent, recent-unwrapped, visible or detection.",
                "输出范围：recent、recent-unwrapped、visible 或 detection。",
            ),
            (CommandPath::AgentIntegrationList, "--outdated-only") => (
                "",
                "Show only integrations that need updating.",
                "仅显示需要更新的集成。",
            ),
            (CommandPath::PanelInput | CommandPath::PanelSplit, "--right-click") => (
                "<orchbee|panel>",
                "Handle right-clicks in Orchbee or pass them to the panel.",
                "由 Orchbee 处理右键，或将右键传给面板。",
            ),
            (CommandPath::LayoutSetSplitRatio, "--path") => (
                "<root|0.1>",
                "Split-tree path; root selects the top-level split.",
                "分屏树路径；root 选择最上层分屏。",
            ),
            #[cfg(windows)]
            (
                CommandPath::TerminalCopyMotion
                | CommandPath::TerminalCopySearch
                | CommandPath::TerminalLinkResolve
                | CommandPath::TerminalLinkActivate
                | CommandPath::TerminalSelectionRead
                | CommandPath::CommandInvoke,
                "--cursor",
            ) => (
                "<ROW:COL>",
                "Terminal row and column position.",
                "终端行列位置。",
            ),
            _ => match flag {
                "--adapter-source" => (
                    "<source>",
                    "Adapter that owns this update.",
                    "拥有本次更新的适配器。",
                ),
                "--after-cursor" => (
                    "<json>",
                    "Continue after a previously returned event cursor.",
                    "从此前返回的事件游标之后继续。",
                ),
                "--after-revision" => (
                    "<revision>",
                    "Wait for a revision newer than this value.",
                    "等待比此值更新的修订。",
                ),
                "--agent" => (
                    "<name>",
                    "Agent associated with this terminal.",
                    "此终端关联的 Agent。",
                ),
                "--agent-session-id" => (
                    "<id>",
                    "Agent provider's session identifier.",
                    "Agent 提供方的会话标识。",
                ),
                "--agent-session-path" => (
                    "<path>",
                    "Agent provider's saved session path.",
                    "Agent 提供方保存的会话路径。",
                ),
                "--amount" => (
                    "<step>",
                    "Split ratio step; default: 0.05, absolute value capped at 0.5.",
                    "分隔比例的调整步长；默认 0.05，绝对值最大为 0.5。",
                ),
                "--anchor" => (
                    "<ROW:COL>",
                    "Start of the terminal selection.",
                    "终端选区起点。",
                ),
                "--ansi" => (
                    "",
                    "Preserve terminal colors and styles in the output.",
                    "在输出中保留终端颜色与样式。",
                ),
                "--applies-to-source" => (
                    "<source>",
                    "Apply metadata only to this source.",
                    "仅将元数据应用于此来源。",
                ),
                "--arg" => (
                    "<value>",
                    "Program argument; repeat for additional arguments.",
                    "程序参数；可重复传入多个参数。",
                ),
                "--asset-root" => (
                    "<path>",
                    "Integration asset directory to inspect; repeatable.",
                    "要检查的集成资源目录；可重复。",
                ),
                "--base" => (
                    "<ref>",
                    "Git starting reference; default: HEAD.",
                    "Git 起始引用；默认 HEAD。",
                ),
                "--before" => (
                    "<target>",
                    "Insert before the selected item.",
                    "插入到指定项之前。",
                ),
                "--body" => ("<text>", "Notification body text.", "通知正文。"),
                "--branch" => ("<name>", "Git branch name.", "Git 分支名称。"),
                "--cell-height" => (
                    "<pixels>",
                    "Pixel height of a terminal cell.",
                    "终端单元格的像素高度。",
                ),
                "--cell-width" => (
                    "<pixels>",
                    "Pixel width of a terminal cell.",
                    "终端单元格的像素宽度。",
                ),
                "--child" => (
                    "",
                    "Create a child terminal in the panel.",
                    "在面板中创建子终端。",
                ),
                "--clear" => (
                    "",
                    "Clear the saved name instead of assigning one.",
                    "清除已保存的名称。",
                ),
                "--clear-display-agent" => (
                    "",
                    "Clear the displayed Agent override.",
                    "清除显示的 Agent 覆盖值。",
                ),
                "--clear-state-labels" => {
                    ("", "Clear the custom state labels.", "清除自定义状态标签。")
                }
                "--clear-title" => ("", "Clear the title override.", "清除标题覆盖值。"),
                "--clear-token" => (
                    "<name>",
                    "Clear the named metadata token.",
                    "清除指定名称的元数据标记。",
                ),
                "--client" => (
                    "<id>",
                    "Select a connected terminal client explicitly.",
                    "显式选择已连接的终端客户端。",
                ),
                "--cols" => ("<n>", "Terminal width in columns.", "终端列数。"),
                "--confirm" => (
                    "",
                    "Confirm closing or removing the selected resources.",
                    "确认关闭或移除所选资源。",
                ),
                "--content-revision" => (
                    "<revision>",
                    "Require the expected terminal content revision.",
                    "要求终端内容修订与预期一致。",
                ),
                "--context" => (
                    "<json>",
                    "Supply action context as JSON.",
                    "以 JSON 提供操作上下文。",
                ),
                "--current" => (
                    "",
                    "Use the caller's current panel.",
                    "使用调用者的当前面板。",
                ),
                "--cursor" => (
                    "<cursor>",
                    "Continue a page or select a terminal position.",
                    "继续分页或指定终端位置。",
                ),
                "--cwd" => (
                    "<path>",
                    "Working directory or supported directory policy.",
                    "工作目录或受支持的目录策略。",
                ),
                "--detach" => (
                    "",
                    "Return the request ID without waiting for completion.",
                    "返回请求 ID，不等待操作完成。",
                ),
                "--order" => (
                    "<previous|next|first|last>",
                    "Order within the Tab list.",
                    "标签页列表内的顺序。",
                ),
                "--search-direction" => (
                    "<forward|backward>",
                    "Direction of text search.",
                    "文本搜索方向。",
                ),
                "--terminal" => (
                    "<terminal>",
                    "Select an exact primary or child terminal.",
                    "选择精确的主终端或子终端。",
                ),
                "--direction" => (
                    "<direction>",
                    "Direction for navigation, movement or splitting.",
                    "导航、移动或分屏的方向。",
                ),
                "--directory" => (
                    "<id>",
                    "Select a recorded directory by its ID.",
                    "按 ID 选择已记录的目录。",
                ),
                "--disabled" => (
                    "",
                    "Register the plugin as disabled.",
                    "注册插件并将其设为禁用。",
                ),
                "--display-agent" => ("<text>", "Agent name to display.", "要显示的 Agent 名称。"),
                "--dry-run" => (
                    "",
                    "Preview the change without writing it.",
                    "预览变更，不写入结果。",
                ),
                "--enabled" => (
                    "",
                    "Register the plugin as enabled.",
                    "注册插件并将其设为启用。",
                ),
                "--enter" => (
                    "",
                    "Send Enter after the supplied text.",
                    "在所提供文本之后发送回车。",
                ),
                "--entrypoint" => ("<id>", "Plugin entrypoint to open.", "要打开的插件入口。"),
                "--env" => (
                    "<NAME=VALUE>",
                    "Environment entry; repeat for additional entries.",
                    "环境变量；可重复设置多个条目。",
                ),
                "--event" => ("<name>", "Event kind to observe.", "要观察的事件类型。"),
                "--exec" => (
                    "<absolute-path>",
                    "Executable to launch instead of the default shell.",
                    "要启动的程序绝对路径；替代默认 Shell。",
                ),
                "--file" => (
                    "<path>",
                    "Read input from a local file.",
                    "从本地文件读取输入。",
                ),
                "--filter" => (
                    "<json>",
                    "Typed event filter; do not mix with shorthand filters.",
                    "结构化事件筛选；不可与简写筛选混用。",
                ),
                "--focus" => (
                    "",
                    "Switch an existing client to the created resource.",
                    "让已有客户端切换到新创建的资源。",
                ),
                "--force" => (
                    "",
                    "Allow the documented forced removal behavior.",
                    "允许说明中列明的强制移除行为。",
                ),
                "--forever" => (
                    "",
                    "Wait without an overall deadline.",
                    "不设置整体等待期限。",
                ),
                "--generation" => (
                    "<n>",
                    "Require the observed operation or terminal generation.",
                    "要求操作或终端代次与已观测值一致。",
                ),
                "--height" => (
                    "<cells|percent%>",
                    "Popup height; applies only to popup placement.",
                    "弹出窗口高度；仅适用于 popup 布局。",
                ),
                "--history" => (
                    "",
                    "Read the legacy history-only output form.",
                    "读取兼容的纯历史输出格式。",
                ),
                "--include-ended" => (
                    "",
                    "Include ended Agent sessions.",
                    "包含已结束的 Agent 会话。",
                ),
                "--insert-index" => (
                    "<n>",
                    "Zero-based insertion position.",
                    "从零开始的插入位置。",
                ),
                "--into-tab" => (
                    "<target>",
                    "Destination tab for the moved panel.",
                    "面板移入的目标标签页。",
                ),
                "--json" => (
                    "",
                    "Use JSON output; alias for --format json.",
                    "使用 JSON 输出；等同于 --format json。",
                ),
                "--key" => (
                    "<key>",
                    "Semantic key to send; repeatable.",
                    "要发送的语义按键；可重复。",
                ),
                "--label" => (
                    "<text>",
                    "Display name. If it resembles an ID or @kind:selector, address the resource by its real ID.",
                    "显示名称。如果名称形似 ID 或 @kind:选择器，请用资源的实际 ID 作为目标。",
                ),
                "--limit" => (
                    "<n>",
                    "Maximum number of entries in this result.",
                    "本次结果的最大条目数。",
                ),
                "--lines" => (
                    "<n>",
                    "Maximum number of output lines to inspect.",
                    "要读取的最大输出行数。",
                ),
                "--match" => ("<text>", "Literal text to match.", "要匹配的文本。"),
                "--message" => (
                    "<text>",
                    "Human-readable status message.",
                    "可读的状态说明。",
                ),
                "--motion" => (
                    "<motion>",
                    "Selection movement to apply.",
                    "要执行的选区移动。",
                ),
                "--mouse" => (
                    "<on|off>",
                    "Mouse capture in the terminal interface; default: on.",
                    "终端界面鼠标捕获；默认 on。",
                ),
                "--name" => (
                    "<name>",
                    "Select an Agent by its saved session name.",
                    "按保存的会话名称选择 Agent。",
                ),
                "--new-tab" => ("", "Move into a newly created tab.", "移入新建的标签页。"),
                "--new-workspace" => (
                    "",
                    "Move into a newly created workspace.",
                    "移入新建的工作区。",
                ),
                "--no-focus" => (
                    "",
                    "Keep the client's current selection.",
                    "保持客户端当前选中项。",
                ),
                "--off" => ("", "Turn panel zoom off.", "关闭面板放大。"),
                "--offset-from-bottom" => (
                    "<n>",
                    "Row offset from the bottom of terminal output.",
                    "相对终端输出底部的行偏移。",
                ),
                "--on" => ("", "Turn panel zoom on.", "开启面板放大。"),
                "--outdated-only" => (
                    "",
                    "Select only clients using an older version.",
                    "仅选择使用旧版本的客户端。",
                ),
                "--output" => (
                    "<path>",
                    "Also write the prepared document to this file.",
                    "同时将生成的文档写入此文件。",
                ),
                "--panel" => (
                    "<target>",
                    "Select a panel by its accepted ID or selector.",
                    "通过支持的 ID 或选择器指定面板。",
                ),
                "--paste" => (
                    "",
                    "Send text as a bracketed paste.",
                    "以括号粘贴方式发送文本。",
                ),
                "--path" => (
                    "<path>",
                    "Filesystem path used by this command.",
                    "此命令使用的文件系统路径。",
                ),
                "--placement" => (
                    "<placement>",
                    "Plugin pane placement: tab, split, zoomed, overlay or popup.",
                    "插件面板布局：tab、split、zoomed、overlay 或 popup。",
                ),
                "--plain" => (
                    "",
                    "Export plain UTF-8 text without terminal styling.",
                    "导出不含终端样式的 UTF-8 纯文本。",
                ),
                "--plugin" => (
                    "<id>",
                    "Select the plugin that owns the action or pane.",
                    "选择拥有该操作或面板的插件。",
                ),
                "--position" => ("<position>", "Notification placement.", "通知显示位置。"),
                "--presentation" => (
                    "<id>",
                    "Select a specific displayed terminal surface.",
                    "选择指定的终端显示画面。",
                ),
                "--previous" => (
                    "<ROW:COL-ROW:COL>",
                    "Previous match to continue a search from.",
                    "用于继续搜索的上一次匹配。",
                ),
                "--profile" => (
                    "<selector>",
                    "Use a saved launch profile.",
                    "使用已保存的启动配置。",
                ),
                "--project" => (
                    "<target>",
                    "Select an existing project grouping.",
                    "选择已有的项目分组。",
                ),
                "--provider" => (
                    "<id>",
                    "Limit the operation to this Agent provider.",
                    "将操作限定到此 Agent 提供方。",
                ),
                "--query" => ("<text>", "Text to search for.", "要搜索的文本。"),
                "--ratio" => ("<n>", "Desired split ratio.", "目标分屏比例。"),
                "--ref" => (
                    "<ref>",
                    "Git reference to download for plugin installation.",
                    "安装插件时要下载的 Git 引用。",
                ),
                "--regex" => (
                    "<pattern>",
                    "Regular expression to match instead of literal text.",
                    "用正则表达式代替文本匹配。",
                ),
                "--remote-session" => (
                    "<name>",
                    "Remote session name; default: default.",
                    "远端会话名称；默认 default。",
                ),
                "--request" => (
                    "<id>",
                    "Retained operation ID to inspect or resume.",
                    "要查看或继续的操作 ID。",
                ),
                "--request-id" => (
                    "<id>",
                    "Explicit identity for this new request.",
                    "此次新请求的显式标识。",
                ),
                "--retry" => (
                    "",
                    "Explicitly retry an eligible retained attempt.",
                    "显式重试符合条件的保留尝试。",
                ),
                "--right-click" => (
                    "<action>",
                    "Action when the notification is right-clicked.",
                    "右键点击通知时执行的操作。",
                ),
                "--rows" => (
                    "<n>",
                    "Terminal height or requested history row count.",
                    "终端行数或请求的历史行数。",
                ),
                "--seq" => (
                    "<n>",
                    "Monotonic sequence number for this source.",
                    "此来源单调递增的序号。",
                ),
                "--agent-session" => (
                    "<id>",
                    "Select an Agent session explicitly.",
                    "显式选择 Agent 会话。",
                ),
                "--session" => (
                    "<name>",
                    "Select the local Orchbee session; default: default.",
                    "选择本地 Orchbee 会话；默认 default。",
                ),
                "--session-start-source" => (
                    "<source>",
                    "Record how the Agent session was started.",
                    "记录 Agent 会话的启动来源。",
                ),
                "--sound" => ("<sound>", "Notification sound.", "通知声音。"),
                "--source" => (
                    "<source>",
                    "Select the input or ownership source.",
                    "选择输入来源或所属来源。",
                ),
                "--spec" => (
                    "<json>",
                    "Command specification; see the command's accepted input form.",
                    "命令定义；输入形式见本命令说明。",
                ),
                "--standalone" => (
                    "",
                    "Use the standalone tabs outside any workspace.",
                    "使用不属于任何工作区的独立标签页。",
                ),
                "--state" => ("<state>", "Agent state to report.", "要报告的 Agent 状态。"),
                "--state-label" => (
                    "<state=text>",
                    "Custom label for one Agent state; repeatable.",
                    "某个 Agent 状态的自定义标签；可重复。",
                ),
                "--stdin" => (
                    "",
                    "Read input from standard input.",
                    "从标准输入读取内容。",
                ),
                "--tab" => ("<target>", "Select a tab.", "选择标签页。"),
                "--tab-label" => (
                    "<text>",
                    "Name for a newly created destination tab.",
                    "新建目标标签页的名称。",
                ),
                "--takeover" => (
                    "",
                    "Explicitly take terminal control from its current owner.",
                    "显式接管当前拥有者的终端控制权。",
                ),
                "--target-pane" => (
                    "<id>",
                    "Existing pane used as the plugin placement target.",
                    "作为插件布局目标的已有面板。",
                ),
                "--target-panel" => (
                    "<target>",
                    "Existing panel at the destination.",
                    "目标位置的已有面板。",
                ),
                "--text" => ("<text>", "Text to submit.", "要提交的文本。"),
                "--timeout" => (
                    "<ms>",
                    "Deadline in milliseconds; command defaults and bounds appear below.",
                    "毫秒时限；默认值和范围见下方说明。",
                ),
                "--title" => ("<text>", "Title to display.", "要显示的标题。"),
                "--toggle" => ("", "Toggle panel zoom.", "切换面板放大状态。"),
                "--token" => (
                    "<name=value>",
                    "Report a named metadata token.",
                    "上报命名元数据标记。",
                ),
                "--trust-repository" => (
                    "",
                    "Trust this repository for this operation only.",
                    "仅为本次操作信任此仓库。",
                ),
                "--ttl-ms" => (
                    "<ms>",
                    "Lifetime of the reported metadata in milliseconds.",
                    "上报元数据的有效期，单位为毫秒。",
                ),
                "--until" => (
                    "<condition>",
                    "Condition that ends the wait.",
                    "结束等待的条件。",
                ),
                "--unwrap" => ("", "Join soft-wrapped terminal lines.", "合并终端软换行。"),
                "--wait" => (
                    "",
                    "Wait for activity and the requested final state.",
                    "等待活动及请求的最终状态。",
                ),
                "--width" => (
                    "<cells|percent%>",
                    "Popup width; applies only to popup placement.",
                    "弹出窗口宽度；仅适用于 popup 布局。",
                ),
                "--with" => (
                    "<target>",
                    "Other panel to swap with.",
                    "要交换的另一面板。",
                ),
                "--workspace" => (
                    "<target>",
                    "Select a workspace or directory source.",
                    "选择工作区或目录来源。",
                ),
                "--yes" => (
                    "",
                    "Allow downloaded plugin code to run its build steps.",
                    "允许下载的插件代码执行构建步骤。",
                ),
                "--format" => (
                    "<text|json>",
                    "Output format; default: text.",
                    "输出格式；默认 text。",
                ),
                "--help" => ("", "Show this help and exit.", "显示本帮助后退出。"),
                _ => unreachable!("admitted help option lacks localized copy: {flag}"),
            },
        };
        (
            value,
            match self {
                Self::EnUs => english,
                Self::ZhCn => chinese,
            },
        )
    }
}
