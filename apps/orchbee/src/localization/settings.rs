//! Copy for the explicit-save native Settings surface and separate disk/live outcomes.
use super::Locale;
use crate::{
    native_settings::{Row, View},
    terminal_session::SettingsNotice,
};
use orchbee_workbench_protocol::ConfigurationThemeName;

impl Locale {
    pub(crate) const fn settings_title(self) -> &'static str {
        match self {
            Self::EnUs => "Settings",
            Self::ZhCn => "设置",
        }
    }
    pub(crate) const fn settings_row(self, row: Row) -> &'static str {
        match (self, row) {
            (Self::EnUs, Row::Theme) => "Theme",
            (Self::ZhCn, Row::Theme) => "主题",
            (Self::EnUs, Row::Reminders) => "Agent reminders",
            (Self::ZhCn, Row::Reminders) => "后台 Agent 提醒",
            (Self::EnUs, Row::Sound) => "Sound",
            (Self::ZhCn, Row::Sound) => "声音",
            (Self::EnUs, Row::Save) => "Save and apply",
            (Self::ZhCn, Row::Save) => "保存并应用",
        }
    }
    pub(crate) const fn settings_theme(self, theme: ConfigurationThemeName) -> &'static str {
        match (self, theme) {
            (Self::EnUs, ConfigurationThemeName::TerminalNative) => "Terminal default",
            (Self::ZhCn, ConfigurationThemeName::TerminalNative) => "终端默认",
            (_, ConfigurationThemeName::OneLight) => "One Light",
            (_, ConfigurationThemeName::OneDark) => "One Dark",
        }
    }
    pub(crate) const fn settings_toggle(self, value: bool) -> &'static str {
        match (self, value) {
            (Self::EnUs, true) => "On",
            (Self::EnUs, false) => "Off",
            (Self::ZhCn, true) => "开启",
            (Self::ZhCn, false) => "关闭",
        }
    }
    pub(crate) const fn settings_footer(self, saving: bool) -> &'static str {
        match (self, saving) {
            (Self::EnUs, false) => "Tab select · Left/Right edit",
            (Self::ZhCn, false) => "Tab 选择 · 左右键修改",
            (Self::EnUs, true) => "Esc close; save continues",
            (Self::ZhCn, true) => "Esc 关闭；保存继续执行",
        }
    }
    pub(crate) const fn settings_cancel(self) -> &'static str {
        match self {
            Self::EnUs => "Esc cancel",
            Self::ZhCn => "Esc 取消",
        }
    }
    pub(crate) const fn settings_pending(self, saving: bool) -> &'static str {
        match (self, saving) {
            (Self::EnUs, false) => "[..] Opening",
            (Self::ZhCn, false) => "[..] 正在打开",
            (Self::EnUs, true) => "[..] Saving",
            (Self::ZhCn, true) => "[..] 正在保存",
        }
    }
    pub(crate) fn settings_hint(self, view: View, pending: bool) -> String {
        let hint = if pending {
            self.settings_pending(matches!(view, View::Saving { .. }))
        } else if matches!(view, View::Draft { .. }) {
            self.settings_footer(false)
        } else {
            self.settings_cancel()
        };
        format!("{} · {hint}", self.settings_title())
    }
    pub(crate) const fn settings_notice(self, notice: SettingsNotice) -> &'static str {
        use SettingsNotice::*;
        match (self, notice) {
            (Self::EnUs, Saved) => "Saved and applied",
            (Self::ZhCn, Saved) => "已保存并应用",
            (Self::EnUs, NoChange) => "No changes to save",
            (Self::ZhCn, NoChange) => "没有需要保存的更改",
            (Self::EnUs, SavedNotApplied) => "Saved; apply failed · Menu → Reload configuration",
            (Self::ZhCn, SavedNotApplied) => "已保存；应用失败 · 请从菜单重载配置",
            (Self::EnUs, SourceChanged) => "File changed; nothing saved · reopen Settings",
            (Self::ZhCn, SourceChanged) => "文件已修改，未保存 · 请重开设置",
            (Self::EnUs, ConfigurationChanged) => {
                "Configuration changed · reload, then reopen Settings"
            }
            (Self::ZhCn, ConfigurationChanged) => "配置已变化 · 请先重载，再重开设置",
            (Self::EnUs, Failed) => "Settings failed; nothing saved · reopen Settings",
            (Self::ZhCn, Failed) => "设置操作失败，未保存 · 请重开设置",
            (Self::EnUs, Unknown) => "Save result unknown · reopen Settings to check",
            (Self::ZhCn, Unknown) => "保存结果未知 · 请重开设置核对",
            (Self::EnUs, Busy) => "Settings busy · finish the current action first",
            (Self::ZhCn, Busy) => "设置正忙 · 请先完成当前操作",
            (Self::EnUs, TooSmall) => "Settings hidden: window too small · enlarge and reopen",
            (Self::ZhCn, TooSmall) => "窗口太小，设置已关闭 · 请放大后重开",
            (Self::EnUs, Background) => "Save continues · wait for its result",
            (Self::ZhCn, Background) => "保存继续执行 · 请等待结果",
        }
    }
}
