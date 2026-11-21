//! Localized Settings page vocabulary and the outcomes of its edits.
use super::Locale;

/// Settings feedback for the status line; persistence and expiry remain in the coordinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsNotice {
    /// The file changed under the edit; nothing was written.
    SourceChanged,
    Failed,
    /// The edit's result never arrived; the page shows the file's values.
    Unknown,
    Busy,
    PresentationUnavailable,
    TooSmall,
    /// Another writer or program held the settings file; nothing was written.
    FileBusy,
    /// The settings file refused a value; nothing was written.
    InvalidValue,
    /// The Server does not know a key the edit changed; nothing was written.
    Unsupported,
    /// The settings file does not parse; nothing was written.
    SyntaxError,
    ReadOnly,
    TooLarge,
    /// The settings file's location cannot be used; nothing was written.
    UnusableFile,
    /// The settings file cannot be read, or is not a regular file; nothing was written.
    Unreadable,
    WriteFailed,
}

use crate::terminal_text::{sanitize_terminal_text, truncate_display};
mod parameters;
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, SettingsLocale,
    TerminalDisplayAppearance,
};
use parameters::SettingsChoice as Choice;
pub(crate) use parameters::{
    SettingUnit, SettingsChoice, SettingsDetail, SettingsDetailKind, SettingsFieldRequirement,
    SettingsFileFact, SettingsHintLabel, SettingsProfileLabel, SettingsTextRequirement,
    SettingsValue, StartFolder,
};
impl Locale {
    fn setting(self, english: &'static str, chinese: &'static str) -> &'static str {
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        }
    }
    pub(crate) fn settings_title(self) -> &'static str {
        self.setting("Settings", "设置")
    }
    /// The tag on the theme list in effect.
    pub(crate) fn settings_in_use(self) -> &'static str {
        self.setting("In use now", "正在使用")
    }
    pub(crate) fn settings_restore(self) -> &'static str {
        self.setting("Restore default", "恢复默认")
    }
    pub(crate) fn settings_config_file(self) -> &'static str {
        self.setting("More options in config.toml", "更多选项在 config.toml 中")
    }
    pub(crate) fn settings_search_prompt(self) -> &'static str {
        self.setting("Search settings", "搜索设置")
    }
    pub(crate) fn settings_search_results(self) -> &'static str {
        self.setting("Search results", "搜索结果")
    }
    pub(crate) fn settings_no_results(self) -> &'static str {
        self.setting("No matching settings.", "没有匹配的设置。")
    }

    /// The file line for a settings file the page cannot use: the file, where a syntax error is,
    /// and that the page shows the last valid settings. A file the Server could not name is "the
    /// settings file".
    pub(crate) fn settings_file_fact(self, fact: SettingsFileFact<'_>) -> String {
        let named = |path: Option<&str>| {
            path.map(sanitize_terminal_text).map_or_else(
                || self.setting("Settings file", "设置文件").to_owned(),
                std::borrow::Cow::into_owned,
            )
        };
        match (self, fact) {
            (Self::EnUs, SettingsFileFact::SyntaxError { path, line, column }) => format!(
                "{}: syntax error at line {line}, column {column} · showing last valid settings",
                named(path)
            ),
            (Self::ZhCn, SettingsFileFact::SyntaxError { path, line, column }) => format!(
                "{}：第 {line} 行第 {column} 列有语法错误 · 显示上次有效的设置",
                named(path)
            ),
            (Self::EnUs, SettingsFileFact::Unreadable { path }) => {
                format!(
                    "{} can't be read · showing last valid settings",
                    named(path)
                )
            }
            (Self::ZhCn, SettingsFileFact::Unreadable { path: Some(path) }) => format!(
                "无法读取 {} · 显示上次有效的设置",
                sanitize_terminal_text(path)
            ),
            (Self::ZhCn, SettingsFileFact::Unreadable { path: None }) => {
                "无法读取设置文件 · 显示上次有效的设置".to_owned()
            }
        }
    }

    /// The fact without the file's path, for a line too narrow for it.
    pub(crate) fn settings_file_fact_compact(self, fact: SettingsFileFact<'_>) -> String {
        match (self, fact) {
            (Self::EnUs, SettingsFileFact::SyntaxError { line, column, .. }) => {
                format!("Syntax error at line {line}, column {column}")
            }
            (Self::ZhCn, SettingsFileFact::SyntaxError { line, column, .. }) => {
                format!("第 {line} 行第 {column} 列有语法错误")
            }
            (_, SettingsFileFact::Unreadable { .. }) => self
                .setting("Settings file can't be read", "无法读取设置文件")
                .to_owned(),
        }
    }

    pub(crate) fn settings_choice(self, choice: impl Into<SettingsChoice>) -> &'static str {
        match choice.into() {
            Choice::Locale(SettingsLocale::System)
            | Choice::ApplicationTheme(ApplicationAppearance::System) => {
                self.setting("Follow system", "跟随系统")
            }
            Choice::Locale(SettingsLocale::EnUs) => "English",
            Choice::Locale(SettingsLocale::ZhCn) => "简体中文",
            Choice::ApplicationTheme(ApplicationAppearance::Light) => self.setting("Light", "浅色"),
            Choice::ApplicationTheme(ApplicationAppearance::Dark) => self.setting("Dark", "深色"),
            Choice::ColorTheme("orchbee-light") => self.setting("Orchbee Light", "Orchbee 浅色"),
            Choice::ColorTheme("orchbee-dark") => self.setting("Orchbee Dark", "Orchbee 深色"),
            Choice::ColorTheme(id) => orchbee_color_themes::shipped()
                .iter()
                .find(|theme| theme.id() == id)
                .map_or(id, |theme| theme.name()),
            Choice::TerminalTheme(TerminalDisplayAppearance::Theme) => {
                self.setting("Match theme", "跟随主题")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Host) => {
                self.setting("Your terminal's colors", "所在终端的颜色")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Custom) => {
                self.setting("Custom", "自定义")
            }
            Choice::AgentReminders(AgentReminderMode::Never)
            | Choice::CompletionReminders(CompletionReminderMode::Never) => {
                self.setting("Off", "关闭")
            }
            Choice::AgentReminders(AgentReminderMode::Hidden) => {
                self.setting("When the Agent isn’t visible", "智能体不在视野内时")
            }
            Choice::CompletionReminders(CompletionReminderMode::Unfocused) => {
                self.setting("When the terminal isn’t focused", "终端未聚焦时")
            }
            Choice::CompletionReminders(CompletionReminderMode::Always) => {
                self.setting("Always", "始终")
            }
            Choice::NewTerminalFolder(StartFolder::Workspace) => {
                self.setting("Workspace folder", "工作区目录")
            }
            Choice::NewTerminalFolder(StartFolder::Home) => self.setting("Home folder", "主目录"),
            Choice::NewTerminalFolder(StartFolder::Follow) => {
                self.setting("Same as the current terminal", "与当前终端相同")
            }
            Choice::NewTerminalFolder(StartFolder::Custom) => {
                self.setting("Custom folder…", "自定义目录…")
            }
        }
    }
    pub(crate) fn settings_toggle(self, value: bool) -> &'static str {
        if value {
            self.setting("On", "开启")
        } else {
            self.setting("Off", "关闭")
        }
    }
    pub(crate) fn settings_value<'a>(self, value: impl Into<SettingsValue<'a>>) -> String {
        match value.into() {
            SettingsValue::Choice(choice) => self.settings_choice(choice).into(),
            SettingsValue::Text(value) => value.into(),
            SettingsValue::Toggle(value) => self.settings_toggle(value).into(),
            SettingsValue::Seconds(value) => match self {
                Self::EnUs => format!("{value} s"),
                Self::ZhCn => format!("{value} 秒"),
            },
            SettingsValue::ScrollbackMegabytes(value) => format!("{value} MB"),
            SettingsValue::ProfileName(name) => name
                .unwrap_or_else(|| self.setting("System default", "系统默认"))
                .into(),
            SettingsValue::SystemShell => self.setting("System shell", "系统 Shell").into(),
            SettingsValue::CustomExecutable => {
                self.setting("Custom executable", "自定义可执行文件").into()
            }
            SettingsValue::Required => self.setting("Required", "必填").into(),
            SettingsValue::Dash => "—".into(),
            SettingsValue::Unavailable => self.setting("Unavailable", "不适用").into(),
            SettingsValue::ArgumentPosition { position, total } => format!("{position} / {total}"),
            SettingsValue::Default => self.setting("Default", "默认").into(),
            SettingsValue::Empty => String::new(),
        }
    }

    pub(crate) fn settings_hint_label(self, label: SettingsHintLabel) -> &'static str {
        match label {
            SettingsHintLabel::Close => self.setting("close", "关闭"),
            SettingsHintLabel::Back => self.setting("back", "返回"),
            SettingsHintLabel::Cancel => self.setting("cancel", "取消"),
            SettingsHintLabel::Confirm => self.setting("confirm", "确认"),
            SettingsHintLabel::Accept => self.setting("accept", "确定"),
            SettingsHintLabel::Move => self.setting("move", "移动"),
            SettingsHintLabel::Switch => self.setting("switch", "切换栏"),
            SettingsHintLabel::Open => self.setting("open", "打开"),
            SettingsHintLabel::Change => self.setting("change", "更改"),
            SettingsHintLabel::Toggle => self.setting("turn on/off", "开启／关闭"),
            SettingsHintLabel::Select => self.setting("select", "选择"),
            SettingsHintLabel::Search => self.setting("search", "搜索"),
            SettingsHintLabel::GoTo => self.setting("go to", "前往"),
            SettingsHintLabel::Restore => self.setting("restore", "恢复默认"),
            SettingsHintLabel::Check => self.setting("check", "检查"),
            SettingsHintLabel::Update => self.setting("update", "更新"),
        }
    }
    pub(crate) fn settings_pending(self) -> &'static str {
        self.setting("[..] Opening", "[..] 正在打开")
    }
    pub(crate) fn settings_no_color(self) -> &'static str {
        self.setting(
            "NO_COLOR: terminal colors disabled.",
            "终端禁用了颜色（NO_COLOR）。",
        )
    }

    pub(crate) fn settings_detail<'a>(self, detail: impl Into<SettingsDetail<'a>>) -> String {
        match detail.into() {
            SettingsDetail::Field { name, requirement } => {
                let name = match name {
                    SettingsProfileLabel::Text(name) => name,
                    SettingsProfileLabel::SystemDefault => {
                        self.setting("System default", "系统默认")
                    }
                };
                let name = truncate_display(&sanitize_terminal_text(name), 18, "…");
                let requirement = match requirement {
                    SettingsFieldRequirement::SystemName => self.setting(
                        "Name: max 512 UTF-8 bytes; trim spaces; blank uses system name.",
                        "名称：最多 512 个 UTF-8 字节，无首尾空格；留空使用系统名称。",
                    ),
                    SettingsFieldRequirement::Name => self.setting(
                        "Name: required, max 512 UTF-8 bytes, no surrounding spaces/NUL.",
                        "名称：必填，最多 512 个 UTF-8 字节，无首尾空格或 NUL。",
                    ),
                    SettingsFieldRequirement::Executable => self.setting(
                        "Executable: absolute path required (max 32 KiB, no NUL).",
                        "可执行文件：必填绝对路径（最多 32 KiB，无 NUL）。",
                    ),
                    SettingsFieldRequirement::Argument => self.setting(
                        "Argument: max 32 KiB, no NUL; empty is allowed.",
                        "参数：最多 32 KiB，无 NUL；允许为空。",
                    ),
                };
                format!("{name} → {requirement}")
            }
            SettingsDetail::Message(kind) => self.settings_detail_text(kind).into(),
        }
    }

    fn settings_detail_text(self, kind: SettingsDetailKind) -> &'static str {
        match kind {
            SettingsDetailKind::ConfirmDeleteProfile => {
                self.setting("Delete this profile?", "删除此启动配置？")
            }
            SettingsDetailKind::InvalidValue => self.setting(
                "Invalid value; fix it, or press Esc again to discard these edits.",
                "值无效；请修正，或再按 Esc 放弃这些修改。",
            ),
            SettingsDetailKind::CustomColorsRequired => self.setting(
                "Choose Custom under Terminal colors to edit.",
                "终端配色选“自定义”后可编辑。",
            ),
            SettingsDetailKind::TruecolorRequired => self.setting(
                "Themes need a terminal with 24-bit color; this one uses its own colors.",
                "主题需要终端支持 24 位真彩色；此终端使用其自身的配色。",
            ),
            SettingsDetailKind::LoginUnsupported => self.setting(
                "Login shells are not supported on this platform.",
                "此平台不支持登录 Shell 选项。",
            ),
            SettingsDetailKind::CustomExecutableRequired => self.setting(
                "Select a custom executable; add an argument before editing it (maximum 128).",
                "选择自定义可执行文件后可添加并编辑参数（最多 128 个）。",
            ),
            SettingsDetailKind::MinimumProfile => self.setting(
                "Keep at least one launch profile.",
                "至少保留一个启动配置。",
            ),
            SettingsDetailKind::MaximumProfiles => {
                self.setting("At most 64 launch profiles.", "最多保留 64 个启动配置。")
            }
            SettingsDetailKind::AlreadyDefault => self.setting(
                "This is already the default profile.",
                "这已是默认启动配置。",
            ),
            SettingsDetailKind::Unavailable => self.setting(
                "This option is unavailable for the selected profile.",
                "此选项不适用于当前启动配置。",
            ),
            SettingsDetailKind::LiteralArguments => self.setting(
                "Arguments pass as written. Edits apply when you leave this page.",
                "参数按原文传递。离开此页时应用修改。",
            ),
        }
    }

    /// Why a row editor kept a typed value it did not send, in the Desktop's words.
    pub(crate) fn settings_text_requirement(self, requirement: &SettingsTextRequirement) -> String {
        match (self, requirement) {
            (_, SettingsTextRequirement::Color) => self
                .setting(
                    "Enter a color in #RRGGBB format.",
                    "请输入 #RRGGBB 格式的颜色。",
                )
                .to_owned(),
            (Self::EnUs, SettingsTextRequirement::WholeNumber { minimum, maximum }) => {
                format!("Enter a whole number from {minimum} to {maximum}.")
            }
            (Self::ZhCn, SettingsTextRequirement::WholeNumber { minimum, maximum }) => {
                format!("请输入 {minimum} 到 {maximum} 之间的整数。")
            }
            (Self::EnUs, SettingsTextRequirement::Number { minimum, maximum }) => {
                format!("Enter a number from {minimum} to {maximum}.")
            }
            (Self::ZhCn, SettingsTextRequirement::Number { minimum, maximum }) => {
                format!("请输入 {minimum} 到 {maximum} 之间的数字。")
            }
        }
    }

    pub(crate) fn settings_notice(self, notice: SettingsNotice) -> &'static str {
        use SettingsNotice::*;
        match notice {
            SourceChanged | FileBusy | WriteFailed => {
                self.setting("Couldn't save the setting.", "没能保存设置。")
            }
            InvalidValue => self.setting("Couldn't save that value.", "没能保存这个值。"),
            Unsupported => self.setting("Couldn't change this setting here.", "没能在这里更改此设置。"),
            SyntaxError => self.setting(
                "Couldn't save the setting. The settings file has a syntax error; run orchbee config check.",
                "没能保存设置。设置文件有语法错误，请运行 orchbee config check。",
            ),
            ReadOnly => self.setting(
                "Couldn't save the setting. The settings file is read-only.",
                "没能保存设置。设置文件是只读的。",
            ),
            TooLarge => self.setting(
                "Couldn't save the setting. The settings file would be too large.",
                "没能保存设置。设置文件会超出大小上限。",
            ),
            UnusableFile => self.setting(
                "Couldn't save the setting. The settings file's location can't be used.",
                "没能保存设置。无法使用设置文件所在的位置。",
            ),
            Unreadable => self.setting("Couldn't read the settings file.", "没能读取设置文件。"),
            Failed => self.setting("Couldn't change the setting.", "没能更改设置。"),
            Unknown => self.outcome_unknown(),
            Busy => self.setting("Finish the current action first.", "请先完成当前操作。"),
            PresentationUnavailable => self.setting(
                "Couldn't open Settings. Enlarge the window.",
                "没能打开设置。请放大窗口。",
            ),
            TooSmall => self.setting(
                "Enlarge the window to show Settings.",
                "放大窗口以显示设置。",
            ),
        }
    }

    /// The status line's shorter form of a settings notice too wide for it, for the notices that
    /// can be; it keeps the outcome and drops the advice.
    pub(crate) fn settings_notice_shorter(self, notice: SettingsNotice) -> Option<&'static str> {
        use SettingsNotice::*;
        match notice {
            SyntaxError | ReadOnly | TooLarge | UnusableFile => {
                Some(self.setting("Couldn't save the setting.", "没能保存设置。"))
            }
            PresentationUnavailable => {
                Some(self.setting("Couldn't open Settings.", "没能打开设置。"))
            }
            SourceChanged | FileBusy | Unknown | InvalidValue | Failed | Busy | TooSmall
            | Unsupported | Unreadable | WriteFailed => None,
        }
    }
}
