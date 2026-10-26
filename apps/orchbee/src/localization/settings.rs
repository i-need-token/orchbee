//! Localized common-setting vocabulary and explicit draft/save outcomes.
use super::Locale;
use crate::{
    native_settings::{
        Choice, Confirmation, DraftView, Page, Reconciliation, Row, SettingCategory, SettingUnit,
        SettingsHintLabel, setting_for_row,
    },
    terminal_text::{sanitize_terminal_text, truncate_display},
    workbench_client::SettingsNotice,
};
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, PreferencesLocale,
    TerminalDisplayAppearance, UserLaunchCommand, UserLaunchField,
};
impl Locale {
    pub(crate) fn settings_back_button(self) -> &'static str {
        match self {
            Self::EnUs => "Back",
            Self::ZhCn => "返回",
        }
    }
    pub(crate) fn settings_accept_button(self) -> &'static str {
        match self {
            Self::EnUs => "Confirm",
            Self::ZhCn => "确认",
        }
    }
    pub(crate) fn settings_close_button(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
            Self::ZhCn => "关闭",
        }
    }
    pub(crate) fn settings_row_action(self, row: Row) -> Option<&'static str> {
        match row {
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
    pub(crate) fn settings_row(self, row: Row) -> &'static str {
        let (en, zh) = match row {
            Row::Language => ("Language", "语言"),
            Row::Theme => ("Application theme", "应用主题"),
            Row::TerminalTheme => ("Terminal display theme", "终端显示主题"),
            Row::Foreground => ("Foreground color", "前景色"),
            Row::Background => ("Background color", "背景色"),
            Row::CopyOnSelect => ("Copy on select", "选中即复制"),
            Row::TrimAgents => (
                "Hide empty space below Agent output",
                "收起智能体输出底部空白",
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
        };
        self.setting(en, zh)
    }
    pub(crate) fn settings_category(self, row: Row) -> &'static str {
        if let Some(metadata) = setting_for_row(row) {
            return self.settings_category_name(metadata.category);
        }
        match row {
            Row::ResetAll | Row::Reload | Row::Save => self.setting("General", "通用"),
            Row::Profile
            | Row::ProfileName
            | Row::CommandType
            | Row::Executable
            | Row::Login
            | Row::Argument
            | Row::ArgumentValue
            | Row::AddArgument
            | Row::DeleteArgument
            | Row::SetDefault
            | Row::AddProfile
            | Row::DeleteProfile
            | Row::ResetProfiles
            | Row::Back => self.settings_category_name(SettingCategory::LaunchProfiles),
            _ => unreachable!("metadata-backed setting row resolved above"),
        }
    }
    pub(crate) fn settings_category_name(self, category: SettingCategory) -> &'static str {
        match category {
            SettingCategory::Appearance => self.setting("Appearance", "外观"),
            SettingCategory::Terminal => self.setting("Terminal", "终端"),
            SettingCategory::Notifications => self.setting("Notifications", "通知"),
            SettingCategory::LaunchProfiles => self.setting("Launch profiles", "启动配置"),
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
    pub(crate) fn settings_compact_state(self, draft: DraftView<'_>) -> &'static str {
        match draft.reconciliation {
            Some(Reconciliation::OutcomeUnknown) => {
                self.setting("Save unknown; Reload required", "保存结果未知；请重新读取")
            }
            Some(Reconciliation::SourceChanged) => {
                self.setting("Changed elsewhere; Reload", "已在其他位置更改；请重新读取")
            }
            None if draft.conflict => {
                self.setting("Changed elsewhere; Reload", "已在其他位置更改；请重新读取")
            }
            None if draft.dirty => self.setting("Unsaved changes", "有未保存的更改"),
            None => self.setting("No unsaved changes", "没有未保存的更改"),
        }
    }
    pub(crate) fn settings_invalid_short(self) -> &'static str {
        self.setting("Invalid value", "值无效")
    }
    pub(crate) fn settings_description(self, row: Row) -> &'static str {
        match row {
            Row::Language => self.setting(
                "Language for application text; previews locally.",
                "应用文字的语言；在本地预览。",
            ),
            Row::Theme => self.setting(
                "Application appearance; previews locally.",
                "应用外观主题；在本地预览。",
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
            _ => self.setting(
                "Edits stay in this draft until Save.",
                "编辑保留在此草稿中，保存后生效。",
            ),
        }
    }
    pub(crate) fn settings_choice(self, choice: Choice) -> &'static str {
        match choice {
            Choice::Locale(PreferencesLocale::System)
            | Choice::ApplicationTheme(ApplicationAppearance::System) => {
                self.setting("Follow system", "跟随系统")
            }
            Choice::Locale(PreferencesLocale::EnUs) => "English",
            Choice::Locale(PreferencesLocale::ZhCn) => "简体中文",
            Choice::ApplicationTheme(ApplicationAppearance::Light)
            | Choice::TerminalTheme(TerminalDisplayAppearance::Light) => {
                self.setting("Light", "浅色")
            }
            Choice::ApplicationTheme(ApplicationAppearance::Dark)
            | Choice::TerminalTheme(TerminalDisplayAppearance::Dark) => {
                self.setting("Dark", "深色")
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
    pub(crate) fn settings_value(self, draft: DraftView<'_>, row: Row) -> String {
        let values = draft.values;
        let profile = &values.launch_profiles.profiles[draft.profile];
        let profile_name = |profile: &orchbee_workbench_protocol::UserLaunchProfile| {
            profile
                .name
                .clone()
                .unwrap_or_else(|| self.setting("System default", "系统默认").into())
        };
        let value = match row {
            Row::Language => match values.locale {
                PreferencesLocale::System => self.setting("Follow system", "跟随系统"),
                PreferencesLocale::EnUs => "English",
                PreferencesLocale::ZhCn => "简体中文",
            },
            Row::Theme => match values.application_theme {
                ApplicationAppearance::System => self.setting("Follow system", "跟随系统"),
                ApplicationAppearance::Light => self.setting("Light", "浅色"),
                ApplicationAppearance::Dark => self.setting("Dark", "深色"),
            },
            Row::TerminalTheme => match values.terminal_display_theme {
                TerminalDisplayAppearance::Host => self.setting("Host default", "宿主默认"),
                TerminalDisplayAppearance::Light => self.setting("Light", "浅色"),
                TerminalDisplayAppearance::Dark => self.setting("Dark", "深色"),
                TerminalDisplayAppearance::Custom => self.setting("Custom", "自定义"),
            },
            Row::Foreground => &values.custom_foreground,
            Row::Background => &values.custom_background,
            Row::CopyOnSelect => self.settings_toggle(values.copy_on_select),
            Row::TrimAgents => self.settings_toggle(values.trim_trailing_blank_rows_for_agents),
            Row::Reminders => match values.agent_notification_mode {
                AgentReminderMode::Never => self.setting("Never", "从不"),
                AgentReminderMode::Hidden => {
                    self.setting("When Agent location is hidden", "智能体位置不可见时")
                }
            },
            Row::CompletionReminders => match values.completion_reminder_mode {
                CompletionReminderMode::Never => self.setting("Never", "从不"),
                CompletionReminderMode::Unfocused => {
                    self.setting("When terminal is unfocused", "终端未聚焦时")
                }
                CompletionReminderMode::Always => self.setting("Always", "始终"),
            },
            Row::CompletionThreshold => {
                return values.completion_reminder_minimum_seconds.to_string();
            }
            Row::Sound => self.settings_toggle(values.sound_enabled),
            Row::Scrollback => {
                return format!(
                    "{} MB",
                    crate::native_settings::scrollback_megabytes(values.scrollback_bytes)
                );
            }
            Row::LaunchProfiles => {
                return values
                    .launch_profiles
                    .profiles
                    .iter()
                    .find(|p| p.id == values.launch_profiles.default_profile_id)
                    .map(profile_name)
                    .unwrap_or_default();
            }
            Row::Profile | Row::ProfileName => return profile_name(profile),
            Row::CommandType => match &profile.command {
                UserLaunchCommand::SystemShell { .. } => self.setting("System shell", "系统 Shell"),
                _ => self.setting("Custom executable", "自定义可执行文件"),
            },
            Row::Executable => match &profile.command {
                UserLaunchCommand::CustomExecutable { executable, .. } if executable.is_empty() => {
                    self.setting("Required", "必填")
                }
                UserLaunchCommand::CustomExecutable { executable, .. } => executable,
                _ => "—",
            },
            Row::Login => match &profile.command {
                UserLaunchCommand::SystemShell { login } if !cfg!(windows) => {
                    self.settings_toggle(*login)
                }
                _ => self.setting("Unavailable", "不适用"),
            },
            Row::Argument => match &profile.command {
                UserLaunchCommand::CustomExecutable { arguments, .. } if !arguments.is_empty() => {
                    return format!("{} / {}", draft.argument + 1, arguments.len());
                }
                _ => "—",
            },
            Row::ArgumentValue => match &profile.command {
                UserLaunchCommand::CustomExecutable { arguments, .. } => {
                    arguments.get(draft.argument).map_or("—", String::as_str)
                }
                _ => "—",
            },
            Row::SetDefault if profile.id == values.launch_profiles.default_profile_id => {
                self.setting("Default", "默认")
            }
            _ => "",
        };
        value.into()
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
            SettingsHintLabel::Clear => self.setting("clear", "清空"),
            SettingsHintLabel::ActivateEdit => self.setting("activate/edit", "操作／编辑"),
            SettingsHintLabel::Select => self.setting("select", "选择"),
            SettingsHintLabel::Change => self.setting("change", "切换"),
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
    pub(crate) fn settings_state_facts(self, draft: DraftView<'_>) -> String {
        match draft.reconciliation {
            Some(Reconciliation::OutcomeUnknown) => self
                .setting(
                    "Save result unknown · Reload required",
                    "保存结果未知 · 需要重新读取",
                )
                .into(),
            Some(Reconciliation::SourceChanged) => self
                .setting(
                    "Saved settings changed elsewhere · Reload required",
                    "已保存的设置已在其他位置更改 · 需要重新读取",
                )
                .into(),
            None if draft.conflict => self
                .setting(
                    "Saved settings changed elsewhere · Reload required",
                    "已保存的设置已在其他位置更改 · 需要重新读取",
                )
                .into(),
            None if draft.dirty => self.setting("Unsaved changes", "有未保存的更改").into(),
            None => self
                .setting("No unsaved changes", "没有未保存的更改")
                .into(),
        }
    }
    pub(crate) fn settings_no_color(self) -> &'static str {
        self.setting(
            "NO_COLOR: terminal colors disabled.",
            "终端禁用了颜色（NO_COLOR）。",
        )
    }

    pub(crate) fn settings_detail(self, draft: DraftView<'_>) -> String {
        if draft.confirmation.is_none() {
            let field = draft.invalid_field.or_else(|| {
                if draft.page != Page::Profiles || !draft.enabled(draft.selected) {
                    return None;
                }
                let field = match draft.selected {
                    Row::ProfileName => UserLaunchField::Name,
                    Row::Executable => UserLaunchField::Executable,
                    Row::ArgumentValue => UserLaunchField::Argument(draft.argument),
                    _ => return None,
                };
                Some((draft.profile, field))
            });
            if let Some((index, field)) = field {
                let profile = &draft.values.launch_profiles.profiles[index];
                let name = profile.name.as_deref().unwrap_or_else(|| {
                    if profile.id == "system-default" {
                        self.setting("System default", "系统默认")
                    } else {
                        &profile.id
                    }
                });
                let name = truncate_display(&sanitize_terminal_text(name), 18, "…");
                let requirement = match field {
                    UserLaunchField::Name if profile.id == "system-default" => self.setting(
                        "Name: max 512 UTF-8 bytes; trim spaces; blank uses system name.",
                        "名称：最多 512 个 UTF-8 字节，无首尾空格；留空使用系统名称。",
                    ),
                    UserLaunchField::Name => self.setting(
                        "Name: required, max 512 UTF-8 bytes, no surrounding spaces/NUL.",
                        "名称：必填，最多 512 个 UTF-8 字节，无首尾空格或 NUL。",
                    ),
                    UserLaunchField::Executable => self.setting(
                        "Executable: absolute path required (max 32 KiB, no NUL).",
                        "可执行文件：必填绝对路径（最多 32 KiB，无 NUL）。",
                    ),
                    UserLaunchField::Argument(_) => self.setting(
                        "Argument: max 32 KiB, no NUL; empty is allowed.",
                        "参数：最多 32 KiB，无 NUL；允许为空。",
                    ),
                };
                return format!("{name} → {requirement}");
            }
        }
        self.settings_detail_text(draft).into()
    }

    fn settings_detail_text(self, draft: DraftView<'_>) -> &'static str {
        if let Some(confirmation) = draft.confirmation {
            return match confirmation {
                Confirmation::ResetAll => self.setting(
                    "Reset common settings and launch profiles in this draft?",
                    "在草稿中重置全部通用设置和启动配置？",
                ),
                Confirmation::ResetProfiles => self.setting(
                    "Reset launch profiles in this draft? Other settings will be kept.",
                    "在草稿中重置启动配置？其他设置将保留。",
                ),
                Confirmation::Reload => self.setting(
                    "Discard this draft and read the latest shared settings?",
                    "丢弃此草稿并读取最新共享设置？",
                ),
                Confirmation::DeleteProfile => self.setting(
                    "Delete this profile from the draft?",
                    "从草稿中删除此启动配置？",
                ),
            };
        }
        if draft.invalid {
            return self.setting("Invalid value; the draft was kept.", "值无效；草稿已保留。");
        }
        if !draft.enabled(draft.selected) {
            return match draft.selected {
                Row::Save => match draft.reconciliation {
                    Some(Reconciliation::SourceChanged) => self.setting(
                        "Source changed; reload before saving.",
                        "来源已变化；请重新读取后再保存。",
                    ),
                    Some(Reconciliation::OutcomeUnknown) => self.setting(
                        "Save outcome unknown; reload before saving again.",
                        "保存结果未知；请重新读取后再保存。",
                    ),
                    None if draft.conflict => self.setting(
                        "Settings changed elsewhere; reload before saving.",
                        "另一端已修改设置；请重新读取后再保存。",
                    ),
                    None => self.setting("No changes to save.", "没有需要保存的更改。"),
                },
                Row::Foreground | Row::Background => self.setting(
                    "Select the custom terminal theme to edit these colors.",
                    "选择自定义终端主题后可编辑这些颜色。",
                ),
                Row::Login if cfg!(windows) => self.setting(
                    "Login shells are unavailable on Windows.",
                    "Windows 不支持登录 Shell 选项。",
                ),
                Row::Executable
                | Row::Argument
                | Row::ArgumentValue
                | Row::AddArgument
                | Row::DeleteArgument => self.setting(
                    "Select a custom executable; add an argument before editing it (maximum 128).",
                    "选择自定义可执行文件后可添加并编辑参数（最多 128 个）。",
                ),
                Row::DeleteProfile => self.setting(
                    "Keep at least one launch profile.",
                    "至少保留一个启动配置。",
                ),
                Row::AddProfile => {
                    self.setting("At most 64 launch profiles.", "最多保留 64 个启动配置。")
                }
                Row::SetDefault => self.setting(
                    "This is already the default profile.",
                    "这已是默认启动配置。",
                ),
                _ => self.setting(
                    "This option is unavailable for the selected profile.",
                    "此选项不适用于当前启动配置。",
                ),
            };
        }
        if draft.page == Page::Profiles {
            return self.setting(
                "Literal arguments; new terminals use the saved default.",
                "参数按原文传递；新终端使用已保存的默认配置。",
            );
        }
        if draft.reconciliation == Some(Reconciliation::OutcomeUnknown) {
            return self.setting(
                "Preview paused; reload to check the unknown Save result.",
                "预览已暂停；请重新读取以核对未知的保存结果。",
            );
        }
        match draft.selected {
            Row::Language | Row::Theme | Row::TerminalTheme | Row::Foreground | Row::Background => {
                self.setting(
                    "Local preview; Save keeps changes. Close discards the draft.",
                    "本地预览；保存后保留更改，关闭则丢弃草稿。",
                )
            }
            Row::CopyOnSelect => self.setting(
                "After Save: later selection completion. Clipboard delivery needs host support.",
                "保存后用于后续选区完成；剪贴板交付需要宿主支持。",
            ),
            Row::TrimAgents => self.setting(
                "After Save: existing detected agents; active selection and history stay put.",
                "保存后用于现有已检测智能体；保留正在操作的选区和历史视图。",
            ),
            Row::Reminders => self.setting(
                "After Save: future hidden-agent events; host delivery and per-agent mute apply.",
                "保存后用于后续隐藏智能体事件；仍遵循宿主交付与各智能体静音设置。",
            ),
            Row::CompletionReminders | Row::CompletionThreshold => self.setting(
                "After Save: future command completions; requires shell integration; no replay.",
                "保存后用于后续命令完成；需要 Shell 集成，不补发历史事件。",
            ),
            Row::Sound => self.setting(
                "After Save: eligible future reminders; host sound and per-agent mute apply.",
                "保存后用于后续符合条件的提醒；仍遵循宿主声音能力与各智能体静音设置。",
            ),
            Row::Scrollback => self.setting(
                "After Save: new terminals only. Existing terminals keep their captured limit.",
                "保存后仅用于新终端；现有终端保留创建时的回滚上限。",
            ),
            Row::LaunchProfiles => self.setting(
                "After Save: new default launches. Existing programs keep running unchanged.",
                "保存后用于新的默认启动；现有程序继续运行。",
            ),
            _ => self.setting(
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
