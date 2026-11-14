//! Localized Workspace command details; does not mutate Workspace state.

pub(super) const HELP_WORKSPACE_LIST_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace list [--project <t>] [--worktrees] [--client <id>] [--format text|json]",
        "List all Workspaces, including ungrouped ones, or filter by the Project selected by\n--project (exact ID, @project:0x<hex>, or exact label). Fields: ids,\nlabel, primary directory\n(null means none), and terminal context.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\nWindows --worktrees adds each Workspace's live Git worktree observation: one query per Workspace with a directory, within the same 5000 ms total. Without it, JSON rows report worktreeObservation not-requested.",
    );

pub(super) const HELP_WORKSPACE_LIST_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace list [--project <t>] [--worktrees] [--client <id>] [--format text|json]",
        "列出所有工作区（包括未分组），或仅列 --project（精确 ID、@project:0x<hex> 或精确 label）所选\nProject 内的。字段：各 ID、label、主目录（null 表示无）与终端上下文。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；工作区结果包含记忆的活动标签页。不猜测客户端，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活智能体会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\nWindows --worktrees 附加每个工作区的实时 Git 工作树观察结果：每个有目录的工作区一次查询，共用同一个 5000 毫秒总时限。未指定时，JSON 行的 worktreeObservation 为 not-requested。",
    );

pub(super) const HELP_WORKSPACE_GET_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace get <target> [--client <id>] [--format text|json]",
        "Show one Workspace plus the Tabs that live in its terminal context.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
    );

pub(super) const HELP_WORKSPACE_GET_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace get <target> [--client <id>] [--format text|json]",
        "显示一个工作区及其终端上下文中存在的标签页。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；工作区结果包含记忆的活动标签页。不猜测客户端，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活智能体会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
    );

pub(super) const HELP_WORKSPACE_CREATE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace create [--project <target>] [--label <label>] [--workspace <source>] [--cwd <path|follow|home|current|workspace>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--env NAME=VALUE ...] [--exec <absolute-path> --arg <value> ... | --profile <selector>] [--format text|json]",
        "Create a Windows workspace and its first shell. Project and label are optional; no Agent starts automatically. Without --project the workspace is ungrouped. --workspace selects a directory source without inheriting its project.\n\nThe default directory follows the source terminal's verified working directory, then its workspace directory, then Windows home. An explicit path, home or current selection ignores unrelated caller identity. Relative and tilde paths are accepted literally without shell expansion.\n\nThe default label is the directory basename, or Workspace at a root. Occupied automatic labels add (2), (3), etc.; the 512-character limit includes this suffix. An explicit --label is kept unchanged.\n\nDefault: --no-focus. --focus requires an existing ready terminal client and waits for the workspace to appear; intervening navigation stops that focus. Timeout: 1..3600000 ms, default 30000.\n\n--exec/--arg and --profile cannot be combined. Custom geometry requires all four size fields. Environment and launch-profile rules still apply. Creation can succeed before a later terminal or focus failure. Terminal launch reports pending/started/failed/unavailable; use the returned workspace and request IDs to inspect the result. Never automatically retry an unknown outcome.",
    );

pub(super) const HELP_WORKSPACE_CREATE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace create [--project <target>] [--label <label>] [--workspace <source>] [--cwd <path|follow|home|current|workspace>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--env NAME=VALUE ...] [--exec <absolute-path> --arg <value> ... | --profile <selector>] [--format text|json]",
        "创建 Windows 工作区及其首个 Shell。项目和名称可选，不自动启动智能体。不指定 --project 时创建未分组工作区；--workspace 只选择目录来源，不继承其项目。\n\n默认目录依次取来源终端已验证的工作目录、其工作区目录、Windows 用户主目录。显式路径、home 或 current 忽略无关调用者身份；相对路径和波浪号路径按字面接受，不做 Shell 展开。\n\n默认名称为目录末级名称，根目录时为工作区。自动名称重名时追加 (2)、(3) 等后缀；512 字符上限包含后缀。显式 --label 保持原样。\n\n默认 --no-focus。--focus 要求已有就绪的终端客户端，并等待工作区显示；期间的其他导航会停止此次聚焦。时限为 1..3600000 毫秒，默认 30000 毫秒。\n\n--exec/--arg 与 --profile 不能混用；自定义尺寸必须提供四个尺寸字段。环境变量和启动配置规则仍适用。后续终端或聚焦失败时，工作区可能已创建。终端启动报告 pending/started/failed/unavailable；使用返回的工作区和请求 ID 检查结果，不自动重试未知结果。",
    );

pub(super) const HELP_WORKSPACE_RENAME_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace rename <target> --label <label> [--format text|json]",
        "Rename one Workspace selected by exact ID, @workspace:0x<hex>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.",
    );

pub(super) const HELP_WORKSPACE_RENAME_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace rename <target> --label <label> [--format text|json]",
        "重命名一个由精确 ID、@workspace:0x<hex> 或精确 label 选择的工作区。命令先读取一次快照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。",
    );

pub(super) const HELP_WORKSPACE_SET_DIRECTORY_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace set-directory <target> --path <path> [--format text|json]",
        "Set a literal absolute local-platform path as the primary directory of the\nWorkspace selected by exact ID, @workspace:0x<hex>, or exact label.\n'--path' is required: a\nliteral absolute path of at most 4,096 UTF-8 bytes without NUL — never\nexpanded ('~', '$'), created, or probed by the CLI. The command reads one\nsnapshot, resolves the target and its owning Project, then submits\nexactly one command under a fresh request id; the Server authoritatively\nprobes the path and rejects a Workspace that already has a directory.\nA committed result carries the new revision, the Server-allocated\ndirectory and worker ids, the resolved project and workspace ids, and the\nrequest's literal path.",
    );

pub(super) const HELP_WORKSPACE_SET_DIRECTORY_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace set-directory <target> --path <path> [--format text|json]",
        "将一个字面本机绝对路径设为由精确 ID、@workspace:0x<hex> 或精确 label 选择的\nWorkspace 的主目录。'--path' 为必填：字面绝对路径，至多 4,096 个\nUTF-8 字节且不含 NUL——CLI 不展开（'~'、'$'）、不创建、不探测。命令先读取一次快照、解析目标及其所属项目，然后以全新请求 ID 只提交一个命令；由 Server 权威探测该路径，并拒绝已有目录的\nWorkspace。committed 结果携带新修订、Server 分配的 directory 与\nworker ID、已解析的 project 与 workspace ID，以及请求的字面路径。",
    );

pub(super) const HELP_WORKSPACE_CLOSE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace close <target> (--confirm [--detach | --timeout <ms>] | --dry-run) [--format text|json]",
        "Close only the selected Workspace and all its terminals for every Client.\nTargets resolve globally by exact ID, @workspace:0x<hex>, or exact label.\n--confirm is required. Checkout directories and branches remain; other Workspaces\nsharing its repository or Project remain open. By default the command waits until\nthe Workspace is gone (--timeout, default 30000 ms; timeout exits 7 and the close\ncontinues). --detach returns the accepted receipt with the close request ID and\nstable Workspace/context IDs. --dry-run lists the terminals, titles and Agents that\nwould end, without --confirm. Observe with workspace status <request-id>;\nonly an explicit workspace resume resumes an eligible retained attempt.\nFor all members of a Project use project close-all --confirm.",
    );

pub(super) const HELP_WORKSPACE_CLOSE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace close <target> (--confirm [--detach | --timeout <ms>] | --dry-run) [--format text|json]",
        "仅为所有客户端关闭选定工作区及其所有终端。目标按精确 ID、@workspace:0x<hex> 或精确名称全局解析。必须指定 --confirm。checkout 目录与分支保留；同仓库或项目的其他工作区\n继续运行。默认等待工作区消失（--timeout，默认 30000 毫秒；超时以 7 退出，关闭继续进行）。--detach 返回已接纳回执，包含关闭请求 ID 与稳定的工作区/context ID。--dry-run 无需 --confirm，列出将结束的终端、标题与智能体。用 workspace status <request-id> 观测；只有显式 workspace resume 才恢复符合条件的保留尝试。关闭项目全部成员使用 project close-all --confirm。",
    );

pub(super) const HELP_WORKSPACE_MOVE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace move <target> --insert-index <n> [--format text|json]",
        "Reorder one workspace globally on Windows. Project membership, running terminals and every client's selection remain unchanged.\n\n--insert-index selects a boundary in the original order, from 0 through the workspace count (maximum 1023). Selecting the source boundary or its next boundary leaves the order unchanged.\n\nTargets resolve before the change; invalid input has no effect. No-op keeps the revision. Use workspace list to inspect order. An unknown outcome is never retried automatically.",
    );

pub(super) const HELP_WORKSPACE_MOVE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace move <target> --insert-index <n> [--format text|json]",
        "在 Windows 上全局重排一个工作区，保留项目归属、运行中的终端及各客户端的选择。\n\n--insert-index 选择原顺序的插入边界，从 0 到当前工作区数量，最多 1023。选择来源边界或其下一边界时，顺序保持不变。\n\n变更前先解析目标；无效输入不产生变更，无变化时保留修订。使用 workspace list 查看顺序；结果未知时不自动重试。",
    );
