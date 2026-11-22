//! Words for Orchbee answering an Agent's routine confirmations. The window Client says the same
//! in its `workbench.agentConfirmation.*` messages; a change here changes those too.

use super::Locale;
use crate::terminal_text::sanitize_terminal_text;
use orchbee_workbench_protocol::{AgentConfirmationKind, AgentConfirmationWait};

/// One answer a person gives about Orchbee answering an Agent's routine confirmations. `No`
/// answers an offer and ends offers for every Agent; the others set the choice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationAnswer {
    Off,
    Run,
    AgentType,
    No,
}

impl Locale {
    /// The offer as the status line states it, then the form that replaces it on a short line.
    /// `agent` is the product name and `key` the slot of the key that opens the answers.
    pub(crate) fn confirmation_offer_forms(self, agent: &str, key: &str) -> Vec<String> {
        let agent = sanitize_terminal_text(agent);
        match self {
            Self::EnUs => vec![
                format!("Let Orchbee allow routine {agent} confirmations? · {key} answer"),
                format!("Allow routine confirmations? · {key} answer"),
            ],
            Self::ZhCn => vec![
                format!("让 Orchbee 自动允许 {agent} 的常规确认？ · {key} 回答"),
                format!("自动允许常规确认？ · {key} 回答"),
            ],
        }
    }

    /// What Orchbee just answered for the person: the command, or the file it let the Agent
    /// create or edit.
    pub(crate) fn confirmation_allowed(self, kind: AgentConfirmationKind, text: &str) -> String {
        let text = sanitize_terminal_text(text);
        match (self, kind) {
            (Self::EnUs, AgentConfirmationKind::Command) => format!("Allowed: {text}"),
            (Self::ZhCn, AgentConfirmationKind::Command) => format!("已允许：{text}"),
            (Self::EnUs, AgentConfirmationKind::Create) => format!("Allowed: create {text}"),
            (Self::ZhCn, AgentConfirmationKind::Create) => format!("已允许：创建 {text}"),
            (Self::EnUs, AgentConfirmationKind::Edit) => format!("Allowed: edit {text}"),
            (Self::ZhCn, AgentConfirmationKind::Edit) => format!("已允许：编辑 {text}"),
        }
    }

    /// Answering stopped for one Agent run, then the form for a short line.
    pub(crate) fn confirmation_stopped_forms(self) -> [&'static str; 2] {
        match self {
            Self::EnUs => [
                "Stopped allowing: the same confirmation keeps coming back.",
                "Stopped allowing confirmations.",
            ],
            Self::ZhCn => [
                "已停止自动允许：同一个确认反复出现。",
                "已停止自动允许确认。",
            ],
        }
    }

    /// Why the confirmation an Agent shows waits for the person.
    pub(crate) const fn confirmation_wait(self, wait: AgentConfirmationWait) -> &'static str {
        use AgentConfirmationWait as W;
        match (self, wait) {
            (Self::EnUs, W::LeavesWorkspace) => "file outside the Workspace, waiting for you",
            (Self::ZhCn, W::LeavesWorkspace) => "工作区外的文件，等你确认",
            (Self::EnUs, W::HomeWorkspace) => {
                "Workspace includes your home folder, waiting for you"
            }
            (Self::ZhCn, W::HomeWorkspace) => "工作区包含你的主文件夹，等你确认",
            (Self::EnUs, W::ProtectedFile) => "protected file, waiting for you",
            (Self::ZhCn, W::ProtectedFile) => "受保护的文件，等你确认",
            (Self::EnUs, W::NotUnderstood) => "path Orchbee cannot read, waiting for you",
            (Self::ZhCn, W::NotUnderstood) => "Orchbee 无法读取的路径，等你确认",
            (Self::EnUs, W::NotVisible) => "path not fully shown, waiting for you",
            (Self::ZhCn, W::NotVisible) => "路径未完整显示，等你确认",
            (Self::EnUs, W::MarkedDangerous) => "marked dangerous by the Agent, waiting for you",
            (Self::ZhCn, W::MarkedDangerous) => "智能体标记为危险，等你确认",
            (Self::EnUs, W::NotAnswered) => "not a routine confirmation, waiting for you",
            (Self::ZhCn, W::NotAnswered) => "不是常规确认，等你确认",
            (Self::EnUs, W::Person) => "you typed in this terminal, waiting for you",
            (Self::ZhCn, W::Person) => "你在此终端输入过，等你确认",
            (Self::EnUs, W::Changing) => "screen keeps changing, waiting for you",
            (Self::ZhCn, W::Changing) => "屏幕持续变化，等你确认",
            (Self::EnUs, W::Repeating) => "same confirmation keeps coming back, waiting for you",
            (Self::ZhCn, W::Repeating) => "同一个确认反复出现，等你确认",
            (Self::EnUs, W::HostTooOld) => "terminal started by an older Orchbee, waiting for you",
            (Self::ZhCn, W::HostTooOld) => "此终端由旧版 Orchbee 启动，等你确认",
        }
    }
}
