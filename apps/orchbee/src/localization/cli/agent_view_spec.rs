use super::Locale;
#[cfg(windows)]
use orchbee_workbench_protocol::{AgentViewFailure as F, AgentViewOutcome};
impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_agent_view_exchange_failed(self) -> &'static str {
        match self {
            Self::EnUs => "Agent view exchange failed\n",
            Self::ZhCn => "Agent 视图请求交换失败\n",
        }
    }
    #[cfg(windows)]
    pub(crate) fn agent_view_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee agent view get|set|clear [--format text|json]\n\nSet: agent view set --spec <json>\nClear: agent view clear [--source <name>]\nGet: agent view get\n\nThe selected Windows Site owns one transient override. The latest valid set\nreplaces it; source-qualified clear preserves a different owner's view.\nSpec: source, optional label/filter/sort; all/any/not/eq/in/exists filters,\nstatus/workspace_id/tab_id/pane_id/agent/seen/state_change_seq/token fields,\nand up to eight asc/desc sort keys. ID operands use the active machine scope.\nExample: --spec '{\"source\":\"cli.views\",\"filter\":{\"op\":\"eq\",\"field\":\"status\",\"value\":\"blocked\"},\"sort\":[{\"field\":\"attention\",\"order\":\"desc\"}]}'\nSchema: orchbee api schema orchbee.agent.view ($defs/spec).\nInvalid input is rejected before Site discovery. Lost mutation replies are\nunknown (exit 8); they are never retried. Restart clears the override.\n"
            }
            Self::ZhCn => {
                "orchbee agent view get|set|clear [--format text|json]\n\n设置：agent view set --spec <json>\n清除：agent view clear [--source <name>]\n读取：agent view get\n\n所选 Windows Site 拥有一份临时覆盖。最近一次有效设置替换旧规格；\n指定来源清除会保留属于其他来源的视图。\n规格含 source 和可选 label/filter/sort；过滤器支持 all/any/not/eq/in/exists，\n字段支持 status/workspace_id/tab_id/pane_id/agent/seen/state_change_seq/token，\n最多八个 asc/desc 排序项。ID 操作数由当前机器限定。\n示例：--spec '{\"source\":\"cli.views\",\"filter\":{\"op\":\"eq\",\"field\":\"status\",\"value\":\"blocked\"},\"sort\":[{\"field\":\"attention\",\"order\":\"desc\"}]}'\n规格字典：orchbee api schema orchbee.agent.view（$defs/spec）。\n无效输入在发现 Site 前拒绝。修改回复丢失表示未知（退出码 8），不重试。\n重启清除覆盖。\n"
            }
        }
    }
    #[cfg(windows)]
    pub(crate) fn cli_agent_view(self, outcome: Option<&AgentViewOutcome>) -> String {
        match outcome {
            Some(AgentViewOutcome::Available { snapshot }) => match &snapshot.spec {
                Some(spec) => format!(
                    "{}: {} · {}{}\n",
                    if self == Self::ZhCn {
                        "Agent 视图"
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
                    if self == Self::ZhCn {
                        "Agent 视图已清除"
                    } else {
                        "Agent view cleared"
                    },
                    snapshot.revision
                ),
            },
            Some(AgentViewOutcome::Failed { reason }) => format!(
                "{}\n",
                match (self, reason) {
                    (Self::EnUs, F::Denied) => "Agent view permission denied",
                    (Self::ZhCn, F::Denied) => "没有 Agent 视图操作权限",
                    (Self::EnUs, F::SiteMismatch) => "Agent view Site changed",
                    (Self::ZhCn, F::SiteMismatch) => "Agent 视图 Site 已改变",
                    (Self::EnUs, F::InvalidRequest) => "Invalid Agent view specification",
                    (Self::ZhCn, F::InvalidRequest) => "Agent 视图规格无效",
                    (Self::EnUs, F::PluginNotFound) => "Agent view plugin not found",
                    (Self::ZhCn, F::PluginNotFound) => "Agent 视图插件不存在",
                    (Self::EnUs, F::PluginDisabled) => "Agent view plugin disabled",
                    (Self::ZhCn, F::PluginDisabled) => "Agent 视图插件已禁用",
                    (Self::EnUs, F::SourceUnavailable) => "Agent view source unavailable",
                    (Self::ZhCn, F::SourceUnavailable) => "Agent 视图来源不可用",
                    (Self::EnUs, F::RevisionExhausted) => "Agent view revision exhausted",
                    (Self::ZhCn, F::RevisionExhausted) => "Agent 视图修订号已耗尽",
                    (Self::EnUs, F::Unavailable) => "Agent view unavailable",
                    (Self::ZhCn, F::Unavailable) => "Agent 视图不可用",
                }
            ),
            None => format!(
                "{}\n",
                if self == Self::ZhCn {
                    "Agent 视图修改结果未知；未重试"
                } else {
                    "Agent view mutation outcome unknown; not retried"
                }
            ),
        }
    }
}
