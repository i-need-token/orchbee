//! CLI-only organization vocabulary; shared TUI message enums remain independent.
use super::Locale;

pub(super) enum Help {
    ProjectDissolve,
    ProjectCloseAll,
    WorkspaceSetProject,
}

impl super::CliText {
    pub(crate) const fn cli_ungrouped(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Ungrouped",
            Locale::ZhCn => "未分组",
        }
    }
}

impl Help {
    pub(super) const fn text(self, locale: Locale) -> crate::cli_text::HelpDetails {
        match locale {
            Locale::EnUs => match self {
                Self::ProjectDissolve => crate::cli_text::HelpDetails::new(
                    "orchbee project dissolve <target> [--format text|json]",
                    "Remove one Project grouping and retain all its Workspaces as ungrouped.\nOne metadata command preserves Workspace, Directory, terminal and checkout identities.\nNo terminal closes and no directory, branch or cwd changes.",
                ),
                Self::ProjectCloseAll => crate::cli_text::HelpDetails::new(
                    "orchbee project close-all <target> --confirm [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
                    "Close exactly the captured Workspaces in one Project for every Client.\n--confirm is required. Git checkouts, branches and the Project record remain.\nAn empty Project succeeds with no members. Default timeout: 120000 ms; range: 1..3600000.\n--detach returns the accepted operation identity; waiting never replays a mutation.\nUse project status <operation-id> to observe and project dissolve separately to remove the grouping.\nJSON: orchbee.project.close-all.",
                ),
                Self::WorkspaceSetProject => crate::cli_text::HelpDetails::new(
                    "orchbee workspace set-project <target> --project <target> [--format text|json]",
                    "Move one globally resolved Workspace into an existing Project with one metadata command.\nTargets are exact IDs, @workspace:0x<hex>/@project:0x<hex> selectors, or exact labels.\nAll scope IDs, terminals, cwd and filesystem data remain unchanged.\nUse workspace move for navigation reordering.",
                ),
            },
            Locale::ZhCn => match self {
                Self::ProjectDissolve => crate::cli_text::HelpDetails::new(
                    "orchbee project dissolve <target> [--format text|json]",
                    "解散一个项目分组，将其所有工作区保留为未分组状态。一次元数据命令保留工作区、Directory、终端与 checkout 身份。不关闭终端，不改变目录、分支或 cwd。",
                ),
                Self::ProjectCloseAll => crate::cli_text::HelpDetails::new(
                    "orchbee project close-all <target> --confirm [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
                    "为所有客户端关闭一个项目中捕获的工作区。必须指定 --confirm；Git checkout、分支与项目记录保留。空项目以零成员成功。默认超时 120000 毫秒，范围 1..3600000。\n--detach 返回已接纳的操作身份；等待不会重放变更。用 project status <operation-id> 观测，用独立的 project dissolve 解散分组。\nJSON：orchbee.project.close-all。",
                ),
                Self::WorkspaceSetProject => crate::cli_text::HelpDetails::new(
                    "orchbee workspace set-project <target> --project <target> [--format text|json]",
                    "通过一次元数据命令，将全局解析的工作区移入现有项目。目标接受精确 ID、@workspace:0x<hex>/@project:0x<hex> 选择器或精确名称。所有作用域 ID、终端、cwd 与文件系统数据保持不变。导航排序使用 workspace move。",
                ),
            },
        }
    }
}
