//! Localized Workbench query headings and failures; does not select or mutate resources.

use super::Locale;

impl super::CliText {
    pub(super) fn cli_client_query_failure(
        self,
        kind: crate::cli_outcome::FailureKind,
    ) -> &'static str {
        use crate::cli_outcome::FailureKind;
        match (self.locale, kind) {
            (Locale::EnUs, FailureKind::Rejected) => {
                "The Client view changed or refused the read. Retry when the TUI is ready.\n"
            }
            (Locale::ZhCn, FailureKind::Rejected) => {
                "客户端视图已变化或拒绝读取。请在 TUI 就绪后重试。\n"
            }
            (Locale::EnUs, FailureKind::NotFound) => {
                "The selected Client was not found. Check orchbee client list.\n"
            }
            (Locale::ZhCn, FailureKind::NotFound) => {
                "未找到指定客户端。请使用 orchbee client list 检查。\n"
            }
            (Locale::EnUs, FailureKind::ServiceUnavailable) => {
                "The selected Client is unavailable or not ready.\n"
            }
            (Locale::ZhCn, FailureKind::ServiceUnavailable) => "指定客户端不可用或尚未就绪。\n",
            (Locale::EnUs, FailureKind::Timeout) => {
                "The Client query exceeded its five-second deadline.\n"
            }
            (Locale::ZhCn, FailureKind::Timeout) => "客户端查询超过了五秒期限。\n",
            (Locale::EnUs, FailureKind::Cancelled) => "The Client query was cancelled.\n",
            (Locale::ZhCn, FailureKind::Cancelled) => "客户端查询已取消。\n",
            (Locale::EnUs, FailureKind::ProtocolInvalid) => {
                "The Client query returned inconsistent or invalid data.\n"
            }
            (Locale::ZhCn, FailureKind::ProtocolInvalid) => {
                "客户端查询返回了不一致或无效的数据。\n"
            }
            (Locale::EnUs, FailureKind::TransportLoss) => "The Client query connection was lost.\n",
            (Locale::ZhCn, FailureKind::TransportLoss) => "客户端查询连接已断开。\n",
            (Locale::EnUs, FailureKind::Denied) => "Access to the Client query was denied.\n",
            (Locale::ZhCn, FailureKind::Denied) => "客户端查询访问被拒绝。\n",
            (Locale::EnUs, _) => "The Client query could not be completed.\n",
            (Locale::ZhCn, _) => "无法完成客户端查询。\n",
        }
    }

    pub(crate) const fn cli_query_client_labels(self) -> [&'static str; 4] {
        match self.locale {
            Locale::EnUs => [
                "Client",
                "Selected Tab",
                "Selected Panel",
                "Context / active Tab",
            ],
            Locale::ZhCn => ["客户端", "所选标签页", "所选面板", "上下文 / 活动标签页"],
        }
    }

    pub(crate) const fn cli_workbench_agents_heading(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Agent summary: resource / status / sessions",
            Locale::ZhCn => "智能体汇总：资源 / 状态 / 会话数",
        }
    }

    pub(crate) const fn cli_workspace_worktrees_heading(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Workspace worktrees",
            Locale::ZhCn => "工作区工作树",
        }
    }

    pub(crate) const fn cli_workspace_worktree_states(self) -> [&'static str; 4] {
        match self.locale {
            Locale::EnUs => [
                "no directory",
                "unavailable",
                "main checkout",
                "linked checkout",
            ],
            Locale::ZhCn => ["没有目录", "不可用", "主检出", "链接检出"],
        }
    }

    pub(crate) fn cli_workbench_agent_status(self, status: Option<&str>) -> &'static str {
        match (self.locale, status) {
            (Locale::EnUs, Some("blocked")) => "needs attention",
            (Locale::EnUs, Some("done")) => "completed",
            (Locale::EnUs, Some("error")) => "error",
            (Locale::EnUs, Some("working")) => "working",
            (Locale::EnUs, Some("idle")) => "idle",
            (Locale::EnUs, None) => "state unavailable",
            (Locale::EnUs, _) => "unknown",
            (Locale::ZhCn, Some("blocked")) => "需要处理",
            (Locale::ZhCn, Some("done")) => "已完成",
            (Locale::ZhCn, Some("error")) => "错误",
            (Locale::ZhCn, Some("working")) => "工作中",
            (Locale::ZhCn, Some("idle")) => "空闲",
            (Locale::ZhCn, None) => "状态不可用",
            (Locale::ZhCn, _) => "未知",
        }
    }

    pub(super) fn cli_workbench_query_failure(self, reason: Option<&str>) -> &'static str {
        match (self.locale, reason) {
            (Locale::EnUs, Some("agent-capacity")) => {
                "The Agent list exceeds the bounded Workbench query capacity. Query Agent pages with orchbee agent list.\n"
            }
            (Locale::ZhCn, Some("agent-capacity")) => {
                "智能体列表超过工作台查询容量。请使用 orchbee agent list 分页查询。\n"
            }
            (Locale::EnUs, Some("agent-page-invalid")) => {
                "The Agent list returned inconsistent page data. No Workbench summary was published.\n"
            }
            (Locale::ZhCn, Some("agent-page-invalid")) => {
                "智能体列表返回了不一致的分页数据，未输出工作台汇总。\n"
            }
            (Locale::EnUs, Some("agent-revision-changed")) => {
                "Agent state changed between query pages. Run the query again.\n"
            }
            (Locale::ZhCn, Some("agent-revision-changed")) => {
                "智能体状态在分页查询期间发生变化，请重新查询。\n"
            }
            (Locale::EnUs, Some("metadata-changed")) => {
                "Workspace metadata changed or expired during the query. Run the query again.\n"
            }
            (Locale::ZhCn, Some("metadata-changed")) => {
                "工作区元数据在查询期间发生变化或过期，请重新查询。\n"
            }
            (Locale::EnUs, Some("workbench-changed")) => {
                "Workbench membership changed during the query. Run the query again.\n"
            }
            (Locale::ZhCn, Some("workbench-changed")) => {
                "工作台结构在查询期间发生变化，请重新查询。\n"
            }
            (Locale::EnUs, _) => {
                "The selected Client view changed during the query. Run the query again.\n"
            }
            (Locale::ZhCn, _) => "所选客户端视图在查询期间发生变化，请重新查询。\n",
        }
    }
}
