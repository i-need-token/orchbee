//! The words of `uninstall`: the listing a person reads before confirming, the release prompt and
//! refusals, the final message, and the diagnostic that says what was done and what remains. The
//! uninstall owner supplies names, counts and paths; nothing here decides anything.
use super::Locale;

impl super::CliText {
    /// One fixed phrase of the listing or the result, by key.
    pub(crate) fn cli_uninstall_phrase(self, key: &str) -> &'static str {
        let (en, zh) = match key {
            "listing" => ("orchbee uninstall removes:", "orchbee uninstall 将移除："),
            "program-folder" => ("Program folder: ", "程序文件夹："),
            "path-entry" => ("User Path entry: ", "用户 Path 条目："),
            "installer-entries" => ("Installer entries: ", "安装程序条目："),
            "start-menu-shortcut" => ("Start menu shortcut", "开始菜单快捷方式"),
            "uninstall-registration" => ("uninstall registration", "卸载登记"),
            "integrations" => ("Agent integrations: ", "智能体集成："),
            "integrations-none" => ("none found enabled", "未发现已启用的"),
            "data-folder" => (
                "Data folder, settings and Sessions included: ",
                "数据文件夹（含设置和会话）：",
            ),
            "keeps" => ("Kept: settings and Sessions in ", "保留：设置和会话，位于 "),
            "keeps-settings" => ("Kept: settings and Sessions", "保留：设置和会话"),
            "sessions" => ("Sessions it stops:", "将停止的会话："),
            "no-sessions" => ("No Session is running.", "没有正在运行的会话。"),
            "agents-unknown" => ("Agents not known", "智能体数未知"),
            "separator" => (", ", "、"),
            "period" => (".", "。"),
            // Between two sentences of one diagnostic.
            "space" => (" ", ""),
            "done" => ("Orchbee is uninstalled.", "Orchbee 已卸载。"),
            "store-scheduled" => (
                "The program folder goes a few seconds after this command ends: ",
                "本命令结束几秒后移除程序文件夹：",
            ),
            "store-removed" => ("Removed the program folder ", "已移除程序文件夹 "),
            "store-files-scheduled" => (
                "Orchbee's program files in this folder go a few seconds after this command ends: ",
                "本命令结束几秒后移除此文件夹中 Orchbee 的程序文件：",
            ),
            "store-files-removed" => (
                "Removed Orchbee's program files from ",
                "已移除 Orchbee 的程序文件，位于 ",
            ),
            "data-deleted" => ("Deleted the data folder ", "已删除数据文件夹 "),
            "data-files-deleted" => (
                "Deleted Orchbee's data from ",
                "已删除 Orchbee 的数据，位于 ",
            ),
            "disabled" => ("Disabled Agent integrations: ", "已停用智能体集成："),
            "program-not-found" => (
                "Not checked, their program is not on PATH: ",
                "未检查（其程序不在 PATH 中）：",
            ),
            "new-terminal" => (
                "Terminals open now keep the old Path until they are reopened.",
                "当前打开的终端在重新打开前仍使用旧的 Path。",
            ),
            "npm" => (
                "Last step, remove the npm package: npm rm -g orchbee",
                "最后一步，移除 npm 包：npm rm -g orchbee",
            ),
            "nothing-changed" => ("Nothing was changed.", "未做任何改动。"),
            "stopped" => ("Uninstall stopped: ", "卸载已中止："),
            "was-done" => ("Done: ", "已完成："),
            "remains" => ("Remains: ", "尚余："),
            "step-sessions" => ("Sessions stopped", "会话已停止"),
            "step-integrations" => ("Agent integrations disabled", "智能体集成已停用"),
            "step-entries" => (
                "Path entry and installer entries removed",
                "Path 条目和安装程序条目已移除",
            ),
            "step-data" => ("data removed", "数据已移除"),
            "remain-integrations" => ("Agent integrations", "智能体集成"),
            "remain-path-entry" => ("the user Path entry", "用户 Path 条目"),
            "remain-installer-entries" => ("installer entries", "安装程序条目"),
            "remain-data" => (
                "logs, caches and runtime state in the data folder",
                "数据文件夹中的日志、缓存和运行状态",
            ),
            "remain-data-folder" => ("the data folder", "数据文件夹"),
            "remain-store" => ("the program folder", "程序文件夹"),
            _ => ("", ""),
        };
        match self.locale {
            Locale::EnUs => en,
            Locale::ZhCn => zh,
        }
    }

    /// One running Session in the listing.
    pub(crate) fn cli_uninstall_session(
        self,
        name: &str,
        terminals: Option<u32>,
        agents: Option<u32>,
    ) -> String {
        let name = super::escape::escape(name);
        let terminals = match (self.locale, terminals) {
            (Locale::EnUs, Some(1)) => "1 terminal open".to_owned(),
            (Locale::EnUs, Some(count)) => format!("{count} terminals open"),
            (Locale::EnUs, None) => "terminals not known".to_owned(),
            (Locale::ZhCn, Some(count)) => format!("{count} 个终端打开"),
            (Locale::ZhCn, None) => "终端数未知".to_owned(),
        };
        let agents = match (self.locale, agents) {
            (Locale::EnUs, Some(1)) => "1 Agent running".to_owned(),
            (Locale::EnUs, Some(count)) => format!("{count} Agents running"),
            (Locale::ZhCn, Some(count)) => format!("{count} 个智能体运行中"),
            (_, None) => self.cli_uninstall_phrase("agents-unknown").to_owned(),
        };
        match self.locale {
            Locale::EnUs => format!("{name}: {terminals}, {agents}"),
            Locale::ZhCn => format!("{name}：{terminals}，{agents}"),
        }
    }

    /// The listing's line for what a default uninstall removes from the data folder `data`, already
    /// rendered for display.
    pub(crate) fn cli_uninstall_data_entries(self, data: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("From {data}: logs, caches and temporary files"),
            Locale::ZhCn => format!("{data} 中的日志、缓存和临时文件"),
        }
    }

    /// The line for `items` entries of `folder` that are not Orchbee's and stay; `folder` is
    /// already rendered for display.
    pub(crate) fn cli_uninstall_kept(self, folder: &str, items: usize) -> String {
        match (self.locale, items) {
            (Locale::EnUs, 1) => format!("Kept 1 other item in {folder}"),
            (Locale::EnUs, _) => format!("Kept {items} other items in {folder}"),
            (Locale::ZhCn, _) => format!("保留 {folder} 中的其他 {items} 项"),
        }
    }

    /// How to continue after a stop; `program` is the full path to run once `orchbee` may no longer
    /// be on `Path`, already rendered for display.
    pub(crate) fn cli_uninstall_again(self, program: Option<&str>) -> String {
        match (self.locale, program) {
            (Locale::EnUs, None) => "Run orchbee uninstall again to continue.".to_owned(),
            (Locale::ZhCn, None) => "再次运行 orchbee uninstall 即可继续。".to_owned(),
            (Locale::EnUs, Some(program)) => {
                format!("Run \"{program}\" uninstall again to continue.")
            }
            (Locale::ZhCn, Some(program)) => format!("再次运行 \"{program}\" uninstall 即可继续。"),
        }
    }

    /// The release prompt that follows the listing.
    pub(crate) fn cli_uninstall_prompt(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Type orchbee to uninstall: ",
            Locale::ZhCn => "输入 orchbee 以卸载：",
        }
    }

    /// A release refusal for one `reason` token of the `confirmation` stage.
    pub(crate) fn cli_uninstall_refusal(self, reason: &str) -> String {
        match (self.locale, reason) {
            (Locale::EnUs, "confirm-option") => "The release build does not accept --confirm here. Nothing was changed. Run orchbee uninstall yourself in a console and type orchbee when asked.\n",
            (Locale::ZhCn, "confirm-option") => "正式版在此不接受 --confirm，未做任何改动。请自己在控制台运行 orchbee uninstall，并按提示输入 orchbee。\n",
            (Locale::EnUs, "not-interactive") => "The release build runs uninstall only in an interactive console, never from an Agent, script or pipe. Nothing was changed. Run orchbee uninstall yourself in a console and type orchbee when asked.\n",
            (Locale::ZhCn, "not-interactive") => "正式版只在交互式控制台中执行卸载，不接受智能体、脚本或管道调用，未做任何改动。请自己在控制台运行 orchbee uninstall，并按提示输入 orchbee。\n",
            (Locale::EnUs, _) => "The typed text is not orchbee. Nothing was changed. Run orchbee uninstall again and type orchbee exactly.\n",
            (Locale::ZhCn, _) => "输入的内容不是 orchbee，未做任何改动。请重新运行 orchbee uninstall，并准确输入 orchbee。\n",
        }
        .to_owned()
    }

    /// Why a run stopped, for one `error` token of the uninstall document. `subject` is the Session,
    /// provider, path or process the token concerns, already rendered for display; `detail` is a
    /// count, version or reason token.
    pub(crate) fn cli_uninstall_cause(self, error: &str, subject: &str, detail: &str) -> String {
        match (self.locale, error) {
            (Locale::EnUs, "interrupted") => "It was interrupted.".to_owned(),
            (Locale::ZhCn, "interrupted") => "已被中断。".to_owned(),
            (Locale::EnUs, "location") => "The program folder, the data folder or the registry key could not be determined.".to_owned(),
            (Locale::ZhCn, "location") => "无法确定程序文件夹、数据文件夹或注册表项。".to_owned(),
            (Locale::EnUs, "inside-session") => "This terminal belongs to an Orchbee Session, which uninstall must stop. Run orchbee uninstall in a terminal outside Orchbee.".to_owned(),
            (Locale::ZhCn, "inside-session") => "此终端属于一个 Orchbee 会话，而卸载必须停止该会话。请在 Orchbee 之外的终端中运行 orchbee uninstall。".to_owned(),
            (Locale::EnUs, "roots-overlap") => "The program folder and the data folder overlap, so neither can be removed alone.".to_owned(),
            (Locale::ZhCn, "roots-overlap") => "程序文件夹与数据文件夹相互重叠，无法单独移除其中之一。".to_owned(),
            (Locale::EnUs, "through-link") => format!("{subject} is a link or junction on the way to what uninstall removes, and uninstall never follows one."),
            (Locale::ZhCn, "through-link") => format!("{subject} 是通往卸载移除内容的链接或联接点，卸载从不经由链接操作。"),
            (Locale::EnUs, "not-orchbee-store") if !detail.is_empty() => format!("{subject} does not look like an Orchbee program folder: it has no readable active.json. {detail} chose this folder; check that variable."),
            (Locale::EnUs, "not-orchbee-store") => format!("{subject} does not look like an Orchbee program folder: it has no readable active.json."),
            (Locale::ZhCn, "not-orchbee-store") if !detail.is_empty() => format!("{subject} 看起来不是 Orchbee 的程序文件夹：其中没有可读的 active.json。该文件夹由 {detail} 指定，请检查这个变量。"),
            (Locale::ZhCn, "not-orchbee-store") => format!("{subject} 看起来不是 Orchbee 的程序文件夹：其中没有可读的 active.json。"),
            (Locale::EnUs, "not-orchbee-data") if !detail.is_empty() => format!("{subject} does not look like an Orchbee data folder: it has no readable internal\\version.json. {detail} chose this folder; check that variable."),
            (Locale::EnUs, "not-orchbee-data") => format!("{subject} does not look like an Orchbee data folder: it has no readable internal\\version.json."),
            (Locale::ZhCn, "not-orchbee-data") if !detail.is_empty() => format!("{subject} 看起来不是 Orchbee 的数据文件夹：其中没有可读的 internal\\version.json。该文件夹由 {detail} 指定，请检查这个变量。"),
            (Locale::ZhCn, "not-orchbee-data") => format!("{subject} 看起来不是 Orchbee 的数据文件夹：其中没有可读的 internal\\version.json。"),
            (Locale::EnUs, "unsafe-root" | "outside-roots" | "root-parent") => format!("{subject} is not a folder uninstall may remove."),
            (Locale::ZhCn, "unsafe-root" | "outside-roots" | "root-parent") => format!("{subject} 不是卸载可以移除的文件夹。"),
            (Locale::EnUs, "inventory") => "The Sessions or the Agent integrations of this environment could not be read.".to_owned(),
            (Locale::ZhCn, "inventory") => "无法读取本环境的会话或智能体集成。".to_owned(),
            (Locale::EnUs, "terminals-open") if detail == "1" => format!("Quit Orchbee first: Session {subject} has 1 open terminal."),
            (Locale::EnUs, "terminals-open") => format!("Quit Orchbee first: Session {subject} has {detail} open terminals."),
            (Locale::ZhCn, "terminals-open") => format!("请先退出 Orchbee：会话 {subject} 有 {detail} 个打开的终端。"),
            (Locale::EnUs, "session-unstoppable") => format!("Session {subject} runs Orchbee {detail} and has open terminals, which this version cannot end. Close them or stop the Session with that version first."),
            (Locale::ZhCn, "session-unstoppable") => format!("会话 {subject} 运行的是 Orchbee {detail}，且有打开的终端，此版本无法结束它们。请先关闭这些终端，或用该版本停止会话。"),
            (Locale::EnUs, "program-running") => format!("Quit Orchbee first: {subject} (process {detail}) runs from the program folder."),
            (Locale::ZhCn, "program-running") => format!("请先退出 Orchbee：{subject}（进程 {detail}）正从程序文件夹运行。"),
            (Locale::EnUs, "still-running") => format!("{subject} (process {detail}) still runs from the program folder, which cannot be removed until it ends."),
            (Locale::ZhCn, "still-running") => format!("{subject}（进程 {detail}）仍在从程序文件夹运行，它结束前无法移除程序文件夹。"),
            (Locale::EnUs, "sessions") => format!("Session {subject} did not stop ({detail})."),
            (Locale::ZhCn, "sessions") => format!("会话 {subject} 未能停止（{detail}）。"),
            (Locale::EnUs, "integrations") => format!("The Agent integration {subject} could not be disabled ({detail})."),
            (Locale::ZhCn, "integrations") => format!("无法停用智能体集成 {subject}（{detail}）。"),
            (Locale::EnUs, "path-entry") => format!("The user Path entry in {subject} could not be removed."),
            (Locale::ZhCn, "path-entry") => format!("无法移除 {subject} 中的用户 Path 条目。"),
            (Locale::EnUs, "installer-entries") => format!("{subject} could not be removed."),
            (Locale::ZhCn, "installer-entries") => format!("无法移除 {subject}。"),
            (Locale::EnUs, _) => format!("{subject} could not be removed completely."),
            (Locale::ZhCn, _) => format!("无法完全移除 {subject}。"),
        }
    }
}
