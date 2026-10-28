use super::Locale;
use orchbee_workbench_protocol::PanelAgentState;
impl Locale {
    pub const fn panel_report_state(self, state: PanelAgentState) -> &'static str {
        match (self, state) {
            (Self::EnUs, PanelAgentState::Idle) => "idle",
            (Self::ZhCn, PanelAgentState::Idle) => "空闲",
            (Self::EnUs, PanelAgentState::Working) => "working",
            (Self::ZhCn, PanelAgentState::Working) => "工作中",
            (Self::EnUs, PanelAgentState::Blocked) => "blocked",
            (Self::ZhCn, PanelAgentState::Blocked) => "待处理",
            (Self::EnUs, PanelAgentState::Done) => "completed",
            (Self::ZhCn, PanelAgentState::Done) => "已完成",
            (Self::EnUs, PanelAgentState::Error) => "error",
            (Self::ZhCn, PanelAgentState::Error) => "错误",
            (Self::EnUs, PanelAgentState::Unknown) => "unknown",
            (Self::ZhCn, PanelAgentState::Unknown) => "未知",
        }
    }
}
