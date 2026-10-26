//! Localized Project command details; does not create or edit Projects.

pub(super) const HELP_PROJECT_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project list [--format text|json]",
        "List every Project in the running local session: id, label, directory and\nworkspace counts. An empty session prints just the table header.",
    );

pub(super) const HELP_PROJECT_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project list [--format text|json]",
        "列出本地会话中所有 Project：ID、label、目录与工作区计数。空会话仅输出表头。",
    );

pub(super) const HELP_PROJECT_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project get <target> [--format text|json]",
        "Show one Project: its directories (with worker ids and paths) and its\nworkspaces. The target is an exact 32-hex ID, an @project:0x<hex>\nselector, or an exact case-sensitive label; ambiguous labels\nreport candidates instead of guessing.",
    );

pub(super) const HELP_PROJECT_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project get <target> [--format text|json]",
        "显示一个 Project：其目录（含 worker id 与路径）与工作区。target 为精确\n32 位十六进制 ID、@project:0x<hex> 选择器或精确区分大小写的 label；歧义 label 报告候选而非猜测。",
    );

pub(super) const HELP_PROJECT_CREATE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project create --label <label> [--format text|json]",
        "Create one Project with the given label. The command reads one snapshot,\nthen submits exactly one command under a fresh request id. The result\ncarries the committed revision and the Server-allocated project id.\nA new Project has no directory or Workspace, and creating one touches no\nfilesystem path. Labels may repeat across Projects; address the new\nProject by its returned id.",
    );

pub(super) const HELP_PROJECT_CREATE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project create --label <label> [--format text|json]",
        "以给定 label 创建一个 Project。命令先读取一次快照，然后以全新请求 ID\n只提交一个命令。结果携带已提交修订与 Server 分配的 project id。新 Project 无目录、无 Workspace，创建不触碰任何文件系统路径。\nlabel 允许跨 Project 重复；请按返回的 ID 寻址新 Project。",
    );

pub(super) const HELP_PROJECT_RENAME_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project rename <target> --label <label> [--format text|json]",
        "Rename one Project selected by exact ID, @project:0x<hex>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.",
    );

pub(super) const HELP_PROJECT_RENAME_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project rename <target> --label <label> [--format text|json]",
        "重命名一个由精确 ID、@project:0x<hex> 或精确 label 选择的 Project。命令先读取一次快照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。",
    );
