//! Localized common-setting vocabulary and explicit draft/save outcomes.
use super::Locale;
use crate::{
    native_settings::{Confirmation, DraftView, Page, Row, View},
    terminal_session::SettingsNotice,
};
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, PreferencesLocale,
    TerminalDisplayAppearance, UserLaunchCommand,
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
    pub(crate) fn settings_row(self, row: Row) -> &'static str {
        let (en, zh) = match row {
            Row::Language => ("Language", "语言"),
            Row::Theme => ("Application theme", "应用主题"),
            Row::TerminalTheme => ("Terminal display theme", "终端显示主题"),
            Row::Foreground => ("Foreground color", "前景色"),
            Row::Background => ("Background color", "背景色"),
            Row::CopyOnSelect => ("Copy on select", "选中即复制"),
            Row::TrimAgents => (
                "Trim trailing blank rows for CLI agents",
                "裁剪 CLI Agent 尾部空白行",
            ),
            Row::Reminders => ("Agent status notifications", "Agent 状态通知"),
            Row::CompletionReminders => ("Completion reminders", "命令完成提醒"),
            Row::CompletionThreshold => {
                ("Minimum command duration (seconds)", "最短命令时长（秒）")
            }
            Row::Sound => ("Notification sound", "通知声音"),
            Row::Scrollback => ("Scrollback limit (bytes)", "回滚上限（字节）"),
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
        match row {
            Row::Theme => self.setting("Application appearance", "应用外观"),
            Row::TerminalTheme | Row::Foreground | Row::Background => {
                self.setting("Terminal colors", "终端颜色")
            }
            Row::CopyOnSelect => self.setting("Selection and clipboard", "选择与剪贴板"),
            Row::TrimAgents | Row::Scrollback => self.setting("History", "历史记录"),
            Row::Reminders | Row::CompletionReminders | Row::CompletionThreshold | Row::Sound => {
                self.setting("Shell integration", "Shell 集成")
            }
            Row::Language => self.setting("Language", "语言"),
            Row::ResetAll | Row::Reload | Row::Save => self.setting("General", "通用"),
            _ => self.setting("Launch profiles", "启动配置"),
        }
    }
    pub(crate) fn settings_toggle(self, value: bool) -> &'static str {
        if value {
            self.setting("On", "开启")
        } else {
            self.setting("Off", "关闭")
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
                    self.setting("When Agent location is hidden", "Agent 位置不可见时")
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
            Row::Scrollback => return values.scrollback_bytes.to_string(),
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
    pub(crate) fn settings_footer(self, view: View<'_>) -> &'static str {
        match view {
            View::Saving { .. } => {
                self.setting("Esc close; save continues", "Esc 关闭；保存继续执行")
            }
            View::Draft(draft) if draft.confirmation.is_some() => {
                self.setting("Enter confirm · Esc cancel", "Enter 确认 · Esc 取消")
            }
            View::Draft(draft) if draft.text.is_some() => self.setting(
                "Enter accept · Esc cancel · Ctrl+U clear",
                "Enter 接受 · Esc 取消 · Ctrl+U 清空",
            ),
            _ => self.setting(
                "Tab select · Left/Right change · Enter edit",
                "Tab 选择 · 左右键切换 · Enter 编辑",
            ),
        }
    }
    pub(crate) fn settings_cancel(self) -> &'static str {
        self.setting("Esc back / cancel", "Esc 返回／取消")
    }
    pub(crate) fn settings_pending(self, saving: bool) -> &'static str {
        match (self, saving) {
            (Self::EnUs, false) => "[..] Opening",
            (Self::ZhCn, false) => "[..] 正在打开",
            (Self::EnUs, true) => "[..] Saving",
            (Self::ZhCn, true) => "[..] 正在保存",
        }
    }
    pub(crate) fn settings_hint(self, view: View<'_>, pending: bool) -> String {
        if pending {
            self.settings_pending(matches!(view, View::Saving { .. }))
                .into()
        } else {
            self.settings_footer(view).into()
        }
    }
    pub(crate) fn settings_detail(self, draft: DraftView<'_>) -> &'static str {
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
                Row::Save => self.setting("No changes to save.", "没有需要保存的更改。"),
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
        self.setting(
            "Shared on this device/configuration; font and opacity belong to your terminal.",
            "在此设备／配置范围共享；字体和透明度由宿主终端管理。",
        )
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
            TooSmall => self.setting(
                "Settings hidden: enlarge the window; draft kept",
                "窗口太小，设置暂时隐藏；草稿已保留",
            ),
            Background => self.setting(
                "Save continues · wait for its result",
                "保存继续执行 · 请等待结果",
            ),
        }
    }
}
