//! Formats Agent-view help and received view outcomes without creating or validating a view request.
use super::Locale;
use orchbee_workbench_protocol::{AgentViewFailure as F, AgentViewOutcome};
impl super::CliText {
    pub(crate) fn cli_agent_view_exchange_failed(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Agent view exchange failed\n",
            Locale::ZhCn => "智能体视图请求交换失败\n",
        }
    }
    pub(crate) fn agent_view_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::cli_text::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::AgentViewGet => (
                "orchbee agent view get [--format text|json]",
                "Read the selected Windows session's Agent view override. The override lasts until cleared or the session restarts.",
                "读取选中 Windows 会话的智能体视图覆盖设置；设置保留至清除或会话重启。",
            ),
            C::AgentViewSet => (
                "orchbee agent view set --spec <json> [--format text|json]",
                "Replace the selected Windows session's temporary Agent view. The JSON specification has source and optional label/filter/sort. Filters support all/any/not/eq/in/exists; fields include status, workspace_id, tab_id, pane_id, agent, seen, state_change_seq and token. Up to eight ascending or descending sort keys are accepted. IDs use the selected machine's scope.\n\nInspect the input schema with orchbee api schema orchbee.agent.view ($defs/spec). Invalid input is rejected before connecting. A lost reply is unknown (exit 8) and never retried automatically. Restart clears the override.",
                "替换选中 Windows 会话的临时智能体视图。JSON 定义包含 source 和可选的 label/filter/sort。筛选支持 all/any/not/eq/in/exists；字段包括 status、workspace_id、tab_id、pane_id、agent、seen、state_change_seq 和 token。最多接受八个升序或降序排序键；ID 使用选中机器的作用域。\n\n使用 orchbee api schema orchbee.agent.view 查看输入结构（$defs/spec）。连接前即拒绝无效输入；应答丢失时结果未知（退出 8），不自动重试。重启会清除设置。",
            ),
            C::AgentViewClear => (
                "orchbee agent view clear [--source <name>] [--format text|json]",
                "Clear the selected Windows session's Agent view override. With --source, clear only an override owned by that source; a different owner's view is preserved. A lost reply is unknown (exit 8) and never retried automatically.",
                "清除选中 Windows 会话的智能体视图覆盖设置。使用 --source 时仅清除该来源拥有的设置，保留其他来源的视图。应答丢失时结果未知（退出 8），不自动重试。",
            ),
            _ => unreachable!("command help ownership"),
        };
        crate::cli_text::HelpDetails::new(
            usage,
            match self.locale {
                Locale::EnUs => english,
                Locale::ZhCn => chinese,
            },
        )
    }
    pub(crate) fn cli_agent_view(self, outcome: Option<&AgentViewOutcome>) -> String {
        match outcome {
            Some(AgentViewOutcome::Available { snapshot }) => match &snapshot.spec {
                Some(spec) => format!(
                    "{}: {} · {}{}\n",
                    if self.locale == Locale::ZhCn {
                        "智能体视图"
                    } else {
                        "Agent view"
                    },
                    snapshot.revision,
                    crate::machine_discovery::display_label(&spec.source),
                    spec.label.as_ref().map_or(String::new(), |s| format!(
                        " · {}",
                        crate::machine_discovery::display_label(s)
                    ))
                ),
                None => format!(
                    "{}: {}\n",
                    if self.locale == Locale::ZhCn {
                        "智能体视图已清除"
                    } else {
                        "Agent view cleared"
                    },
                    snapshot.revision
                ),
            },
            Some(AgentViewOutcome::Failed { reason }) => format!(
                "{}\n",
                match (self.locale, reason) {
                    (Locale::EnUs, F::Denied) => "Agent view permission denied",
                    (Locale::ZhCn, F::Denied) => "没有智能体视图操作权限",
                    (Locale::EnUs, F::SessionMismatch) => "Agent view session changed",
                    (Locale::ZhCn, F::SessionMismatch) => "智能体视图会话已改变",
                    (Locale::EnUs, F::InvalidRequest) => "Invalid Agent view specification",
                    (Locale::ZhCn, F::InvalidRequest) => "智能体视图规格无效",
                    (Locale::EnUs, F::PluginNotFound) => "Agent view plugin not found",
                    (Locale::ZhCn, F::PluginNotFound) => "智能体视图插件不存在",
                    (Locale::EnUs, F::PluginDisabled) => "Agent view plugin disabled",
                    (Locale::ZhCn, F::PluginDisabled) => "智能体视图插件已禁用",
                    (Locale::EnUs, F::SourceUnavailable) => "Agent view source unavailable",
                    (Locale::ZhCn, F::SourceUnavailable) => "智能体视图来源不可用",
                    (Locale::EnUs, F::RevisionExhausted) => "Agent view revision exhausted",
                    (Locale::ZhCn, F::RevisionExhausted) => "智能体视图修订号已耗尽",
                    (Locale::EnUs, F::Unavailable) => "Agent view unavailable",
                    (Locale::ZhCn, F::Unavailable) => "智能体视图不可用",
                }
            ),
            None => format!(
                "{}\n",
                if self.locale == Locale::ZhCn {
                    "智能体视图修改结果未知；未重试"
                } else {
                    "Agent view mutation outcome unknown; not retried"
                }
            ),
        }
    }
}
