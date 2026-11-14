//! Localized Tab command details; does not change Tabs.

pub(super) const HELP_TAB_LIST_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tab list [--workspace <t>] [--client <id>] [--format text|json]",
    "List Tabs. Filters select one Workspace's context (exact ID,\n@workspace:0x<hex>, or label). Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
);

pub(super) const HELP_TAB_LIST_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tab list [--workspace <t>] [--client <id>] [--format text|json]",
    "列出标签页。过滤参数可选择一个工作区的上下文（精确 ID、@workspace:0x<hex> 或 label）。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；工作区结果包含记忆的活动标签页。不猜测客户端，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活智能体会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
);

pub(super) const HELP_TAB_GET_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tab get <target> [--client <id>] [--format text|json]",
    "Show one Tab including its complete PanelNode root subtree.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
);

pub(super) const HELP_TAB_GET_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tab get <target> [--client <id>] [--format text|json]",
    "显示一个标签页，包括其完整 PanelNode 根子树。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；工作区结果包含记忆的活动标签页。不猜测客户端，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活智能体会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
);

pub(super) const HELP_TAB_CREATE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab create [--workspace <target>] [--label <label>] [--profile <s> | --exec <path|name> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
        "Create one Tab in the selected terminal context: a Workspace chosen by\nexact ID, @workspace:0x<hex>, or exact label. The result carries the committed revision, the new\ntab, panel, and presentation ids, and the terminal spawn outcome.\n'--exec' selects an absolute executable or a program name on the final CLI PATH and each '--arg'\nappends one verbatim argument; omitting both launches the default shell.\n'--profile' instead selects a stored Launch Profile by exact ID or\nsingly-matching name, resolved by the Server — it cannot combine with\n'--exec' or '--arg'.\nWindows --cwd defaults to follow: sample the resolved source Panel root process cwd.\nAn absolute path overrides it; home/current use the CLI home/current directory;\nworkspace keeps Server Workspace/Worker-home defaults. Known absent cwd keeps\nthose defaults; read failures stop before creation. Explicit headless Tab context\ndoes not select a Client just to find cwd. Other platforms keep absolute paths. '--env NAME=VALUE' repeats to overlay inherited variables;\nvalues are literal, including empty values. These options also work with\n'--profile' and the default shell. Names: ASCII letters/digits/underscore,\n1..128 bytes, no leading digit; case-insensitive duplicates, ORCHBEE_*,\nTERM, COLORTERM and TERM_PROGRAM are rejected. Limit: 32 entries, 1,024\nUTF-8 name/value bytes total, no NUL. A bad directory fails the launch.\n\nWindows: omitted targets use validated caller context or one current Client selection. Invalid caller metadata never falls back; --client ignores caller environment. Unnamed Tabs display their current one-based position; --label fixes the name, including numeric names. Split labels default to the preflight Panel count plus one. No focus by default; --focus waits for the original Client to focus the new Panel after creation. Explicit targets without Client effects work headlessly. Total timeout: 30000ms (1..3600000). orchbee.creation.result v1 records directory policy/source/path and preserves creation and subsequent phase facts; later failure never undoes creation. Do not blindly repeat.",
    );

pub(super) const HELP_TAB_CREATE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab create [--workspace <target>] [--label <label>] [--profile <s> | --exec <path|name> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
        "在所选终端上下文内创建一个标签页：由精确 ID、@workspace:0x<hex> 或精确 label 选择的\nWorkspace。结果携带已提交修订、新的 tab、panel、presentation ID 及终端拉起结果。\n'--exec' 接受绝对路径或在最终 CLI 的 PATH 中解析程序名，每个 '--arg' 逐字追加一个参数；两者均省略时启动默认 shell。\n'--profile' 改为按精确 ID 或唯一匹配的名称选择已存储的启动配置，由 Server 解析——不能与 '--exec' 或 '--arg' 组合。\nWindows --cwd 默认为 follow：采样已解析来源面板的根进程目录。绝对路径直接覆盖；\nhome/current 使用 CLI 的主目录／当前目录；workspace 保留 Server 工作区／Worker 主目录默认值。明确缺失 cwd 时保留默认值；读取失败则在创建前停止。显式无界面标签页上下文不会为查目录选择客户端。其他平台仍只使用绝对路径。\n'--env NAME=VALUE' 可重复使用，覆盖继承的变量；值逐字保留，允许空值。这些选项也可配合 '--profile' 或默认 shell 使用。名称只允许 ASCII 字母、数字和下划线，1..128 字节且不能以数字开头；拒绝忽略大小写后重复的名称、\nORCHBEE_*、TERM、COLORTERM 和 TERM_PROGRAM。至多 32 项，名称和值合计不超过 1,024 UTF-8 字节，不能含 NUL。目录不可用时启动失败。\n\nWindows：省略目标时使用经校验的调用者上下文，或唯一 current 客户端的选择。无效调用者信息不回退；显式 --client 忽略调用者环境。未命名标签页显示当前从一开始的位置序号；--label 固定名称，包括数字名称。分屏标签默认为创建前面板数加一。默认不聚焦；--focus 在创建后等待原客户端聚焦新面板。显式目标且无需客户端操作时可无 TUI 运行。总超时默认 30000ms（1..3600000）。orchbee.creation.result v1 记录目录策略／来源／路径，并保留已创建身份及后续步骤结果；后续失败不会撤销创建，勿盲目重试。",
    );

pub(super) const HELP_TAB_RENAME_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab rename <target> --label <label> [--format text|json]",
        "Rename one Tab selected by exact ID, @tab:0x<hex>, or exact label.\nA committed result\ncarries the new revision; unchanged means an already fixed name matched. Renaming an automatic Tab fixes its name even when the visible text is equal.",
    );

pub(super) const HELP_TAB_RENAME_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab rename <target> --label <label> [--format text|json]",
        "重命名一个由精确 ID、@tab:0x<hex> 或精确 label 选择的标签页。committed 结果携带新修订；unchanged 表示固定名称本已相同。自动命名标签页即使文本相同，重命名也会将其改为固定名称。",
    );

pub(super) const HELP_TAB_MOVE_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tab move <target> (--order <previous|next|first|last> | --insert-index <N>) [--format text|json]",
    "Move one Tab within its strip. A committed result carries the new\nrevision; unchanged means the tab was already in position.\nWindows --insert-index uses a boundary in the original order: 0 is first,\nTab count is last; source/source+1 are unchanged. Out of range is refused.",
);

pub(super) const HELP_TAB_MOVE_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee tab move <target> (--order <previous|next|first|last> | --insert-index <N>) [--format text|json]",
    "在标签栏内移动一个标签页。committed 结果携带新修订；unchanged 表示标签页\n本已在目标位置。\nWindows --insert-index 使用原顺序的边界：0 为首位，标签页总数为末尾；源位置及其后一边界无变化。超出当前数量会拒绝。",
);

pub(super) const HELP_TAB_CLOSE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab close <target> (--confirm [--detach | --timeout <ms>] | --dry-run) [--format text|json]",
        "Close one Tab selected by exact ID, @tab:0x<hex>, or exact label.\n'--confirm' is\nrequired. By default the command waits until the Tab is gone (--timeout, default\n30000 ms; timeout exits 7 and the close continues). --detach returns once accepted,\nwith the accepted revision and close request id. --dry-run lists the terminals,\ntitles and Agents that would end, without --confirm.",
    );

pub(super) const HELP_TAB_CLOSE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab close <target> (--confirm [--detach | --timeout <ms>] | --dry-run) [--format text|json]",
        "关闭一个由精确 ID、@tab:0x<hex> 或精确 label 选择的标签页。'--confirm' 为必填。\n默认等待标签页消失（--timeout，默认 30000 毫秒；超时以 7 退出，关闭继续进行）。--detach 在接纳后立即返回，携带已接受修订与关闭请求 ID。--dry-run 无需 --confirm，列出将结束的终端、标题与智能体。",
    );
