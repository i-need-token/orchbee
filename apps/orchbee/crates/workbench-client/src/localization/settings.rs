//! Localized common-setting vocabulary and explicit draft/save outcomes.
use super::Locale;

/// Settings draft and save feedback; persistence and expiry remain in the coordinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsNotice {
    Saved,
    NoChange,
    SourceChanged,
    Failed,
    Unreadable,
    Unknown,
    Busy,
    PresentationUnavailable,
    TooSmall,
    Background,
}

use crate::terminal_text::{sanitize_terminal_text, truncate_display};
mod integrations;
pub(crate) use integrations::{
    IntegrationAdapter, IntegrationBlock, IntegrationFactLabel, IntegrationFailure,
    IntegrationFinding, IntegrationStatus, IntegrationStep, IntegrationUnknown,
};
mod parameters;
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, PreferencesLocale,
    TerminalDisplayAppearance, TuiColorSource,
};
pub(crate) use parameters::{
    SettingUnit, SettingsAcceptLabel, SettingsCategory, SettingsChoice, SettingsDetail,
    SettingsDetailKind, SettingsDraftState, SettingsFieldRequirement, SettingsHintLabel,
    SettingsProfileLabel, SettingsRow, SettingsValue,
};
use parameters::{
    SettingsCategory as SettingCategory, SettingsChoice as Choice, SettingsRow as Row,
};
impl Locale {
    pub(crate) fn settings_back_button(self) -> &'static str {
        match self {
            Self::EnUs => "Back",
            Self::ZhCn => "返回",
        }
    }
    /// The child step's primary names its outcome: applying an edit, or the confirmed action.
    pub(crate) fn settings_accept_button(self, label: SettingsAcceptLabel) -> &'static str {
        match (self, label) {
            (Self::EnUs, SettingsAcceptLabel::Apply) => "Apply",
            (Self::ZhCn, SettingsAcceptLabel::Apply) => "应用",
            (Self::EnUs, SettingsAcceptLabel::Reset) => "Reset",
            (Self::ZhCn, SettingsAcceptLabel::Reset) => "重置",
            (Self::EnUs, SettingsAcceptLabel::Reload) => "Reload",
            (Self::ZhCn, SettingsAcceptLabel::Reload) => "重新读取",
            (Self::EnUs, SettingsAcceptLabel::Delete) => "Delete",
            (Self::ZhCn, SettingsAcceptLabel::Delete) => "删除",
        }
    }
    pub(crate) fn settings_close_button(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
            Self::ZhCn => "关闭",
        }
    }
    pub(crate) fn settings_row_action(self, row: impl Into<SettingsRow>) -> Option<&'static str> {
        match row.into() {
            Row::ResetAll => Some(self.setting("Reset all", "全部重置")),
            Row::Reload => Some(self.setting("Reload", "重新读取")),
            Row::Save => Some(self.setting("Save", "保存")),
            _ => None,
        }
    }
    fn setting(self, english: &'static str, chinese: &'static str) -> &'static str {
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        }
    }
    pub(crate) fn settings_title(self) -> &'static str {
        self.setting("Settings", "设置")
    }
    pub(crate) fn settings_row(self, row: impl Into<SettingsRow>) -> &'static str {
        let (en, zh) = match row.into() {
            Row::Language => ("Language", "语言"),
            Row::Theme => ("Application theme", "应用主题"),
            Row::LightTheme => ("Light theme", "浅色主题"),
            Row::DarkTheme => ("Dark theme", "深色主题"),
            Row::ColorSource => ("Colors", "配色"),
            Row::TerminalTheme => ("Terminal display theme", "终端显示主题"),
            Row::Foreground => ("Foreground color", "前景色"),
            Row::Background => ("Background color", "背景色"),
            Row::CopyOnSelect => ("Copy on select", "选中即复制"),
            Row::TrimAgents => (
                "Hide empty space below Agent output",
                "收起智能体输出底部空白",
            ),
            Row::ResumeAgents => ("Resume Agents after restart", "重启后恢复智能体"),
            Row::RestoreOutput => (
                "Restore terminal output after restart",
                "重启后恢复终端输出",
            ),
            Row::Reminders => ("Agent status notifications", "智能体状态通知"),
            Row::CompletionReminders => ("Completion reminders", "命令完成提醒"),
            Row::CompletionThreshold => {
                ("Minimum command duration (seconds)", "最短命令时长（秒）")
            }
            Row::Sound => ("Notification sound", "通知声音"),
            Row::Scrollback => ("Terminal history (MB)", "终端历史容量（MB）"),
            Row::LaunchProfiles => ("Launch profiles", "启动配置"),
            Row::ResetAll => ("Reset all common settings", "重置全部通用设置"),
            Row::Reload => ("Reload and discard drafts", "重新读取并丢弃草稿"),
            Row::Save => ("Save", "保存"),
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
            Row::ResetProfiles => ("Reset launch profiles", "重置启动配置"),
            Row::Back => ("Back to settings", "返回设置"),
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
            SettingCategory::General => self.setting("General", "通用"),
            SettingCategory::Appearance => self.setting("Appearance", "外观"),
            SettingCategory::Terminal => self.setting("Terminal", "终端"),
            SettingCategory::Notifications => self.setting("Notifications", "通知"),
            SettingCategory::LaunchProfiles => self.setting("Launch profiles", "启动配置"),
            SettingCategory::AgentIntegrations => self.integrations_title(),
            SettingCategory::LocalAgentAdapters => self.integrations_local_section(),
            SettingCategory::RemoteAgentHosts => self.integrations_remote_section(),
        }
    }
    pub(crate) fn settings_search_label(self) -> &'static str {
        self.setting("Search", "搜索")
    }
    /// Follows the search key on an idle search row, as in `/ to search settings`.
    pub(crate) fn settings_search_prompt(self) -> &'static str {
        self.setting("to search settings", "搜索设置")
    }
    pub(crate) fn settings_no_results(self) -> &'static str {
        self.setting(
            "No matching settings. Clear the search to see all settings.",
            "没有匹配的设置。清除搜索以查看全部设置。",
        )
    }
    pub(crate) fn settings_search_count(self, count: usize) -> String {
        match self {
            Self::EnUs => format!("{count} found"),
            Self::ZhCn => format!("{count} 项"),
        }
    }
    pub(crate) fn settings_compact_state(
        self,
        state: impl Into<SettingsDraftState>,
    ) -> &'static str {
        match state.into() {
            SettingsDraftState::Unknown => {
                self.setting("Save unknown; Reload required", "保存结果未知；请重新读取")
            }
            SettingsDraftState::SourceChanged => {
                self.setting("Changed elsewhere; Reload", "已在其他位置更改；请重新读取")
            }
            SettingsDraftState::Dirty => self.setting("Unsaved changes", "有未保存的更改"),
            SettingsDraftState::Clean => self.setting("No unsaved changes", "没有未保存的更改"),
        }
    }

    pub(crate) fn settings_invalid_short(self) -> &'static str {
        self.setting("Invalid value", "值无效")
    }
    pub(crate) fn settings_description(self, row: impl Into<SettingsRow>) -> &'static str {
        match row.into() {
            Row::Language => self.setting(
                "Language for application text; previews locally.",
                "应用文字的语言；在本地预览。",
            ),
            Row::Theme => self.setting(
                "Application appearance; previews locally.",
                "应用外观主题；在本地预览。",
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
                "Use the selected theme, or your terminal's own colors.",
                "使用所选主题，或使用终端自身的配色。",
            ),
            Row::TerminalTheme => self.setting(
                "Terminal default colors; program colors stay exact.",
                "终端默认颜色；保留程序指定的颜色。",
            ),
            Row::Foreground | Row::Background => self.setting(
                "Terminal colors in Custom; use #RRGGBB.",
                "终端自定义颜色；使用 #RRGGBB。",
            ),
            Row::CopyOnSelect => self.setting(
                "Copy completed selections after Save; needs host clipboard support.",
                "保存后复制完成的选区；需要宿主剪贴板支持。",
            ),
            Row::TrimAgents => self.setting(
                "Trim blank agent rows after Save; preserves active history and selection.",
                "保存后裁剪智能体空白行；保留正在操作的历史和选区。",
            ),
            Row::ResumeAgents => self.setting(
                "Resume Claude Code and Codex Agent sessions in their Panels after a restart.",
                "重启后在原面板中恢复 Claude Code 和 Codex 智能体会话。",
            ),
            Row::RestoreOutput => self.setting(
                "Keeps recent output on this computer and shows it after a restart; may hold secrets.",
                "在本机保存最近的终端输出并在重启后显示；其中可能含有密钥等敏感信息。",
            ),
            Row::Scrollback => self.setting(
                "History capacity for new terminals: 0–1000 MB; 1 MB = 1,000,000 bytes.",
                "新终端历史容量：0–1000 MB；1 MB = 1,000,000 字节。",
            ),
            Row::Reminders => self.setting(
                "Future hidden-agent notifications after Save; host and mute rules apply.",
                "保存后用于后续隐藏智能体通知；遵循宿主与静音规则。",
            ),
            Row::CompletionReminders => self.setting(
                "Future command reminders after Save; requires shell integration.",
                "保存后用于后续命令完成提醒；需要 Shell 集成。",
            ),
            Row::CompletionThreshold => self.setting(
                "Minimum command duration: 1–3600 seconds; applies after Save.",
                "最短命令时长：1–3600 秒；保存后生效。",
            ),
            Row::Sound => self.setting(
                "Sound for future reminders after Save; needs host support.",
                "保存后用于后续提醒的声音；需要宿主支持。",
            ),
            Row::LaunchProfiles => self.setting(
                "Shell and literal arguments for new default launches after Save.",
                "保存后用于新的默认启动的 Shell 和字面参数。",
            ),
            Row::AgentIntegrations => self.integrations_description(),
            _ => self.setting(
                "Edits stay in this draft until Save.",
                "编辑保留在此草稿中，保存后生效。",
            ),
        }
    }
    pub(crate) fn settings_choice(self, choice: impl Into<SettingsChoice>) -> &'static str {
        match choice.into() {
            Choice::Locale(PreferencesLocale::System)
            | Choice::ApplicationTheme(ApplicationAppearance::System) => {
                self.setting("Follow system", "跟随系统")
            }
            Choice::Locale(PreferencesLocale::EnUs) => "English",
            Choice::Locale(PreferencesLocale::ZhCn) => "简体中文",
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
                self.setting("Terminal colors", "终端配色")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Theme) => {
                self.setting("Theme", "主题")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Host) => {
                self.setting("Host default", "宿主默认")
            }
            Choice::TerminalTheme(TerminalDisplayAppearance::Custom) => {
                self.setting("Custom", "自定义")
            }
            Choice::AgentReminders(AgentReminderMode::Never)
            | Choice::CompletionReminders(CompletionReminderMode::Never) => {
                self.setting("Never", "从不")
            }
            Choice::AgentReminders(AgentReminderMode::Hidden) => {
                self.setting("When Agent location is hidden", "智能体位置不可见时")
            }
            Choice::CompletionReminders(CompletionReminderMode::Unfocused) => {
                self.setting("When terminal is unfocused", "终端未聚焦时")
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
    pub(crate) fn settings_unit(self, unit: SettingUnit) -> &'static str {
        match unit {
            SettingUnit::Seconds => self.setting("s", "秒"),
            SettingUnit::Bytes => self.setting("B", "字节"),
        }
    }
    pub(crate) fn settings_value_with_unit(self, value: &str, unit: SettingUnit) -> String {
        match self {
            Self::EnUs => format!("{value} {}", self.settings_unit(unit)),
            Self::ZhCn => format!("{value}{}", self.settings_unit(unit)),
        }
    }
    pub(crate) fn settings_value<'a>(self, value: impl Into<SettingsValue<'a>>) -> String {
        match value.into() {
            SettingsValue::Choice(choice) => self.settings_choice(choice).into(),
            SettingsValue::Text(value) => value.into(),
            SettingsValue::Toggle(value) => self.settings_toggle(value).into(),
            SettingsValue::Integer(value) => value.to_string(),
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
            SettingsHintLabel::Search => self.setting("search", "搜索"),
            SettingsHintLabel::Save => self.setting("save", "保存"),
            SettingsHintLabel::Close => self.setting("close", "关闭"),
            SettingsHintLabel::Back => self.setting("back", "返回"),
            SettingsHintLabel::Cancel => self.setting("cancel", "取消"),
            SettingsHintLabel::Confirm => self.setting("confirm", "确认"),
            SettingsHintLabel::Accept => self.setting("accept", "接受"),
            SettingsHintLabel::ActivateEdit => self.setting("activate/edit", "操作／编辑"),
            SettingsHintLabel::Select => self.setting("select", "选择"),
            SettingsHintLabel::Change => self.setting("change", "切换"),
            SettingsHintLabel::Toggle => self.setting("turn on/off", "开启／关闭"),
            SettingsHintLabel::Explain => self.setting("explain", "诊断"),
        }
    }
    pub(crate) fn settings_cancel(self) -> &'static str {
        self.setting("Cancel", "取消")
    }
    pub(crate) fn settings_pending(self, saving: bool) -> &'static str {
        match (self, saving) {
            (Self::EnUs, false) => "[..] Opening",
            (Self::ZhCn, false) => "[..] 正在打开",
            (Self::EnUs, true) => "[..] Saving",
            (Self::ZhCn, true) => "[..] 正在保存",
        }
    }
    pub(crate) fn settings_saving_fact(self) -> &'static str {
        self.setting("Saving continues after close", "保存将在关闭后继续")
    }
    pub(crate) fn settings_state_facts(self, state: impl Into<SettingsDraftState>) -> String {
        match state.into() {
            SettingsDraftState::Unknown => self
                .setting(
                    "Save result unknown · Reload required",
                    "保存结果未知 · 需要重新读取",
                )
                .into(),
            SettingsDraftState::SourceChanged => self
                .setting(
                    "Saved settings changed elsewhere · Reload required",
                    "已保存的设置已在其他位置更改 · 需要重新读取",
                )
                .into(),
            SettingsDraftState::Dirty => self.setting("Unsaved changes", "有未保存的更改").into(),
            SettingsDraftState::Clean => self
                .setting("No unsaved changes", "没有未保存的更改")
                .into(),
        }
    }

    /// The slot a theme row sets is used only when the host matches, while Appearance follows it.
    pub(crate) fn settings_system_notice(
        self,
        row: impl Into<SettingsRow>,
    ) -> Option<&'static str> {
        match row.into() {
            Row::LightTheme => {
                Some(self.setting("Used when the system is light.", "系统为浅色时使用。"))
            }
            Row::DarkTheme => {
                Some(self.setting("Used when the system is dark.", "系统为深色时使用。"))
            }
            _ => None,
        }
    }
    /// Joins two complete sentences as the locale writes them.
    pub(crate) fn settings_sentences(self, first: &str, second: &str) -> String {
        match self {
            Self::EnUs => format!("{first} {second}"),
            Self::ZhCn => format!("{first}{second}"),
        }
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
            SettingsDetailKind::ConfirmResetAll => self.setting(
                "Reset common settings and launch profiles in this draft?",
                "在草稿中重置全部通用设置和启动配置？",
            ),
            SettingsDetailKind::ConfirmResetProfiles => self.setting(
                "Reset launch profiles in this draft? Other settings will be kept.",
                "在草稿中重置启动配置？其他设置将保留。",
            ),
            SettingsDetailKind::ConfirmReload => self.setting(
                "Discard this draft and read the latest shared settings?",
                "丢弃此草稿并读取最新共享设置？",
            ),
            SettingsDetailKind::ConfirmDeleteProfile => self.setting(
                "Delete this profile from the draft?",
                "从草稿中删除此启动配置？",
            ),
            SettingsDetailKind::InvalidValue => {
                self.setting("Invalid value; the draft was kept.", "值无效；草稿已保留。")
            }
            SettingsDetailKind::SourceChangedBeforeSave => self.setting(
                "Source changed; reload before saving.",
                "来源已变化；请重新读取后再保存。",
            ),
            SettingsDetailKind::UnknownSave => self.setting(
                "Save outcome unknown; reload before saving again.",
                "保存结果未知；请重新读取后再保存。",
            ),
            SettingsDetailKind::ConflictBeforeSave => self.setting(
                "Settings changed elsewhere; reload before saving.",
                "另一端已修改设置；请重新读取后再保存。",
            ),
            SettingsDetailKind::NoChanges => {
                self.setting("No changes to save.", "没有需要保存的更改。")
            }
            SettingsDetailKind::CustomColorsRequired => self.setting(
                "Select the custom terminal theme to edit these colors.",
                "选择自定义终端主题后可编辑这些颜色。",
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
                "Literal arguments; new terminals use the saved default.",
                "参数按原文传递；新终端使用已保存的默认配置。",
            ),
            SettingsDetailKind::UnknownPreview => self.setting(
                "Preview paused; reload to check the unknown Save result.",
                "预览已暂停；请重新读取以核对未知的保存结果。",
            ),
            SettingsDetailKind::LocalPreview => self.setting(
                "Local preview; Save keeps changes. Close discards the draft.",
                "本地预览；保存后保留更改，关闭则丢弃草稿。",
            ),
            SettingsDetailKind::CopyAfterSave => self.setting(
                "After Save: later selection completion. Clipboard delivery needs host support.",
                "保存后用于后续选区完成；剪贴板交付需要宿主支持。",
            ),
            SettingsDetailKind::TrimAgentsAfterSave => self.setting(
                "After Save: existing detected agents; active selection and history stay put.",
                "保存后用于现有已检测智能体；保留正在操作的选区和历史视图。",
            ),
            SettingsDetailKind::ResumeAgentsAfterSave => self.setting(
                "After Save: applies at the next restart; Off also stops saving Agent session IDs.",
                "保存后于下次重启生效；关闭后也不再保存智能体会话 ID。",
            ),
            SettingsDetailKind::RestoreOutputAfterSave => self.setting(
                "After Save: kept from the next save; Off deletes every kept copy.",
                "保存后从下次保存起生效；关闭会删除所有已保存的输出。",
            ),
            SettingsDetailKind::AgentNotificationsAfterSave => self.setting(
                "After Save: future hidden-agent events; host delivery and per-agent mute apply.",
                "保存后用于后续隐藏智能体事件；仍遵循宿主交付与各智能体静音设置。",
            ),
            SettingsDetailKind::CompletionAfterSave => self.setting(
                "After Save: future command completions; requires shell integration; no replay.",
                "保存后用于后续命令完成；需要 Shell 集成，不补发历史事件。",
            ),
            SettingsDetailKind::SoundAfterSave => self.setting(
                "After Save: eligible future reminders; host sound and per-agent mute apply.",
                "保存后用于后续符合条件的提醒；仍遵循宿主声音能力与各智能体静音设置。",
            ),
            SettingsDetailKind::ScrollbackAfterSave => self.setting(
                "After Save: new terminals only. Existing terminals keep their captured limit.",
                "保存后仅用于新终端；现有终端保留创建时的回滚上限。",
            ),
            SettingsDetailKind::LaunchProfilesAfterSave => self.setting(
                "After Save: new default launches. Existing programs keep running unchanged.",
                "保存后用于新的默认启动；现有程序继续运行。",
            ),
            SettingsDetailKind::DeviceSettings => self.setting(
                "Shared on this device/configuration; font and opacity belong to your terminal.",
                "在此设备／配置范围共享；字体和透明度由宿主终端管理。",
            ),
        }
    }

    pub(crate) fn settings_notice(self, notice: SettingsNotice) -> &'static str {
        use SettingsNotice::*;
        match notice {
            Saved => self.setting("Settings saved", "设置已保存"),
            NoChange => self.setting("No changes to save", "没有需要保存的更改"),
            SourceChanged => self.setting(
                "Settings changed elsewhere; draft kept · reload to resolve",
                "另一端已修改设置；草稿已保留 · 请重新读取",
            ),
            Unreadable => self.setting(
                "Settings file can't be read · using defaults · Reset all in Settings",
                "无法读取设置文件 · 正在使用默认值 · 请在设置中全部重置",
            ),
            Failed => self.setting(
                "Settings operation failed; no save confirmed",
                "设置操作失败；未确认保存",
            ),
            Unknown => self.setting(
                "Save result unknown; reload to check before saving again",
                "保存结果未知；再次保存前请重新读取核对",
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
            Background => self.setting(
                "Save continues · wait for its result",
                "保存继续执行 · 请等待结果",
            ),
        }
    }
}
