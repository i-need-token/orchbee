//! Localized Panel command details; does not change Panel topology or input.

pub(super) const HELP_PANEL_RENAME_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel rename <panel> (--label <text> | --clear) [--format text|json]",
        "Set a shared custom Panel title by exact ID or @panel:0x<hex>. Empty or\nwhitespace-only text clears the title and restores its creation label. Input\nis limited to 512 Unicode characters before trimming. No focus change.\nrenamed reports a committed revision; unchanged means the override matched.",
    );

pub(super) const HELP_PANEL_RENAME_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel rename <panel> (--label <text> | --clear) [--format text|json]",
        "用精确 ID 或 @panel:0x<hex> 设置共享的窗格自定义标题。空文本或纯空白文本会清除标题，恢复创建时的标签。裁剪前最多 512 个 Unicode 字符。不会切换焦点。\nrenamed 表示已提交新修订；unchanged 表示自定义标题未变。",
    );

pub(super) const HELP_PANEL_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel list [--tab <t> | --workspace <t>] [--client <id>] [--format text|json]",
        "List Panels, optionally inside the Tab selected by --tab or the Workspace\nselected by --workspace (exact ID, selector, or label). Panels carry no label; presentation labels are\nnot panel names.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\nWindows also observes primary terminal title, directory and process facts.\nUnavailable observations are explicit; the query never controls or resizes terminals.",
    );

pub(super) const HELP_PANEL_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel list [--tab <t> | --workspace <t>] [--client <id>] [--format text|json]",
        "列出面板，或仅列 --tab 所选标签页或 --workspace 所选工作区（精确 ID、选择器或 label）内的。面板无\nlabel；Presentation 的 label 不是面板名。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；工作区结果包含记忆的活动标签页。不猜测客户端，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活智能体会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\nWindows 还观察主终端标题、目录和进程事实。不可用观察会明确标记；查询不控制或调整终端大小。",
    );

pub(super) const HELP_PANEL_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel get <panel> [--client <id>] [--format text|json]",
        "Show one Panel by complete typed ID or @panel:0x<hex>: owning tab,\ncontext, slot path, and its primary and child Presentations.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\nWindows also observes primary terminal title, directory and process facts.\nUnavailable observations are explicit; the query never controls or resizes terminals.",
    );

pub(super) const HELP_PANEL_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel get <panel> [--client <id>] [--format text|json]",
        "按完整类型化 ID 或 @panel:0x<hex> 显示一个面板：所属标签页、上下文、slot 路径及其主与子 Presentation。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；工作区结果包含记忆的活动标签页。不猜测客户端，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活智能体会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\nWindows 还观察主终端标题、目录和进程事实。不可用观察会明确标记；查询不控制或调整终端大小。",
    );

pub(super) const HELP_PANEL_SPLIT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel split [<panel> | --current] --direction <left|right|up|down> [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <path|name> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--right-click orchbee|panel] [--ratio <number>] [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
        "Split one Panel, creating a sibling panel on the selected side. The\nfour geometry flags are an all-or-none group: with all four absent the\ncommand requests an 80x24 logical grid with unknown cell pixels (0,0) —\na request policy, not a measured display fact; supplying any of the\nfour requires all four. The result carries the committed revision,\nthe new panel, split, slot, and presentation ids, and the terminal spawn\noutcome. '--exec' selects an absolute executable or a program name on the final CLI PATH and each\n'--arg' appends one verbatim argument; omitting both launches the\ndefault shell. '--profile' instead selects a stored Launch Profile by\nexact ID or singly-matching name, resolved by the Server — it cannot\ncombine with '--exec' or '--arg'.\nWindows --cwd defaults to follow: sample the resolved source Panel root process cwd.\nAn absolute path overrides it; home/current use the CLI home/current directory;\nworkspace keeps Server Workspace/Worker-home defaults. Known absent cwd keeps\nthose defaults; read failures stop before creation. Explicit headless Tab context\ndoes not select a Client just to find cwd. Other platforms keep absolute paths. '--env NAME=VALUE' repeats to overlay inherited variables;\nvalues are literal, including empty values. These options also work with\n'--profile' and the default shell. Names: ASCII letters/digits/underscore,\n1..128 bytes, no leading digit; case-insensitive duplicates, ORCHBEE_*,\nTERM, COLORTERM and TERM_PROGRAM are rejected. Limit: 32 entries, 1,024\nUTF-8 name/value bytes total, no NUL. A bad directory fails the launch.\n\nWindows: omitted targets use validated caller context or one current Client selection. Invalid caller metadata never falls back; --client ignores caller environment. Unnamed Tabs display their current one-based position; --label fixes the name, including numeric names. Split labels default to the preflight Panel count plus one. No focus by default; --focus waits for the original Client to focus the new Panel after creation. Explicit targets without Client effects work headlessly. Total timeout: 30000ms (1..3600000). orchbee.creation.result v1 records directory policy/source/path and preserves creation and subsequent phase facts; later failure never undoes creation. Do not blindly repeat. --ratio is the first child share, clamped from a finite number to 0.1..0.9; it is shared across windows, saved for restart and does not imply focus. --right-click sets only the new Panel policy atomically with creation (default orchbee); panel permits unmodified right clicks inside a mouse-reporting terminal.",
    );

pub(super) const HELP_PANEL_SPLIT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel split [<panel> | --current] --direction <left|right|up|down> [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <path|name> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--right-click orchbee|panel] [--ratio <number>] [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
        "分裂一个面板，在所选侧创建相邻面板。四个几何 flag 为全有或全无组：全部省略时请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请求策略而非实测显示事实；提供其中任何一个就必须提供全部四个。结果携带已提交修订、新的 panel、split、slot、presentation ID 及终端拉起结果。'--exec' 接受绝对路径或在最终 CLI 的 PATH 中解析程序名，每个 '--arg' 逐字追加一个参数；两者均省略时启动默认 shell。'--profile' 改为按精确 ID 或唯一匹配的名称选择已存储的启动配置，由 Server 解析——不能与 '--exec'\n或 '--arg' 组合。\nWindows --cwd 默认为 follow：采样已解析来源面板的根进程目录。绝对路径直接覆盖；\nhome/current 使用 CLI 的主目录／当前目录；workspace 保留 Server 工作区／Worker 主目录默认值。明确缺失 cwd 时保留默认值；读取失败则在创建前停止。显式无界面标签页上下文不会为查目录选择客户端。其他平台仍只使用绝对路径。\n'--env NAME=VALUE' 可重复使用，覆盖继承的变量；值逐字保留，允许空值。这些选项也可配合 '--profile' 或默认 shell 使用。名称只允许 ASCII 字母、数字和下划线，1..128 字节且不能以数字开头；拒绝忽略大小写后重复的名称、\nORCHBEE_*、TERM、COLORTERM 和 TERM_PROGRAM。至多 32 项，名称和值合计不超过 1,024 UTF-8 字节，不能含 NUL。目录不可用时启动失败。\n\nWindows：省略目标时使用经校验的调用者上下文，或唯一 current 客户端的选择。无效调用者信息不回退；显式 --client 忽略调用者环境。未命名标签页显示当前从一开始的位置序号；--label 固定名称，包括数字名称。分屏标签默认为创建前面板数加一。默认不聚焦；--focus 在创建后等待原客户端聚焦新面板。显式目标且无需客户端操作时可无 TUI 运行。总超时默认 30000ms（1..3600000）。orchbee.creation.result v1 记录目录策略／来源／路径，并保留已创建身份及后续步骤结果；后续失败不会撤销创建，勿盲目重试。 --ratio 是第一子节点的份额，有限数值限制到 0.1..0.9；跨窗口共享并保存在重启存档中，不隐含聚焦。 --right-click 随创建原子设置新面板的策略（默认 orchbee）；panel 允许无修饰键的右键在启用鼠标上报的终端内容内透传。",
    );

pub(super) const HELP_PANEL_CLOSE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel close <panel> --confirm [--format text|json]",
        "Close one Panel selected by complete typed ID or @panel:0x<hex>.\n'--confirm' is required.\nAn accepted result carries the accepted revision and the close request\nid; the session finishes the close asynchronously.",
    );

pub(super) const HELP_PANEL_CLOSE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel close <panel> --confirm [--format text|json]",
        "关闭一个由完整类型化 ID 或 @panel:0x<hex> 选择的面板。'--confirm' 为必填。accepted\n结果携带已接受修订与关闭请求 ID；会话异步完成关闭。",
    );

pub(super) const HELP_PANEL_CURRENT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel current [<panel>] [--current|--client <client-id>] [--format text|json]",
        "Read a Panel, Tab and Workspace from a running Windows session.\nAn explicit Panel ID or @panel:0x<hex> ignores caller environment and needs no TUI.\nOtherwise use validated ORCHBEE_TERMINAL_ID and ORCHBEE_WORKER_ID. If both are absent,\nuse the only current TUI Client; multiple Clients require --client. --client ignores\ncaller environment and cannot combine with --panel. Partial, invalid or stale caller\nmetadata never falls back. The Client must be ready or busy with a valid selected Panel.\nIncludes primary Agent status, terminal title/cwd and process observation.\nA selected Client supplies focus and scroll; changes during the read are rejected.\nFive-second total deadline; Ctrl+C cancels. No focus changes or session activation.",
    );

pub(super) const HELP_PANEL_CURRENT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel current [<panel>] [--current|--client <client-id>] [--format text|json]",
        "从运行中的 Windows 会话读取面板、标签页和工作区。显式面板 ID 或 @panel:0x<hex> 忽略调用者环境，无需 TUI。否则校验 ORCHBEE_TERMINAL_ID 与 ORCHBEE_WORKER_ID；两个变量均缺失时，使用唯一 current TUI 客户端，多个客户端必须用 --client 指定。--client 忽略调用者环境，不能与 --panel 同用。部分缺失、无效或过期的调用者信息绝不回退。\nClient 必须处于就绪或忙碌状态，且所选面板仍有效。补齐主智能体状态、终端标题/cwd 和进程观察。选定客户端时包含焦点和滚动信息；读取中途变化则拒绝。总期限五秒；Ctrl+C 取消。不改变焦点，不激活会话。",
    );

pub(super) const HELP_PANEL_PROCESS_INFO_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel process-info [<panel> | --current] [--client <id>] [--format text|json]",
        "Read the primary Terminal's Windows Job process information. Complete Panel IDs\nand @panel:0x<hex> are accepted. Explicit Panel selectors are exclusive and conflict\nwith --client. Otherwise use validated caller context; only when both caller variables\nare absent, select the only current TUI Client. --client selects a Client and ignores\ncaller environment. Partial, invalid or stale caller metadata never falls back.\nThe Client must be ready or busy with a valid selected Panel. Each sample reports owned\nprocesses with PID, creation time, image, command line and cwd; unavailable optional\nfields are null. Five-second total deadline; Ctrl+C cancels. No focus or input changes.",
    );

pub(super) const HELP_PANEL_PROCESS_INFO_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel process-info [<panel> | --current] [--client <id>] [--format text|json]",
        "读取主 Terminal 的 Windows Job 进程信息，接受完整面板 ID 或 @panel:0x<hex>。显式面板选择器互斥，也不能与 --client 同用。否则使用经校验的调用者上下文；仅当两个调用者变量均缺失时，选择唯一 current TUI 客户端。--client 指定客户端\n并忽略调用者环境。部分缺失、无效或过期的调用者信息绝不回退。客户端必须已就绪且所选面板仍有效。样本列出归属进程的 PID、创建时间、映像、命令行和 cwd；不可用的可选字段为 null。总期限五秒；Ctrl+C 取消。不改变焦点或输入。",
    );

pub(super) const HELP_PANEL_SWAP_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel swap [<panel>] (--with <target> | --direction left|right|up|down) [--current] [--client <id>] [--timeout <ms>] [--format text|json]",
        "Explicit source and --with without --client perform a shared, headless exchange with no focus phase (orchbee.panel.swap v1). Other modes require one Ready Client and preserve the existing focus workflow. Exchange two complete Panel positions in one Tab, then focus the source in the selected Windows TUI. Source selectors are exclusive; omitted source uses valid caller context or the selected Client. Targets are exact IDs or @panel:0x<hex>. Multiple Clients require an explicit selection.\n\nSame Panel, cross-Tab pair and no neighbor leave layout and focus unchanged. Positions commit before focus acknowledgment: a later failure preserves the committed receipt; never automatically retry a swap. One total timeout defaults to 30000 ms (1..3600000). No --detach or --request-id.",
    );

pub(super) const HELP_PANEL_SWAP_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel swap [<panel>] (--with <target> | --direction left|right|up|down) [--current] [--client <id>] [--timeout <ms>] [--format text|json]",
        "显式来源及 --with 且省略 --client 时，无头交换共享位置，不执行聚焦阶段（orchbee.panel.swap v1）。其他模式需要 Ready 客户端，并保留既有聚焦流程。 交换同一标签页中两个完整面板的位置，再在所选 Windows TUI 聚焦来源。来源选择器互斥；省略时使用有效调用者上下文或所选客户端。目标为精确 ID 或 @panel:0x<hex>。多个客户端时须明确选择。\n\n同一面板、跨标签页或没有邻居不改变布局和焦点。位置提交先于聚焦确认；后续失败保留提交回执，绝不自动重试交换。总超时默认 30000 ms（1..3600000）。不支持 --detach 或 --request-id。",
    );

pub(super) const HELP_PANEL_INPUT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel input (<panel> | --current) --right-click orchbee|panel [--format text|json]",
        "Windows: set the shared Panel right-button policy without changing focus.\n--current uses validated calling Terminal/Worker context; explicit targets\nbypass that context. orchbee reserves right-click to the Client; panel permits\nunmodified right clicks inside a reporting terminal. Existing controller and\nreadiness gates still apply. A policy change applies to the next gesture.\nconfigured or unchanged acknowledges canonical state, not every Client draw.\nAn unknown outcome is never automatically retried.",
    );

pub(super) const HELP_PANEL_INPUT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel input (<panel> | --current) --right-click orchbee|panel [--format text|json]",
        "Windows：设置共享面板右键策略，不改变焦点。--current 校验调用者\nTerminal/Worker 上下文；显式目标绕过该上下文。orchbee 将右键保留给\nClient；panel 允许无修饰键的右键在启用鼠标上报的终端内容中透传。现有控制权及就绪约束仍适用，策略改变作用于下一个手势。\nconfigured 或 unchanged 确认规范状态，不确认所有客户端已绘制。未知结果不会自动重试。",
    );

pub(super) const HELP_PANEL_MOVE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel move <panel> --into-tab <tab> --direction left|right|up|down [--target-panel <panel>] [--ratio <n>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-tab [--workspace <workspace>] [--label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
        "Move a panel while keeping its terminal and process running. An existing destination tab can belong to any workspace. Its target defaults to the selected client's focused panel.\n\nFocus is on by default. With no client, use --no-focus and omit --ratio; the first target leaf and default ratio are used. Ratios are finite and clamped to 0.1..0.9. Timeout defaults to 30000 ms. Zoomed tabs leave the layout unchanged. Selecting the current tab reorders its panels through the same --into-tab form.\n\nA new workspace follows the terminal's working directory, falling back to the CLI working directory when unavailable. Moving the last panel removes the empty source tab and, across contexts, its empty workspace.\n\nDirectory, ratio and focus results are recorded separately from the move. Partial failure preserves committed changes; an uncertain submission is never replayed.",
    );

pub(super) const HELP_PANEL_MOVE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel move <panel> --into-tab <tab> --direction left|right|up|down [--target-panel <panel>] [--ratio <n>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-tab [--workspace <workspace>] [--label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
        "移动面板并保留其终端和进程。已有目标标签页可以属于任何工作区；目标面板默认取选中客户端的焦点面板。\n\n默认开启聚焦。没有客户端时使用 --no-focus 并省略 --ratio，采用首个目标叶子和默认比例。比例必须是有限值，并限制到 0.1..0.9。默认时限 30000 毫秒。已放大时布局保持不变。使用相同的 --into-tab 形式选择当前标签页，可调整其面板排列。\n\n新工作区采用终端工作目录，不可用时回退到 CLI 工作目录。移走最后面板会移除空来源标签页；跨上下文时也移除空来源工作区。\n\n目录、比例和聚焦结果与迁移分别记录。部分失败保留已提交变更，结果不确定时不重放。",
    );
