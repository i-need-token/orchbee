//! Localized copy for persisted data versions (ORC-397): the refusal of a data folder whose layout
//! this version cannot read, and the two record messages with each record's name and consequence.
use super::Locale;
use orchbee_local_runtime::{LayoutRefusal, RecordNotice, RecordProblem, VersionedRecord};
use std::path::Path;

impl Locale {
    /// Why this version will not open the data folder at `root`, before it reads anything else.
    pub fn data_root_layout_refusal(self, root: &Path, refusal: &LayoutRefusal) -> String {
        let record = root.join("internal").join("version.json");
        let (root, record) = (root.display(), record.display());
        match (self, refusal.layout, refusal.newest_version.as_deref()) {
            (Self::EnUs, Some(_), Some(version)) => format!(
                "{root} was last used by Orchbee {version}, which changed how the folder is laid out. This version can't open it; use Orchbee {version} or later.\n"
            ),
            (Self::ZhCn, Some(_), Some(version)) => format!(
                "{root} 上次由 Orchbee {version} 使用，该版本更改了文件夹的布局。此版本无法打开它；请使用 Orchbee {version} 或更高版本。\n"
            ),
            (Self::EnUs, Some(_), None) => format!(
                "{root} was last used by a newer version of Orchbee, which changed how the folder is laid out. This version can't open it; use that version or later.\n"
            ),
            (Self::ZhCn, Some(_), None) => format!(
                "{root} 上次由更新版本的 Orchbee 使用，该版本更改了文件夹的布局。此版本无法打开它；请使用该版本或更高版本。\n"
            ),
            (Self::EnUs, None, _) => format!(
                "Orchbee can't read {record}, so it won't open {root}. Restore the file, or move the folder away to start again.\n"
            ),
            (Self::ZhCn, None, _) => format!(
                "Orchbee 无法读取 {record}，因此不会打开 {root}。请恢复该文件，或将文件夹移走后重新开始。\n"
            ),
        }
    }

    /// The message for a record this process cannot use, naming the Session whose folder holds
    /// it, without a trailing newline.
    pub fn record_notice(self, notice: &RecordNotice) -> String {
        let session = notice
            .path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str());
        self.record_message(notice.record, notice.problem, session)
    }

    /// The message for `record`; a Session record without `session` is the current Session's.
    pub fn record_message(
        self,
        record: VersionedRecord,
        problem: RecordProblem,
        session: Option<&str>,
    ) -> String {
        let item = self.record_item(record, session);
        let consequence = self.record_consequence(record);
        match (self, problem) {
            (Self::EnUs, RecordProblem::NotUpdated) => format!(
                "Orchbee couldn't update {} from an earlier version and left it unchanged. {consequence}",
                lowercase_first(&item)
            ),
            (Self::EnUs, RecordProblem::Newer) => format!(
                "{item} was saved by a newer version of Orchbee, so this version won't change it. {consequence}"
            ),
            (Self::ZhCn, RecordProblem::NotUpdated) => {
                format!("Orchbee 无法从早期版本更新{item}，已保持其原样。{consequence}")
            }
            (Self::ZhCn, RecordProblem::Newer) => {
                format!("{item}由更新版本的 Orchbee 保存，因此此版本不会更改它。{consequence}")
            }
        }
    }

    /// The status line's shorter form of the message for `record` of the current Session, which
    /// replaces a message too wide for the status line; `config check` keeps the whole message.
    pub(crate) fn record_status(self, record: VersionedRecord, problem: RecordProblem) -> String {
        let item = self.record_item(record, None);
        match (self, problem) {
            (Self::EnUs, RecordProblem::NotUpdated) => format!("{item} couldn't be updated"),
            (Self::EnUs, RecordProblem::Newer) => format!("{item} is from a newer Orchbee"),
            (Self::ZhCn, RecordProblem::NotUpdated) => format!("{item}无法更新"),
            (Self::ZhCn, RecordProblem::Newer) => format!("{item}来自更新版本的 Orchbee"),
        }
    }

    /// The record's name as the messages' `{item}`, capitalized for the start of a sentence.
    fn record_item(self, record: VersionedRecord, session: Option<&str>) -> String {
        match (self, record) {
            (Self::EnUs, VersionedRecord::Onboarding) => "The onboarding notice state".into(),
            (Self::ZhCn, VersionedRecord::Onboarding) => "入门提示状态".into(),
            (Self::EnUs, VersionedRecord::Machines) => "The saved machine list".into(),
            (Self::ZhCn, VersionedRecord::Machines) => "已保存的机器列表".into(),
            (Self::EnUs, VersionedRecord::PluginRegistry) => "The plugin registry".into(),
            (Self::ZhCn, VersionedRecord::PluginRegistry) => "插件注册表".into(),
            (Self::EnUs, VersionedRecord::SessionLifecycle) => session.map_or_else(
                || "This Session's record".into(),
                |session| format!("The record of Session {session}"),
            ),
            (Self::ZhCn, VersionedRecord::SessionLifecycle) => session.map_or_else(
                || "此会话的记录".into(),
                |session| format!("会话 {session} 的记录"),
            ),
            (Self::EnUs, VersionedRecord::TuiPosition) => session.map_or_else(
                || "This Session's TUI position".into(),
                |session| format!("The TUI position of Session {session}"),
            ),
            (Self::ZhCn, VersionedRecord::TuiPosition) => session.map_or_else(
                || "此会话的 TUI 位置".into(),
                |session| format!("会话 {session} 的 TUI 位置"),
            ),
        }
    }

    /// What the person sees while the record stays unusable: the store works as it does without
    /// it, as its owner's README states.
    const fn record_consequence(self, record: VersionedRecord) -> &'static str {
        match (self, record) {
            (Self::EnUs, VersionedRecord::Onboarding) => {
                "The onboarding notice shows again, and dismissing it lasts only until Orchbee closes."
            }
            (Self::ZhCn, VersionedRecord::Onboarding) => {
                "入门提示会再次显示，关闭它只在 Orchbee 退出前有效。"
            }
            (Self::EnUs, VersionedRecord::Machines) => {
                "Saved machines aren't shown, and changes last only until Orchbee closes."
            }
            (Self::ZhCn, VersionedRecord::Machines) => {
                "不显示已保存的机器，所做更改只保留到 Orchbee 退出。"
            }
            (Self::EnUs, VersionedRecord::PluginRegistry) => {
                "Plugins don't load, and installing or removing plugins is unavailable."
            }
            (Self::ZhCn, VersionedRecord::PluginRegistry) => "插件不会加载，也无法安装或移除插件。",
            (Self::EnUs, VersionedRecord::SessionLifecycle) => {
                "This version can't start or delete the Session."
            }
            (Self::ZhCn, VersionedRecord::SessionLifecycle) => "此版本无法启动或删除该会话。",
            (Self::EnUs, VersionedRecord::TuiPosition) => {
                "The TUI opens at the first Workspace and doesn't remember where you were."
            }
            (Self::ZhCn, VersionedRecord::TuiPosition) => {
                "TUI 会在第一个工作区打开，不会记住你上次的位置。"
            }
        }
    }
}

/// `The plugin registry` reads `the plugin registry` inside a sentence.
fn lowercase_first(item: &str) -> String {
    let mut characters = item.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(characters).collect()
    })
}
