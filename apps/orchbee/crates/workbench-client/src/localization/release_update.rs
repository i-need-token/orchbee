use super::Locale;
use crate::release_update::{CheckUnavailable, UpdateCheck, UpdatePhase, moves_session};
use orchbee_local_runtime::EnvironmentKind;

impl Locale {
    /// The Update control's label on the Settings row.
    pub(crate) const fn update_control(self) -> &'static str {
        match self {
            Self::EnUs => "Update",
            Self::ZhCn => "更新",
        }
    }

    /// The Update control's effect line: its tooltip, and the sentence its action menu row carries,
    /// for a switch of this Client or, by [`moves_session`], the move of an older Session.
    pub(crate) fn update_tooltip(self, version: &str) -> String {
        match (self, moves_session(version)) {
            (Self::EnUs, false) => format!("Switch to Orchbee {version}. Terminals keep running."),
            (Self::EnUs, true) => {
                format!("Update the Session to Orchbee {version}. Terminals keep running.")
            }
            (Self::ZhCn, false) => format!("切换到 Orchbee {version}，终端继续运行。"),
            (Self::ZhCn, true) => format!("将会话更新到 Orchbee {version}，终端继续运行。"),
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

    /// What About's Updates row shows in place of its action while a run is under way, its
    /// check, its download of `version` with the share that has arrived, or its install, in the
    /// row's degradation order: a download's shorter forms drop the version and then the verb, so
    /// a narrow row never cuts the share or the version.
    pub(crate) fn about_running(self, check: &UpdateCheck) -> Vec<String> {
        match (self, check) {
            (Self::EnUs, UpdateCheck::Downloading { version, percent }) => vec![
                format!("Downloading {version}… {percent}%"),
                format!("Downloading… {percent}%"),
                format!("{percent}%"),
            ],
            (Self::ZhCn, UpdateCheck::Downloading { version, percent }) => vec![
                format!("正在下载 {version}… {percent}%"),
                format!("正在下载… {percent}%"),
                format!("{percent}%"),
            ],
            (Self::EnUs, UpdateCheck::Installing) => vec!["Installing…".to_owned()],
            (Self::ZhCn, UpdateCheck::Installing) => vec!["正在安装…".to_owned()],
            (Self::EnUs, _) => vec!["Checking…".to_owned()],
            (Self::ZhCn, _) => vec!["正在检查…".to_owned()],
        }
    }

    /// The Updates row's description: the offered version's switch while the Update control
    /// shows, else what the check came to, a failure named by the phase it ended in.
    pub(crate) fn about_updates(self, update: Option<&str>, check: &UpdateCheck) -> String {
        if let Some(version) = update {
            return self.update_tooltip(version);
        }
        let (en, zh) = match check {
            UpdateCheck::Idle
            | UpdateCheck::Checking
            | UpdateCheck::Downloading { .. }
            | UpdateCheck::Installing => (
                "Orchbee also checks every hour while automatic updates are on.",
                "自动更新开启时，Orchbee 也会每小时检查一次。",
            ),
            UpdateCheck::UpToDate => ("Orchbee is up to date.", "Orchbee 已是最新版本。"),
            UpdateCheck::Failed(UpdatePhase::Check) => (
                "Couldn't check: orchbee update or orchbee.com; terminals/Agents stay.",
                "无法检查。运行 orchbee update 或去 orchbee.com，终端和 Agent 不断。",
            ),
            UpdateCheck::Failed(UpdatePhase::Download) => (
                "Download failed: orchbee update or orchbee.com; terminals/Agents run.",
                "下载失败。运行 orchbee update 或去 orchbee.com，终端和 Agent 不断。",
            ),
            UpdateCheck::Failed(UpdatePhase::Install) => (
                "Downloaded but couldn't install. Install from orchbee.com.",
                "已下载，但无法安装。请从 orchbee.com 安装。",
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

    /// The question's sentence once the Session runs the new version and `count` terminals could
    /// not move from a Host it must replace: a restart of the Session restarts every terminal.
    pub(crate) fn host_replacement_staying(self, count: u32) -> String {
        match (self, count) {
            (Self::EnUs, 1) => "1 terminal couldn't move to this update. Restarting the Session restarts every terminal in its folder, and the programs running in them end. Cancel keeps that terminal running on the previous version.".to_owned(),
            (Self::EnUs, _) => format!(
                "{count} terminals couldn't move to this update. Restarting the Session restarts every terminal in its folder, and the programs running in them end. Cancel keeps those terminals running on the previous version."
            ),
            (Self::ZhCn, _) => format!(
                "有 {count} 个终端无法转移到此更新。重启会话会让每个终端在原来的文件夹中重新启动，其中运行的程序会结束。取消则让这些终端继续在旧版本上运行。"
            ),
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
