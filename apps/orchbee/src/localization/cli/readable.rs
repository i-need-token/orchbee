//! Conclusion lines, column headings and empty states for check and list
//! views: the result comes first and every empty list says how to fill it.
//! Raw reason structures stay in the JSON documents.
use super::Locale;

impl Locale {
    /// The `config check` conclusion printed before the per-check rows.
    pub(crate) fn cli_check_verdict(self, total: usize, warnings: usize, failed: usize) -> String {
        match (self, failed, warnings) {
            (Self::EnUs, 0, 0) => format!("Configuration OK: all {total} checks passed.\n"),
            (Self::ZhCn, 0, 0) => format!("配置正常：{total} 项检查全部通过。\n"),
            (Self::EnUs, 0, _) => format!("Configuration usable with {warnings} warning(s).\n"),
            (Self::ZhCn, 0, _) => format!("配置可用，有 {warnings} 项警告。\n"),
            (Self::EnUs, _, _) => {
                format!("Configuration has problems: {failed} check(s) failed.\n")
            }
            (Self::ZhCn, _, _) => format!("配置有问题：{failed} 项检查失败。\n"),
        }
    }

    pub(crate) const fn cli_check_status(self, status: &str) -> &str {
        match (self, status.as_bytes()) {
            (Self::EnUs, b"pass") => "ok",
            (Self::ZhCn, b"pass") => "通过",
            (Self::EnUs, b"warning") => "warning",
            (Self::ZhCn, b"warning") => "警告",
            (Self::EnUs, b"fail") => "failed",
            (Self::ZhCn, b"fail") => "失败",
            (Self::EnUs, b"not-applicable") => "n/a",
            (Self::ZhCn, b"not-applicable") => "不适用",
            _ => status,
        }
    }

    pub(crate) const fn cli_check_headers(self) -> [&'static str; 3] {
        match self {
            Self::EnUs => ["CHECK", "STATUS", "DETAIL"],
            Self::ZhCn => ["检查项", "状态", "详情"],
        }
    }

    /// Where every effective setting and raw check detail is available.
    pub(crate) const fn cli_check_json_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Every effective setting: orchbee config check --format json\n",
            Self::ZhCn => "全部有效设置：orchbee config check --format json\n",
        }
    }

    pub(crate) const fn cli_doctor_headers(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => ["PROVIDER", "EXECUTABLE", "NODE", "HOOK"],
            Self::ZhCn => ["提供方", "可执行文件", "NODE", "钩子"],
        }
    }

    pub(crate) const fn cli_doctor_json_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Raw evidence: orchbee agent doctor --format json\n",
            Self::ZhCn => "原始证据：orchbee agent doctor --format json\n",
        }
    }

    pub(crate) const fn cli_session_headers(self) -> [&'static str; 3] {
        match self {
            Self::EnUs => ["SESSION", "STATE", "RUNTIME DIRECTORY"],
            Self::ZhCn => ["会话", "状态", "运行时目录"],
        }
    }

    pub(crate) const fn cli_session_empty(self) -> &'static str {
        match self {
            Self::EnUs => "No sessions yet. `orchbee server start` creates the default session.\n",
            Self::ZhCn => "尚无会话。`orchbee server start` 会创建默认会话。\n",
        }
    }

    /// Explains the `absent` state when at least one listed session has it.
    pub(crate) const fn cli_session_absent_note(self) -> &'static str {
        match self {
            Self::EnUs => {
                "\"not created\" sessions are named but have no runtime yet; `orchbee server start --session <name>` starts one.\n"
            }
            Self::ZhCn => {
                "“尚未创建”的会话已命名但还没有运行时；`orchbee server start --session <名称>` 可启动。\n"
            }
        }
    }

    pub(crate) const fn cli_command_headers(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => ["COMMAND", "ACTION", "KEYS", "DESCRIPTION"],
            Self::ZhCn => ["命令", "动作", "按键", "说明"],
        }
    }

    pub(crate) const fn cli_command_empty(self) -> &'static str {
        match self {
            Self::EnUs => {
                "No custom commands. Add a [[keys.command]] entry to the config file; `orchbee --default-config` shows the rules.\n"
            }
            Self::ZhCn => {
                "没有自定义命令。在配置文件中添加 [[keys.command]] 条目；`orchbee --default-config` 列出规则。\n"
            }
        }
    }

    pub(crate) const fn cli_launch_profile_empty(self) -> &'static str {
        match self {
            Self::EnUs => {
                "No launch profiles. Create one with `orchbee launch-profile create --name <name> --exec <path>`.\n"
            }
            Self::ZhCn => {
                "没有启动配置。使用 `orchbee launch-profile create --name <名称> --exec <路径>` 创建。\n"
            }
        }
    }

    pub(crate) const fn cli_schema_catalog_headers(self) -> [&'static str; 3] {
        match self {
            Self::EnUs => ["SCHEMA", "VERSIONS", "DESCRIPTION"],
            Self::ZhCn => ["SCHEMA", "版本", "说明"],
        }
    }

    pub(crate) const fn cli_schema_catalog_footer(self) -> &'static str {
        match self {
            Self::EnUs => {
                "One schema: orchbee api schema <schema-id>. Every document: orchbee api schema --format json.\n"
            }
            Self::ZhCn => {
                "单个 schema：orchbee api schema <schema-id>。全部文档：orchbee api schema --format json。\n"
            }
        }
    }

    /// The `workspace close-status` conclusion, keyed by the scope status.
    pub(crate) const fn cli_close_status_verdict(self, status: &str) -> &'static str {
        match (self, status.as_bytes()) {
            (Self::EnUs, b"closed") => "The workspace is closed.\n",
            (Self::ZhCn, b"closed") => "工作区已关闭。\n",
            (Self::EnUs, b"closing") => "The workspace is closing.\n",
            (Self::ZhCn, b"closing") => "工作区正在关闭。\n",
            (Self::EnUs, b"cleanup-blocked") => "Closing is blocked while cleaning up.\n",
            (Self::ZhCn, b"cleanup-blocked") => "关闭在清理阶段受阻。\n",
            (Self::EnUs, b"outcome-unknown") => "The close outcome is unknown.\n",
            (Self::ZhCn, b"outcome-unknown") => "关闭结果未知。\n",
            (Self::EnUs, _) => "The workspace is gone; its removal is not yet published.\n",
            (Self::ZhCn, _) => "工作区已释放，移除尚未发布。\n",
        }
    }
}
