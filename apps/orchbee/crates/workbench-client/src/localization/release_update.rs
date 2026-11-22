use super::Locale;
use crate::release_update::{CheckUnavailable, UpdateCheck};
use orchbee_local_runtime::EnvironmentKind;

impl Locale {
    /// The Update control's label on the Settings row.
    pub(crate) const fn update_control(self) -> &'static str {
        match self {
            Self::EnUs => "Update",
            Self::ZhCn => "更新",
        }
    }

    /// The Update control's effect line: its tooltip, and the sentence its action menu row carries.
    pub(crate) fn update_tooltip(self, version: &str) -> String {
        match self {
            Self::EnUs => format!("Switch to Orchbee {version}. Terminals keep running."),
            Self::ZhCn => format!("切换到 Orchbee {version}，终端继续运行。"),
        }
    }

    /// What kind of build About's Version row names.
    pub(crate) const fn about_build(self, kind: EnvironmentKind) -> &'static str {
        match (self, kind) {
            (Self::EnUs, EnvironmentKind::Release) => "Release build",
            (Self::EnUs, EnvironmentKind::Test) => "Test build",
            (Self::EnUs, EnvironmentKind::Development) => "Development build",
            (Self::ZhCn, EnvironmentKind::Release) => "正式版",
            (Self::ZhCn, EnvironmentKind::Test) => "测试版",
            (Self::ZhCn, EnvironmentKind::Development) => "开发版",
        }
    }

    /// About's action that asks the Session to check now.
    pub(crate) const fn about_check(self) -> &'static str {
        match self {
            Self::EnUs => "Check for updates",
            Self::ZhCn => "检查更新",
        }
    }

    /// What About's Updates row shows in place of its action while a check runs.
    pub(crate) const fn about_checking(self) -> &'static str {
        match self {
            Self::EnUs => "Checking…",
            Self::ZhCn => "正在检查…",
        }
    }

    /// The Updates row's description: the offered version's switch while the Update control
    /// shows, else what the check came to.
    pub(crate) fn about_updates(self, update: Option<&str>, check: UpdateCheck) -> String {
        if let Some(version) = update {
            return self.update_tooltip(version);
        }
        let (en, zh) = match check {
            UpdateCheck::Idle | UpdateCheck::Checking => (
                "Orchbee also checks every hour while automatic updates are on.",
                "自动更新开启时，Orchbee 也会每小时检查一次。",
            ),
            UpdateCheck::UpToDate => ("Orchbee is up to date.", "Orchbee 已是最新版本。"),
            UpdateCheck::Failed => (
                "Couldn't check: orchbee update or orchbee.com; terminals/Agents stay.",
                "无法检查。运行 orchbee update 或去 orchbee.com，终端和 Agent 不断。",
            ),
            UpdateCheck::Unavailable(CheckUnavailable::Development) => {
                ("Development builds don't update.", "开发版不会更新。")
            }
            UpdateCheck::Unavailable(CheckUnavailable::Session) => (
                "Available once the Session updates. Or orchbee update; terminals run.",
                "会话更新后可用。也可运行 orchbee update，终端和 Agent 不断。",
            ),
            UpdateCheck::Unavailable(CheckUnavailable::Installation) => (
                "Not available for this installation.",
                "此安装不支持检查更新。",
            ),
        };
        match self {
            Self::EnUs => en.to_owned(),
            Self::ZhCn => zh.to_owned(),
        }
    }

    /// The title of the question asked before a Host replacement restarts every terminal.
    pub(crate) const fn host_replacement_title(self) -> &'static str {
        match self {
            Self::EnUs => "Restart terminals to update?",
            Self::ZhCn => "重启终端以完成更新？",
        }
    }

    pub(crate) const fn host_replacement_sentence(self) -> &'static str {
        match self {
            Self::EnUs => {
                "This update can't keep the terminals running. Each terminal restarts in its folder, and the programs running in them end."
            }
            Self::ZhCn => {
                "此更新无法让终端继续运行。每个终端会在原来的文件夹中重新启动，其中运行的程序会结束。"
            }
        }
    }

    pub(crate) const fn host_replacement_confirm(self) -> &'static str {
        match self {
            Self::EnUs => "Restart",
            Self::ZhCn => "重启",
        }
    }

    /// The one line a TUI started without the launcher prints when it leaves for `version`.
    pub(crate) fn update_without_launcher(self, version: &str) -> String {
        match self {
            Self::EnUs => format!("Run orchbee to open version {version}.\n"),
            Self::ZhCn => format!("运行 orchbee 打开 {version} 版本。\n"),
        }
    }
}
