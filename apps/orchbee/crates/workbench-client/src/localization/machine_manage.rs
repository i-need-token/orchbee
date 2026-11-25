//! The words of managing saved Machines in the picker: the Add row and form, the checks of an
//! add, the action menu, the remove-and-stop confirmation and the outcomes the picker names.

use super::{Locale, RememberText};
use crate::machine_add::Stage;
use crate::machine_picker::{ManageNotice, ManageProblem, MenuItem};
use crate::terminal_text::sanitize_terminal_text;

/// One fixed phrase of the management pages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManageText {
    AddMachine,
    EmptyState,
    AddTitle,
    Address,
    Hosts,
    Color,
    ChooseColor,
    /// The color list's last entry, which asks for a color of the person's own.
    CustomColor,
    /// The field a custom color is typed in, before anything is typed.
    CustomColorHint,
    /// A typed custom color that is not one.
    CustomColorInvalid,
    Cancel,
    Add,
    Actions,
    Rename,
    ConfirmTitle,
    RemoveAndStop,
    FilesStay,
    Back,
    NextField,
    Choose,
}

impl Locale {
    /// A saved machine's color by name; a custom color is named Custom, its value written beside.
    pub(crate) const fn machine_color_name(
        self,
        color: crate::machine_color::MachineColor,
    ) -> &'static str {
        use crate::machine_color::MachineColor;
        const EN_US: [&str; 12] = [
            "Cyan", "Teal", "Sky", "Blue", "Indigo", "Violet", "Purple", "Magenta", "Pink",
            "Brown", "Slate", "Grey",
        ];
        const ZH_CN: [&str; 12] = [
            "青色",
            "蓝绿色",
            "天蓝色",
            "蓝色",
            "靛蓝色",
            "紫罗兰色",
            "紫色",
            "品红色",
            "粉色",
            "棕色",
            "石板灰",
            "灰色",
        ];
        match (self, color) {
            (Self::EnUs, MachineColor::Preset(preset)) => EN_US[preset.index()],
            (Self::ZhCn, MachineColor::Preset(preset)) => ZH_CN[preset.index()],
            (Self::EnUs, MachineColor::Custom(_)) => "Custom",
            (Self::ZhCn, MachineColor::Custom(_)) => "自定义",
        }
    }

    pub(crate) const fn machine_manage_text(self, text: ManageText) -> &'static str {
        use ManageText::*;
        match (self, text) {
            (Self::EnUs, AddMachine) => "Add machine…",
            (Self::ZhCn, AddMachine) => "添加机器…",
            (Self::EnUs, EmptyState) => {
                "Connect another computer over SSH to use its terminals and Agents here."
            }
            (Self::ZhCn, EmptyState) => "通过 SSH 连接另一台电脑，在这里使用它的终端和智能体。",
            (Self::EnUs, AddTitle) => "Add machine",
            (Self::ZhCn, AddTitle) => "添加机器",
            (Self::EnUs, Address) => "Address",
            (Self::ZhCn, Address) => "地址",
            (Self::EnUs, Hosts) => "Hosts in SSH config",
            (Self::ZhCn, Hosts) => "SSH 配置里的主机",
            (Self::EnUs, Color) => "Color",
            (Self::ZhCn, Color) => "颜色",
            (Self::EnUs, ChooseColor) => "Choose color",
            (Self::ZhCn, ChooseColor) => "选择颜色",
            (Self::EnUs, CustomColor) => "Custom…",
            (Self::ZhCn, CustomColor) => "自定义…",
            (Self::EnUs, CustomColorHint) => "#3b82f6",
            (Self::ZhCn, CustomColorHint) => "#3b82f6",
            (Self::EnUs, CustomColorInvalid) => "Type # and six hex digits, such as #3b82f6.",
            (Self::ZhCn, CustomColorInvalid) => "请输入 # 加 6 位十六进制数，例如 #3b82f6。",
            (Self::EnUs, Cancel) => "Cancel",
            (Self::ZhCn, Cancel) => "取消",
            (Self::EnUs, Add) => "Add",
            (Self::ZhCn, Add) => "添加",
            (Self::EnUs, Actions) => "Actions",
            (Self::ZhCn, Actions) => "操作",
            (Self::EnUs, Rename) => "Rename",
            (Self::ZhCn, Rename) => "重命名",
            (Self::EnUs, ConfirmTitle) => "Remove and stop remote Session?",
            (Self::ZhCn, ConfirmTitle) => "移除并停止远端会话？",
            (Self::EnUs, RemoveAndStop) => "Remove and stop",
            (Self::ZhCn, RemoveAndStop) => "移除并停止",
            (Self::EnUs, FilesStay) => "Orchbee and files on the remote stay.",
            (Self::ZhCn, FilesStay) => "远端的 Orchbee 和文件保留。",
            (Self::EnUs, Back) => "Back",
            (Self::ZhCn, Back) => "返回",
            (Self::EnUs, NextField) => "Next field",
            (Self::ZhCn, NextField) => "下一项",
            (Self::EnUs, Choose) => "Choose",
            (Self::ZhCn, Choose) => "选择",
        }
    }

    pub(crate) const fn machine_menu_item(self, item: MenuItem) -> &'static str {
        match (self, item) {
            (_, MenuItem::Rename) => self.machine_manage_text(ManageText::Rename),
            (Self::EnUs, MenuItem::Disable) => "Disable",
            (Self::ZhCn, MenuItem::Disable) => "停用",
            (Self::EnUs, MenuItem::Enable) => "Enable",
            (Self::ZhCn, MenuItem::Enable) => "启用",
            (Self::EnUs, MenuItem::Disconnect) => "Disconnect",
            (Self::ZhCn, MenuItem::Disconnect) => "断开",
            (_, MenuItem::ForgetPassword) => self.machine_remember_text(RememberText::Forget),
            (Self::EnUs, MenuItem::Remove) => "Remove",
            (Self::ZhCn, MenuItem::Remove) => "移除",
            (Self::EnUs, MenuItem::RemoveAndStop) => "Remove and stop remote Session",
            (Self::ZhCn, MenuItem::RemoveAndStop) => "移除并停止远端会话",
        }
    }

    /// Why "Remove and stop remote Session" is unavailable for the Machine named `label`.
    pub(crate) fn machine_cannot_stop(self, label: &str) -> String {
        let label = sanitize_terminal_text(label);
        match self {
            Self::EnUs => format!("Can't reach {label}, so its Session can't be stopped."),
            Self::ZhCn => format!("连不上 {label}，无法停止它的会话。"),
        }
    }

    /// What the stop ends, by the terminal count the Machine's overview holds when one is known.
    pub(crate) fn machine_ends_terminals(self, terminals: Option<usize>, label: &str) -> String {
        let label = sanitize_terminal_text(label);
        match (self, terminals) {
            (Self::EnUs, Some(1)) => format!("Ends 1 terminal on {label}."),
            (Self::EnUs, Some(count)) => format!("Ends {count} terminals on {label}."),
            (Self::EnUs, None) => format!("Ends every terminal on {label}."),
            (Self::ZhCn, Some(count)) => format!("结束 {label} 上的 {count} 个终端。"),
            (Self::ZhCn, None) => format!("结束 {label} 上的所有终端。"),
        }
    }

    pub(crate) fn machine_stopping(self, label: &str) -> String {
        let label = sanitize_terminal_text(label);
        match self {
            Self::EnUs => format!("Stopping the Session on {label}…"),
            Self::ZhCn => format!("正在停止 {label} 上的会话…"),
        }
    }

    /// The check of an add, by its place in the design's order.
    const fn machine_stage(self, stage: Stage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, Stage::Ssh) => "OpenSSH on this computer",
            (Self::ZhCn, Stage::Ssh) => "本机的 OpenSSH",
            (Self::EnUs, Stage::Reach) => "reaching the host",
            (Self::ZhCn, Stage::Reach) => "连接主机",
            (Self::EnUs, Stage::Trust) => "host key",
            (Self::ZhCn, Stage::Trust) => "主机密钥",
            (Self::EnUs, Stage::Login) => "login",
            (Self::ZhCn, Stage::Login) => "登录",
            (Self::EnUs, Stage::System) => "remote system",
            (Self::ZhCn, Stage::System) => "远端系统",
            (Self::EnUs, Stage::Orchbee) => "Orchbee on the remote",
            (Self::ZhCn, Stage::Orchbee) => "远端的 Orchbee",
            (Self::EnUs, Stage::Compatibility) => "both versions working together",
            (Self::ZhCn, Stage::Compatibility) => "两端能否配合",
            (Self::EnUs, Stage::Session) => "remote Session",
            (Self::ZhCn, Stage::Session) => "远端会话",
        }
    }

    /// The form's status while `stage` runs.
    pub(crate) fn machine_checking(self, stage: Stage) -> String {
        let (position, name) = (stage_position(stage), self.machine_stage(stage));
        match self {
            Self::EnUs => format!("Checking {position} of 8: {name}…"),
            Self::ZhCn => format!("检查第 {position}/8 步：{name}…"),
        }
    }

    /// The form's status once the add ended at `stage`, or before any check.
    pub(crate) fn machine_check_failed(self, stage: Option<Stage>) -> String {
        match (self, stage) {
            (Self::EnUs, Some(stage)) => format!(
                "Check {} of 8 failed: {}.",
                stage_position(stage),
                self.machine_stage(stage)
            ),
            (Self::ZhCn, Some(stage)) => format!(
                "第 {}/8 步未通过：{}。",
                stage_position(stage),
                self.machine_stage(stage)
            ),
            (Self::EnUs, None) => "The machine couldn't be added.".to_owned(),
            (Self::ZhCn, None) => "没能添加这台机器。".to_owned(),
        }
    }

    pub(crate) const fn machine_saving(self) -> &'static str {
        match self {
            Self::EnUs => "Every check passed. Saving…",
            Self::ZhCn => "全部通过，正在保存…",
        }
    }

    pub(crate) const fn machine_manage_problem(self, problem: ManageProblem) -> &'static str {
        match (self, problem) {
            (Self::EnUs, ManageProblem::Address) => {
                "Enter an address such as user@10.0.0.8 or a host from your SSH config."
            }
            (Self::ZhCn, ManageProblem::Address) => {
                "请输入地址，例如 user@10.0.0.8，或 SSH 配置里的主机。"
            }
            (_, ManageProblem::Name) => self.machine_name_rule(),
            (_, ManageProblem::Save) => self.machine_save_failed(),
        }
    }

    /// The outcome the picker's status row names until the next key.
    pub(crate) fn machine_manage_notice(self, notice: &ManageNotice) -> String {
        match (self, notice) {
            (Self::EnUs, ManageNotice::Removed(label)) => format!(
                "Removed {} · remote Session keeps running",
                sanitize_terminal_text(label)
            ),
            (Self::ZhCn, ManageNotice::Removed(label)) => {
                format!(
                    "已移除 {} · 远端会话继续运行",
                    sanitize_terminal_text(label)
                )
            }
            (Self::EnUs, ManageNotice::RemovedStopped(label)) => format!(
                "Removed {} · remote Session stopped",
                sanitize_terminal_text(label)
            ),
            (Self::ZhCn, ManageNotice::RemovedStopped(label)) => {
                format!("已移除 {} · 远端会话已停止", sanitize_terminal_text(label))
            }
            (Self::EnUs, ManageNotice::StopFailed(label)) => format!(
                "The Session on {} couldn't be stopped; the machine stays.",
                sanitize_terminal_text(label)
            ),
            (Self::ZhCn, ManageNotice::StopFailed(label)) => format!(
                "没能停止 {} 上的会话，机器保留。",
                sanitize_terminal_text(label)
            ),
            (_, ManageNotice::NotOnline(label)) => self.machine_cannot_stop(label),
            (_, ManageNotice::Failed(problem)) => self.machine_manage_problem(*problem).to_owned(),
        }
    }
}

/// The check's place in the design's order of eight.
const fn stage_position(stage: Stage) -> u8 {
    match stage {
        Stage::Ssh => 1,
        Stage::Reach => 2,
        Stage::Trust => 3,
        Stage::Login => 4,
        Stage::System => 5,
        Stage::Orchbee => 6,
        Stage::Compatibility => 7,
        Stage::Session => 8,
    }
}
