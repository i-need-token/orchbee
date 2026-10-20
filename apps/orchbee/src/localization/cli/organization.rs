//! CLI-only organization vocabulary; shared TUI message enums remain independent.
use super::Locale;

pub(super) enum Help {
    ProjectDissolve,
    ProjectCloseAll,
    ProjectCloseStatus,
    ProjectCloseResume,
    WorkspaceSetProject,
    WorkspaceCreateProject,
    WorkspaceUngroup,
}

impl Locale {
    pub(crate) const fn cli_ungrouped(self) -> &'static str {
        match self {
            Self::EnUs => "Ungrouped",
            Self::ZhCn => "未分组",
        }
    }
}

impl Help {
    pub(super) const fn text(self, locale: Locale) -> crate::localization::HelpDetails {
        match locale {
            Locale::EnUs => match self {
                Self::ProjectDissolve => crate::localization::HelpDetails::new(
                    "orchbee project dissolve <target> [--format text|json]",
                    "Remove one Project grouping and retain all its Workspaces as ungrouped.\nOne metadata command preserves Workspace, Directory, terminal and checkout identities.\nNo terminal closes and no directory, branch or cwd changes.",
                ),
                Self::ProjectCloseAll => crate::localization::HelpDetails::new(
                    "orchbee project close-all <target> --confirm [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
                    "Close exactly the captured Workspaces in one Project for every Client.\n--confirm is required. Git checkouts, branches and the Project record remain.\nAn empty Project succeeds with no members. Default timeout: 120000 ms; range: 1..3600000.\n--detach returns the accepted operation identity; waiting never replays a mutation.\nUse project close-status to observe and project dissolve separately to remove the grouping.\nJSON: orchbee.project.close-all.",
                ),
                Self::ProjectCloseStatus => crate::localization::HelpDetails::new(
                    "orchbee project close-status --request <operation-id> [--format text|json]",
                    "Read one retained Project close-all operation by its complete stable operation ID.\nThis five-second read works after its Workspaces disappear and never changes state.",
                ),
                Self::ProjectCloseResume => crate::localization::HelpDetails::new(
                    "orchbee project close-resume --request <operation-id> --generation <1..64> --confirm [--detach] [--timeout <ms>] [--format text|json]",
                    "Explicitly resume one eligible Project close-all attempt under the observed generation fence.\nCaptured members and child Close identities stay fixed; unknown effects never resume.\nDefault timeout: 120000 ms; range: 1..3600000. Waiting does not cancel accepted work.",
                ),
                Self::WorkspaceSetProject => crate::localization::HelpDetails::new(
                    "orchbee workspace set-project <target> --project <target> [--format text|json]",
                    "Move one globally resolved Workspace into an existing Project with one metadata command.\nTargets are exact IDs, @workspace:0x<hex>/@project:0x<hex> selectors, or exact labels.\nAll scope IDs, terminals, cwd and filesystem data remain unchanged.\nUse workspace move or move-block for navigation reordering.",
                ),
                Self::WorkspaceCreateProject => crate::localization::HelpDetails::new(
                    "orchbee workspace create-project <target> --label <label> [--format text|json]",
                    "Atomically create a Project and move one globally resolved Workspace into it.\nThis is one metadata command, with no terminal, cwd, Git or filesystem effect.",
                ),
                Self::WorkspaceUngroup => crate::localization::HelpDetails::new(
                    "orchbee workspace ungroup <target> [--format text|json]",
                    "Remove one Workspace's optional Project membership with one metadata command.\nThe Workspace and all its scope IDs, terminals, cwd and checkout remain.",
                ),
            },
            Locale::ZhCn => match self {
                Self::ProjectDissolve => crate::localization::HelpDetails::new(
                    "orchbee project dissolve <target> [--format text|json]",
                    "解散一个 Project 分组，将其所有 Workspace 保留为未分组状态。一次元数据命令保留 Workspace、Directory、终端与 checkout 身份。不关闭终端，不改变目录、分支或 cwd。",
                ),
                Self::ProjectCloseAll => crate::localization::HelpDetails::new(
                    "orchbee project close-all <target> --confirm [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
                    "为所有客户端关闭一个 Project 中捕获的 Workspace。必须指定 --confirm；Git checkout、分支与 Project 记录保留。空 Project 以零成员成功。默认超时 120000 毫秒，范围 1..3600000。\n--detach 返回已接纳的操作身份；等待不会重放变更。用 project close-status 观测，用独立的 project dissolve 解散分组。\nJSON：orchbee.project.close-all。",
                ),
                Self::ProjectCloseStatus => crate::localization::HelpDetails::new(
                    "orchbee project close-status --request <operation-id> [--format text|json]",
                    "按完整稳定操作 ID 读取一个保留的 Project close-all 操作。此五秒读取在 Workspace 消失后仍可使用，且不改变状态。",
                ),
                Self::ProjectCloseResume => crate::localization::HelpDetails::new(
                    "orchbee project close-resume --request <operation-id> --generation <1..64> --confirm [--detach] [--timeout <ms>] [--format text|json]",
                    "按已观测代次栅栏显式恢复一个符合条件的 Project close-all 尝试。捕获成员与子 Close 身份固定；结果未知的效果不会恢复。默认超时 120000 毫秒，范围 1..3600000。结束等待不会取消已接纳工作。",
                ),
                Self::WorkspaceSetProject => crate::localization::HelpDetails::new(
                    "orchbee workspace set-project <target> --project <target> [--format text|json]",
                    "通过一次元数据命令，将全局解析的 Workspace 移入现有 Project。目标接受精确 ID、@workspace:0x<hex>/@project:0x<hex> 选择器或精确名称。所有作用域 ID、终端、cwd 与文件系统数据保持不变。导航排序使用 workspace move 或 move-block。",
                ),
                Self::WorkspaceCreateProject => crate::localization::HelpDetails::new(
                    "orchbee workspace create-project <target> --label <label> [--format text|json]",
                    "原子创建一个 Project，并将全局解析的 Workspace 移入其中。仅一次元数据命令，不改变终端、cwd、Git 或文件系统。",
                ),
                Self::WorkspaceUngroup => crate::localization::HelpDetails::new(
                    "orchbee workspace ungroup <target> [--format text|json]",
                    "通过一次元数据命令移除 Workspace 的可选 Project 归属。\nWorkspace 及其所有作用域 ID、终端、cwd 与 checkout 保留。",
                ),
            },
        }
    }
}
