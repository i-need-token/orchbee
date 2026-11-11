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
mod integrations;
pub(crate) use integrations::{
    IntegrationAdapter, IntegrationBlock, IntegrationFactLabel, IntegrationFailure,
    IntegrationFinding, IntegrationStatus, IntegrationStep, IntegrationUnknown,
};
mod parameters;
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, SettingsLocale,
    TerminalDisplayAppearance, TuiColorSource,
};
pub(crate) use parameters::{
    SettingUnit, SettingsCategory, SettingsChoice, SettingsDetail, SettingsDetailKind,
    SettingsFieldRequirement, SettingsFileFact, SettingsGroup, SettingsHintLabel,
    SettingsProfileLabel, SettingsRow, SettingsTextRequirement, SettingsValue,
};
use parameters::{SettingsChoice as Choice, SettingsGroup as Group, SettingsRow as Row};
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
    pub(crate) fn settings_back_button(self) -> &'static str {
        self.setting("Back", "返回")
    }
    pub(crate) fn settings_row(self, row: impl Into<SettingsRow>) -> &'static str {
        let (en, zh) = match row.into() {
            Row::Language => ("Language", "语言"),
            Row::Theme => ("Application theme", "应用主题"),
            Row::LightTheme => ("Light theme", "浅色主题"),
            Row::DarkTheme => ("Dark theme", "深色主题"),
            Row::ColorSource => ("Interface colors", "界面配色"),
            Row::TerminalTheme => ("Terminal colors", "终端配色"),
            Row::Foreground => ("Text color", "文字颜色"),
            Row::Background => ("Background color", "背景色"),
            Row::CopyOnSelect => ("Copy on select", "选中即复制"),
            Row::TrimAgents => (
                "Hide empty space below Agent output",
                "收起智能体输出下方的空白",
            ),
            Row::ResumeAgents => (
                "Resume Agent conversations after restart",
                "重启后恢复智能体会话",
            ),
            Row::RestoreOutput => (
                "Restore terminal output after restart",
                "重启后恢复终端输出",
            ),
            Row::Reminders => ("Agent status notifications", "智能体状态通知"),
            Row::CompletionReminders => ("Command completion reminders", "命令完成提醒"),
            Row::CompletionThreshold => ("Minimum command duration", "最短命令时长"),
            Row::Sound => ("Notification sound", "通知声音"),
            Row::Scrollback => ("Terminal history", "终端历史"),
            Row::LaunchProfiles => ("Default profile", "默认启动配置"),
            Row::NewTerminalFolder => ("New terminal folder", "新终端文件夹"),
            Row::PrefixKey => ("Prefix key", "前缀键"),
            Row::Profile => ("Profile", "启动配置"),
            Row::ProfileName => ("Name", "名称"),
            Row::CommandType => ("Command", "命令"),
            Row::Executable => ("Executable (absolute path)", "可执行文件（绝对路径）"),
            Row::Login => ("Login shell", "登录 Shell"),
            Row::Argument => ("Argument", "参数"),
            Row::ArgumentValue => ("Literal argument value", "参数原文"),
            Row::AddArgument => ("Add argument", "添加参数"),
            Row::DeleteArgument => ("Remove argument", "移除参数"),
            Row::SetDefault => ("Use as default", "设为默认"),
            Row::AddProfile => ("Add profile", "添加启动配置"),
            Row::DeleteProfile => ("Delete profile", "删除启动配置"),
            Row::Back => ("Back", "返回"),
            Row::AgentIntegrations => return self.integrations_title(),
            Row::Agent(id) => return self.agent_integration_name(id),
            Row::RemoteHosts => return self.integrations_remote_title(),
            Row::RefreshIntegrations => return self.integrations_refresh(),
        };
        self.setting(en, zh)
    }
    pub(crate) fn settings_category_name(
        self,
        category: impl Into<SettingsCategory>,
    ) -> &'static str {
        match category.into() {
            SettingsCategory::General => self.setting("General", "通用"),
            SettingsCategory::Appearance => self.setting("Appearance", "外观"),
            SettingsCategory::Themes => self.setting("Themes", "主题"),
            SettingsCategory::Terminal => self.setting("Terminal", "终端"),
            SettingsCategory::Notifications => self.setting("Notifications", "通知"),
            SettingsCategory::Keyboard => self.setting("Keyboard", "快捷键"),
            SettingsCategory::Agents => self.setting("Agents", "智能体"),
        }
    }
    /// A group's heading; `None` for a group framed without one.
    pub(crate) fn settings_group_name(
        self,
        group: impl Into<SettingsGroup>,
    ) -> Option<&'static str> {
        Some(match group.into() {
            Group::Language | Group::Prefix => return None,
            Group::AfterRestart => self.setting("After a restart", "重启之后"),
            Group::AppAppearance => self.setting("App appearance", "应用外观"),
            Group::TerminalColors => self.setting("Terminal colors", "终端颜色"),
            Group::LightThemes => self.setting("Light theme", "浅色主题"),
            Group::DarkThemes => self.setting("Dark theme", "深色主题"),
            Group::SelectionKeys => self.setting("Selection and keys", "选择与按键"),
            Group::Output => self.setting("Output", "输出"),
            Group::LaunchProfiles => self.setting("Launch profiles", "启动配置"),
            Group::WhenToNotify => self.setting("When to notify", "何时通知"),
            Group::SoundDelivery => self.setting("Sound and delivery", "声音与显示"),
            Group::AgentIntegrations => self.integrations_title(),
            Group::LocalAgents => self.integrations_local_section(),
            Group::RemoteHosts => self.integrations_remote_section(),
        })
    }
    /// The line under a described group's heading.
    pub(crate) fn settings_group_description(
        self,
        group: impl Into<SettingsGroup>,
    ) -> Option<&'static str> {
        Some(match group.into() {
            Group::LaunchProfiles => self.setting(
                "Which shell a new terminal runs, plus your own profiles.",
                "新终端运行哪个 Shell，以及你自己的启动配置。",
            ),
            Group::LightThemes => self.setting(
                "Applies when the app appearance is Light.",
                "应用外观为浅色时才会使用。",
            ),
            Group::DarkThemes => self.setting(
                "Applies when the app appearance is Dark.",
                "应用外观为深色时才会使用。",
            ),
            _ => return None,
        })
    }
    /// The tag on the theme list in effect.
    pub(crate) fn settings_in_use(self) -> &'static str {
        self.setting("In use now", "正在使用")
    }
    /// When a change takes effect, for a row that does not take effect at once.
    pub(crate) fn settings_effect(self, row: impl Into<SettingsRow>) -> Option<&'static str> {
        match row.into() {
            Row::Scrollback | Row::NewTerminalFolder | Row::LaunchProfiles => {
                Some(self.setting("new terminals", "新终端生效"))
            }
            _ => None,
        }
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

    pub(crate) fn settings_description(self, row: impl Into<SettingsRow>) -> &'static str {
        match row.into() {
            Row::Language => {
                self.setting("Language for menus and messages.", "菜单和提示使用的语言。")
            }
            Row::Theme => self.setting(
                "Light, dark, or follow the system. Pick each theme under Themes.",
                "浅色、深色或跟随系统。浅色和深色各用哪套主题，在“主题”里选。",
            ),
            Row::LightTheme => self.setting(
                "Colors for the interface and terminals when the appearance is light.",
                "外观为浅色时，界面和终端使用的配色。",
            ),
            Row::DarkTheme => self.setting(
                "Colors for the interface and terminals when the appearance is dark.",
                "外观为深色时，界面和终端使用的配色。",
            ),
            Row::ColorSource => self.setting(
                "Use the theme, or your terminal’s own colors, for menus and panels.",
                "菜单和面板使用主题配色，或使用终端自己的配色。",
            ),
            Row::TerminalTheme => self.setting(
                "Default text and background in terminals; program colors are kept.",
                "终端默认的文字和背景颜色；程序自己指定的颜色不变。",
            ),
            Row::Foreground => self.setting(
                "Terminal text color for Custom, as #RRGGBB.",
                "“自定义”时终端的文字颜色，格式为 #RRGGBB。",
            ),
            Row::Background => self.setting(
                "Terminal background color for Custom, as #RRGGBB.",
                "“自定义”时终端的背景色，格式为 #RRGGBB。",
            ),
            Row::CopyOnSelect => self.setting(
                "Copies text as soon as you finish selecting it.",
                "选择完成后立即复制文字。",
            ),
            Row::TrimAgents => self.setting(
                "Keeps an Agent’s latest output at the bottom of its panel.",
                "让智能体的最新输出贴住面板底部。",
            ),
            Row::ResumeAgents => self.setting(
                "Reopens Claude Code and Codex conversations in place after restart.",
                "重启后在原面板中恢复 Claude Code 和 Codex 的会话。",
            ),
            Row::RestoreOutput => self.setting(
                "Shows recent output after a restart. Kept locally; may hold secrets.",
                "重启后再次显示最近的输出。保存在本机，可能包含密钥等敏感信息。",
            ),
            Row::Scrollback => self.setting(
                "How much output each terminal keeps for scrolling back.",
                "每个终端保留多少输出用于回看。",
            ),
            Row::Reminders => self.setting(
                "Tells you when an Agent you can’t see finishes or needs you.",
                "看不到的智能体完成或需要你时通知你。",
            ),
            Row::CompletionReminders => self.setting(
                "Reminds you when a long command ends. Command text is never shown.",
                "长命令结束时提醒你；不会显示命令内容。",
            ),
            Row::CompletionThreshold => self.setting(
                "Shorter commands don’t remind you.",
                "比这更短的命令不提醒。",
            ),
            Row::Sound => self.setting(
                "Plays a sound with notifications and reminders.",
                "通知和提醒时播放声音。",
            ),
            Row::LaunchProfiles => self.setting(
                "Edit profiles and choose the one new terminals use.",
                "编辑启动配置，并选择新终端使用哪一个。",
            ),
            Row::NewTerminalFolder => self.setting(
                "A folder, or follow, home, current or workspace. Empty for default.",
                "起始文件夹：路径，或 follow、home、current、workspace；留空用默认。",
            ),
            Row::PrefixKey => self.setting(
                "Press it before a command key, as in Ctrl+B c for a new tab.",
                "先按它再按命令键，例如 Ctrl+B c 新建标签页。",
            ),
            Row::AgentIntegrations => self.setting(
                "Shows each Agent’s status. Turning one on changes one of its files.",
                "让 Orchbee 显示各智能体的状态；开启一项会修改该智能体的一个文件。",
            ),
            Row::RemoteHosts => self.integrations_remote_description(),
            Row::RefreshIntegrations => self.integrations_refresh_description(),
            _ => "",
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
            Choice::ColorSource(TuiColorSource::Theme) => self.setting("Theme", "主题"),
            Choice::ColorSource(TuiColorSource::Terminal) => {
                self.setting("Match my terminal", "跟随我的终端")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Theme) => {
                self.setting("Match theme", "跟随主题")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Host) => {
                self.setting("Terminal default", "终端默认")
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
            SettingsValue::Integration(status) => self.integration_status(status).into(),
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
            SettingsHintLabel::Explain => self.setting("explain", "诊断"),
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
            SourceChanged => self.setting(
                "Settings changed elsewhere; change not applied · try again",
                "设置已在其他位置更改；未应用 · 请重试",
            ),
            FileBusy => self.setting(
                "Settings file busy; change not applied · try again shortly",
                "设置文件正忙；未应用 · 请稍后重试",
            ),
            InvalidValue => self.setting(
                "The value was refused; change not applied",
                "该值被拒绝；未应用",
            ),
            Unsupported => self.setting(
                "This setting isn't supported here; change not applied",
                "此处不支持该设置；未应用",
            ),
            SyntaxError => self.setting(
                "Settings file has a syntax error; change not applied · run orchbee config check",
                "设置文件有语法错误；未应用 · 请运行 orchbee config check",
            ),
            ReadOnly => self.setting(
                "Settings file is read-only; change not applied",
                "设置文件为只读；未应用",
            ),
            TooLarge => self.setting(
                "Settings file would be too large; change not applied",
                "设置文件将超出大小上限；未应用",
            ),
            UnusableFile => self.setting(
                "Can't use the settings file's location; not applied",
                "无法使用设置文件位置；未应用",
            ),
            Unreadable => self.setting(
                "Settings file can't be read; change not applied",
                "无法读取设置文件；未应用",
            ),
            WriteFailed => self.setting(
                "Settings file couldn't be written; change not applied",
                "无法写入设置文件；未应用",
            ),
            Failed => self.setting("Settings operation failed", "设置操作失败"),
            Unknown => self.setting(
                "Change result unknown · the page shows the file's values",
                "更改结果未知 · 页面显示文件中的值",
            ),
            Busy => self.setting(
                "Settings busy · finish the current action first",
                "设置正忙 · 请先完成当前操作",
            ),
            PresentationUnavailable => self.setting(
                "Cannot open Settings here · enlarge the window and try again",
                "当前窗口无法打开设置 · 放大窗口后重试",
            ),
            TooSmall => self.setting(
                "Settings hidden: enlarge the window to resume",
                "窗口太小，设置暂时隐藏；放大窗口后继续",
            ),
        }
    }

    /// The status line's shorter form of a settings notice too wide for it, for the notices that
    /// can be; it keeps the outcome and drops the advice.
    pub(crate) fn settings_notice_shorter(self, notice: SettingsNotice) -> Option<&'static str> {
        use SettingsNotice::*;
        match notice {
            SourceChanged => Some(self.setting(
                "Settings changed elsewhere; change not applied",
                "设置已在其他位置更改；未应用",
            )),
            FileBusy => Some(self.setting(
                "Settings file busy; change not applied",
                "设置文件正忙；未应用",
            )),
            SyntaxError => Some(self.setting(
                "Settings file has a syntax error; change not applied",
                "设置文件有语法错误；未应用",
            )),
            Unknown => Some(self.setting("Change result unknown", "更改结果未知")),
            PresentationUnavailable => {
                Some(self.setting("Cannot open Settings here", "当前窗口无法打开设置"))
            }
            InvalidValue | Failed | Busy | TooSmall | Unsupported | ReadOnly | TooLarge
            | UnusableFile | Unreadable | WriteFailed => None,
        }
    }
}
